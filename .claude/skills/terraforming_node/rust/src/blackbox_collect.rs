//! `blackbox_collect.py` — the 1 s raw sampler of the node blackbox (plan_26073109 §2.4·§2.5, SoC thermal zones
//! plan_26082319 §5.3). Same command line, exit codes, texts and files as the original: no batching (every sample is
//! written, flushed and fsynced at once), the compact `easy-vllm-bb ...` line to `/dev/kmsg` for the off-box path,
//! `nvidia-smi -l` spawned once and read non-blocking (respawned when it goes silent), the CSV schema with its
//! header-drift boundary line, and directories/files made with the owner of the nearest existing ancestor.
//!
//! Signals: SIGINT ends the loop the way the original's `KeyboardInterrupt` does (the GPU child is closed, the sample
//! count printed, status 0); other signals keep their default action, as in the original.
//!
//! Exit codes: 0 = normal · 1 = precondition failed · 2 = self-test failed (argparse usage errors are 2 as well).

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, pct, Fail, R};
use crate::py;
use crate::raise::Raised;
use agent_runtime_support::pynumber;
use agent_runtime_support::pystr::PyStr;
use std::io::{Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

pub const CSV_HEADER: &str =
    "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n,soc_temp,soc_zone\n";
pub const KMSG_PREFIX: &str = "easy-vllm-bb";
pub const GPU_QUERY: &str = "temperature.gpu,power.draw,clocks.sm,utilization.gpu,memory.used";
pub const THERMAL_ROOT: &str = "/sys/class/thermal";
/// Plausibility band of a SoC zone reading (°C); out-of-band zones are treated as unread. Single owner of the band
/// (`blackbox_thermal` derives it).
pub const SOC_TEMP_PLAUSIBLE_C: (i64, i64) = (-40, 150);

/// A sample value as the original holds it: absent (`None`), an int or a float.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum V {
    #[default]
    N,
    I(i64),
    F(f64),
}

fn opt_i(v: Option<i64>) -> V {
    v.map_or(V::N, V::I)
}
fn opt_f(v: Option<f64>) -> V {
    v.map_or(V::N, V::F)
}

/// `time.time()`.
pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn read_text_lossy(path: &str) -> Option<String> {
    let b = std::fs::read(path).ok()?;
    let t = std::str::from_utf8(&b).ok()?;
    Some(bb::universal_newlines(t))
}

/// `read_mem_avail_mib(path)`: MemAvailable in MiB; None on failure (never a silent 0).
pub fn read_mem_avail_mib(path: &str) -> Option<i64> {
    let text = read_text_lossy(path)?;
    for line in bb::text_lines(&text) {
        if line.starts_with("MemAvailable:") {
            let f = py::split_ws(line);
            return Some(bb::py_int_text(f.get(1)?)?.div_euclid(1024));
        }
    }
    None
}

/// `read_load1(path)`.
pub fn read_load1(path: &str) -> Option<f64> {
    let text = read_text_lossy(path)?;
    let f = py::split_ws(&text);
    pynumber::float_from_str(&PyStr::new(f.first()?)).ok()
}

/// `parse_gpu_line(line)`: gpu_temp, gpu_pwr, gpu_sm, gpu_util, gpu_mem; None for `{}`.
pub fn parse_gpu_line(line: &str) -> Option<[V; 5]> {
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split(',').map(py::strip).collect();
    if parts.len() < 5 {
        return None;
    }
    let mut out = [V::N; 5];
    for (i, v) in parts.iter().take(5).enumerate() {
        out[i] = if v.contains('.') {
            pynumber::float_from_str(&PyStr::new(v)).map_or(V::N, V::F)
        } else {
            bb::py_int_text(v).map_or(V::N, V::I)
        };
    }
    Some(out)
}

/// The sysfs thermal zones, enumerated once.
pub struct ThermalZones {
    pub root: String,
    pub zones: Vec<(i64, String, String)>,
    pub enabled: bool,
}

impl ThermalZones {
    pub fn new(root: &str) -> ThermalZones {
        let mut names: Vec<String> = std::fs::read_dir(root)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.starts_with("thermal_zone"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        let mut zones = vec![];
        for n in names {
            let Some(idx) = bb::py_int_text(&n["thermal_zone".len()..]) else {
                continue;
            };
            let dir = bb::join(root, &n);
            let tpath = bb::join(&dir, "temp");
            if std::fs::metadata(&tpath).is_err() {
                continue;
            }
            let ztype = read_text_lossy(&bb::join(&dir, "type"))
                .map(|t| py::strip(&t).to_string())
                .unwrap_or_default();
            zones.push((idx, tpath, ztype));
        }
        let enabled = !zones.is_empty();
        ThermalZones {
            root: root.to_string(),
            zones,
            enabled,
        }
    }

    pub fn types(&self) -> Vec<(i64, String)> {
        self.zones.iter().map(|(i, _, t)| (*i, t.clone())).collect()
    }

    /// (hottest plausible zone in °C, its index) or (None, None).
    pub fn read_max(&self) -> (Option<i64>, Option<i64>) {
        let (lo, hi) = SOC_TEMP_PLAUSIBLE_C;
        let (mut best_c, mut best_idx): (Option<i64>, Option<i64>) = (None, None);
        for (idx, tpath, _) in &self.zones {
            let Some(text) = read_text_lossy(tpath) else {
                continue;
            };
            let Some(milli) = bb::py_int_text(py::strip(&text)) else {
                continue;
            };
            let c = milli.div_euclid(1000);
            if c < lo || c > hi {
                continue;
            }
            if best_c.is_none_or(|b| c > b) {
                best_c = Some(c);
                best_idx = Some(*idx);
            }
        }
        (best_c, best_idx)
    }
}

/// `compute_rate(prev, cur, dt)`: positive = falling; None without a previous sample or a usable interval.
pub fn compute_rate(prev: Option<i64>, cur: Option<i64>, dt: Option<f64>) -> Option<f64> {
    let (Some(p), Some(c), Some(d)) = (prev, cur, dt) else {
        return None;
    };
    if d == 0.0 || d <= 0.0 {
        return None;
    }
    Some(pynumber::round_to_places((p - c) as f64 / d, 2))
}

/// One sample (keys absent from the original's dict are `V::N`).
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub ts: Option<i64>,
    pub mem_avail: V,
    pub mem_rate: V,
    pub gpu_temp: V,
    pub gpu_pwr: V,
    pub gpu_sm: V,
    pub gpu_util: V,
    pub gpu_mem: V,
    pub load1: V,
    pub ctr_n: V,
    pub soc_temp: V,
    pub soc_zone: V,
}

impl Sample {
    fn set_gpu(&mut self, g: Option<[V; 5]>) {
        if let Some(g) = g {
            self.gpu_temp = g[0];
            self.gpu_pwr = g[1];
            self.gpu_sm = g[2];
            self.gpu_util = g[3];
            self.gpu_mem = g[4];
        }
    }
}

fn fv(v: V, absent: &str) -> String {
    match v {
        V::N => absent.to_string(),
        V::I(n) => n.to_string(),
        V::F(x) => pct("%g", x),
    }
}

pub fn format_csv_row(s: &Sample) -> String {
    let f = |v: V| fv(v, "");
    [
        s.ts.map_or(String::new(), |t| t.to_string()),
        f(s.mem_avail),
        f(s.mem_rate),
        f(s.gpu_temp),
        f(s.gpu_pwr),
        f(s.gpu_sm),
        f(s.gpu_util),
        f(s.gpu_mem),
        f(s.load1),
        f(s.ctr_n),
        f(s.soc_temp),
        f(s.soc_zone),
    ]
    .join(",")
        + "\n"
}

pub fn format_kmsg(s: &Sample) -> String {
    let f = |v: V| fv(v, "na");
    format!(
        "{} ts={} mem={} rate={} t={} w={} s={}",
        KMSG_PREFIX,
        s.ts.map_or(String::new(), |t| t.to_string()),
        f(s.mem_avail),
        f(s.mem_rate),
        f(s.gpu_temp),
        f(s.gpu_pwr),
        f(s.soc_temp)
    )
}

enum Src {
    Real {
        child: Child,
        out: ChildStdout,
        buf: Vec<u8>,
    },
    /// The self-test's stand-in for a child (its output is a fixed text).
    Fake { data: Vec<u8> },
}

/// `nvidia-smi -l N` streaming: spawned once, read non-blocking, respawned when silent past `stale_after_s`.
pub struct GpuStream {
    proc_: Option<Src>,
    pub last: String,
    pub interval: i64,
    pub stale_after_s: f64,
    pub last_ts: Option<f64>,
    pub respawns: i64,
    pub enabled: bool,
    fake_spawn: bool,
}

pub const STALE_MULTIPLIER: i64 = 15;

fn set_nonblocking(fd: i32) {
    extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const O_NONBLOCK: i32 = 0o4000;
    unsafe {
        let fl = fcntl(fd, F_GETFL);
        if fl >= 0 {
            fcntl(fd, F_SETFL, fl | O_NONBLOCK);
        }
    }
}

impl GpuStream {
    pub fn new(interval: i64, enabled: bool) -> GpuStream {
        let interval = interval.max(1);
        let mut g = GpuStream {
            proc_: None,
            last: String::new(),
            interval,
            stale_after_s: (interval * STALE_MULTIPLIER) as f64,
            last_ts: None,
            respawns: 0,
            enabled: enabled && crate::proc::which("nvidia-smi").is_some(),
            fake_spawn: false,
        };
        if !g.enabled {
            return g;
        }
        g.spawn();
        g.last_ts = if g.proc_.is_some() { Some(now()) } else { None };
        g
    }

    fn spawn(&mut self) {
        if self.fake_spawn {
            self.proc_ = Some(Src::Fake { data: vec![] });
            return;
        }
        let r = Command::new("nvidia-smi")
            .arg(format!("--query-gpu={GPU_QUERY}"))
            .arg("--format=csv,noheader,nounits")
            .arg("-l")
            .arg(self.interval.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        match r {
            Ok(mut child) => {
                let out = child.stdout.take().expect("piped stdout");
                use std::os::unix::io::AsRawFd;
                set_nonblocking(out.as_raw_fd());
                self.proc_ = Some(Src::Real {
                    child,
                    out,
                    buf: vec![],
                });
                self.respawns += 1;
            }
            Err(_) => {
                self.proc_ = None;
                self.enabled = false;
            }
        }
    }

    /// Complete lines available now (the most recent one wins).
    fn drain(&mut self) -> Vec<String> {
        let mut lines = vec![];
        match self.proc_.as_mut() {
            None => {}
            Some(Src::Fake { data }) => {
                let text = String::from_utf8_lossy(data).into_owned();
                data.clear();
                lines.extend(bb::text_lines(&text).into_iter().map(String::from));
            }
            Some(Src::Real { out, buf, .. }) => {
                let mut chunk = [0u8; 4096];
                loop {
                    match out.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        Err(_) => break,
                    }
                }
                while let Some(i) = buf.iter().position(|&b| b == b'\n') {
                    let l: Vec<u8> = buf.drain(..=i).collect();
                    lines.push(String::from_utf8_lossy(&l).into_owned());
                }
            }
        }
        lines
    }

    pub fn poll(&mut self) -> Option<[V; 5]> {
        self.proc_.as_ref()?;
        let mut got = false;
        for line in self.drain() {
            self.last = py::strip(&line).to_string();
            got = true;
        }
        let t = now();
        if got {
            self.last_ts = Some(t);
        } else if let Some(lt) = self.last_ts.filter(|lt| *lt != 0.0) {
            if t - lt > self.stale_after_s {
                let age = t - lt;
                eprintln!(
                    "[collect] 경고: GPU 스트림 {}s 무출력 — 자식을 재기동한다(낡은 값을 신선한 것처럼 싣지 않는다)",
                    pct("%.0f", age)
                );
                self.last.clear();
                self.respawn();
            }
        }
        parse_gpu_line(&self.last)
    }

    fn respawn(&mut self) {
        self.close();
        self.proc_ = None;
        self.spawn();
        self.last_ts = if self.proc_.is_some() { Some(now()) } else { None };
    }

    /// terminate, wait up to 3 s, then kill.
    pub fn close(&mut self) {
        if let Some(Src::Real { child, .. }) = self.proc_.as_mut() {
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            unsafe {
                kill(child.id() as i32, 15);
            }
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if start.elapsed() < std::time::Duration::from_secs(3) => {
                        std::thread::sleep(std::time::Duration::from_millis(10))
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
    }
}

/// `container_count()`: `docker ps -q` lines; None when docker cannot be run.
pub fn container_count() -> Option<i64> {
    let o = crate::proc::run(
        &["docker".to_string(), "ps".to_string(), "-q".to_string()],
        None,
        Some(5),
    )
    .ok()?;
    Some(o.stdout.split('\n').filter(|x| !py::strip(x).is_empty()).count() as i64)
}

/// Daily rollover + fsync of every sample.
pub struct SampleWriter {
    dir: String,
    fsync: bool,
    day: Option<String>,
    fh: Option<std::fs::File>,
}

impl SampleWriter {
    pub fn new(samples_dir: &str, fsync: bool) -> Result<SampleWriter, Raised> {
        bb::makedirs_as_ancestor_owner(samples_dir)?;
        Ok(SampleWriter {
            dir: samples_dir.to_string(),
            fsync,
            day: None,
            fh: None,
        })
    }

    /// Does the file already hold the current schema header? Unreadable = True (writes fewer headers).
    fn has_current_header(path: &str) -> bool {
        match std::fs::read(path) {
            Ok(b) => {
                let t = bb::universal_newlines(&String::from_utf8_lossy(&b));
                bb::text_lines(&t).into_iter().any(|l| l == CSV_HEADER)
            }
            Err(_) => true,
        }
    }

    fn rotate(&mut self, day: &str) -> Result<(), Raised> {
        self.fh = None;
        let path = bb::join(&self.dir, &format!("{day}.csv"));
        let new = std::fs::metadata(&path).map_or(true, |m| m.len() == 0);
        let drift = !new && !Self::has_current_header(&path);
        let mut fh = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| Raised::os(&e, &path))?;
        bb::inherit_dir_owner(&path, &self.dir);
        if new || drift {
            fh.write_all(CSV_HEADER.as_bytes())
                .map_err(|e| Raised::os(&e, &path))?;
        }
        self.fh = Some(fh);
        self.day = Some(day.to_string());
        Ok(())
    }

    pub fn write(&mut self, day: &str, row: &str) -> Result<(), Raised> {
        if self.day.as_deref() != Some(day) {
            self.rotate(day)?;
        }
        let fh = self.fh.as_mut().expect("rotated");
        fh.write_all(row.as_bytes())
            .and_then(|_| fh.flush())
            .map_err(|e| Raised::new("OSError", &e.to_string()))?;
        if self.fsync {
            fh.sync_all()
                .map_err(|e| Raised::new("OSError", &e.to_string()))?;
        }
        Ok(())
    }

    pub fn close(&mut self) {
        self.fh = None;
    }
}

/// `write_kmsg(text, path)`: False (never an exception) when it cannot be written.
pub fn write_kmsg(text: &str, path: &str) -> bool {
    match std::fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut f) => f.write_all(format!("{text}\n").as_bytes()).is_ok(),
        Err(_) => false,
    }
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn install_sigint() {
    extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }
    unsafe {
        signal(2, on_sigint);
    }
}

/// The run loop's configuration (the original's keyword arguments).
pub struct RunCfg<'a> {
    pub interval: f64,
    pub use_kmsg: bool,
    pub fsync: bool,
    pub max_samples: Option<i64>,
    pub docker_every: i64,
    pub kmsg_path: &'a str,
    pub thermal_root: &'a str,
}

fn interruptible_sleep(secs: f64) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs_f64(secs.max(0.0));
    while !INTERRUPTED.load(Ordering::SeqCst) {
        let now = std::time::Instant::now();
        if now >= end {
            break;
        }
        std::thread::sleep((end - now).min(std::time::Duration::from_millis(50)));
    }
}

pub fn run(node_dir: &str, c: &RunCfg) -> Result<i64, Raised> {
    let mut writer = SampleWriter::new(&bb::join(node_dir, "samples"), c.fsync)?;
    let mut gpu = GpuStream::new(c.interval.max(1.0) as i64, true);
    let soc = ThermalZones::new(c.thermal_root);
    if soc.enabled {
        let list: Vec<String> = soc.types().iter().map(|(i, t)| format!("{i}:{t}")).collect();
        eprintln!("[collect] SoC 열 zone {}개: {}", soc.zones.len(), list.join(", "));
    } else {
        eprintln!(
            "[collect] SoC 열 zone 없음({}) — soc_temp/soc_zone 은 상시 빈 칸(부재)",
            c.thermal_root
        );
    }
    let (mut prev_mem, mut prev_t, mut n, mut ctr_n): (Option<i64>, Option<f64>, i64, Option<i64>) =
        (None, None, 0, None);
    let mut kmsg_ok: Option<bool> = None;
    let res = (|| -> Result<(), Raised> {
        while c.max_samples.is_none_or(|m| n < m) {
            if INTERRUPTED.load(Ordering::SeqCst) {
                break;
            }
            let t0 = now();
            let mem = read_mem_avail_mib("/proc/meminfo");
            if n % c.docker_every == 0 {
                ctr_n = container_count();
            }
            let (soc_c, soc_idx) = soc.read_max();
            let mut s = Sample {
                ts: Some(t0 as i64),
                mem_avail: opt_i(mem),
                mem_rate: opt_f(compute_rate(
                    prev_mem,
                    mem,
                    prev_t.filter(|p| *p != 0.0).map(|p| t0 - p),
                )),
                load1: opt_f(read_load1("/proc/loadavg")),
                ctr_n: opt_i(ctr_n),
                soc_temp: opt_i(soc_c),
                soc_zone: opt_i(soc_idx),
                ..Default::default()
            };
            s.set_gpu(gpu.poll());
            let day = crate::pytime::DateTime::from_epoch(t0.floor() as i64).date_iso();
            writer.write(&day, &format_csv_row(&s))?;
            if c.use_kmsg {
                let ok = write_kmsg(&format_kmsg(&s), c.kmsg_path);
                if kmsg_ok.is_none() {
                    kmsg_ok = Some(ok);
                    if !ok {
                        eprintln!(
                            "[collect] 경고: {} 기록 불가(비-root?) — 오프박스 경로 없음",
                            c.kmsg_path
                        );
                    }
                }
            }
            let elapsed = now() - t0;
            if c.interval > 0.0 && elapsed > c.interval * 5.0 {
                eprintln!(
                    "[collect] 경고: 한 샘플 주기가 {}s 걸렸다(간격 {}s) — 외부 호출 지연 의심",
                    pct("%.1f", elapsed),
                    pct("%.1f", c.interval)
                );
            }
            prev_mem = mem;
            prev_t = Some(t0);
            n += 1;
            interruptible_sleep(c.interval - (now() - t0));
        }
        Ok(())
    })();
    gpu.close();
    writer.close();
    res?;
    Ok(n)
}

fn self_test() -> i32 {
    let mut checks: Vec<(String, bool)> = vec![];
    let mut c = |n: &str, b: bool| checks.push((n.to_string(), b));
    let td = bb::TempDir::new();
    let t = td.path().to_string();
    let w = |p: &str, s: &str| {
        let _ = std::fs::write(p, s);
    };
    let mi = bb::join(&t, "meminfo");
    w(&mi, "MemTotal:  131072000 kB\nMemAvailable:  12058624 kB\nCached: 1 kB\n");
    c("MemAvailable MiB 파싱", read_mem_avail_mib(&mi) == Some(12058624 / 1024));
    w(&mi, "MemTotal: 1 kB\n");
    c("MemAvailable 부재 -> None(0 대체 금지)", read_mem_avail_mib(&mi).is_none());
    c("파일 부재 -> None", read_mem_avail_mib(&bb::join(&t, "nope")).is_none());
    let la = bb::join(&t, "loadavg");
    w(&la, "1.31 0.90 0.72 2/1234 5678\n");
    c("load1 파싱", read_load1(&la) == Some(1.31));
    let g = parse_gpu_line("54, 31.20, 1275, 0, 2048");
    c(
        "GPU 파싱",
        g == Some([V::I(54), V::F(31.2), V::I(1275), V::I(0), V::I(2048)]),
    );
    let g2 = parse_gpu_line("54, [N/A], 1275, 0, 2048");
    c("GPU N/A -> None(정직 부재)", g2.is_some_and(|g| g[1] == V::N));
    c("GPU 빈 입력 -> {}", parse_gpu_line("").is_none());
    c("GPU 필드부족 -> {}", parse_gpu_line("54, 31.2").is_none());

    let tz = bb::join(&t, "thermal");
    let mkzone = |idx: i64, milli: i64, ztype: &str| {
        let d = bb::join(&tz, &format!("thermal_zone{idx}"));
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(bb::join(&d, "temp"), format!("{milli}\n"));
        let _ = std::fs::write(bb::join(&d, "type"), format!("{ztype}\n"));
    };
    mkzone(0, 46800, "acpitz");
    mkzone(1, 45000, "acpitz");
    mkzone(5, 91200, "acpitz");
    mkzone(6, 45800, "acpitz");
    let z = ThermalZones::new(&tz);
    c("SoC zone 열거(4개)", z.zones.len() == 4 && z.enabled);
    c(
        "SoC zone type 보존",
        z.types().iter().find(|x| x.0 == 5).is_some_and(|x| x.1 == "acpitz"),
    );
    c("SoC 최고 zone·온도(91C @zone5)", z.read_max() == (Some(91), Some(5)));
    let z0 = ThermalZones::new(&bb::join(&t, "no-thermal"));
    c(
        "SoC zone 부재 -> (None,None)·enabled False",
        z0.read_max() == (None, None) && !z0.enabled,
    );
    mkzone(7, 2147483647, "acpitz");
    mkzone(8, -274000, "acpitz");
    let z2 = ThermalZones::new(&tz);
    c(
        "SoC 타당성 밴드 밖 zone 무시(잡음이 최대값을 먹지 않음)",
        z2.read_max() == (Some(91), Some(5)),
    );
    let t5 = bb::join(&bb::join(&tz, "thermal_zone5"), "temp");
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&t5, std::fs::Permissions::from_mode(0o000));
        let z3 = ThermalZones::new(&tz);
        let unreadable_ok = if bb::geteuid() != 0 {
            z3.read_max() == (Some(46), Some(0))
        } else {
            true
        };
        c("SoC 판독 불가 zone 은 건너뛰고 나머지로 판정", unreadable_ok);
        let _ = std::fs::set_permissions(&t5, std::fs::Permissions::from_mode(0o644));
    }
    let _ = std::fs::create_dir_all(bb::join(&tz, "cooling_device0"));
    c("cooling_device 는 zone 이 아니다", ThermalZones::new(&tz).zones.len() == 6);

    c(
        "하강 -> 양수",
        compute_rate(Some(40104), Some(11419), Some(15.0))
            == Some(pynumber::round_to_places(28685.0 / 15.0, 2)),
    );
    c(
        "상승 -> 음수",
        compute_rate(Some(10186), Some(117283), Some(1.0)).is_some_and(|r| r < 0.0),
    );
    c("prev 없음 -> None", compute_rate(None, Some(100), Some(1.0)).is_none());
    c("dt 0 -> None", compute_rate(Some(200), Some(100), Some(0.0)).is_none());

    let row = format_csv_row(&Sample {
        ts: Some(1785),
        mem_avail: V::I(117380),
        mem_rate: V::F(-12.0),
        gpu_temp: V::I(54),
        gpu_pwr: V::F(31.2),
        load1: V::F(1.31),
        ..Default::default()
    });
    let cols: Vec<&str> = py::strip(CSV_HEADER).split(',').collect();
    let rc = |r: &str| -> Vec<String> { py::strip(r).split(',').map(String::from).collect() };
    c("CSV 헤더와 열 수 일치", rc(&row).len() == cols.len());
    c("CSV 부재 필드 빈칸", rc(&row)[6].is_empty());
    c(
        "CSV 스키마에 soc_temp·soc_zone 존재",
        cols[cols.len() - 2..] == ["soc_temp", "soc_zone"],
    );
    c("SoC 부재 -> 빈 칸 2개", rc(&row)[10..] == ["".to_string(), "".to_string()]);
    let row2 = format_csv_row(&Sample {
        ts: Some(1785),
        soc_temp: V::I(91),
        soc_zone: V::I(5),
        ..Default::default()
    });
    c("SoC 값 기록", rc(&row2)[10..] == ["91".to_string(), "5".to_string()]);
    let row3 = format_csv_row(&Sample {
        ts: Some(1),
        soc_temp: V::I(46),
        soc_zone: V::I(0),
        ..Default::default()
    });
    c(
        "soc_zone=0 이 빈 칸으로 삼켜지지 않음",
        rc(&row3).last().is_some_and(|x| x == "0"),
    );
    let km = format_kmsg(&Sample {
        ts: Some(1785),
        mem_avail: V::I(117380),
        mem_rate: V::F(-12.0),
        gpu_temp: V::I(54),
        gpu_pwr: V::F(31.2),
        ..Default::default()
    });
    c("kmsg 접두어", km.starts_with(&format!("{KMSG_PREFIX} ")));
    c("kmsg 압축 길이 <120B", km.chars().count() < 120);
    c(
        "kmsg 부재는 na",
        format_kmsg(&Sample {
            ts: Some(1),
            ..Default::default()
        })
        .contains("na"),
    );
    let km2 = format_kmsg(&Sample {
        ts: Some(1785),
        mem_avail: V::I(19251),
        mem_rate: V::I(0),
        gpu_temp: V::I(63),
        gpu_pwr: V::F(90.33),
        soc_temp: V::I(91),
        ..Default::default()
    });
    c("kmsg 에 SoC 열 포함(s=)", km2.contains(" s=91"));
    c("kmsg SoC 포함해도 <120B", km2.chars().count() < 120);
    c(
        "kmsg SoC 부재는 s=na",
        format_kmsg(&Sample {
            ts: Some(1),
            ..Default::default()
        })
        .contains(" s=na"),
    );

    let sd = bb::join(&bb::join(&t, "node"), "samples");
    let rd = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    if let Ok(mut wr) = SampleWriter::new(&sd, true) {
        let _ = wr.write("2026-07-31", "a\n");
        let _ = wr.write("2026-07-31", "b\n");
        let _ = wr.write("2026-08-01", "c\n");
        wr.close();
    }
    let d1 = rd(&bb::join(&sd, "2026-07-31.csv"));
    let d2 = rd(&bb::join(&sd, "2026-08-01.csv"));
    c("일자 롤오버", bb::is_file(&bb::join(&sd, "2026-08-01.csv")));
    c(
        "헤더 파일당 1회",
        d1.matches(CSV_HEADER).count() == 1 && d2.matches(CSV_HEADER).count() == 1,
    );
    c("행 append", d1.ends_with("a\nb\n"));
    if let Ok(mut wr) = SampleWriter::new(&sd, true) {
        let _ = wr.write("2026-07-31", "d\n");
        wr.close();
    }
    c(
        "재기동 시 헤더 중복 없음",
        rd(&bb::join(&sd, "2026-07-31.csv")).matches(CSV_HEADER).count() == 1,
    );
    let legacy = bb::join(&sd, "2026-06-01.csv");
    let old_hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n";
    w(&legacy, &format!("{old_hdr}1,2,3,4,5,6,7,,8,9\n"));
    if let Ok(mut wr) = SampleWriter::new(&sd, true) {
        let _ = wr.write("2026-06-01", "x\n");
        wr.close();
    }
    let txt = rd(&legacy);
    c(
        "옛 헤더 파일 -> 새 헤더 1줄 삽입(경계 표시)",
        txt.matches(CSV_HEADER).count() == 1 && txt.starts_with(old_hdr) && txt.ends_with("x\n"),
    );
    if let Ok(mut wr) = SampleWriter::new(&sd, true) {
        let _ = wr.write("2026-06-01", "y\n");
        wr.close();
    }
    c(
        "드리프트 없는 재기동은 헤더를 더 쓰지 않음",
        rd(&legacy).matches(CSV_HEADER).count() == 1,
    );
    c(
        "kmsg 쓰기 불가 -> False(예외 아님)",
        !write_kmsg("x", &format!("{t}/no/such/kmsg")),
    );

    let nd = bb::join(&t, "node2");
    let km_path = bb::join(&t, "fake_kmsg");
    w(&km_path, "");
    let today = || crate::pytime::DateTime::from_epoch(now().floor() as i64).date_iso();
    let mut sizes = vec![];
    for _ in 0..3 {
        let cfg = RunCfg {
            interval: 0.0,
            use_kmsg: true,
            fsync: true,
            max_samples: Some(1),
            docker_every: 5,
            kmsg_path: &km_path,
            thermal_root: &tz,
        };
        let _ = run(&nd, &cfg);
        let p = bb::join(&bb::join(&nd, "samples"), &format!("{}.csv", today()));
        sizes.push(std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0));
    }
    c(
        "샘플마다 즉시 기록(배칭 없음)",
        sizes[0] < sizes[1] && sizes[1] < sizes[2],
    );
    let kml = rd(&km_path);
    c(
        "kmsg 매 샘플 송출",
        py::strip(&kml).split('\n').count() == 3,
    );
    let written = rd(&bb::join(&bb::join(&nd, "samples"), &format!("{}.csv", today())));
    let last: Vec<String> = py::strip(&written)
        .split('\n')
        .next_back()
        .unwrap_or("")
        .split(',')
        .map(String::from)
        .collect();
    let n = last.len();
    c(
        "run() 이 CSV 에 SoC 를 실제로 기록(배선 실증)",
        n >= 2 && last[n - 2] == "91" && last[n - 1] == "5",
    );
    c(
        "run() 이 kmsg 에 SoC 를 실제로 송출(배선 실증)",
        py::strip(&kml)
            .split('\n')
            .next_back()
            .is_some_and(|l| l.contains(" s=91")),
    );
    let nd2 = bb::join(&t, "node3");
    let no_thermal = bb::join(&t, "no-thermal");
    let cfg = RunCfg {
        interval: 0.0,
        use_kmsg: false,
        fsync: false,
        max_samples: Some(1),
        docker_every: 5,
        kmsg_path: "/dev/kmsg",
        thermal_root: &no_thermal,
    };
    let _ = run(&nd2, &cfg);
    let l2: Vec<String> = py::strip(&rd(&bb::join(
        &bb::join(&nd2, "samples"),
        &format!("{}.csv", today()),
    )))
    .split('\n')
    .next_back()
    .unwrap_or("")
    .split(',')
    .map(String::from)
    .collect();

    // GPU stream freshness self-healing, with a stand-in child
    let fake = |data: &str| Some(Src::Fake {
        data: data.as_bytes().to_vec(),
    });
    let mut gs = GpuStream {
        proc_: fake(""),
        last: "50, 90.0, 1000, 30, 1024".into(),
        interval: 1,
        stale_after_s: 5.0,
        last_ts: Some(now()),
        respawns: 1,
        enabled: true,
        fake_spawn: true,
    };
    let fresh = gs.poll();
    c(
        "무출력이지만 임계 이내면 마지막 값을 유지(정상 지터 오탐 ✗)",
        fresh.is_some_and(|g| g[0] == V::I(50)),
    );
    gs.last_ts = Some(now() - 99.0);
    let stale = gs.poll();
    c("무출력이 임계를 넘으면 낡은 값을 버린다(거짓 신선도 ✗)", stale.is_none());
    c(
        "무출력이 임계를 넘으면 자식을 되살린다",
        gs.respawns >= 1 && gs.proc_.is_some(),
    );
    let mut gs2 = GpuStream {
        proc_: fake("51, 91.0, 1001, 31, 1025\n"),
        last: String::new(),
        interval: 1,
        stale_after_s: 5.0,
        last_ts: Some(now() - 99.0),
        respawns: 0,
        enabled: true,
        fake_spawn: true,
    };
    let got = gs2.poll();
    c(
        "출력이 있으면 임계를 넘겨 늙었어도 재기동하지 않는다(대조군)",
        got.is_some_and(|g| g[0] == V::I(51)) && gs2.respawns == 0,
    );
    let n2 = l2.len();
    c(
        "zone 부재 노드는 SoC 빈 칸(음성대조)",
        n2 >= 2 && l2[n2 - 2].is_empty() && l2[n2 - 1].is_empty(),
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

const A_NODE: usize = 1;
const A_INTERVAL: usize = 2;
const A_NOKMSG: usize = 3;
const A_NOFSYNC: usize = 4;
const A_MAX: usize = 5;
const A_SELF: usize = 6;

static ARGS: [Arg; 7] = [
    HELP,
    store(&["--node-dir"]),
    store(&["--interval"]).float(),
    flag(&["--no-kmsg"]),
    flag(&["--no-fsync"]),
    store(&["--max-samples"]).int(),
    flag(&["--self-test"]),
];

pub static SPEC: Spec = Spec {
    prog: "blackbox_collect.py",
    usage: include_str!("../help/blackbox_collect.usage"),
    help: include_str!("../help/blackbox_collect.help"),
    args: &ARGS,
    subs: None,
};

fn main_run(a: &argp::Parsed) -> R<i32> {
    if a.flag(A_SELF) {
        return Ok(self_test());
    }
    let Some(nd) = a.text(A_NODE).filter(|s| !s.is_empty()) else {
        return Err(bb::ap_error(&SPEC, "--node-dir 가 필요합니다"));
    };
    install_sigint();
    let cfg = RunCfg {
        interval: a.float(A_INTERVAL).unwrap_or(1.0),
        use_kmsg: !a.flag(A_NOKMSG),
        fsync: !a.flag(A_NOFSYNC),
        max_samples: a.int(A_MAX),
        docker_every: 5,
        kmsg_path: "/dev/kmsg",
        thermal_root: THERMAL_ROOT,
    };
    let n = run(nd, &cfg).map_err(Fail::Raise)?;
    println!("[collect] {n} 샘플 기록 -> {nd}/samples");
    Ok(0)
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(a) => bb::finish(main_run(&a)),
    }
}
