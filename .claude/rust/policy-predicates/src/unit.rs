//! A minimal unittest-style runner for the two companion suites (`python3 -m unittest` replacement).
//!
//! Exit code: 0 when every case passes, 1 otherwise (unittest's own contract, which is what harness_verify reads).
//! Output (stderr, like unittest's default runner): one `.`/`F`/`E` per case, then a `FAIL:`/`ERROR:` block per failed
//! case, the `Ran N tests in X.XXXs` line and `OK` / `FAILED (failures=N, errors=M)`. The block bodies carry the
//! failure message instead of a Python traceback; the timing is measured. That is the only documented difference.
use crate::support::{Fail, R};

pub struct Case {
    pub module: &'static str,
    pub class: &'static str,
    pub name: &'static str,
    pub f: fn() -> R,
}

/// Run the cases in unittest's discovery order (module, class, method name — all alphabetical).
pub fn run(mut cases: Vec<Case>) -> i32 {
    cases.sort_by(|a, b| (a.module, a.class, a.name).cmp(&(b.module, b.class, b.name)));
    let start = std::time::Instant::now();
    let mut progress = String::new();
    let mut blocks = Vec::new();
    let (mut failures, mut errors) = (0, 0);
    for c in &cases {
        let outcome = std::panic::catch_unwind(c.f);
        let id = format!("{} ({}.{}.{})", c.name, c.module, c.class, c.name);
        match outcome {
            Ok(Ok(())) => progress.push('.'),
            Ok(Err(Fail { kind, message })) => {
                let assertion = kind == "PredicateFailure" || kind == "AssertionError";
                if assertion { failures += 1; progress.push('F'); } else { errors += 1; progress.push('E'); }
                let label = if assertion { "FAIL" } else { "ERROR" };
                let exc = if assertion { "AssertionError".to_string() } else { kind };
                blocks.push(format!("======================================================================\n{label}: {id}\n----------------------------------------------------------------------\n{exc}: {message}\n"));
            }
            Err(_) => {
                errors += 1;
                progress.push('E');
                blocks.push(format!("======================================================================\nERROR: {id}\n----------------------------------------------------------------------\npanic\n"));
            }
        }
    }
    let mut err = String::new();
    err.push_str(&progress);
    err.push('\n');
    for b in &blocks {
        err.push_str(b);
        err.push('\n');
    }
    err.push_str("----------------------------------------------------------------------\n");
    err.push_str(&format!("Ran {} test{} in {:.3}s\n\n", cases.len(), if cases.len() == 1 { "" } else { "s" }, start.elapsed().as_secs_f64()));
    if failures == 0 && errors == 0 {
        err.push_str("OK\n");
    } else {
        let mut parts = Vec::new();
        if failures > 0 { parts.push(format!("failures={failures}")); }
        if errors > 0 { parts.push(format!("errors={errors}")); }
        err.push_str(&format!("FAILED ({})\n", parts.join(", ")));
    }
    eprint!("{err}");
    if failures == 0 && errors == 0 { 0 } else { 1 }
}
