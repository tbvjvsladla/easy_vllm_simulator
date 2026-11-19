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
