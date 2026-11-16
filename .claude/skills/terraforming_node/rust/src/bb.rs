//! Shared pieces of the node blackbox ports (`blackbox_eta`, `blackbox_session`, `blackbox_collect`, `blackbox_events`,
//! `blackbox_thermal`, `regen_envelope`, `agent_guard`, `publish_install_request`): how a program ends (`SystemExit`,
//! `parser.error`, an uncaught exception), Python `%`-formatting of numbers, `json.dumps(..., sort_keys=True)`, the
//! owner-inheriting directory maker of `blackbox_eta.makedirs_as_ancestor_owner` and the atomic file replace.

use crate::argp::{self, Spec};
use crate::raise::Raised;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{float_repr, PyDict, PyValue};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// How a command ends without returning a status.
#[derive(Debug)]
pub enum Fail {
    /// `raise SystemExit("message")`: the message on standard error, status 1.
    Exit(String),
    /// A status already decided (e.g. `parser.error` printed usage and returned 2).
    Code(i32),
    /// An exception the original does not catch (traceback, status 1).
    Raise(Raised),
}

impl From<Raised> for Fail {
    fn from(r: Raised) -> Self {
        Fail::Raise(r)
    }
}

pub type R<T> = Result<T, Fail>;

/// The exit status of a finished command, printing what Python prints for each way of ending.
pub fn finish(r: R<i32>) -> i32 {
    match r {
        Ok(c) => c,
        Err(Fail::Exit(m)) => {
            let mut e = std::io::stderr().lock();
            let _ = writeln!(e, "{m}");
            1
        }
        Err(Fail::Code(c)) => c,
        Err(Fail::Raise(r)) => crate::raise::exit(&r),
    }
}

/// `parser.error(message)` as a `Fail`.
pub fn ap_error(spec: &Spec, message: &str) -> Fail {
    let _ = std::io::stdout().flush();
    Fail::Code(argp::error(spec, message))
}

pub fn repr(s: &str) -> String {
    PyStr::new(s).repr()
}

/// `repr(float)` / `str(float)`.
pub fn fr(x: f64) -> String {
    float_repr(x)
}

/// `fmt % x` for one float conversion (`%g`, `%.2f`, `%.0f`, ...).
pub fn pct(fmt: &str, x: f64) -> String {
    let cps: Vec<u32> = fmt.chars().map(|c| c as u32).collect();
    let out = agent_runtime_support::pyformat::percent_float(&cps, x).expect("float format");
    out.iter()
        .map(|&p| char::from_u32(p).unwrap_or('\u{fffd}'))
        .collect()
}

/// `"%-Ns" % text` (left-justified to N code points).
pub fn ljust(text: &str, n: usize) -> String {
    let len = text.chars().count();
    if len >= n {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(n - len))
    }
}

/// `"%Ns" % text` (right-justified to N code points).
pub fn rjust(text: &str, n: usize) -> String {
    let len = text.chars().count();
    if len >= n {
        text.to_string()
    } else {
        format!("{}{text}", " ".repeat(n - len))
    }
}

/// Python `str(v)`.
pub fn s(v: &PyValue) -> String {
    crate::py::display(v)
}

/// A copy of `v` whose dict keys are sorted at every level (`sort_keys=True`).
pub fn sorted(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut items: Vec<(PyStr, PyValue)> =
                d.iter().map(|(k, x)| (k.clone(), sorted(x))).collect();
            items.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in items {
                out.insert(k, x);
            }
            PyValue::Dict(out)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// `json.dumps(v, ensure_ascii=False, sort_keys=True)`.
pub fn dumps_sorted(v: &PyValue) -> String {
    crate::py::dumps_unicode(&sorted(v))
}

/// `json.dumps(v, ensure_ascii=False, indent=2, sort_keys=True)`.
pub fn dumps_sorted_pretty(v: &PyValue) -> String {
    crate::py::dumps_pretty(&sorted(v))
}

/// `json.load(open(path, encoding="utf-8"))`.
pub fn load_json(path: &str) -> Result<PyValue, Raised> {
    let bytes = std::fs::read(path).map_err(|e| Raised::os(&e, path))?;
    let text = pyjson::decode_file_text(&bytes)
        .map_err(|e| Raised::new("UnicodeDecodeError", &e.to_string()))?;
    pyjson::loads_text(&text).map_err(|e| Raised::new("json.decoder.JSONDecodeError", &e.to_string()))
}

/// Text mode read with universal newlines (`open(path, encoding="utf-8").read()`): `\r\n` and `\r` become `\n`.
pub fn read_text(path: &str) -> Result<String, Raised> {
    let bytes = std::fs::read(path).map_err(|e| Raised::os(&e, path))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| Raised::new("UnicodeDecodeError", &e.to_string()))?;
    Ok(universal_newlines(text))
}

pub fn universal_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Lines of a text-mode file iteration (`for line in f`): each keeps its `\n` except possibly the last.
pub fn text_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// `os.path.abspath`.
pub fn abspath(path: &str) -> String {
    let joined = if path.starts_with('/') {
        path.to_string()
    } else {
        format!(
            "{}/{}",
            std::env::current_dir().unwrap_or_default().display(),
            path
        )
    };
    normpath(&joined)
}

/// `os.path.normpath` for an absolute or relative POSIX path.
pub fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let initial = if path.starts_with("//") && !path.starts_with("///") {
        2
    } else if path.starts_with('/') {
        1
    } else {
        0
    };
    let mut comps: Vec<&str> = vec![];
    for c in path.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".."
            || (initial == 0 && comps.is_empty())
            || comps.last().is_some_and(|l| *l == "..")
        {
            comps.push(c);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let body = comps.join("/");
    let out = format!("{}{}", "/".repeat(initial), body);
    if out.is_empty() {
        ".".into()
    } else {
        out
    }
}

/// `os.path.dirname`.
pub fn dirname(path: &str) -> String {
    match path.rfind('/') {
        None => String::new(),
        Some(i) => {
            let head = &path[..=i];
            if head.chars().all(|c| c == '/') {
                head.to_string()
            } else {
                head.trim_end_matches('/').to_string()
            }
        }
    }
}

/// `os.path.basename`.
pub fn basename(path: &str) -> String {
    match path.rfind('/') {
        None => path.to_string(),
        Some(i) => path[i + 1..].to_string(),
    }
}

/// `os.path.join(a, b)`.
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() }
}

fn exists(p: &str) -> bool {
    std::fs::metadata(p).is_ok()
}

/// `os.makedirs(path, mode=mode, exist_ok=True)`.
pub fn makedirs(path: &str, mode: u32) -> Result<(), Raised> {
    use std::os::unix::fs::DirBuilderExt;
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(Raised::new(
                "FileExistsError",
                &format!("[Errno 17] File exists: {}", repr(path)),
            ))
        }
        Err(_) => {}
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(mode)
        .create(path)
        .map_err(|e| Raised::os(&e, path))
}

/// `blackbox_eta.makedirs_as_ancestor_owner(path, mode=0o775)`: the chain is made and, when running as root, every
/// directory this call made gets the owner of the nearest pre-existing ancestor. Returns the directories it made
/// (deepest first).
pub fn makedirs_as_ancestor_owner(path: &str) -> Result<Vec<String>, Raised> {
    let path = abspath(path);
    let mut missing = vec![];
    let mut probe = path.clone();
    while !exists(&probe) {
        missing.push(probe.clone());
        let parent = dirname(&probe);
        if parent == probe {
            break;
        }
        probe = parent;
    }
    makedirs(&path, 0o775)?;
    if missing.is_empty() {
        return Ok(vec![]);
    }
    let Ok(st) = std::fs::metadata(&probe) else {
        return Ok(missing);
    };
    let (uid, gid) = (st.uid(), st.gid());
    if geteuid() != 0 {
        return Ok(missing);
    }
    for d in missing.iter().rev() {
        if let Ok(m) = std::fs::metadata(d) {
            if m.uid() != uid {
                let _ = std::os::unix::fs::chown(d, Some(uid), Some(gid));
            }
        }
    }
    Ok(missing)
}

/// `blackbox_eta.inherit_dir_owner(path, parent)`: as root, give `path` the owner of `parent`. True when it changed.
pub fn inherit_dir_owner(path: &str, parent: &str) -> bool {
    if geteuid() != 0 {
        return false;
    }
    let (Ok(st), Ok(pm)) = (std::fs::metadata(parent), std::fs::metadata(path)) else {
        return false;
    };
    if pm.uid() != st.uid() {
        return std::os::unix::fs::chown(path, Some(st.uid()), Some(st.gid())).is_ok();
    }
    false
}

/// Write `body` to `path + ".tmp"`, fsync, `os.replace` onto `path`.
pub fn write_atomic(path: &str, body: &str) -> Result<(), Raised> {
    let tmp = format!("{path}.tmp");
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| Raised::os(&e, &tmp))?;
        f.write_all(body.as_bytes())
            .map_err(|e| Raised::os(&e, &tmp))?;
        f.sync_all().map_err(|e| Raised::os(&e, &tmp))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| Raised::os(&e, path))
}

/// `open(path, "a").write(text)`.
pub fn append(path: &str, text: &str) -> Result<(), Raised> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Raised::os(&e, path))?;
    f.write_all(text.as_bytes()).map_err(|e| Raised::os(&e, path))
}

/// `open(path, "w").write(text)`.
pub fn write(path: &str, text: &str) -> Result<(), Raised> {
    std::fs::write(path, text).map_err(|e| Raised::os(&e, path))
}

pub fn is_file(p: &str) -> bool {
    Path::new(p).is_file()
}
pub fn is_dir(p: &str) -> bool {
    Path::new(p).is_dir()
}

/// `sorted(os.listdir(d))` (names compared by code points, as Python compares `str`).
pub fn listdir_sorted(d: &str) -> Result<Vec<String>, Raised> {
    let rd = std::fs::read_dir(d).map_err(|e| Raised::os(&e, d))?;
    let mut v: Vec<String> = rd
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    Ok(v)
}

pub fn int(n: i64) -> PyValue {
    PyValue::int(n)
}
pub fn st(t: &str) -> PyValue {
    PyValue::str(t)
}

/// Python `int(text)` (BigInt-free; values outside i64 are refused).
pub fn py_int_text(text: &str) -> Option<i64> {
    let t: String = text
        .chars()
        .map(|c| {
            agent_runtime_support::pystr::decimal_digit_value(c as u32)
                .map(|d| char::from_digit(d, 10).unwrap())
                .unwrap_or(c)
        })
        .collect();
    argp::py_int(&t)
}

/// Prints the `self-test` check list the way every blackbox self-test does (`  [PASS] name`).
pub fn print_checks(checks: &[(String, bool)]) -> bool {
    let mut o = std::io::stdout().lock();
    let mut ok = true;
    for (name, passed) in checks {
        let _ = writeln!(o, "  [{}] {}", if *passed { "PASS" } else { "FAIL" }, name);
        ok = ok && *passed;
    }
    ok
}

/// A scratch directory under the system temporary directory (`tempfile.TemporaryDirectory()`), removed on drop.
pub struct TempDir(pub String);
impl TempDir {
    pub fn new() -> TempDir {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::var("TMPDIR")
            .ok()
            .filter(|t| !t.is_empty() && is_dir(t))
            .unwrap_or_else(|| "/tmp".into());
        loop {
            let n = N.fetch_add(1, Ordering::SeqCst);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let p = format!("{base}/tmp{}{:x}{n}", std::process::id(), nanos);
            if std::fs::create_dir(&p).is_ok() {
                return TempDir(p);
            }
        }
    }
    pub fn path(&self) -> &str {
        &self.0
    }
}
impl Default for TempDir {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
