//! Command-line bodies of the two programs (argument parsing, help texts, exit handling).

use std::path::{Path, PathBuf};

use crate::argp::{parse_level, prog_name, Opt, Stop};
use crate::completion_gate::{self as cg, Env, Schemas};

const VERIFY_OPTS: &[Opt] = &[Opt::help(), Opt::value(&["--manifest"], "manifest", true), Opt::value(&["--repo-root"], "repo_root", false)];
const AUTHORIZE_OPTS: &[Opt] = &[
    Opt::help(),
    Opt::value(&["--manifest"], "manifest", true),
    Opt::choice(&["--mode"], "mode", true, &["experimental", "promotion"]),
    Opt::choice(&["--action"], "action", true, &cg::ALLOWED_ACTIONS),
    Opt::value(&["--repo-root"], "repo_root", false),
];
const TOP_OPTS: &[Opt] = &[Opt::help()];

fn top_help(prog: &str) -> String {
    format!(
        "usage: {prog} [-h] {{verify,authorize}} ...\n\n\
manifest-driven 3-state completion gate (runtime-ready | evidence-complete |\n\
promotion-ready)\n\n\
positional arguments:\n\
\x20 {{verify,authorize}}\n\
\x20   verify            evaluate a work-manifest and print a completion-manifest\n\
\x20                     JSON to stdout\n\
\x20   authorize         decide whether ONE side-effect action is authorized for\n\
\x20                     a work-manifest\n\n\
options:\n\
\x20 -h, --help          show this help message and exit\n\n\
exit codes: 0=success(promotion-eligible) 1=policy-block 2=invalid-input\n"
    )
}

fn verify_help(prog: &str) -> String {
    let head = format!("usage: {prog} verify ");
    let pad = " ".repeat(head.chars().count());
    format!(
        "{head}[-h] --manifest MANIFEST\n{pad}[--repo-root REPO_ROOT]\n\n\
options:\n\
\x20 -h, --help            show this help message and exit\n\
\x20 --manifest MANIFEST   path to a work-manifest JSON file\n\
\x20 --repo-root REPO_ROOT\n\
\x20                       override repo root detection (bypasses .git lookup --\n\
\x20                       for hermetic tests)\n"
    )
}

fn authorize_help(prog: &str) -> String {
    let head = format!("usage: {prog} authorize ");
    let pad = " ".repeat(head.chars().count());
    format!(
        "{head}[-h] --manifest MANIFEST --mode\n{pad}{{experimental,promotion}} --action\n{pad}{{sync_to_sub,sync_branches,hint_finalize,hint_verify,hint_push}}\n{pad}[--repo-root REPO_ROOT]\n\n\
options:\n\
\x20 -h, --help            show this help message and exit\n\
\x20 --manifest MANIFEST   path to a work-manifest JSON file\n\
\x20 --mode {{experimental,promotion}}\n\
\x20 --action {{sync_to_sub,sync_branches,hint_finalize,hint_verify,hint_push}}\n\
\x20 --repo-root REPO_ROOT\n\
\x20                       override repo root detection (bypasses .git lookup --\n\
\x20                       for hermetic tests)\n"
    )
}

/// `Path(__file__)` of the original script: argv[0] when the launcher set it (`exec -a .../completion_gate.py`),
/// else the canonical location beside this crate's build output.
pub fn script_path(argv0: &str, script_name: &str) -> PathBuf {
    let p = Path::new(argv0);
    if p.file_name().is_some_and(|n| n == script_name) && p.is_absolute() {
        return cg::resolve_lenient(p);
    }
    if let Ok(exe) = std::env::current_exe().and_then(std::fs::canonicalize) {
        // <repo>/.claude/rust/policy-gate/target/release/<bin>
        let mut repo = exe.clone();
        for _ in 0..6 {
            repo.pop();
        }
        let candidate = repo.join(".claude/policies/runtime").join(script_name);
        if repo.join(".claude/rust/policy-gate").is_dir() {
            return candidate;
        }
    }
    cg::resolve_lenient(p)
}

/// Process outcome: stdout text, stderr text, exit code.
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

fn fatal(text: String) -> Outcome {
    Outcome { stdout: String::new(), stderr: format!("{text}\n"), code: 1 }
}

/// The whole `completion_gate.py` command line.
pub fn completion_gate_main(argv: &[String]) -> Outcome {
    let argv0 = argv.first().cloned().unwrap_or_default();
    let env = Env { script: script_path(&argv0, "completion_gate.py"), argv0: argv0.clone() };
    let sch = match Schemas::load(&env) {
        Ok(s) => s,
        Err(e) => return fatal(format!("Traceback (most recent call last):\n  completion_gate: schema load failed: {e}")),
    };
    let prog = prog_name(&argv0, "completion_gate.py");
    let args: Vec<String> = argv.iter().skip(1).cloned().collect();
    let finish = |r: Result<cg::Exit, String>| match r {
        Ok(e) => Outcome { stdout: e.stdout, stderr: String::new(), code: e.code },
        Err(t) => fatal(format!("Traceback (most recent call last):\nRuntimeError: {t}")),
    };
    let usage_error = |sub_prog: &str, message: &str| {
        let msg = format!("invalid command-line invocation: {message}");
        if sub_prog.rsplit(' ').next() == Some("authorize") {
            finish(cg::emit_with_schema(
                with_defaults_auth(cg::authorize_bare_result(None, None, "CLI_USAGE_ERROR", &msg)),
                2,
                &sch.side_effect,
            ))
        } else {
            finish(cg::emit_with_schema(cg::bare_result("CLI_USAGE_ERROR", &msg), 2, &sch.completion))
        }
    };
    let top = match parse_level(&prog, TOP_OPTS, &args, Some(&["verify", "authorize"])) {
        Ok(t) => t,
        Err(Stop::Help { .. }) => return Outcome { stdout: top_help(&prog), stderr: String::new(), code: 0 },
        Err(Stop::Error { prog: p, message }) => return usage_error(&p, &message),
    };
    let (cmd, rest) = top.positional.expect("required positional");
    let sub_prog = format!("{prog} {cmd}");
    let opts = if cmd == "verify" { VERIFY_OPTS } else { AUTHORIZE_OPTS };
    let sub = match parse_level(&sub_prog, opts, &rest, None) {
        Ok(s) => s,
        Err(Stop::Help { .. }) => {
            let text = if cmd == "verify" { verify_help(&prog) } else { authorize_help(&prog) };
            return Outcome { stdout: text, stderr: String::new(), code: 0 };
        }
        Err(Stop::Error { prog: p, message }) => return usage_error(&p, &message),
    };
    let mut extras = top.extras.clone();
    extras.extend(sub.extras.iter().cloned());
    if !extras.is_empty() {
        return usage_error(&prog, &format!("unrecognized arguments: {}", extras.join(" ")));
    }
    let v = &sub.values;
    let manifest = v.get("manifest").unwrap_or_default().to_string();
    let repo_root = v.get("repo_root");
    if cmd == "verify" {
        finish(cg::cmd_verify(&sch, &env, &manifest, repo_root))
    } else {
        finish(cg::cmd_authorize(&sch, &env, v.get("mode").unwrap_or_default(), v.get("action").unwrap_or_default(), &manifest, repo_root))
    }
}

fn with_defaults_auth(mut v: agent_runtime_support::pyvalue::PyValue) -> agent_runtime_support::pyvalue::PyValue {
    use agent_runtime_support::pyvalue::{PyDict, PyValue};
    if let PyValue::Dict(d) = &mut v {
        if !d.contains_key("messages") {
            d.set("messages", PyValue::Dict(PyDict::new()));
        }
        if !d.contains_key("propagation_scope") {
            d.set("propagation_scope", PyValue::None);
        }
    }
    v
}

/// Write an outcome and return its exit code.
pub fn finish_process(o: Outcome) -> i32 {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(o.stdout.as_bytes());
    let _ = out.flush();
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(o.stderr.as_bytes());
    o.code
}
