// Regression lock (h30): the Rust forms the review validators of rounds 1 and 2 ran, each with its source.
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

// r1 v1-control-call · v1-arc-fnvalue · v1-get-or-init
fn v1_call() { let _ = Instant::now(); }
fn v1_arc() { let _c = Arc::new(Instant::now); }
static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
fn v1_once() -> Instant { *START.get_or_init(Instant::now) }

// r1 v1-unwrap-or-else · v1-wrapper-value · v1-let-fn-ptr · v1-qualified-path · v1-map-fnvalue
fn v1_unwrap(x: Option<std::time::SystemTime>) -> std::time::SystemTime { x.unwrap_or_else(std::time::SystemTime::now) }
fn v1_wrapper(x: Option<UtcInstant>) -> UtcInstant { x.unwrap_or_else(UtcInstant::now) }
fn v1_fn_ptr() { let clock = std::time::SystemTime::now; let _t = clock(); }
fn v1_qualified() { let _t = <std::time::SystemTime>::now(); let _u = <Instant as Clock>::now(); }
fn v1_map(v: Vec<u8>) { let _t: Vec<_> = v.iter().map(|_| ()).map(|_| 0).collect(); let _s = Some(()).map(|_| std::time::Instant::now()); let _r = None.map_or_else(std::time::Instant::now, |x| x); }

// r1 v2-modified-control · v2-mtime-family; W8 `.atime_nsec()`
use std::os::unix::fs::MetadataExt;
fn v2_modified(p: &std::path::Path) { let _m = std::fs::metadata(p).unwrap().modified(); }
fn v2_family(p: &std::path::Path) -> bool {
    let m = std::fs::metadata(p).unwrap();
    let _a = m.mtime(); let _b = m.mtime_nsec(); let _c = m.atime(); let _d = m.ctime(); let _e = m.ctime_nsec();
    let _f = m.atime_nsec();
    m.mtime() > 1_700_000_000
}

// r1 v4c-rs-literal-control · v4d-rs-argv
fn v4c() { let _ = std::process::Command::new("bash").args(["-c", "date +%s"]).output(); let _ = std::process::Command::new("ssh").args(["h", "date -u +%s"]).output(); }
fn v4d(p: &str, d: &str, h: &str) {
    let _ = Command::new("stat").args(["-c", "%Y", p]).output();
    let _ = Command::new("find").args([d, "-mmin", "-5"]).output();
    let _ = Command::new("git").args(["log", "--since=2.weeks"]).output();
    let _ = Command::new("ssh").args([h, "date", "+%s"]).output();
    let _ = Command::new("date").arg("+%s").output();
    let _ = Command::new("/bin/date").output();
}

// r2 fix l4more (a/x.rs); W8 `args(vec![…])`
fn l4more(p: &str, d: &std::path::Path) {
    let _ = Command::new("stat").arg("-c").arg("%Y").arg(p).output();
    let _ = Command::new("git").current_dir(d).args(&["log", "--since=2.weeks"]).output();
    let _ = Command::new("git").args(vec!["log", "--until=yesterday"]).output();
    let _ = Command::new("find").arg(d).args(["-newer", p]).output();
    let _ = Command::new("ssh").args([p, "date +%s"]).output();
    let _ = Command::new("bash").args(["-c", "n=$(git log --since=2.weeks | wc -l)"]).output();
    let s = "n=$(git log --since=2.weeks | wc -l)";
    let _ = s;
    let _ = Command::new("/usr/bin/stat").args(["--format=%Y", p]).output();
    let _ = Command::new("touch").args(["-d", "now", p]).output();
    let _ = Command::new("stat").args(vec!["-c", "%Z", p]).output();
}

// r2 fix gitargv
fn gitargv() {
    let _ = Command::new("git").args(["log", "--since=2 weeks ago"]).output();
    let _ = Command::new("git").arg("log").arg("--since").arg("2 weeks ago").output();
    let _ = Command::new("sh").arg("-c").arg("x=$(git log --since=2.weeks)").output();
    let unit = "ExecStart=/bin/sh -c 'test $(git log --since=1.day | wc -l) -gt 0'";
    let _ = unit;
}

// r2 L2 w2 (git filters inside substitutions, quotes and unit text)
fn w2() {
    let a = "x=$(git log --since=2.weeks)";
    let b = "git log --since=2.weeks";
    let c = "[Service]\nExecStart=/bin/sh -c 'n=$(git log --since=2.weeks | wc -l)'\n";
    let _ = (a, b, c);
}

// r2 L2 w4 (a value-less -d, a continued literal)
fn w4(day: &str) {
    let _ = std::process::Command::new("date").args(["-d"]).output();
    let _ = std::process::Command::new("date").arg("-d").arg(day).output();
    let s = "date -d \
             yesterday +%s";
    let _ = s;
}

// r2 L2 w5 · w5b (the output of a wrapped bare `date` taken by `.output()`, `.stdout(…)` or a substitution)
fn w5(h: &str) {
    let _ = Command::new("ssh").args([h, "date"]).output();
    let _ = Command::new("sh").args(["-c", "date"]).output();
    let _ = Command::new("ssh").args([h, "date +%s"]).output();
    let _ = Command::new("sh").args(["-c", "date +%s"]).output();
    let _ = Command::new("ssh").arg(h).arg("date").arg("+%s").output();
    let _ = Command::new("ssh").args(["node1", "date"]).output();
    let _ = Command::new("ssh").arg("node1").arg("date").output();
    let s = "x=$(ssh h date)";
    let _ = s;
    let _ = Command::new("ssh").args([h, "date"]).stdout(std::process::Stdio::piped()).spawn();
    let _ = Command::new("ssh").args([h, "date"]).status();
}

// rounds 1-2 plants (reconstructed from the logged path, line and text)
fn plants(p: &std::path::Path) -> bool {
    const SCRIPT: &str = "if [[ \"$(date -d \"$last + 90 days\" +%F)\" < \"$(date -d today +%F)\" ]]; then exit 1; fi";
    let _ = (SCRIPT, UtcInstant::now());
    std::fs::metadata(p).unwrap().mtime() > 1_700_000_000
}

// W8 `now(…)` with an argument; h32 `openssl verify` with and without a given time
fn more(zone: Zone, ca: &str, cert: &str) {
    let _ = Clock::now(zone);
    let _ = Command::new("openssl").args(["verify", "-CAfile"]).arg(ca).arg(cert).output();
    let _ = Command::new("openssl").args(["verify", "-attime", "1735689600", "-CAfile", ca, cert]).output();
    let _ = Command::new("openssl").args(["verify", "-no_check_time", "-CAfile", ca, cert]).output();
    let _ = Command::new("openssl").args(["x509", "-in", cert, "-noout", "-subject_hash"]).output();
}

// r1 v1-base-clock-rs: the two reads of the base policy-distribution clock.rs (lines 20 and 120 there)
impl UtcInstant {
    pub fn now() -> Self {
        let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        Self::from_unix(d.as_secs() as i64, d.subsec_micros())
    }
}
fn observe(now_utc: Option<UtcInstant>) -> UtcInstant {
    now_utc.unwrap_or_else(UtcInstant::now)
}
