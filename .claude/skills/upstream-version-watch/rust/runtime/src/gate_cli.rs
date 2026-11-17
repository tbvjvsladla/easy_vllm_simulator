//! The argument handling the three serve-gate programs (`budget_preflight`, `check_smoke_model`, `slave_forward`) share: the argparse model of the
//! recipe package (usage, help, error text and exit status 2 as argparse writes them) and the plain output of a command line that is a thin
//! wrapper over a pure computation.

use agent_runtime_support::pystr::PyStr;
use num_traits::ToPrimitive;
use recipe_runtime::cliparse::{parse, Outcome, Spec, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

pub type Values = BTreeMap<&'static str, Value>;

/// The arguments after `argv[0]` as the interpreter sees them (undecodable bytes survive as surrogate escapes).
pub fn process_args() -> (Option<PyStr>, Vec<PyStr>) {
    let mut all = std::env::args_os().map(|arg| PyStr::from_surrogateescape(arg.as_bytes()));
    let script = all.next();
    (script, all.collect())
}

pub fn columns() -> String {
    recipe_runtime::cliparse::effective_columns(std::env::var("COLUMNS").ok().as_deref())
}

pub fn write_out(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

pub fn write_err(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
    let _ = err.flush();
}

fn emit(text: &PyStr, stderr: bool) {
    let bytes = text
        .to_surrogateescape_bytes()
        .unwrap_or_else(|_| text.to_backslashreplace_bytes());
    if stderr {
        let _ = std::io::stderr().lock().write_all(&bytes);
    } else {
        let _ = std::io::stdout().lock().write_all(&bytes);
    }
}

/// `ap.parse_args()`: the values, or the status argparse ends with after writing its help or its error.
pub fn parse_args(spec: &Spec, args: &[PyStr]) -> Result<Values, i32> {
    let columns = columns();
    match parse(spec, args, Some(&columns)) {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            emit(&stdout, false);
            emit(&stderr, true);
            Err(status)
        }
    }
}

pub fn flag(values: &Values, key: &str) -> bool {
    matches!(values.get(key), Some(Value::Flag))
}

pub fn int(values: &Values, key: &str) -> Option<i64> {
    match values.get(key) {
        Some(Value::Int(number)) => number.to_i64(),
        _ => None,
    }
}

pub fn float(values: &Values, key: &str) -> Option<f64> {
    match values.get(key) {
        Some(Value::Float(number)) => Some(*number),
        _ => None,
    }
}

pub fn text(values: &Values, key: &str) -> Option<String> {
    match values.get(key) {
        Some(Value::Text(text)) => Some(text.to_utf8().unwrap_or_else(|| {
            String::from_utf8_lossy(&text.to_backslashreplace_bytes()).into_owned()
        })),
        _ => None,
    }
}

/// `repr(float)` for the values these programs print (an ordinary finite number).
pub fn float_repr(number: f64) -> String {
    format!("{number:?}")
}

/// The directory of the skill's `scripts/` that this program stands in for: from the caller's `argv[0]` when a launcher executed it as
/// `<scripts>/<name>.sh` (its own launcher path), otherwise from the executable's own place (`<skill>/rust/runtime/target/release/<bin>`).
pub fn scripts_dir(script: Option<&PyStr>, name: &str) -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(path) = script.and_then(|s| s.to_utf8()) {
        let candidate = PathBuf::from(&path);
        if candidate.file_name().and_then(|n| n.to_str()) == Some(&format!("{name}.sh")) {
            if let Some(parent) = candidate.parent() {
                let parent = if parent.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    parent.to_path_buf()
                };
                return std::fs::canonicalize(&parent)
                    .map(|p| std::path::absolute(&p).unwrap_or(p))
                    .unwrap_or(parent);
            }
        }
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let mut dir = exe;
    for _ in 0..5 {
        dir.pop();
    }
    dir.join("scripts")
}
