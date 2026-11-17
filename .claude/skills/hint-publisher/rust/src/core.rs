//! hintlib.core — 공통 기반: 저장소 경로 · 실패 표현 · git · 주입 시각 · 결정론 직렬화 · 소유 CLI 실행.
//! Port of `scripts/hintlib/core.py`. Invariants kept: no side effect before a call (paths are computed when asked),
//! failures are `HintError(code, message, remedy)` (the CLI alone maps them to exit codes), time is injected only.
//!
//! Port boundary (intended difference): the Python `load_owner_module` imported other skills' Python modules
//! in-process. Rust cannot import Python, and this port never starts Python: every owner is reached through its own
//! native entry (a library path dependency, or the owner's launcher via [`run_owner`]); an owner that has no native
//! entry yet fails closed with `HINT_OWNER_LAUNCHER_MISSING` (no fallback implementation, no Python).
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use crate::value::{dumps_pretty_sorted, dumps_tight_sorted, loads, J};

// ── 파일 위치 ─────────────────────────────────────────────────────────────────────────────────
pub const REL_SKILL: &str = ".claude/skills/hint-publisher";
pub const REL_SCRIPTS: &str = ".claude/skills/hint-publisher/scripts";
/// The single CLI entry (Rust launcher). Python: `scripts/hint.py`.
pub const REL_HINT_CLI: &str = ".claude/skills/hint-publisher/scripts/hint.sh";
pub const REL_VOCAB: &str = "hints/vocab.json";
pub const REL_INDEX: &str = "hints/index.json";
pub const REL_HINTS_MD: &str = "HINTS.md";
pub const REL_CONTRACT: &str = "hints/HINT_ISSUANCE_CONTRACT.md";
pub const REL_CENTRAL_FLAG: &str = "hints/.central_authority";
pub const REL_PUSH_FLAG: &str = "hints/.push_authority";
pub const REL_DRAFTS: &str = "hints/.drafts";
pub const REL_PII_TERMS: &str = ".claude/pii_terms.txt";
pub const REL_EVIDENCE_DIR: &str = "docs/_evidence";
pub const REL_COMPLETION_GATE: &str = ".claude/policies/runtime/completion_gate.sh";
pub const REL_EVIDENCE_PUBLISHER: &str = ".claude/policies/runtime/evidence_publisher.py";
pub const REL_CAMPAIGN_INIT: &str = ".claude/skills/terraforming_node/scripts/campaign_init.sh";
pub const REL_CAMPAIGN_VALIDATOR: &str = ".claude/skills/terraforming_node/scripts/campaign_template_validator.sh";
/// upstream-version-watch owners. The owner is the **Rust launcher** (plan_26111615 D4): presence = the launcher, never a `.py`.
pub const REL_SLAVE_FORWARD: &str = ".claude/skills/upstream-version-watch/scripts/slave_forward.sh";
pub const REL_RENDER_DOCKERFILE: &str = ".claude/skills/upstream-version-watch/scripts/render_dockerfile.sh";
/// The renderer's revision identity (git log/show · measurement-time revision): the Rust source the launcher's binary is built from.
pub const REL_RENDER_DOCKERFILE_SRC: &str = ".claude/skills/upstream-version-watch/rust/runtime/src/render_dockerfile.rs";
/// The pre-port renderer path — consulted **only in git history** to name a pre-port measurement revision (RustPortBoundary ·
/// fail closed). Never read as the owner, never executed.
pub const REL_RENDER_DOCKERFILE_PRE_PORT: &str = ".claude/skills/upstream-version-watch/scripts/render_dockerfile.py";
/// The bench-section reader (Rust launcher). Python: `scripts/render_bench_section.py`.
pub const REL_RENDER_BENCH_SECTION: &str = ".claude/skills/hint-publisher/scripts/render_bench_section.sh";
/// The prose naming launcher (its grammar authority is the wiki-desk TS source `REL_DOC_NAMING_TS`).
pub const REL_DOC_NAMING_WIKI: &str = ".claude/policies/runtime/doc_naming.sh";
pub const REL_DOC_NAMING_TS: &str = ".claude/skills/wiki-desk/src/doc_naming.ts";
/// The bench naming grammar authority: the adversarial-benchmark Rust `bench_naming` module (plan_26111615 · no `.py` read).
pub const REL_DOC_NAMING_BENCH: &str = ".claude/skills/adversarial-benchmark/rust/src/bench_naming.rs";

pub const HINT_BRANCH_REF: &str = "refs/heads/hint";
pub const HINT_TAG_PREFIX: &str = "hint/";

pub const SYNTHETIC_NAME: &str = "easy-vllm-simulator";
/// `"@".join(("hints", "easy-vllm.invalid"))` — assembled so the tracked file holds no mail-shaped literal.
pub fn synthetic_email() -> String {
    ["hints", "easy-vllm.invalid"].join("@")
}

/// The skill directory (`.claude/skills/hint-publisher`): `HINT_SKILL_DIR` (set by the launchers), else derived from the
/// running binary (`<skill>/rust/target/<profile>/<bin>`).
pub fn skill_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("HINT_SKILL_DIR") {
        let p = PathBuf::from(d);
        return std::fs::canonicalize(&p).unwrap_or(p);
    }
    let exe = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok());
    if let Some(exe) = exe {
        let mut p = exe.as_path();
        for _ in 0..3 {
            match p.parent() {
                Some(q) => p = q,
                None => break,
            }
        }
        // <skill>/rust/target/<profile>/<bin> -> parents: profile, target, rust, skill
        if let Some(skill) = p.parent() {
            if skill.join("SKILL.md").is_file() {
                return skill.to_path_buf();
            }
        }
        // target/<profile>/deps/<test-bin> (cargo test)
        let mut q = exe.as_path();
        for _ in 0..5 {
            match q.parent() {
                Some(r) => q = r,
                None => break,
            }
            if q.join("SKILL.md").is_file() {
                return q.to_path_buf();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}
pub fn scripts_dir() -> PathBuf {
    skill_dir().join("scripts")
}
pub fn templates_dir() -> PathBuf {
    skill_dir().join("templates")
}
fn dist_root_guess() -> PathBuf {
    let s = skill_dir();
    s.ancestors().nth(3).map(Path::to_path_buf).unwrap_or(s)
}

// ── 실패 표현 ────────────────────────────────────────────────────────────────────────────────
/// A failure with a stable reason code. Judge by `code` only. `problems` carries `tag.HintProblemsError`'s list;
/// `gate_json` the deterministic wrapper JSON of an un-runnable gate. A Python built-in exception (`ValueError`, ...)
/// that the original lets escape is `code = "py:<Type>"` (see [`HintError::py`]).
#[derive(Clone, Debug, PartialEq)]
pub struct HintError {
    pub code: String,
    pub message: String,
    pub remedy: Option<String>,
    pub exit_code: i32,
    pub problems: Option<Vec<String>>,
    pub gate_json: Option<String>,
}

pub type HR<T> = Result<T, HintError>;

impl HintError {
    pub fn new(code: &str, message: impl Into<String>, remedy: Option<&str>) -> HintError {
        HintError {
            code: code.to_string(),
            message: message.into(),
            remedy: remedy.map(str::to_string),
            exit_code: 1,
            problems: None,
            gate_json: None,
        }
    }
    pub fn with_exit(mut self, exit_code: i32) -> HintError {
        self.exit_code = exit_code;
        self
    }
    /// A Python built-in exception the original does not translate (`raise ValueError(msg)`).
    pub fn py(type_name: &str, message: impl Into<String>) -> HintError {
        HintError::new(&format!("py:{type_name}"), message, None)
    }
    pub fn is_py(&self) -> bool {
        self.code.starts_with("py:")
    }
    /// `str(exc)` of the Python exception: `"<code>: <message>"` (HintError) or the message (built-ins).
    pub fn str(&self) -> String {
        if self.is_py() {
            self.message.clone()
        } else {
            format!("{}: {}", self.code, self.message)
        }
    }
    pub fn render(&self) -> String {
        let mut out = format!("[hint] FAIL {}: {}", self.code, self.message);
        if let Some(r) = &self.remedy {
            if !r.is_empty() {
                out.push_str(&format!("\n  → {r}"));
            }
        }
        out
    }
}

impl std::fmt::Display for HintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.str())
    }
}

/// `core.fail(code, message, remedy)`.
pub fn fail<T>(code: &str, message: impl Into<String>, remedy: Option<&str>) -> HR<T> {
    Err(HintError::new(code, message, remedy))
}

/// `core.fail(..., exit_code=n)`.
pub fn fail_exit<T>(code: &str, message: impl Into<String>, remedy: Option<&str>, exit_code: i32) -> HR<T> {
    Err(HintError::new(code, message, remedy).with_exit(exit_code))
}

// ── 경로 ────────────────────────────────────────────────────────────────────────────────────
/// `Path.resolve()` (non-strict): canonicalize the longest existing prefix, then append the rest lexically.
pub fn resolve(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut existing = abs.clone();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(c) = std::fs::canonicalize(&existing) {
            let mut out = c;
            for r in rest.iter().rev() {
                let comp = Path::new(r);
                if comp == Path::new("..") {
                    out.pop();
                } else if comp != Path::new(".") {
                    out.push(r);
                }
            }
            return out;
        }
        match (existing.file_name().map(|n| n.to_os_string()), existing.parent()) {
            (Some(n), Some(parent)) => {
                rest.push(n);
                existing = parent.to_path_buf();
            }
            _ => break,
        }
    }
    normalize_lexically(&abs)
}

fn normalize_lexically(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// POSIX text of a path (`Path.as_posix()` on Linux).
pub fn posix(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ── 저장소 ──────────────────────────────────────────────────────────────────────────────────
pub fn resolve_repo(explicit: Option<&str>, require_git: bool) -> HR<PathBuf> {
    if let Some(e) = explicit {
        let p = resolve(Path::new(e));
        if !p.is_dir() {
            return fail("HINT_REPO_ABSENT", format!("저장소 경로가 디렉터리가 아니다: {}", posix(&p)), None);
        }
        if require_git && !p.join(".git").exists() {
            return fail(
                "HINT_REPO_NOT_GIT",
                format!("git 저장소가 아니다: {}", posix(&p)),
                Some("match 이외 명령은 git 체크아웃에서만 실행한다."),
            );
        }
        return Ok(p);
    }
    if let Ok(out) = Command::new("git").args(["rev-parse", "--show-toplevel"]).stdin(Stdio::null()).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return Ok(resolve(Path::new(&s)));
        }
    }
    let guess = dist_root_guess();
    if !require_git && guess.join(REL_INDEX).is_file() {
        return Ok(guess);
    }
    fail(
        "HINT_REPO_NOT_GIT",
        "git 체크아웃(또는 hints/index.json 을 가진 배포 루트)을 찾지 못했다.",
        Some("match 이외 명령은 git 저장소 안에서 실행하거나 --repo 로 지정한다."),
    )
}

// ── 작업공간 보조 루트 ─────────────────────────────────────────────────────────────────────────
static AUX_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Set the auxiliary root; returns the previous value (the caller restores it).
pub fn set_aux_root(root: Option<&Path>) -> Option<PathBuf> {
    let mut g = AUX_ROOT.lock().unwrap();
    let prev = g.clone();
    *g = root.map(resolve);
    prev
}

pub fn aux_root(repo: &Path) -> PathBuf {
    AUX_ROOT.lock().unwrap().clone().unwrap_or_else(|| repo.to_path_buf())
}

// ── 프로세스 ────────────────────────────────────────────────────────────────────────────────
/// `subprocess.CompletedProcess` (text decoded as UTF-8, invalid bytes replaced).
#[derive(Clone, Debug, Default)]
pub struct Proc {
    pub returncode: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Bytes form.
#[derive(Clone, Debug, Default)]
pub struct ProcBytes {
    pub returncode: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Run a program capturing output (`subprocess.run(argv, capture_output=True, cwd=..., env=os.environ|extra, input=...)`).
/// `Err(text)` = the program could not be started (Python `OSError`).
pub fn run_capture(
    argv: &[String],
    cwd: Option<&Path>,
    env_extra: &[(String, String)],
    env_remove: &[&str],
    input: Option<&[u8]>,
) -> Result<ProcBytes, String> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    for k in env_remove {
        cmd.env_remove(k);
    }
    for (k, v) in env_extra {
        cmd.env(k, v);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().map_err(|e| os_error_text(&e, &argv[0]))?;
    if let Some(data) = input {
        let mut si = child.stdin.take().unwrap();
        let data = data.to_vec();
        let h = std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
        let o = child.wait_with_output().map_err(|e| e.to_string())?;
        let _ = h.join();
        return Ok(ProcBytes { returncode: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr });
    }
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    Ok(ProcBytes { returncode: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr })
}

/// Python's text of an `OSError` from `subprocess` (`[Errno 2] No such file or directory: 'git'`).
pub fn os_error_text(e: &std::io::Error, program: &str) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let msg = std::io::Error::from_raw_os_error(n).to_string();
            let msg = msg.split(" (os error").next().unwrap_or(&msg).to_string();
            format!("[Errno {n}] {msg}: {}", crate::value::py_repr(program))
        }
        None => e.to_string(),
    }
}

fn git_env(extra: &[(String, String)]) -> Vec<(String, String)> {
    let mut env = vec![("GIT_TERMINAL_PROMPT".to_string(), "0".to_string())];
    env.extend(extra.iter().cloned());
    env
}

fn argv_git(args: &[&str]) -> Vec<String> {
    let mut v = vec!["git".to_string()];
    v.extend(args.iter().map(|s| s.to_string()));
    v
}

/// Text-mode git. `check=true`: non-zero exit = `HINT_GIT_FAILED`.
pub fn git_full(repo: &Path, args: &[&str], check: bool, input_text: Option<&str>, env_extra: &[(String, String)]) -> HR<Proc> {
    let r = run_capture(&argv_git(args), Some(repo), &git_env(env_extra), &[], input_text.map(str::as_bytes));
    let out = match r {
        Ok(o) => o,
        Err(e) => return fail("HINT_GIT_UNAVAILABLE", format!("git 실행 불가: {e}"), None),
    };
    // `text=True` = universal newlines: `\r\n` and lone `\r` read as `\n`.
    let un = |b: &[u8]| String::from_utf8_lossy(b).replace("\r\n", "\n").replace('\r', "\n");
    let p = Proc { returncode: out.returncode, stdout: un(&out.stdout), stderr: un(&out.stderr) };
    if check && p.returncode != 0 {
        return fail("HINT_GIT_FAILED", format!("git {} (rc={}): {}", args.join(" "), p.returncode, p.stderr.trim()), None);
    }
    Ok(p)
}

/// `core.git(repo, *args)` (check=True).
pub fn git(repo: &Path, args: &[&str]) -> HR<Proc> {
    git_full(repo, args, true, None, &[])
}

/// `core.git(repo, *args, check=False)`.
pub fn git_nocheck(repo: &Path, args: &[&str]) -> HR<Proc> {
    git_full(repo, args, false, None, &[])
}

/// `core.git_out(repo, *args)` = stdout stripped.
pub fn git_out(repo: &Path, args: &[&str]) -> HR<String> {
    Ok(git(repo, args)?.stdout.trim().to_string())
}

/// Binary git. `check=false`: failure = `Ok(None)`.
pub fn git_bytes(repo: &Path, args: &[&str], check: bool) -> HR<Option<Vec<u8>>> {
    git_bytes_env(repo, args, check, &[], None)
}

pub fn git_bytes_env(
    repo: &Path,
    args: &[&str],
    check: bool,
    env_extra: &[(String, String)],
    input: Option<&[u8]>,
) -> HR<Option<Vec<u8>>> {
    let out = match run_capture(&argv_git(args), Some(repo), &git_env(env_extra), &[], input) {
        Ok(o) => o,
        Err(e) => {
            // Python lets the OSError escape here (no translation in git_bytes).
            return Err(HintError::py("FileNotFoundError", e));
        }
    };
    if out.returncode != 0 {
        if check {
            return fail(
                "HINT_GIT_FAILED",
                format!(
                    "git {} (rc={}): {}",
                    args.join(" "),
                    out.returncode,
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
                None,
            );
        }
        return Ok(None);
    }
    Ok(Some(out.stdout))
}

/// Repository-relative posix path. Outside the repository = `HINT_PATH_OUTSIDE_REPO`.
pub fn rel(repo: &Path, path: &Path) -> HR<String> {
    let p = if path.is_absolute() { path.to_path_buf() } else { repo.join(path) };
    let rp = resolve(&p);
    let rr = resolve(repo);
    match rp.strip_prefix(&rr) {
        Ok(r) => {
            let s = posix(r);
            Ok(if s.is_empty() { ".".to_string() } else { s })
        }
        Err(_) => fail("HINT_PATH_OUTSIDE_REPO", format!("저장소 밖 경로: {}", posix(path)), None),
    }
}

// ── 시각 (주입만) ─────────────────────────────────────────────────────────────────────────────
crate::re_static!(utc_re, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$");

pub fn require_utc(value: Option<&str>, what: &str) -> HR<String> {
    match value {
        Some(v) if !v.is_empty() && utc_re().is_match(v) => Ok(v.to_string()),
        _ => fail(
            "HINT_TIME_NOT_INJECTED",
            format!(
                "{what} 는 UTC `YYYY-MM-DDTHH:MM:SSZ` 로 주입해야 한다: {}",
                match value {
                    Some(v) => crate::value::py_repr(v),
                    None => "None".to_string(),
                }
            ),
            Some("벽시계를 읽지 않는다 — 호출자가 시각을 넘긴다."),
        ),
    }
}

/// A broken-down civil time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

impl Civil {
    pub fn epoch(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86400
            + self.hour as i64 * 3600
            + self.minute as i64 * 60
            + self.second as i64
    }
    pub fn from_epoch(t: i64) -> Civil {
        let days = t.div_euclid(86400);
        let secs = t.rem_euclid(86400);
        let (year, month, day) = civil_from_days(days);
        Civil { year, month, day, hour: (secs / 3600) as u32, minute: (secs % 3600 / 60) as u32, second: (secs % 60) as u32 }
    }
    /// `%Y-%m-%dT%H:%M:%S`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }
}

/// `datetime.strptime(text, "%Y-%m-%dT%H:%M:%S")`-shaped civil parse of the 19 leading characters (no zone).
pub fn parse_civil(text: &str) -> Option<Civil> {
    let b = text.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = text.get(r)?;
        if s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    let c = Civil {
        year: n(0..4)?,
        month: n(5..7)? as u32,
        day: n(8..10)? as u32,
        hour: n(11..13)? as u32,
        minute: n(14..16)? as u32,
        second: n(17..19)? as u32,
    };
    if c.month < 1 || c.month > 12 || c.day < 1 || c.day > days_in_month(c.year, c.month) || c.hour > 23 || c.minute > 59 || c.second > 61 {
        return None;
    }
    Some(c)
}

/// `core.parse_utc(value)` → the UTC civil time (validated by `require_utc`; an impossible date is Python's
/// `ValueError` from `strptime`).
pub fn parse_utc(value: &str) -> HR<Civil> {
    let v = require_utc(Some(value), "--generated-utc")?;
    // `_strptime`: the field regex takes month 1-12, day 1-31, hour 0-23, minute 0-59, second 0-61; then `datetime()`
    // rejects a day past the month's end and a second above 59.
    let n = |r: std::ops::Range<usize>| -> u32 { v[r].parse().unwrap_or(99) };
    let (y, mo, d, h, mi, s) = (v[0..4].parse::<i64>().unwrap_or(0), n(5..7), n(8..10), n(11..13), n(14..16), n(17..19));
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 61 {
        return Err(HintError::py(
            "ValueError",
            format!("time data {} does not match format '%Y-%m-%dT%H:%M:%SZ'", crate::value::py_repr(&v)),
        ));
    }
    if d > days_in_month(y, mo) {
        return Err(HintError::py("ValueError", "day is out of range for month"));
    }
    if s > 59 {
        return Err(HintError::py("ValueError", "second must be in 0..59"));
    }
    Ok(Civil { year: y, month: mo, day: d, hour: h, minute: mi, second: s })
}

/// UTC → KST `YYYY-MM-DDTHH:MM:SS`.
pub fn kst_iso(utc: &str) -> HR<String> {
    Ok(Civil::from_epoch(parse_utc(utc)?.epoch() + 9 * 3600).iso())
}

/// UTC → docs naming `YYMMDDHH` (KST).
pub fn kst_token(utc: &str) -> HR<String> {
    let k = Civil::from_epoch(parse_utc(utc)?.epoch() + 9 * 3600);
    Ok(format!("{:02}{:02}{:02}{:02}", k.year.rem_euclid(100), k.month, k.day, k.hour))
}

/// `@<epoch> +0000` (GIT_*_DATE).
pub fn git_date(utc: &str) -> HR<String> {
    Ok(format!("@{} +0000", parse_utc(utc)?.epoch()))
}

// ── 결정론 직렬화 · 해시 ──────────────────────────────────────────────────────────────────────
/// Deterministic JSON (`sort_keys`, `ensure_ascii=False`, indent 2, trailing newline); a `receiver-summary` is compact.
pub fn dumps(obj: &J) -> String {
    if obj.get_str("kind") == Some("receiver-summary") {
        return dumps_tight_sorted(obj) + "\n";
    }
    dumps_pretty_sorted(obj) + "\n"
}

pub fn write_json(path: &Path, obj: &J) -> HR<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_err(&e, parent))?;
    }
    std::fs::write(path, dumps(obj)).map_err(|e| io_err(&e, path))
}

/// Python's `OSError` text for a path (`[Errno 2] No such file or directory: '<path>'`) as a built-in exception.
pub fn io_err(e: &std::io::Error, path: &Path) -> HintError {
    let ty = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    HintError::py(ty, io_err_text(e, path))
}

pub fn io_err_text(e: &std::io::Error, path: &Path) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let msg = std::io::Error::from_raw_os_error(n).to_string();
            let msg = msg.split(" (os error").next().unwrap_or(&msg).to_string();
            format!("[Errno {n}] {msg}: {}", crate::value::py_repr(&posix(path)))
        }
        None => e.to_string(),
    }
}

/// Read a UTF-8 text file (`Path.read_text(encoding="utf-8")`): `Err` carries Python's exception text
/// (`OSError`/`UnicodeDecodeError`), both of which the Python callers catch as `(OSError, ValueError)`.
pub fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| io_err_text(&e, path))?;
    String::from_utf8(bytes).map_err(|e| {
        let pos = e.utf8_error().valid_up_to();
        let b = e.as_bytes().get(pos).copied().unwrap_or(0);
        format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: invalid start byte")
    })
}

pub fn read_json_code(path: &Path, code: &str) -> HR<J> {
    let text = match read_text(path) {
        Ok(t) => t,
        Err(e) => return fail(code, format!("JSON 을 읽을 수 없다({}): {e}", posix(path)), None),
    };
    match loads(&text) {
        Ok(v) => Ok(v),
        Err(e) => fail(code, format!("JSON 을 읽을 수 없다({}): {e}", posix(path)), None),
    }
}

/// `core.read_json(path)`.
pub fn read_json(path: &Path) -> HR<J> {
    read_json_code(path, "HINT_JSON_UNREADABLE")
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, data);
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> HR<String> {
    let data = std::fs::read(path).map_err(|e| io_err(&e, path))?;
    Ok(sha256_bytes(&data))
}

// ── 소유 CLI 실행 ─────────────────────────────────────────────────────────────────────────────
/// The native launcher of an owner script: `<stem>.sh` next to a `<stem>.py` (the repository's launcher convention).
pub fn owner_launcher(repo: &Path, rel_script: &str) -> PathBuf {
    let launcher = rel_script.strip_suffix(".py").map(|s| format!("{s}.sh")).unwrap_or_else(|| rel_script.to_string());
    repo.join(launcher)
}

/// `core.run_python(repo, rel_script, *args)` through the owner's native launcher (never Python). The launcher must
/// exist (`HINT_OWNER_LAUNCHER_MISSING` otherwise — no fallback implementation). `check=true`: non-zero exit =
/// `HINT_OWNER_CLI_FAILED`.
pub fn run_owner(
    repo: &Path,
    rel_script: &str,
    args: &[String],
    input_text: Option<&str>,
    env_extra: &[(String, String)],
    check: bool,
) -> HR<Proc> {
    let launcher = owner_launcher(repo, rel_script);
    if !launcher.is_file() {
        return fail(
            "HINT_OWNER_LAUNCHER_MISSING",
            format!("소유 스크립트의 native 진입점 부재: {} (원본 {rel_script})", rel(repo, &launcher).unwrap_or_else(|_| posix(&launcher))),
            Some("대체 구현으로 계속하지 않는다 — 소유 스킬의 Rust 진입점(launcher)이 설치·빌드된 체크아웃에서 실행한다."),
        );
    }
    let mut argv = vec!["bash".to_string(), posix(&launcher)];
    argv.extend(args.iter().cloned());
    let out = match run_capture(&argv, Some(repo), env_extra, &[], input_text.map(str::as_bytes)) {
        Ok(o) => o,
        Err(e) => return fail("HINT_OWNER_LAUNCHER_MISSING", format!("{rel_script} 진입점 실행 불가: {e}"), None),
    };
    let p = Proc {
        returncode: out.returncode,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    if check && p.returncode != 0 {
        let head: Vec<&str> = args.iter().take(2).map(String::as_str).collect();
        return fail(
            "HINT_OWNER_CLI_FAILED",
            format!("{rel_script} {}… rc={}\n{}{}", head.join(" "), p.returncode, tail_chars(&p.stdout, 2000), tail_chars(&p.stderr, 2000)),
            None,
        );
    }
    Ok(p)
}

/// `s[-n:]` (code points).
pub fn tail_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        s.to_string()
    } else {
        s.chars().skip(count - n).collect()
    }
}

/// `s[:n]` (code points).
pub fn head_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ── 레벨 원시의 도구 귀속 ────────────────────────────────────────────────────────────────────
pub const LEVEL_RAW_BY_TOOL: [(&str, &str); 2] = [("vllm", "bench_{cell}.json"), ("guidellm", "guidellm_{cell}.json")];

pub fn level_raw_is_measured_tool(path: &Path, cell: &str) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut tool = None;
    for (t, pat) in LEVEL_RAW_BY_TOOL {
        if name == pat.replace("{cell}", cell) {
            tool = Some(t);
            break;
        }
    }
    let Some(tool) = tool else { return true };
    let rec_path = path.parent().unwrap_or(Path::new(".")).join(format!("bench_tool_{cell}.json"));
    let Ok(text) = read_text(&rec_path) else { return true };
    let Ok(rec) = loads(&text) else { return true };
    match rec.get("tool") {
        Some(J::Str(said)) => said == tool,
        _ => true,
    }
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
/// A scratch directory removed on drop (`tempfile.TemporaryDirectory()`).
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> TempDir {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = base.join(format!("{prefix}{}-{n}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).expect("temp dir");
        TempDir(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn selftest() -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool| {
        if !cond {
            bad.push(format!("core: {name}"));
        }
    };
    ck("주입 시각 KST 변환", kst_iso("2026-09-21T10:21:01Z").ok().as_deref() == Some("2026-09-21T19:21:01"));
    ck("KST 토큰", kst_token("2026-09-21T10:21:01Z").ok().as_deref() == Some("26092119"));
    ck("git 날짜 결정론", git_date("1970-01-01T00:00:10Z").ok().as_deref() == Some("@10 +0000"));
    match require_utc(Some("2026-09-21 10:21:01"), "--generated-utc") {
        Ok(_) => ck("★음성대조 비-UTC 시각 차단", false),
        Err(e) => ck("★음성대조 비-UTC 시각 차단(code)", e.code == "HINT_TIME_NOT_INJECTED"),
    }
    ck("결정론 직렬화 키 정렬", dumps(&crate::jd! {"b" => 1, "a" => "가"}) == "{\n  \"a\": \"가\",\n  \"b\": 1\n}\n");
    let e = HintError::new("X_CODE", "msg", Some("do this"));
    ck("HintError 렌더", e.render().starts_with("[hint] FAIL X_CODE: msg") && e.render().contains("→ do this"));
    ck("합성 신원은 예약 TLD", synthetic_email().ends_with(".invalid"));
    ck("합성 신원 = 발행된 태그의 신원(바이트 불변)", synthetic_email() == format!("hints{}easy-vllm.invalid", "@"));
    let td = TempDir::new("hintlib-core-");
    let d = td.path();
    let _ = std::fs::write(d.join("bench_c.json"), "{}");
    let _ = std::fs::write(d.join("guidellm_c.json"), "{}");
    ck("도구 기록 없음 → 판정 안 함(옛 배치)", level_raw_is_measured_tool(&d.join("bench_c.json"), "c"));
    let _ = std::fs::write(d.join("bench_tool_c.json"), "{\"tool\": \"guidellm\"}");
    ck("★음성대조 GuideLLM 레벨의 옛 vllm 원시 → 이 측정 아님", !level_raw_is_measured_tool(&d.join("bench_c.json"), "c"));
    ck("GuideLLM 레벨의 GuideLLM 원시 → 이 측정", level_raw_is_measured_tool(&d.join("guidellm_c.json"), "c"));
    ck("도구 원시가 아닌 파일 → 판정 안 함", level_raw_is_measured_tool(&d.join("measured.json"), "c"));
    bad
}

/// Ordered `{k: v}` from pairs (helper for callers building small maps).
pub fn btree<K: Ord, V>(pairs: Vec<(K, V)>) -> BTreeMap<K, V> {
    pairs.into_iter().collect()
}
