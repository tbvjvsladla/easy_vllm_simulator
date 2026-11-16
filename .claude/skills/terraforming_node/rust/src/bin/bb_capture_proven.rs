//! `bb_capture_proven <capture_verified.json>` — the carried-forward `capture_proven` of `verify_node_blackbox.sh`
//! (formerly its inline `python3 - <<PY`). Prints the previous proof as `json.dumps(p, ensure_ascii=False)` or `null`;
//! a schema-1 record (no `capture_proven`, `mode == "post"` with a `verified_utc`) is converted the same way. Any read or
//! parse failure prints `null` (the caller's fallback). Exit 0 always on a readable argument list.
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Write;

fn proven(path: &str) -> Option<PyValue> {
    let bytes = std::fs::read(path).ok()?;
    let text = pyjson::decode_file_text(&bytes).ok()?;
    let d = match pyjson::loads_text(&text).ok()? {
        PyValue::Dict(d) => d,
        _ => return None, // `d.get` on a non-dict raised in the original → `|| echo null`
    };
    let g = |k: &str| d.get(k).map(PyValue::peel).cloned();
    let p = match g("capture_proven") {
        Some(v @ PyValue::Dict(_)) => Some(v),
        _ => {
            let post = matches!(g("mode"), Some(PyValue::Str(s)) if s.to_utf8().as_deref() == Some("post"));
            let verified = g("verified_utc").is_some_and(|v| v.is_truthy());
            if post && verified {
                let mut out = PyDict::new();
                for (to, from) in [("proven_utc", "verified_utc"), ("pass", "pass"), ("fail", "fail"), ("pending", "pending")] {
                    out.set(to, g(from).unwrap_or(PyValue::None));
                }
                Some(PyValue::Dict(out))
            } else {
                None
            }
        }
    };
    p.filter(PyValue::is_truthy)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        eprintln!("usage: bb_capture_proven <capture_verified.json>");
        std::process::exit(2);
    }
    let line = proven(&args[0])
        .and_then(|p| p.dumps_unicode().ok())
        .and_then(|s| s.to_utf8()) // a lone surrogate made the original's print fail → `|| echo null`
        .unwrap_or_else(|| "null".to_string());
    let mut o = std::io::stdout();
    let _ = writeln!(o, "{line}");
}
