use std::process::Command as Run;
use std::time::{Instant as Clock, SystemTime};

const REVIEW_PERIOD_DAYS: i64 = 90;

/// SystemTime::now() in a doc comment is no read.
fn reads(path: &std::path::Path) {
    // Instant::now() in a comment is no read.
    let _ = SystemTime::now();
    let started = Clock::now();
    let _ = started.elapsed();
    let _ = std::fs::metadata(path).and_then(|m| m.modified());
    let _ = Run::new("date").arg("+%s").output();
    let script = "ts=$(date -u +%FT%TZ)\nfind /tmp -mmin -5\n";
    let _ = script;
    let _ = "SystemTime::now()";
    let mut keep_days = 3;
    keep_days += 1;
}

extern "C" {
    fn time(out: *mut i64) -> i64;
}

use std::os::unix::fs::MetadataExt;
use crate::clock::now;

static GRACE_DAYS: u32 = 30;

fn more_reads(explicit: Option<UtcInstant>, m: &std::fs::Metadata, d: &Date, p: &str, h: &str) {
    let _ = explicit.unwrap_or_else(UtcInstant::now);
    let _ = std::sync::Arc::new(Clock::now);
    let clock = SystemTime::now;
    let _ = <std::time::SystemTime>::now();
    let _ = <Clock as Tick>::now();
    let _ = (m.mtime(), m.mtime_nsec(), m.atime(), m.ctime_nsec());
    let _ = std::fs::metadata(p).and_then(|m| m.created());
    let _ = (d.add_days(3), d.days_since(d));
    let _ = Run::new("stat").args(["-c", "%Y", p]).output();
    let _ = std::process::Command::new("find").args([p, "-mmin", "-5"]).output();
    let _ = Run::new("git").args(["log", "--since=2.weeks"]).output();
    let _ = Run::new("ssh").args([h, "date", "+%s"]).output();
    let _ = Run::new("bash").arg("-c").arg("date -u +%s").output();
    let _ = Run::new("git").args(["log", "--before={created_s}"]).output();
    let _ = (clock, now);
}
