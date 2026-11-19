//! The `policy_registry.py` command line: `verify`, `check-removal`, `--self-test`; one stable JSON envelope on stdout
//! and the envelope's own exit code for every outcome (usage errors included); `-h` prints argparse help.

use crate::argparse::{Exit, Opt, Parser};
use crate::pathutil;
use crate::registry::*;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DOC: &str = include_str!("policy_registry_doc.txt");

/// The repository root the original derives from its own file (`Path(__file__).resolve().parents[3]`): the launcher
/// execs this program with argv0 = `<repo>/.claude/policies/runtime/policy_registry.py`; a direct start falls back to the
/// executable's place in `<repo>/.claude/rust/policy-audit/target/<profile>/`.
pub fn repo_root_from(argv0: &str, script: &str) -> PathBuf {
    let p = Path::new(argv0);
    if p.file_name().map_or(false, |n| n == script) {
        let resolved = pathutil::resolve(p);
        if let Some(root) = resolved.ancestors().nth(4) {
            return root.to_path_buf();
        }
    }
    let exe = std::env::current_exe().map(|e| pathutil::resolve(&e)).unwrap_or_default();
    exe.ancestors().nth(6).map(|x| x.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
}

fn sub_options(removal: bool) -> Vec<Opt> {
    let mut v = vec![Opt::help_flag()];
    if removal {
        v.push(Opt::value("--policy-id", "policy_id", true, None));
    }
    v.push(Opt::value("--registry", "registry", false, None));
    v.push(Opt::value("--schema", "schema", false, None));
    v.push(Opt::value("--claim-bindings", "claim_bindings", false, Some("default: <repo-root>/.claude/policies/claim_bindings.json")));
    v.push(Opt::value("--repo-root", "repo_root", false, None));
    v
}

pub fn parser(prog: &str) -> Parser {
    Parser {
        prog: prog.to_string(),
        description: Some(DOC),
        raw_description: false,
        options: vec![Opt::help_flag(), Opt::flag("--self-test", None)],
        subcommands: vec![
            ("verify", Parser { prog: format!("{prog} verify"), description: None, raw_description: false, options: sub_options(false), subcommands: vec![] }),
            (
                "check-removal",
                Parser { prog: format!("{prog} check-removal"), description: None, raw_description: false, options: sub_options(true), subcommands: vec![] },
            ),
        ],
    }
}

pub fn envelope(violations: &[Violation], exit_code: i64) -> String {
    let mut d = PyDict::new();
    d.insert(PyStr::new("exit_code"), PyValue::Int(BigInt::from(exit_code)));
    d.insert(PyStr::new("schema_version"), PyValue::Int(BigInt::from(ENVELOPE_SCHEMA_VERSION)));
    d.insert(PyStr::new("violation_count"), PyValue::Int(BigInt::from(violations.len())));
    d.insert(PyStr::new("violations"), PyValue::List(violations.iter().map(|v| v.to_dict()).collect()));
    let text = PyValue::Dict(d).dumps_indented(2).unwrap_or_default();
    text.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&text.to_surrogateescape_bytes().unwrap_or_default()).into_owned())
}

fn emit(violations: &[Violation], exit_code: i64) -> i32 {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", envelope(violations, exit_code));
    let _ = out.flush();
    exit_code as i32
}

fn resolve_arg(v: Option<&str>) -> Option<PathBuf> {
    v.filter(|x| !x.is_empty()).map(|x| pathutil::resolve(Path::new(x)))
}

struct Inputs {
    repo_root: PathBuf,
    doc: PyValue,
    schema: PyValue,
    claim_bindings: PyValue,
}

fn load_inputs(sub: &crate::argparse::Parsed, repo: &Path) -> Result<Inputs, i32> {
    let repo_root = resolve_arg(sub.value("repo_root")).unwrap_or_else(|| repo.to_path_buf());
    let registry = resolve_arg(sub.value("registry")).unwrap_or_else(|| repo.join(".claude/policies/registry.yaml"));
    let schema = resolve_arg(sub.value("schema")).unwrap_or_else(|| repo.join(".claude/schemas/policy-registry.schema.json"));
    let cb = resolve_arg(sub.value("claim_bindings")).unwrap_or_else(|| repo_root.join(CLAIM_BINDINGS_REL));
    let doc = safe_load_json_object(&registry, "REGISTRY").map_err(|e| emit(&[e], 2))?;
    let schema = safe_load_json_object(&schema, "SCHEMA").map_err(|e| emit(&[e], 2))?;
    let claim_bindings = safe_load_json_object(&cb, "CLAIM_BINDINGS").map_err(|e| emit(&[e], 2))?;
    Ok(Inputs { repo_root, doc, schema, claim_bindings })
}

pub fn main_with(args: &[String], repo: &Path, prog: &str) -> i32 {
    let p = parser(prog);
    let parsed = match p.parse(args) {
        Ok(x) => x,
        Err(Exit::Help(text)) => {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(text.as_bytes());
            let _ = out.flush();
            return 0;
        }
        Err(Exit::Error { message, .. }) => {
            return emit(&[Violation::new("CLI_USAGE_ERROR", format!("invalid command-line invocation: {message}"))], 2);
        }
    };
    if parsed.flag("--self-test") {
        return crate::selftest::self_test(repo);
    }
    match parsed.command {
        Some("verify") => {
            let sub = parsed.sub.as_ref().unwrap();
            let inp = match load_inputs(sub, repo) {
                Ok(i) => i,
                Err(code) => return code,
            };
            let shape = schema_violations(&inp.doc, &inp.schema);
            if !shape.is_empty() {
                return emit(&shape, 2);
            }
            let lifecycle = evaluate_lifecycle(&inp.doc, &inp.repo_root, Some(&inp.claim_bindings));
            let code = if lifecycle.is_empty() { 0 } else { 1 };
            emit(&lifecycle, code)
        }
        Some("check-removal") => {
            let sub = parsed.sub.as_ref().unwrap();
            let inp = match load_inputs(sub, repo) {
                Ok(i) => i,
                Err(code) => return code,
            };
            let pid = sub.value("policy_id").unwrap_or("");
            let v = check_removal_full(&inp.doc, pid, &inp.schema, &inp.repo_root, &inp.claim_bindings);
            if v.iter().any(|x| x.reason_code == "UNKNOWN_POLICY_ID") {
                return emit(&v, 2);
            }
            let code = if v.is_empty() { 0 } else { 1 };
            emit(&v, code)
        }
        _ => emit(&[Violation::new("CLI_USAGE_ERROR", "no subcommand given -- expected 'verify' or 'check-removal'")], 2),
    }
}

/// argv0 base name as `sys.argv[0]` would give it to argparse (`prog`).
pub fn prog_name(argv0: &str, default: &str) -> String {
    Path::new(argv0).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| default.to_string())
}
