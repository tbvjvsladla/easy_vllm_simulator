//! verdict_clock_scan — 하네스 판정은 벽시계를 읽지 않는다(CLAUDE.md 불변식 · 2026-11-19 사용자 결정 h20).
//!
//! Tracked harness sources (`.rs`, shell, `.ts`; the parity records `.claude/rust/parity/data/` and this lint's planted
//! fixtures `checks/data/verdict_clock/` excepted) must not read the system clock, a file time or a calendar period
//! outside the rows of `verdict_clock_allowlist.tsv` (`verdict_clock`). A verdict gets its time injected; a runtime that
//! measures real elapsed time or an outside fact, and a time stamped into a record, are rows, approved through HITL.
//!
//! Usage:
//!   verdict_clock_scan check [--repo DIR] [--allowlist FILE]
//!   verdict_clock_scan --self-test
//!
//! exit: 0 pass · 1 a read outside the allowlist or a stale row · 2 usage or input error (fail-closed): the allowlist
//! cannot be read (the default one missing included), a tracked source cannot be read, or there is no source to scan.
//! A tracked file deleted from the work tree is skipped and counted; gitlinks and symlinks are not files to read.

mod verdict_clock;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use verdict_clock::{Found, Surface};

const USAGE: &str = "Usage: verdict_clock_scan check [--repo DIR] [--allowlist FILE]\n\
     \x20      verdict_clock_scan --self-test\n\
     \x20 tracked .rs, shell and .ts sources: clock reads, file times and calendar periods, against --allowlist\n\
     \x20 (default .claude/rust/py-boundary-guard/verdict_clock_allowlist.tsv)";

const GITLINK_MODE: &str = "160000";
const SYMLINK_MODE: &str = "120000";
const SHEBANG_HEAD: usize = 128;

fn tracked_regular_files(repo: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-s", "-z"])
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let mut files = Vec::new();
    for entry in out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let Some((stage, path)) = entry.split_once('\t') else {
            return Err(format!("git ls-files -s: unexpected entry {entry:?}"));
        };
        let mode = stage.split(' ').next().unwrap_or_default();
        if mode != GITLINK_MODE && mode != SYMLINK_MODE {
            files.push(path.to_string());
        }
    }
    files.dedup();
    Ok(files)
}

struct Scanned {
    found: Found,
    counts: [usize; 3],
    deleted: usize,
}

enum TrackedFile {
    Source(Surface, Vec<u8>),
    Other,
    Deleted,
}

fn read_tracked(repo: &Path, rel: &str) -> Result<TrackedFile, String> {
    use std::io::Read;
    if verdict_clock::in_data_plane(rel) {
        return Ok(TrackedFile::Other);
    }
    let path = repo.join(rel);
    let cannot_read = |e: std::io::Error| format!("cannot read tracked file {rel}: {e}");
    let opened = std::fs::File::open(&path);
    let mut file = match opened {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(TrackedFile::Deleted),
        other => other.map_err(cannot_read)?,
    };
    let mut bytes = Vec::new();
    let surface = match verdict_clock::surface_by_name(rel) {
        Some(surface) => surface,
        None => {
            file.by_ref().take(SHEBANG_HEAD as u64).read_to_end(&mut bytes).map_err(cannot_read)?;
            if !verdict_clock::is_shell_script(&bytes) {
                return Ok(TrackedFile::Other);
            }
            Surface::Shell
        }
    };
    file.read_to_end(&mut bytes).map_err(cannot_read)?;
    Ok(TrackedFile::Source(surface, bytes))
}

fn scan(repo: &Path) -> Result<Scanned, String> {
    let mut scanned = Scanned { found: Found::new(), counts: [0; 3], deleted: 0 };
    for rel in tracked_regular_files(repo)? {
        let (surface, bytes) = match read_tracked(repo, &rel)? {
            TrackedFile::Source(surface, bytes) => (surface, bytes),
            TrackedFile::Other => continue,
            TrackedFile::Deleted => {
                scanned.deleted += 1;
                continue;
            }
        };
        let text = match surface {
            Surface::Rust => String::from_utf8(bytes).map_err(|_| format!("{rel}: not UTF-8"))?,
            _ => String::from_utf8_lossy(&bytes).into_owned(),
        };
        scanned.counts[surface as usize] += 1;
        let reads = verdict_clock::find(surface, &text).map_err(|e| format!("cannot read {rel} as Rust tokens ({e})"))?;
        for read in reads {
            scanned.found.entry((rel.clone(), read.text)).or_default().push(read.line);
        }
    }
    Ok(scanned)
}

fn check(repo: &Path, allowlist: Option<PathBuf>) -> Result<bool, String> {
    let path = allowlist.unwrap_or_else(|| repo.join(verdict_clock::DEFAULT_ALLOWLIST));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read allowlist {}: {e}", path.display()))?;
    let rows = verdict_clock::parse_allowlist(&text)?;
    let scanned = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn({
            let repo = repo.to_path_buf();
            move || scan(&repo)
        })
        .map_err(|e| format!("cannot start the scan: {e}"))?
        .join()
        .map_err(|_| "the scan panicked".to_string())??;
    let [rs, sh, ts] = scanned.counts;
    if rs + sh + ts == 0 {
        return Err(format!("no tracked .rs, shell or .ts source to scan in {}", repo.display()));
    }
    let verdict = verdict_clock::judge(&scanned.found, &rows);
    let coverage = format!("scanned {rs} .rs, {sh} shell, {ts} .ts · {} deleted from the work tree", scanned.deleted);
    if verdict.passed() {
        println!(
            "[verdict_clock_scan] PASS {} clock reads within verdict_clock_allowlist.tsv ({} rows) · {coverage}",
            verdict.allowed, verdict.rows
        );
        return Ok(true);
    }
    for violation in &verdict.violations {
        println!("CLOCK-VIOLATION {violation}");
    }
    for stale in &verdict.stale {
        println!("CLOCK-STALE {stale}");
    }
    eprintln!(
        "[verdict_clock_scan] FAIL {} clock read(s) outside verdict_clock_allowlist.tsv · {} stale row(s) · {coverage}. \
         하네스 판정은 벽시계를 읽지 않는다 — 시각은 주입받거나, 런타임 사실이면 허용목록 행(HITL)으로만.",
        verdict.violations.len(),
        verdict.stale.len()
    );
    Ok(false)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("check") => {}
        Some("--help") | Some("-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some("--self-test") if argv.len() == 1 => {
            return match verdict_clock::selftest::run() {
                Ok(n) => {
                    println!("[verdict_clock_scan] --self-test PASS {n} planted reads");
                    ExitCode::SUCCESS
                }
                Err(failures) => {
                    for failure in failures {
                        println!("SELF-TEST-MISMATCH {failure}");
                    }
                    eprintln!("[verdict_clock_scan] --self-test FAIL");
                    ExitCode::from(1)
                }
            };
        }
        other => {
            let what = other.map_or("missing command".to_string(), |o| format!("unknown command or argument: {o}"));
            eprintln!("[verdict_clock_scan] {what}\n{USAGE}");
            return ExitCode::from(2);
        }
    }
    let (mut repo, mut allowlist) = (PathBuf::from("."), None);
    while let Some(a) = it.next() {
        let value = it.next();
        match (a.as_str(), value) {
            ("--repo", Some(v)) => repo = PathBuf::from(v),
            ("--allowlist", Some(v)) => allowlist = Some(PathBuf::from(v)),
            _ => {
                eprintln!("[verdict_clock_scan] bad argument: {a}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match check(&repo, allowlist) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("[verdict_clock_scan] ERROR {e}");
            ExitCode::from(2)
        }
    }
}
