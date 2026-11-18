//! inline_python_scan — plan_26111615 (pass 3 · inline Python).
//!
//! `py_boundary_guard` 는 추적 `*.py` 파일만 본다. 이 검사는 그 바깥 — 추적되는 **비-`.py`** 실행 표면(셸 ·
//! Dockerfile · compose · systemd 유닛 · 템플릿 · 훅) 안에서 Python 을 **기동하는** 줄을 찾는다:
//! `python3 - <<PY` · `python3 -c` · `python3 "$X.py"` · `#!/usr/bin/env python3` · `exec python3` · `--entrypoint python3`.
//!
//! 하네스는 호스트·서브에서 Python 을 띄우지 않는다. vLLM 컨테이너 **안**에서 도는 것(`docker exec/run … python3`,
//! Dockerfile `RUN python3`, 빌드·런타임 패치)과 원본 Python 대비 골든 재캡처 도구는 그 산출물의 메인 언어이므로
//! 허용목록(`inline_python_allowlist.tsv`)에 **사유와 함께** 적힌 줄만 통과한다. `Dockerfile*` 은 호스트 평면이 없으므로
//! (모든 `RUN` 이 빌드 중인 이미지 안에서 돈다) 줄 단위 허용 없이 in-image 로 센다.
//!
//! `.rs` 모드(plan 26111805 c10 · 요청서 26111719 A-4): 같은 `check` 가 추적 `.rs` 소스도 syn 으로 읽어 인터프리터
//! **기동**(`Command::new("python3")` 와 그 변형 · 인터프리터 이름을 프로그램 자리로 쓰는 경우)을 찾는다. 남은 기동은 임시
//! 허용목록 `rs_python_allowlist.tsv` 에 파일 + 정규화 호출 텍스트 + 개수 + 대체 cell + 사유로만 통과하며, 목록 밖 기동과
//! STALE 행은 둘 다 실패다(`rs_scan`).
//!
//! Usage:
//!   inline_python_scan check [--repo DIR] [--allowlist FILE] [--rs-allowlist FILE]
//!
//! 허용목록 한 줄 = `<repo 상대경로>\t<줄에 들어 있어야 할 리터럴>\t<분류>\t<사유>`. 분류 ∈ container · golden-capture.
//! 적중 줄은 같은 경로의 어느 항목 리터럴을 포함할 때만 허용된다(파일 단위 허용 ✗ — 같은 파일의 새 인라인은 걸린다).
//! 쓰이지 않는 항목은 stderr 에 `STALE` 로 알린다(실패시키지 않는다 — 브랜치별 부재).
//!
//! exit: 0 통과 · 1 위반 · 2 사용법/입력 오류(fail-closed).

mod rs_scan;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const DEFAULT_ALLOWLIST: &str = ".claude/rust/py-boundary-guard/inline_python_allowlist.tsv";
const CLASSES: [&str; 2] = ["container", "golden-capture"];

/// Frozen golden data planes: recorded outputs of the original programs, never executed.
const DATA_PLANES: [&str; 2] = ["/checks/data/", ".claude/rust/parity/data/"];

struct Entry {
    path: String,
    needle: String,
    used: std::cell::Cell<usize>,
}

fn usage() -> &'static str {
    "Usage: inline_python_scan check [--repo DIR] [--allowlist FILE] [--rs-allowlist FILE]\n\
     \x20 tracked non-.py executable surfaces (shell, Dockerfile, compose, units, templates, hooks): lines that start\n\
     \x20 Python, against --allowlist (default .claude/rust/py-boundary-guard/inline_python_allowlist.tsv)\n\
     \x20 tracked .rs sources (.rs mode): interpreter launches (Command::new(\"python3\") and the other program\n\
     \x20 positions), against --rs-allowlist (default .claude/rust/py-boundary-guard/rs_python_allowlist.tsv)"
}

fn parse_allowlist(text: &str) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = Vec::new();
    for (no, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 4 {
            return Err(format!("allowlist line {}: expected <path>\\t<literal>\\t<class>\\t<reason>", no + 1));
        }
        let (path, needle, class, reason) = (f[0].trim(), f[1], f[2].trim(), f[3].trim());
        if path.is_empty() || needle.trim().is_empty() || reason.is_empty() {
            return Err(format!("allowlist line {}: empty path, literal or reason", no + 1));
        }
        if path.starts_with('/') || path.split('/').any(|c| c == "..") {
            return Err(format!("allowlist line {}: path must be repo-relative: {path}", no + 1));
        }
        if !CLASSES.contains(&class) {
            return Err(format!("allowlist line {}: class must be one of {CLASSES:?}: {class}", no + 1));
        }
        if out.iter().any(|e| e.path == path && e.needle == needle) {
            return Err(format!("allowlist line {}: duplicate entry: {path}", no + 1));
        }
        out.push(Entry { path: path.to_string(), needle: needle.to_string(), used: std::cell::Cell::new(0) });
    }
    Ok(out)
}

/// Frozen golden data (never executed), for both scans.
fn in_data_plane(path: &str) -> bool {
    DATA_PLANES.iter().any(|d| path.contains(d) || path.starts_with(d.trim_start_matches('/')))
}

/// The executable surfaces this scan reads (everything else — prose, data, Rust sources — is out of scope; Rust
/// sources are the `.rs` mode's).
fn in_scope(path: &str, head: &[u8]) -> bool {
    if path.ends_with(".py") || in_data_plane(path) {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.starts_with("Dockerfile") || path.starts_with(".claude/hooks/") {
        return true;
    }
    const EXT: [&str; 12] = [".sh", ".bash", ".service", ".timer", ".yaml", ".yml", ".template", ".tmpl", ".j2", ".in", ".conf", ".env"];
    if EXT.iter().any(|e| name.ends_with(e)) || name.starts_with(".env") {
        return true;
    }
    !name.contains('.') && head.starts_with(b"#!")
}

fn is_word_left(c: Option<char>) -> bool {
    match c {
        None => true,
        Some(c) => c.is_whitespace() || "\"'(;|&`=/{$".contains(c),
    }
}

/// The line without a trailing shell comment: a `#` after whitespace, outside single/double quotes (prose, not a command).
fn strip_trailing_comment(line: &str) -> &str {
    let (mut sq, mut dq, mut prev_ws, mut esc) = (false, false, true, false);
    for (i, c) in line.char_indices() {
        if esc {
            esc = false;
            prev_ws = false;
            continue;
        }
        match c {
            '\\' if !sq => esc = true,
            '\'' if !dq => sq = !sq,
            '"' if !sq => dq = !dq,
            '#' if !sq && !dq && prev_ws => return &line[..i],
            _ => {}
        }
        prev_ws = c.is_whitespace();
    }
    line
}

/// Does this line start a Python interpreter? (`python`, `python3`, `python3.12` as a command word.)
fn starts_python(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('#') && !t.starts_with("#!") {
        return false; // shell / Dockerfile comment
    }
    if t.starts_with("#!") {
        return t.contains("python");
    }
    let b: Vec<char> = strip_trailing_comment(line).chars().collect();
    let mut i = 0;
    while i + 6 <= b.len() {
        if b[i..i + 6].iter().collect::<String>() == "python" && is_word_left(if i == 0 { None } else { Some(b[i - 1]) }) {
            let mut j = i + 6;
            if j < b.len() && b[j] == '3' {
                j += 1;
                if j + 1 < b.len() && b[j] == '.' && b[j + 1].is_ascii_digit() {
                    j += 1;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                }
            }
            // command word: followed by end, whitespace or a closing quote — and, when followed by whitespace, by an
            // argument that looks like an interpreter invocation (an option, a script path, stdin `-` or an expansion).
            let next = b.get(j).copied();
            let ok = match next {
                None => true,
                Some('"') | Some('\'') => true,
                Some(c) if c.is_whitespace() => {
                    let rest: String = b[j..].iter().collect();
                    let r = rest.trim_start();
                    r.is_empty()
                        || r.starts_with('-')
                        || r.starts_with('"')
                        || r.starts_with('\'')
                        || r.starts_with('$')
                        || r.starts_with('/')
                        || r.starts_with("<<")
                        || r.split_whitespace().next().is_some_and(|w| w.ends_with(".py") || w.ends_with(".py\"") || w.ends_with(".py'"))
                }
                _ => false,
            };
            if ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn git_ls(repo: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect())
}

fn run(repo: &Path, allow: Option<PathBuf>, rs_allow: Option<PathBuf>) -> Result<bool, String> {
    let allow_path = allow.unwrap_or_else(|| repo.join(DEFAULT_ALLOWLIST));
    let text = std::fs::read_to_string(&allow_path).map_err(|e| format!("cannot read allowlist {}: {e}", allow_path.display()))?;
    let entries = parse_allowlist(&text)?;
    let tracked = git_ls(repo)?;
    let rs = rs_scan::scan(repo, &tracked, rs_allow.as_deref())?;
    let mut scanned = 0usize;
    let mut allowed = 0usize;
    let mut in_image = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for rel in tracked {
        let p = repo.join(&rel);
        let Ok(bytes) = std::fs::read(&p) else { continue }; // tracked but absent in the work tree (deleted, unstaged)
        if bytes.contains(&0) || !in_scope(&rel, &bytes[..bytes.len().min(64)]) {
            continue;
        }
        scanned += 1;
        // A Dockerfile has no host plane: every `RUN`/`CMD` line runs inside the image being built (container-internal).
        let dockerfile = rel.rsplit('/').next().is_some_and(|n| n.starts_with("Dockerfile"));
        let text = String::from_utf8_lossy(&bytes);
        for (no, line) in text.lines().enumerate() {
            if !starts_python(line) {
                continue;
            }
            if dockerfile {
                in_image += 1;
                continue;
            }
            match entries.iter().find(|e| e.path == rel && line.contains(&e.needle)) {
                Some(e) => {
                    e.used.set(e.used.get() + 1);
                    allowed += 1;
                }
                None => violations.push(format!("{rel}:{}: {}", no + 1, line.trim())),
            }
        }
    }
    for e in entries.iter().filter(|e| e.used.get() == 0) {
        eprintln!("[inline_python_scan] STALE allowlist entry (no matching line here): {}\t{}", e.path, e.needle);
    }
    rs.print();
    if violations.is_empty() && rs.passed() {
        println!("[inline_python_scan] PASS scanned {scanned} files · {in_image} Dockerfile lines (in-image) · {allowed} lines within allowlist ({} entries)", entries.len());
        return Ok(true);
    }
    if violations.is_empty() {
        println!("[inline_python_scan] non-.rs surfaces clean: scanned {scanned} files · {in_image} Dockerfile lines (in-image) · {allowed} lines within allowlist ({} entries)", entries.len());
    } else {
        for v in &violations {
            println!("VIOLATION {v}");
        }
        eprintln!(
            "[inline_python_scan] FAIL {} line(s) start Python on the host/sub outside the allowlist. \
             하네스는 Rust 로 쓴다 — 컨테이너 내부 실행만 사유와 함께 허용목록에 둔다.",
            violations.len()
        );
    }
    if !rs.passed() {
        rs.print_failure();
    }
    Ok(false)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("check") => {}
        Some("--help") | Some("-h") => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("[inline_python_scan] {}\n{}", other.map_or("missing command".to_string(), |o| format!("unknown command: {o}")), usage());
            return ExitCode::from(2);
        }
    }
    let (mut repo, mut allow, mut rs_allow) = (PathBuf::from("."), None, None);
    while let Some(a) = it.next() {
        let need = |v: Option<&String>, what: &str| v.cloned().ok_or_else(|| format!("{a} requires {what}"));
        let r = match a.as_str() {
            "--repo" => need(it.next(), "DIR").map(|v| repo = PathBuf::from(v)),
            "--allowlist" => need(it.next(), "FILE").map(|v| allow = Some(PathBuf::from(v))),
            "--rs-allowlist" => need(it.next(), "FILE").map(|v| rs_allow = Some(PathBuf::from(v))),
            other => Err(format!("unknown argument: {other}")),
        };
        if let Err(e) = r {
            eprintln!("[inline_python_scan] {e}\n{}", usage());
            return ExitCode::from(2);
        }
    }
    match run(&repo, allow, rs_allow) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("[inline_python_scan] ERROR {e}");
            ExitCode::from(2)
        }
    }
}
