//! `harness_verify.py`: observe the reference date once, then run (1) the policy registry audit, (2) the runtime
//! regression probe, (3) the production predicate suite, stopping at the first failure. Each stage is the Rust launcher
//! of its owner (no Python): `.claude/policies/runtime/policy_registry.sh verify`, `.claude/policies/runtime/runtime_selftest.sh`,
//! `.claude/policies/predicates/claim_predicates.sh`, then the companion launchers `.claude/policies/predicates/*_predicate.sh`
//! (the original ran `unittest discover -p '*_predicate.py'`). A launcher that does not exist fails its stage closed.
//!
//! stdout keeps the original's buffering: on a terminal every line is written at once; otherwise (a pipe or file, where
//! CPython block-buffers `print`) the harness lines are written when the process ends, after the children's output.

use crate::argparse::{Exit, Opt, Parser};
use crate::pydate::Date;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DOC: &str = include_str!("harness_verify_doc.txt");

pub const PREDICATE_LAUNCHERS: &[&str] = &["companion_predicate.sh", "runtime_patch_predicate.sh"];

pub struct Reference {
    pub as_of: String,
    pub source: &'static str,
    pub observed_utc: Option<String>,
}

/// `observe_as_of(explicit)`; `Err(())` = the original's ValueError (malformed explicit input).
pub fn observe_as_of(explicit: Option<&str>) -> Result<Reference, ()> {
    if let Some(x) = explicit {
        let chars: Vec<char> = x.chars().collect();
        let shape = chars.len() == 10
            && chars.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == '-' } else { c.is_ascii_digit() });
        if !shape || Date::fromisoformat(x).is_none() {
            return Err(());
        }
        return Ok(Reference { as_of: x.to_string(), source: "explicit-caller-input", observed_utc: None });
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() as i64;
    let micros = now.subsec_micros();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let date = Date::new(1970, 1, 1).unwrap().add_days(days).unwrap();
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let time = if micros > 0 { format!("{h:02}:{m:02}:{s:02}.{micros:06}") } else { format!("{h:02}:{m:02}:{s:02}") };
    Ok(Reference { as_of: date.isoformat(), source: "observed-utc-clock", observed_utc: Some(format!("{}T{time}+00:00", date.isoformat())) })
}

pub fn parser(prog: &str) -> Parser {
    Parser {
        prog: prog.to_string(),
        description: Some(DOC),
        raw_description: true,
        options: vec![
            Opt::help_flag(),
            Opt::value(
                "--as-of",
                "as_of",
                false,
                Some("YYYY-MM-DD explicit caller input for a deterministic reproduction; default = the live UTC date observed once per run and recorded -- passed straight through to policy_registry.py verify (the validator itself reads no clock)"),
            ),
            Opt::value("--registry", "registry", false, Some("passthrough to policy_registry.py verify --registry")),
            Opt::value("--schema", "schema", false, Some("passthrough to policy_registry.py verify --schema")),
            Opt::value("--claim-bindings", "claim_bindings", false, Some("passthrough to policy_registry.py verify --claim-bindings")),
            Opt::value("--repo-root", "repo_root", false, Some("passthrough to policy_registry.py verify --repo-root")),
        ],
        subcommands: vec![],
    }
}

/// CPython's `sys.stdout` for `print`: line-buffered on a terminal, otherwise flushed at exit (or past 8 KiB).
struct Out {
    tty: bool,
    pending: Vec<u8>,
}

impl Out {
    fn new() -> Self {
        Out { tty: std::io::stdout().is_terminal(), pending: Vec::new() }
    }
    fn line(&mut self, text: &str) {
        self.pending.extend_from_slice(text.as_bytes());
        self.pending.push(b'\n');
        if self.tty || self.pending.len() > 8192 {
            self.flush();
        }
    }
    fn flush(&mut self) {
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(&self.pending);
        let _ = o.flush();
        self.pending.clear();
    }
}

fn err_line(text: &str) {
    let mut e = std::io::stderr().lock();
    let _ = writeln!(e, "{text}");
    let _ = e.flush();
}

/// Run a launcher with bash in `repo` (stdio inherited); the returncode as Python reports it (negative = signal).
fn run_stage(repo: &Path, launcher: &Path, args: &[String], out: &mut Out) -> i32 {
    if !launcher.is_file() {
        // keep stdout order as the original (its prints are still pending in the buffer)
        err_line(&format!("[harness_verify] ERROR stage launcher is missing (no Python fallback): {}", launcher.display()));
        return 2;
    }
    let _ = out; // pending harness lines stay buffered, like CPython's
    match Command::new("bash").arg(launcher).args(args).current_dir(repo).status() {
        Ok(st) => {
            use std::os::unix::process::ExitStatusExt;
            match st.code() {
                Some(c) => c,
                None => -(st.signal().unwrap_or(1)),
            }
        }
        Err(e) => {
            err_line(&format!("[harness_verify] ERROR cannot start bash for {}: {e}", launcher.display()));
            2
        }
    }
}

pub fn main_with(args: &[String], repo: &Path, prog: &str) -> i32 {
    let p = parser(prog);
    let parsed = match p.parse(args) {
        Ok(x) => x,
        Err(Exit::Help(text)) => {
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(text.as_bytes());
            let _ = o.flush();
            return 0;
        }
        Err(Exit::Error { message, usage, prog }) => {
            let mut e = std::io::stderr().lock();
            // print_usage(): the formatter's help text = usage stripped of '\n' plus one
            let _ = writeln!(e, "{}", usage.trim_matches('\n'));
            let _ = writeln!(e, "{prog}: error: {message}");
            let _ = e.flush();
            return 2;
        }
    };
    let explicit = parsed.value("as_of");
    let reference = match observe_as_of(explicit) {
        Ok(r) => r,
        Err(()) => {
            err_line(&format!("[harness_verify] usage error: --as-of {} is not a valid ISO-8601 date", crate::value::r(explicit.unwrap_or(""))));
            return 2;
        }
    };
    let mut out = Out::new();
    out.line(&format!(
        "[harness_verify] reference date: as_of={} source={} observed_utc={} repro: --as-of {}",
        reference.as_of,
        reference.source,
        reference.observed_utc.as_deref().unwrap_or("None"),
        reference.as_of
    ));
    out.line(&format!("[harness_verify] stage 1/3: policy_registry.py verify --as-of {}", reference.as_of));
    let mut audit_args: Vec<String> = vec!["verify".into(), "--as-of".into(), reference.as_of.clone()];
    for (flag, dest) in [("--registry", "registry"), ("--schema", "schema"), ("--claim-bindings", "claim_bindings"), ("--repo-root", "repo_root")] {
        if let Some(v) = parsed.value(dest).filter(|v| !v.is_empty()) {
            audit_args.push(flag.into());
            audit_args.push(v.to_string());
        }
    }
    let runtime = repo.join(".claude/policies/runtime");
    let predicates = repo.join(".claude/policies/predicates");
    let finish = |out: &mut Out, code: i32| -> i32 {
        out.flush();
        code
    };
    let audit = run_stage(repo, &runtime.join("policy_registry.sh"), &audit_args, &mut out);
    if audit != 0 {
        err_line(&format!("[harness_verify] BLOCKED: policy audit exited {audit} -- correction-cycle completion refused before runtime/predicate probes run."));
        return finish(&mut out, audit);
    }
    out.line("[harness_verify] stage 2/3: constitution-owned runtime regression probe");
    let rt = run_stage(repo, &runtime.join("runtime_selftest.sh"), &[], &mut out);
    if rt != 0 {
        err_line(&format!("[harness_verify] BLOCKED: runtime regression probe exited {rt}."));
        return finish(&mut out, 4);
    }
    out.line("[harness_verify] stage 3/3: constitution-owned production predicates");
    let mut pred = run_stage(repo, &predicates.join("claim_predicates.sh"), &[], &mut out);
    if pred == 0 {
        for name in PREDICATE_LAUNCHERS {
            let rc = run_stage(repo, &predicates.join(name), &[], &mut out);
            if rc != 0 && pred == 0 {
                pred = rc;
            }
        }
    }
    if pred != 0 {
        err_line(&format!("[harness_verify] BLOCKED: production predicate suite exited {pred}."));
        return finish(&mut out, 3);
    }
    out.line("[harness_verify] PASS: policy audit, runtime regression, and production predicates green.");
    finish(&mut out, 0)
}

pub fn repo_from(argv0: &str) -> PathBuf {
    crate::cli::repo_root_from(argv0, "harness_verify.py")
}
