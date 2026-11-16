//! `node_blackbox/adversarial_stress.py` — the pressure generator for defence tests (plan_26080121 ramp). Not a benchmark:
//! it raises pressure until the node blackbox / agent guard intervene. The stop-file contract: every request checks the
//! file first and stops issuing at once (agent_guard `halt_load`). Exit: 0 normal end · 2 stopped by stop-file ·
//! 3 endpoint lost (connection errors outnumber successes) · 1 missing run arguments.
//!
//! Prompts are the original's exactly: `random.Random(seed).choice(words)` (CPython's MT19937 seeding and `_randbelow`).
//! HTTP is the original's `urllib` request (POST JSON, `Connection: close`) over plain `http://` endpoints; `https://`
//! endpoints are not supported by the port (documented residual).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::py;
use agent_runtime_support::pynumber::format_fixed;
use agent_runtime_support::pyvalue::PyValue;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// CPython `random.Random` (MT19937) for the operations the original uses.
pub struct PyRandom {
    mt: [u32; 624],
    idx: usize,
}

impl PyRandom {
    /// `random.Random(seed)` for an int seed.
    pub fn new(seed: u64) -> Self {
        let mut key: Vec<u32> = vec![];
        let mut n = seed;
        while n > 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        let mut r = PyRandom {
            mt: [0; 624],
            idx: 624,
        };
        r.init_genrand(19650218);
        let (mut i, mut j) = (1usize, 0usize);
        let mut k = 624.max(key.len());
        while k > 0 {
            r.mt[i] = (r.mt[i] ^ (r.mt[i - 1] ^ (r.mt[i - 1] >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = 623;
        while k > 0 {
            r.mt[i] = (r.mt[i] ^ (r.mt[i - 1] ^ (r.mt[i - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            k -= 1;
        }
        r.mt[0] = 0x8000_0000;
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..624 {
            self.mt[i] = 1812433253u32
                .wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        self.idx = 624;
    }

    fn genrand_u32(&mut self) -> u32 {
        if self.idx >= 624 {
            for kk in 0..624 {
                let y = (self.mt[kk] & 0x8000_0000) | (self.mt[(kk + 1) % 624] & 0x7fff_ffff);
                let mut v = self.mt[(kk + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_b0df;
                }
                self.mt[kk] = v;
            }
            self.idx = 0;
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `getrandbits(k)` for 1 <= k <= 32.
    fn getrandbits(&mut self, k: u32) -> u32 {
        self.genrand_u32() >> (32 - k)
    }

    /// `_randbelow(n)` for 0 < n < 2**32.
    pub fn randbelow(&mut self, n: u32) -> u32 {
        let k = 32 - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }
}

const WORDS: [&str; 16] = [
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
    "kilo", "lima", "mike", "november", "oscar", "papa",
];

/// Python `int(x)` of a float (truncation toward zero), clamped to the usable range.
fn trunc(x: f64) -> i64 {
    if x.is_nan() {
        0
    } else {
        x.trunc().clamp(i64::MIN as f64, i64::MAX as f64) as i64
    }
}

/// About `tokens` tokens of non-repeating text (randomised on purpose: repetition would be eaten by the prefix cache).
pub fn filler(tokens: i64, seed: u64) -> String {
    let mut rnd = PyRandom::new(seed);
    let n = 1.max(trunc(tokens as f64 * 0.75));
    (0..n)
        .map(|_| WORDS[rnd.randbelow(16) as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// `base64.b64decode` of a well-formed standard-alphabet text.
pub fn b64decode(t: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let clean: Vec<u8> = t.bytes().filter(|&c| c != b'=').collect();
    let mut out = vec![];
    for ch in clean.chunks(4) {
        let mut n: u32 = 0;
        for (i, &c) in ch.iter().enumerate() {
            n |= (val(c) as u32) << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..ch.len().saturating_sub(1)]);
    }
    out
}
const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

pub struct Params {
    pub model: String,
    pub prompt_tokens: i64,
    pub max_tokens: i64,
    pub multimodal: bool,
}

pub fn payload(a: &Params, seed: u64) -> PyValue {
    let content = if a.multimodal {
        PyValue::List(vec![
            py::dict(vec![
                ("type", py::s("text")),
                ("text", py::s(&filler(a.prompt_tokens, seed))),
            ]),
            py::dict(vec![
                ("type", py::s("image_url")),
                (
                    "image_url",
                    py::dict(vec![(
                        "url",
                        py::s(&format!("data:image/png;base64,{PNG_B64}")),
                    )]),
                ),
            ]),
        ])
    } else {
        py::s(&filler(a.prompt_tokens, seed))
    };
    py::dict(vec![
        ("model", py::s(&a.model)),
        (
            "messages",
            PyValue::List(vec![py::dict(vec![
                ("role", py::s("user")),
                ("content", content),
            ])]),
        ),
        ("max_tokens", PyValue::int(a.max_tokens)),
        ("temperature", PyValue::Float(0.8)),
        ("stream", PyValue::Bool(false)),
    ])
}

#[derive(Default, Clone)]
struct Stat {
    sent: u64,
    ok: u64,
    err: u64,
    conn_err: u64,
    last_err: String,
}

extern "C" {
    fn strerror(errnum: i32) -> *const std::os::raw::c_char;
}
fn errno_text(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let msg = unsafe { std::ffi::CStr::from_ptr(strerror(n)) }
                .to_string_lossy()
                .into_owned();
            format!("[Errno {n}] {msg}")
        }
        None => e.to_string(),
    }
}

enum Fail {
    Http(u16),
    Conn(String),
}

/// One `urllib.request.urlopen(Request(url, data=body, headers=Content-Type JSON), timeout)` and `read()`.
fn post(url: &str, body: &[u8], timeout: f64) -> Result<(), Fail> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| Fail::Conn("<urlopen error unknown url type: https>".into()))?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            (h, p.parse::<u16>().unwrap_or(80))
        }
        _ => (hostport, 80),
    };
    let to = if timeout > 0.0 && timeout.is_finite() {
        Some(Duration::from_secs_f64(timeout))
    } else {
        None
    };
    let urlerr = |e: &std::io::Error| match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
            "<urlopen error timed out>".to_string()
        }
        _ => format!("<urlopen error {}>", errno_text(e)),
    };
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|e| Fail::Conn(format!("<urlopen error {}>", dns_text(&e))))?
        .collect();
    let mut last: Option<std::io::Error> = None;
    let mut stream = None;
    for a in addrs {
        let r = match to {
            Some(t) => TcpStream::connect_timeout(&a, t),
            None => TcpStream::connect(a),
        };
        match r {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last = Some(e),
        }
    }
    let mut s = match stream {
        Some(s) => s,
        None => {
            return Err(Fail::Conn(last.map_or_else(
                || "<urlopen error no address>".to_string(),
                |e| urlerr(&e),
            )))
        }
    };
    let _ = s.set_read_timeout(to);
    let _ = s.set_write_timeout(to);
    let head = format!(
        "POST {path} HTTP/1.1\r\nAccept-Encoding: identity\r\nContent-Type: application/json\r\nContent-Length: {}\r\nHost: {hostport}\r\nUser-Agent: Python-urllib/3.12\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let io = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
            Fail::Conn("timed out".into())
        }
        _ => Fail::Conn(errno_text(&e)),
    };
    s.write_all(head.as_bytes()).map_err(io)?;
    s.write_all(body).map_err(io)?;
    let mut resp = vec![];
    s.read_to_end(&mut resp).map_err(io)?;
    let line = resp.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let code: u16 = String::from_utf8_lossy(line)
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if (200..300).contains(&code) {
        Ok(())
    } else if code == 0 {
        Err(Fail::Conn(
            "Remote end closed connection without response".into(),
        ))
    } else {
        Err(Fail::Http(code))
    }
}

fn dns_text(e: &std::io::Error) -> String {
    let t = e.to_string();
    // std renders getaddrinfo failures as "failed to lookup address information: <gai_strerror>"
    match t.strip_prefix("failed to lookup address information: ") {
        Some(m) if m.contains("Name or service not known") => format!("[Errno -2] {m}"),
        Some(m) if m.contains("Temporary failure") => format!("[Errno -3] {m}"),
        Some(m) => format!("[Errno -5] {m}"),
        None => errno_text(e),
    }
}

fn mem_available() -> String {
    let Ok(t) = std::fs::read_to_string("/proc/meminfo") else {
        return "?".into();
    };
    for line in t.lines() {
        if line.starts_with("MemAvailable:") {
            if let Some(kb) = line
                .split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<i64>().ok())
            {
                return format!("{}", kb.div_euclid(1024));
            }
        }
    }
    "?".into()
}

fn self_test() -> i32 {
    let mut ok = true;
    let mut chk = |c: bool, label: &str| {
        println!("  [{}] {}", if c { "PASS" } else { "FAIL" }, label);
        ok = ok && c;
    };
    let (f1, f2) = (filler(100, 1), filler(100, 2));
    chk(f1 != f2, "seed 별 프롬프트가 다르다(prefix-cache 회피)");
    chk(
        py::split_ws(&f1).len() == 75,
        "prompt_tokens→단어수 근사(0.75배)",
    );
    let mut ns = Params {
        model: "m".into(),
        prompt_tokens: 32,
        max_tokens: 8,
        multimodal: false,
    };
    let content = |p: &PyValue| match py::get(p, "messages") {
        Some(PyValue::List(l)) => l.first().and_then(|m| py::get(m, "content")).cloned(),
        _ => None,
    };
    chk(
        matches!(content(&payload(&ns, 1)), Some(PyValue::Str(_))),
        "텍스트 모드 = 문자열 content",
    );
    ns.multimodal = true;
    let c = content(&payload(&ns, 1));
    chk(
        matches!(&c, Some(PyValue::List(l)) if l.iter().any(|x| matches!(py::get(x, "type"), Some(PyValue::Str(t)) if t.equals("image_url")))),
        "멀티모달 모드 = image_url 포함",
    );
    let png = b64decode(PNG_B64);
    chk(
        !png.is_empty() && png.starts_with(b"\x89PNG\r\n\x1a\n"),
        "내장 PNG 시그니처 유효",
    );
    println!("self-test: {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        2
    }
}

pub static SPEC: Spec = Spec {
    prog: "adversarial_stress.py",
    usage: include_str!("../help/adversarial_stress.usage"),
    help: include_str!("../help/adversarial_stress.help"),
    args: &[
        HELP,
        store(&["--endpoint"]),
        store(&["--model"]),
        store(&["--concurrency"]).int(),
        store(&["--prompt-tokens"]).int(),
        store(&["--max-tokens"]).int(),
        flag(&["--multimodal"]),
        store(&["--duration-s"]).float(),
        store(&["--timeout-s"]).float(),
        store(&["--stop-file"]),
        store(&["--report-s"]).float(),
        flag(&["--self-test"]),
    ],
    subs: None,
};

fn f0(x: f64) -> String {
    format_fixed(x, 0)
}

pub fn main(args: &[String]) -> i32 {
    let p = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if p.flag(11) {
        return self_test();
    }
    let endpoint = p.text(1).unwrap_or("").to_string();
    let model = p.text(2).unwrap_or("").to_string();
    let missing: Vec<&str> = [("--endpoint", &endpoint), ("--model", &model)]
        .iter()
        .filter(|(_, v)| v.is_empty())
        .map(|(f, _)| *f)
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "실행 모드에는 {} 가 필요하다(--self-test 는 예외)",
            missing.join(", ")
        );
        return 1;
    }
    let concurrency = p.int(3).unwrap_or(8);
    let params = Arc::new(Params {
        model,
        prompt_tokens: p.int(4).unwrap_or(4096),
        max_tokens: p.int(5).unwrap_or(256),
        multimodal: p.flag(6),
    });
    let duration = p.float(7).unwrap_or(300.0);
    let timeout = p.float(8).unwrap_or(600.0);
    let stop_file = p.text(9).filter(|s| !s.is_empty()).map(String::from);
    let report = p.float(10).unwrap_or(15.0);
    if let Some(sf) = &stop_file {
        if std::path::Path::new(sf).exists() {
            if let Err(e) = std::fs::remove_file(sf) {
                return crate::raise::exit(&crate::raise::Raised::os(&e, sf));
            }
        }
    }
    println!(
        "[stress] endpoint={} model={} conc={} prompt≈{}tok max_tokens={} mm={} dur={}s",
        endpoint,
        params.model,
        concurrency,
        params.prompt_tokens,
        params.max_tokens,
        if params.multimodal { "True" } else { "False" },
        f0(duration)
    );
    let stop = Arc::new(AtomicBool::new(false));
    let stat = Arc::new(Mutex::new(Stat::default()));
    let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));
    let mut threads = vec![];
    for wid in 0..concurrency.max(0) {
        let (stop, stat, params, url, stop_file) = (
            stop.clone(),
            stat.clone(),
            params.clone(),
            url.clone(),
            stop_file.clone(),
        );
        threads.push(std::thread::spawn(move || {
            let mut seed = (wid as u64) * 7919;
            while !stop.load(Ordering::SeqCst) {
                if let Some(sf) = &stop_file {
                    if std::path::Path::new(sf).exists() {
                        stop.store(true, Ordering::SeqCst);
                        break;
                    }
                }
                seed += 1;
                let body = payload(&params, seed).dumps_ascii().unwrap_or_default();
                stat.lock().unwrap().sent += 1;
                let r = post(&url, body.as_bytes(), timeout);
                let mut s = stat.lock().unwrap();
                match r {
                    Ok(()) => s.ok += 1,
                    Err(Fail::Http(code)) => {
                        s.err += 1;
                        s.last_err = format!("HTTP {code}");
                    }
                    Err(Fail::Conn(m)) => {
                        s.conn_err += 1;
                        s.last_err = py::head(&m, 120);
                    }
                }
            }
        }));
    }
    let t0 = Instant::now();
    let mut last = t0;
    while t0.elapsed().as_secs_f64() < duration && !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
        if last.elapsed().as_secs_f64() >= report {
            last = Instant::now();
            let s = stat.lock().unwrap().clone();
            let t = f0(t0.elapsed().as_secs_f64());
            println!(
                "[stress] t={:>4}s sent={} ok={} err={} conn_err={} mem={}MiB {}",
                t,
                s.sent,
                s.ok,
                s.err,
                s.conn_err,
                mem_available(),
                py::head(&s.last_err, 60)
            );
            let _ = std::io::stdout().flush();
        }
    }
    let stopped_by_file = stop.load(Ordering::SeqCst);
    stop.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(5) * threads.len().max(1) as u32;
    for t in threads {
        while !t.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let s = stat.lock().unwrap().clone();
    println!(
        "[stress] 종료 sent={} ok={} err={} conn_err={}{}",
        s.sent,
        s.ok,
        s.err,
        s.conn_err,
        if stopped_by_file {
            "  ← stop-file 로 중단(가드 halt_load)"
        } else {
            ""
        }
    );
    if stopped_by_file {
        return 2;
    }
    if s.conn_err > s.ok {
        eprintln!(
            "[stress] WARN: 연결오류({}) > 성공({}) — 서빙 사망 의심",
            s.conn_err, s.ok
        );
        return 3;
    }
    0
}
