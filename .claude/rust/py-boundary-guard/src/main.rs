//! py_boundary_guard — plan_26111615 D5.
//!
//! 완료 판정: 추적 `*.py` ⊆ 허용목록(`.claude/policies/py_boundary_allowlist.tsv`).
//! 하네스(결정론 로직·테스트) = Rust, vLLM 연동 개발산출물 = 그 메인 언어(Python).
//!
//! Usage:
//!   py_boundary_guard check [--repo DIR] [--allowlist FILE] [--staged-additions]
//!
//! - 기본(전수): `git ls-files` 의 모든 `*.py` 를 본다. 허용목록 밖이 하나라도 있으면 exit 1.
//! - `--staged-additions`: 인덱스에 새로 들어오는(A/C/R) `*.py` 만 본다(pre-commit 재유입 차단).
//! - 허용목록에 있으나 추적되지 않는 항목은 stderr 에 `STALE` 로 알리되 실패시키지 않는다
//!   (단일 허용목록이 single 전용 경로를 들기 때문 — 브랜치별 부재는 결함이 아니다).
//!
//! exit: 0 통과 · 1 위반 · 2 사용법/입력 오류(fail-closed).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const DEFAULT_ALLOWLIST: &str = ".claude/policies/py_boundary_allowlist.tsv";

struct Args {
    repo: PathBuf,
    allowlist: Option<PathBuf>,
    staged_additions: bool,
}

fn usage() -> &'static str {
    "Usage: py_boundary_guard check [--repo DIR] [--allowlist FILE] [--staged-additions]"
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("check") => {}
        Some("--help") | Some("-h") => return Err(String::new()),
        Some(other) => return Err(format!("unknown command: {other}")),
        None => return Err("missing command".into()),
    }
    let mut args = Args {
        repo: PathBuf::from("."),
        allowlist: None,
        staged_additions: false,
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--repo" => args.repo = PathBuf::from(it.next().ok_or("--repo requires DIR")?),
            "--allowlist" => {
                args.allowlist = Some(PathBuf::from(it.next().ok_or("--allowlist requires FILE")?))
            }
            "--staged-additions" => args.staged_additions = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(args)
}

/// 허용목록: `<path>\t<reason>` 줄. 사유가 빈 줄은 거부한다(사유 없는 허용은 승인 근거가 없다).
fn parse_allowlist(text: &str) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    for (no, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let (path, reason) = line
            .split_once('\t')
            .ok_or_else(|| format!("allowlist line {}: expected <path>\\t<reason>", no + 1))?;
        let path = path.trim();
        if path.is_empty() || reason.trim().is_empty() {
            return Err(format!("allowlist line {}: empty path or reason", no + 1));
        }
        if path.starts_with('/') || path.split('/').any(|c| c == "..") {
            return Err(format!(
                "allowlist line {}: path must be repo-relative: {path}",
                no + 1
            ));
        }
        if !path.ends_with(".py") {
            return Err(format!("allowlist line {}: not a .py path: {path}", no + 1));
        }
        if !set.insert(path.to_string()) {
            return Err(format!("allowlist line {}: duplicate path: {path}", no + 1));
        }
    }
    Ok(set)
}

fn git_z(repo: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect())
}

fn is_py(path: &str) -> bool {
    path.ends_with(".py")
}

fn run(args: Args) -> Result<bool, String> {
    let allow_path = args
        .allowlist
        .clone()
        .unwrap_or_else(|| args.repo.join(DEFAULT_ALLOWLIST));
    let text = std::fs::read_to_string(&allow_path)
        .map_err(|e| format!("cannot read allowlist {}: {e}", allow_path.display()))?;
    let allow = parse_allowlist(&text)?;

    let candidates: Vec<String> = if args.staged_additions {
        git_z(
            &args.repo,
            &["diff", "--cached", "--name-only", "--diff-filter=ACR", "-z"],
        )?
    } else {
        git_z(&args.repo, &["ls-files", "-z"])?
    };
    let tracked_py: BTreeSet<String> = candidates.into_iter().filter(|p| is_py(p)).collect();

    let violations: Vec<&String> = tracked_py.iter().filter(|p| !allow.contains(*p)).collect();
    if !args.staged_additions {
        for stale in allow.iter().filter(|p| !tracked_py.contains(*p)) {
            eprintln!("[py_boundary_guard] STALE allowlist entry (not tracked here): {stale}");
        }
    }
    let scope = if args.staged_additions {
        "staged additions"
    } else {
        "tracked"
    };
    if violations.is_empty() {
        println!(
            "[py_boundary_guard] PASS {scope} *.py = {} (all within allowlist {})",
            tracked_py.len(),
            allow.len()
        );
        Ok(true)
    } else {
        for v in &violations {
            println!("VIOLATION {v}");
        }
        eprintln!(
            "[py_boundary_guard] FAIL {} harness *.py outside allowlist ({scope}). \
             하네스는 Rust 로 쓴다 — 허용목록 추가는 개정 plan + HITL 로만 한다.",
            violations.len()
        );
        Ok(false)
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) if e.is_empty() => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("[py_boundary_guard] {e}\n{}", usage());
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("[py_boundary_guard] ERROR {e}");
            ExitCode::from(2)
        }
    }
}
