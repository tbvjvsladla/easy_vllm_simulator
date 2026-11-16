//! Plumbing shared by the predicates: the original's `_require`/`_read`/`_run_bash`/bash extraction helpers, Python
//! `str` index/count semantics over byte offsets (only relative order and containment are ever compared), a hermetic
//! temporary directory, subprocess runs with a timeout, git, the render CLI for `_rendered`, and a JSON reader.
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A clause requirement that failed closed. `kind` is the Python exception class name the original reports
/// (`PredicateFailure`, `ValueError`, `TimeoutExpired`, ...), `message` its `str(exc)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fail {
    pub kind: String,
    pub message: String,
}

impl Fail {
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Fail { kind: kind.to_string(), message: message.into() }
    }
    pub fn predicate(message: impl Into<String>) -> Self {
        Fail::new("PredicateFailure", message)
    }
}

pub type R<T = ()> = Result<T, Fail>;

/// `_require(condition, message)`.
pub fn req(condition: bool, message: impl Into<String>) -> R {
    if condition { Ok(()) } else { Err(Fail::predicate(message)) }
}

/// The repository root: `REPO_ROOT = Path(__file__).resolve().parents[3]` of the original. The binary is started
/// through the launcher next to the original (`exec -a <dir>/<name>.py`), so argv[0] locates the same directory;
/// `POLICY_PREDICATES_REPO` overrides it (tests and fixtures). Falls back to the crate location.
pub fn repo_root() -> PathBuf {
    if let Ok(explicit) = std::env::var("POLICY_PREDICATES_REPO") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    if let Some(arg0) = std::env::args_os().next() {
        let p = PathBuf::from(arg0);
        if p.extension().map_or(false, |e| e == "py") {
            if let Ok(abs) = fs::canonicalize(p.parent().unwrap_or(Path::new("."))) {
                // <repo>/.claude/policies/predicates
                if let Some(root) = abs.parent().and_then(Path::parent).and_then(Path::parent) {
                    return root.to_path_buf();
                }
            }
        }
    }
    // <repo>/.claude/rust/policy-predicates
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or(crate_dir)
}

/// `(REPO_ROOT / rel).read_text(encoding="utf-8")`: a missing file or invalid UTF-8 fails the clause the way the
/// original's uncaught OSError/UnicodeDecodeError does.
pub fn read(rel: &str) -> R<String> {
    read_path(&repo_root().join(rel))
}

pub fn read_path(path: &Path) -> R<String> {
    let bytes = fs::read(path).map_err(|e| {
        let kind = if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" };
        Fail::new(kind, format!("[Errno {}] {}: '{}'", e.raw_os_error().unwrap_or(0), errno_text(&e), path.display()))
    })?;
    String::from_utf8(bytes).map_err(|e| Fail::new("UnicodeDecodeError", format!("'utf-8' codec can't decode {}: {}", path.display(), e)))
}

fn errno_text(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.find(" (os error") { Some(i) => s[..i].to_string(), None => s }
}

pub fn shared_asset(name: &str) -> R<String> {
    read(&format!(".claude/skills/upstream-version-watch/assets/configs/{name}"))
}

// ---------------------------------------------------------------- Python str semantics (byte offsets)

/// `s.index(sub)` -- ValueError when absent.
pub fn index(s: &str, sub: &str) -> R<usize> {
    s.find(sub).ok_or_else(|| Fail::new("ValueError", "substring not found"))
}

/// `s.index(sub, start)`.
pub fn index_from(s: &str, sub: &str, start: usize) -> R<usize> {
    if start > s.len() {
        return Err(Fail::new("ValueError", "substring not found"));
    }
    s[start..].find(sub).map(|i| i + start).ok_or_else(|| Fail::new("ValueError", "substring not found"))
}

/// `s.rindex(sub)`.
pub fn rindex(s: &str, sub: &str) -> R<usize> {
    s.rfind(sub).ok_or_else(|| Fail::new("ValueError", "substring not found"))
}

/// `s.count(sub)` (non-overlapping).
pub fn count(s: &str, sub: &str) -> usize {
    s.matches(sub).count()
}

/// `str.splitlines()` (no keepends): the line boundaries Python's str recognises.
pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0;
    while i < bytes.len() {
        let (pos, c) = bytes[i];
        let is_break = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if is_break {
            out.push(&s[start..pos]);
            let mut next = pos + c.len_utf8();
            if c == '\r' && i + 1 < bytes.len() && bytes[i + 1].1 == '\n' {
                next += 1;
                i += 1;
            }
            start = next;
        }
        i += 1;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// `str.strip()` / `lstrip()` (Python whitespace).
pub fn strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || ('\x1c'..='\x1f').contains(&c))
}
pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_whitespace() || ('\x1c'..='\x1f').contains(&c))
}

/// Python `repr()` of a str (for messages that interpolate `{x!r}`).
pub fn repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => { out.push('\\'); out.push(c); }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python `repr()` of a list of str.
pub fn repr_list<S: AsRef<str>>(items: &[S]) -> String {
    format!("[{}]", items.iter().map(|s| repr(s.as_ref())).collect::<Vec<_>>().join(", "))
}

// ---------------------------------------------------------------- regex (Python `re`)

pub fn re(pattern: &str) -> fancy_regex::Regex {
    fancy_regex::Regex::new(pattern).unwrap_or_else(|e| panic!("predicate regex {pattern:?} must compile: {e}"))
}

/// `re.search(pattern, s)` → the match span.
pub fn search(pattern: &str, s: &str) -> Option<(usize, usize, String)> {
    re(pattern).find(s).ok().flatten().map(|m| (m.start(), m.end(), m.as_str().to_string()))
}

/// `re.findall(pattern, s)` for a pattern with at most one group (group 1 if present, else group 0).
pub fn findall(pattern: &str, s: &str) -> Vec<String> {
    let r = re(pattern);
    let mut out = Vec::new();
    for cap in r.captures_iter(s).flatten() {
        let m = if cap.len() > 1 { cap.get(1) } else { cap.get(0) };
        out.push(m.map(|m| m.as_str().to_string()).unwrap_or_default());
    }
    out
}

/// `re.fullmatch(pattern, s) is not None`.
pub fn fullmatch(pattern: &str, s: &str) -> bool {
    re(&format!("\\A(?:{pattern})\\z")).is_match(s).unwrap_or(false)
}

// ---------------------------------------------------------------- temporary directory

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// `tempfile.TemporaryDirectory()`: removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> R<TempDir> {
        let base = std::env::temp_dir();
        for _ in 0..100 {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
            let p = base.join(format!("{}{}{:x}{:x}", prefix, std::process::id(), nanos, n));
            match fs::create_dir(&p) {
                Ok(()) => return Ok(TempDir(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Fail::new("OSError", e.to_string())),
            }
        }
        Err(Fail::new("FileExistsError", "no usable temporary directory name"))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn write(path: &Path, text: &str) -> R {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    fs::write(path, text).map_err(|e| Fail::new("OSError", e.to_string()))
}

pub fn mkdirs(path: &Path) -> R {
    fs::create_dir_all(path).map_err(|e| Fail::new("OSError", e.to_string()))
}

/// Every regular file under `dir`, relative to it (`dstdir.rglob("*")` + `is_file()`).
pub fn rglob_files(dir: &Path) -> Vec<String> {
    fn walk(d: &Path, base: &Path, out: &mut Vec<String>) {
        if let Ok(rd) = fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, base, out);
                } else if p.is_file() {
                    out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

// ---------------------------------------------------------------- subprocesses

pub struct Proc {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// `subprocess.run(argv, capture_output=True, text=True, timeout=...)`.
pub fn run(argv: &[&str], cwd: Option<&Path>, env: &[(&str, &str)], stdin: Option<&str>, timeout: Option<u64>) -> R<Proc> {
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| {
        let kind = if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" };
        Fail::new(kind, format!("[Errno {}] {}: '{}'", e.raw_os_error().unwrap_or(0), errno_text(&e), argv[0]))
    })?;
    if let Some(text) = stdin {
        use std::io::Write;
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(text.as_bytes());
        }
    }
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || { let mut b = Vec::new(); let _ = out_pipe.read_to_end(&mut b); b });
    let t_err = std::thread::spawn(move || { let mut b = Vec::new(); let _ = err_pipe.read_to_end(&mut b); b });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if let Some(t) = timeout {
                    if start.elapsed() > Duration::from_secs(t) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(Fail::new("TimeoutExpired", format!("Command '{}' timed out after {} seconds", repr_list(argv), t)));
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(Fail::new("OSError", e.to_string())),
        }
    };
    let stdout = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
    use std::os::unix::process::ExitStatusExt;
    let code = status.code().unwrap_or_else(|| -status.signal().unwrap_or(0));
    Ok(Proc { code, stdout, stderr })
}

/// `_run_bash(script)`: `bash -c script`, 30 s.
pub fn run_bash(script: &str) -> R<Proc> {
    run(&["bash", "-c", script], None, &[], None, Some(30))
}

/// The fixed test identity the original passes as env to every fixture git (`GIT_AUTHOR_NAME="t"` ...).
pub const GIT_T_ENV: [(&str, &str); 4] =
    [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];

/// The original's local `git(*args)`: `git -C <dir> ...`, required to succeed (stderr is the failure message), stripped stdout.
pub fn git(dir: &Path, args: &[&str]) -> R<String> {
    let d = dir.to_string_lossy().into_owned();
    let mut argv = vec!["git", "-C", d.as_str()];
    argv.extend_from_slice(args);
    let p = run(&argv, None, &GIT_T_ENV, None, None)?;
    req(p.code == 0, p.stderr.clone())?;
    Ok(strip(&p.stdout).to_string())
}

// ---------------------------------------------------------------- bash source extraction

/// `_extract_bash_function(source, name)`: `^name\s*\(\)\s*\{` (multiline) then brace-depth matching.
pub fn extract_bash_function(source: &str, name: &str) -> R<String> {
    let pat = format!("(?m)^{}\\s*\\(\\)\\s*\\{{", regex_escape(name));
    let m = search(&pat, source);
    let (start, end, _) = match m {
        Some(m) => m,
        None => return Err(Fail::predicate(format!("bash function {} not found in source", repr(name)))),
    };
    let mut depth = 0i64;
    for (j, c) in source[end - 1..].char_indices() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                return Ok(source[start..end - 1 + j + 1].to_string());
            }
        }
    }
    Err(Fail::new("AssertionError", format!("unterminated bash function {}", repr(name))))
}

/// `_extract_bash_array(source, name)`: `^name=\([^)]*\)` (multiline).
pub fn extract_bash_array(source: &str, name: &str) -> R<String> {
    let pat = format!("(?m)^{}=\\([^)]*\\)", regex_escape(name));
    match search(&pat, source) {
        Some((_, _, s)) => Ok(s),
        None => Err(Fail::predicate(format!("bash array {} not found in source", repr(name)))),
    }
}

/// `re.escape`.
pub fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// `shlex.quote`.
pub fn shlex_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

// ---------------------------------------------------------------- `_rendered(kind)` through the render CLI

/// `_rendered(kind, "multi")` = `render_dockerfile.render_shared(kind, "multi", {"cpu_arch": "aarch64", "nas_model_path": "/models"})`.
/// The renderer's command line is the Rust `render_dockerfile` (launcher `render_dockerfile.sh`); its canonical render reads a
/// manifest file through the Flag readiness gate, so the fixture manifest carries the same two facts plus the Flag fields the
/// gate requires (none of them is a placeholder input of the shared templates -- the output is byte-identical to the
/// in-process original, checked by `checks/predicate_checks.rs` against the captured golden).
pub fn rendered(kind: &str) -> R<String> {
    let tmp = TempDir::new("policy-predicates-render-")?;
    let manifest = tmp.path().join("manifest.yaml");
    write(&manifest, "topology: multi\ncpu_arch: aarch64\ngpus_per_node: 1\nmodel_source: managed\nnas_model_path: /models\nterraforming:\n  complete: true\n  branch_verified: true\n")?;
    let launcher = repo_root().join(".claude/skills/upstream-version-watch/scripts/render_dockerfile.sh");
    let l = launcher.to_string_lossy().into_owned();
    let m = manifest.to_string_lossy().into_owned();
    let p = run(&["bash", &l, "--canonical-kind", kind, "--topology", "multi", "--manifest", &m], None, &[], None, Some(120))?;
    if p.code != 0 {
        return Err(Fail::new("ValueError", format!("render_shared({kind:?}, 'multi') failed (rc={}): {}", p.code, strip(&p.stderr))));
    }
    Ok(p.stdout)
}

// ---------------------------------------------------------------- JSON

pub fn json_load(rel: &str) -> R<PyValue> {
    let text = read(rel)?;
    json_loads(&text)
}

pub fn json_loads(text: &str) -> R<PyValue> {
    pyjson::loads(text).map_err(|e| Fail::new("JSONDecodeError", format!("{e:?}")))
}

pub fn jget<'a>(v: &'a PyValue, key: &str) -> R<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key).map(PyValue::peel).ok_or_else(|| Fail::new("KeyError", repr(key))),
        _ => Err(Fail::new("TypeError", format!("not a dict while looking up {}", repr(key)))),
    }
}

pub fn jstr(v: &PyValue) -> Option<String> {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}

pub fn jlist(v: &PyValue) -> Vec<PyValue> {
    match v.peel() {
        PyValue::List(l) | PyValue::Tuple(l) => l.clone(),
        _ => Vec::new(),
    }
}

/// The str items of a JSON list.
pub fn jstrs(v: &PyValue) -> Vec<String> {
    jlist(v).iter().filter_map(jstr).collect()
}

/// Python `json.dumps(s, ensure_ascii=False)` of a str.
pub fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Python's `s[-n:]` on a str (last `n` code points).
pub fn tail_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n { s.to_string() } else { s.chars().skip(count - n).collect() }
}
