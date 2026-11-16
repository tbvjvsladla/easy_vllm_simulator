//! `hint_api`: the JSON-lines library bridge the golden checks drive. One request per stdin line:
//!
//! ```text
//! {"id": "n1", "fn": "naming.parse_tag", "args": [...], "kwargs": {...}}
//! ```
//!
//! one response per line on stdout (`json.dumps(obj, ensure_ascii=False)`):
//!
//! ```text
//! {"id": "n1", "kind": "value", "value": <to_json(result)>[, "stdout": "...", "stderr": "..."]}
//! {"id": "n1", "kind": "exception", "type": "HintError", "code": ..., "message": ..., "remedy": ..., "exit_code": n[, "problems": [...]]}
//! {"id": "n1", "kind": "exception", "type": "ValueError", "message": "<str(exc)>"}
//! ```
//!
//! The Python counterpart is the capture shim `.claude/rust/parity/cases/shims/hint_api.py`; both sides must agree on the
//! value encoding (`to_json` there):
//! - `None/bool/int/float/str` as JSON; `dict` keeps order; `list`/`tuple` = array; `set`/`frozenset` = sorted array;
//! - `Path` = `{"__path__": "<str(path)>"}`; a dataclass = `{"__class__": "<Name>", <fields in declaration order>}`;
//! - a compiled pattern = `{"__re__": "<pattern>"}`; `bytes` = `{"__bytes__": "<hex>"}`; anything else `{"__repr__": repr}`.
//!
//! Argument encoding: `{"__path__": s}` = `Path(s)`; `{"__tuple__": [...]}`; `{"__set__": [...]}`; `{"__bytes__": hex}`;
//! `{"__call__": {<request without id>}}` = the result of that call (so a dataclass made by one function can be handed to
//! another). Pseudo-functions (same on both sides): `__sh__` (`sh -c CMD` → `{"rc", "stdout", "stderr"}`), `__write__`
//! (`path, text` → `None`), `__read__` (`path` → text), `__env__` (`name, value|None` → `None`, sets/unsets an env var).
//! A request with `"hex": true` is answered `{"id", "hex": <hex of the UTF-8 JSON of the response without its id>}` (keeps
//! PII-shaped scanner results out of tracked goldens as text).
use std::path::PathBuf;

use crate::core::HintError;
use crate::value::{Map, J};

pub mod artifacts;
pub mod branch;
pub mod catalog;
pub mod cli;
pub mod core;
pub mod evidence;
pub mod lineage;
pub mod naming;
pub mod pii;
pub mod render_bench_section;
pub mod tag;
pub mod template;

/// One decoded request.
pub struct Call {
    pub module: String,
    pub func: String,
    pub args: Vec<J>,
    pub kwargs: Map,
}

/// A function result: the encoded value, or the exception.
pub type Res = Result<J, HintError>;

impl Call {
    /// Positional `i`, else keyword `name`.
    pub fn arg(&self, i: usize, name: &str) -> Option<&J> {
        self.args.get(i).or_else(|| self.kwargs.get(name))
    }
    /// Keyword-only argument.
    pub fn kw(&self, name: &str) -> Option<&J> {
        self.kwargs.get(name)
    }
    pub fn has(&self, i: usize, name: &str) -> bool {
        self.arg(i, name).is_some()
    }
    pub fn str(&self, i: usize, name: &str) -> String {
        self.arg(i, name).and_then(J::as_str).unwrap_or_default().to_string()
    }
    pub fn opt_str(&self, i: usize, name: &str) -> Option<String> {
        self.arg(i, name).and_then(J::as_str).map(str::to_string)
    }
    pub fn bool_or(&self, i: usize, name: &str, default: bool) -> bool {
        self.arg(i, name).and_then(J::as_bool).unwrap_or(default)
    }
    pub fn int_or(&self, i: usize, name: &str, default: i64) -> i64 {
        self.arg(i, name).and_then(J::as_i64).unwrap_or(default)
    }
    /// A `{"__path__": s}` (or plain string) argument as a path.
    pub fn path(&self, i: usize, name: &str) -> PathBuf {
        self.arg(i, name).map(decode_path).unwrap_or_default()
    }
    pub fn opt_path(&self, i: usize, name: &str) -> Option<PathBuf> {
        self.arg(i, name).filter(|v| !v.is_null()).map(decode_path)
    }
    /// A list argument as strings (`{"__tuple__": [...]}` and `{"__set__": [...]}` accepted).
    pub fn strs(&self, i: usize, name: &str) -> Vec<String> {
        self.arg(i, name).map(|v| unwrap_seq(v).iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
    }
    pub fn j(&self, i: usize, name: &str) -> J {
        self.arg(i, name).cloned().unwrap_or(J::Null)
    }
}

/// `{"__path__": s}` or `s`.
pub fn decode_path(v: &J) -> PathBuf {
    match v {
        J::Str(s) => PathBuf::from(s),
        J::Dict(m) => PathBuf::from(m.get("__path__").and_then(J::as_str).unwrap_or_default()),
        _ => PathBuf::new(),
    }
}

/// The items of `[...]`, `{"__tuple__": [...]}` or `{"__set__": [...]}`.
pub fn unwrap_seq(v: &J) -> Vec<J> {
    match v {
        J::List(l) => l.clone(),
        J::Dict(m) => m.get("__tuple__").or_else(|| m.get("__set__")).map(|x| x.items().to_vec()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `Path` result encoding.
pub fn enc_path(p: &std::path::Path) -> J {
    crate::jd! {"__path__" => crate::core::posix(p)}
}

/// A `set` of strings result encoding (sorted).
pub fn enc_set<I: IntoIterator<Item = S>, S: Into<String>>(it: I) -> J {
    J::from(crate::value::sorted_strs(it))
}

/// Run one request (after resolving nested `__call__` arguments).
pub fn dispatch(req: &J) -> (Res, String, String) {
    // Like the shim: argument decoding (incl. nested `__call__`) runs inside the output capture of this request.
    crate::io::capture(|| dispatch_inner(req))
}

fn dispatch_inner(req: &J) -> Res {
    let qual = req.get_str("fn").unwrap_or_default().to_string();
    let mut args = Vec::new();
    for a in req["args"].items() {
        args.push(resolve_arg(a)?);
    }
    let mut kwargs = Map::new();
    if let Some(m) = req["kwargs"].as_dict() {
        for (k, v) in m.iter() {
            let x = resolve_arg(v)?;
            kwargs.insert(k, x);
        }
    }
    let (module, func) = match qual.split_once('.') {
        Some((m, f)) => (m.to_string(), f.to_string()),
        None => (String::new(), qual.clone()),
    };
    let call = Call { module, func, args, kwargs };
    run(&call, &qual)
}

/// Nested values decode recursively like the shim's `from_json` (a `__call__` anywhere inside lists/dicts).
fn resolve_arg(a: &J) -> Result<J, HintError> {
    match a {
        J::Dict(m) if m.len() == 1 && m.contains_key("__call__") => {
            dispatch_inner(&m.get("__call__").cloned().unwrap_or_default())
        }
        J::List(v) => Ok(J::List(v.iter().map(resolve_arg).collect::<Result<_, _>>()?)),
        J::Dict(m) if m.len() == 1 && ["__path__", "__bytes__"].iter().any(|k| m.contains_key(k)) => Ok(a.clone()),
        J::Dict(m) => {
            let mut out = Map::new();
            for (k, v) in m.iter() {
                out.insert(k, resolve_arg(v)?);
            }
            Ok(J::Dict(out))
        }
        other => Ok(other.clone()),
    }
}

fn run(c: &Call, qual: &str) -> Res {
    if c.module.is_empty() {
        return pseudo(c, qual);
    }
    let r = match c.module.as_str() {
        "core" => core::call(c),
        "naming" => naming::call(c),
        "pii" => pii::call(c),
        "lineage" => lineage::call(c),
        "catalog" => catalog::call(c),
        "branch" => branch::call(c),
        "tag" => tag::call(c),
        "template" => template::call(c),
        "artifacts" => artifacts::call(c),
        "evidence" => evidence::call(c),
        "render_bench_section" => render_bench_section::call(c),
        "hint" => cli::call(c),
        _ => None,
    };
    r.unwrap_or_else(|| Err(HintError::py("AttributeError", format!("module {:?} has no attribute {:?} (not bridged)", c.module, c.func))))
}

fn pseudo(c: &Call, qual: &str) -> Res {
    match qual {
        "__sh__" => {
            let cmd = c.str(0, "cmd");
            let argv = vec!["sh".to_string(), "-c".to_string(), cmd];
            match crate::core::run_capture(&argv, None, &[], &[], None) {
                Ok(o) => Ok(crate::jd! {
                    "rc" => o.returncode as i64,
                    "stdout" => String::from_utf8_lossy(&o.stdout).into_owned(),
                    "stderr" => String::from_utf8_lossy(&o.stderr).into_owned(),
                }),
                Err(e) => Err(HintError::py("OSError", e)),
            }
        }
        "__write__" => {
            let p = c.path(0, "path");
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&p, c.str(1, "text")).map_err(|e| crate::core::io_err(&e, &p))?;
            Ok(J::Null)
        }
        "__read__" => {
            let p = c.path(0, "path");
            crate::core::read_text(&p).map(J::Str).map_err(|e| HintError::py("OSError", e))
        }
        "__env__" => {
            let name = c.str(0, "name");
            match c.opt_str(1, "value") {
                Some(v) => std::env::set_var(&name, v),
                None => std::env::remove_var(&name),
            }
            Ok(J::Null)
        }
        _ => Err(HintError::py("NameError", format!("name {qual:?} is not defined"))),
    }
}

/// Encode a response line.
pub fn response(id: &J, r: &Res, out: &str, err: &str) -> J {
    let mut m = Map::new();
    m.insert("id", id.clone());
    match r {
        Ok(v) => {
            m.insert("kind", "value");
            m.insert("value", v.clone());
        }
        Err(e) if e.is_py() => {
            m.insert("kind", "exception");
            m.insert("type", e.code.trim_start_matches("py:"));
            m.insert("message", e.message.clone());
        }
        Err(e) => {
            m.insert("kind", "exception");
            m.insert("type", if e.problems.is_some() { "HintProblemsError" } else { "HintError" });
            m.insert("code", e.code.clone());
            m.insert("message", e.message.clone());
            m.insert("remedy", J::from(e.remedy.clone()));
            m.insert("exit_code", e.exit_code as i64);
            if let Some(p) = &e.problems {
                m.insert("problems", J::from(p.clone()));
            }
        }
    }
    if !out.is_empty() {
        m.insert("stdout", out);
    }
    if !err.is_empty() {
        m.insert("stderr", err);
    }
    J::Dict(m)
}
