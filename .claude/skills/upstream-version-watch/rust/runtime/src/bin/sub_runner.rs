//! `sub_runner` — the three sub-node helpers that `sync_to_sub.sh` used to run as inline `python3 -c` on the sub node:
//! `prune-empty-dirs ROOT SKIP POST`, `validate-names ROOT SKIP POST` and `retirement-scan STALE` (surface list on stdin).
//! Standard library only: the binary is delivered to the sub by the main node (plane B) and needs no Python and no cargo there.
//! stdout / stderr / exit code follow the original heredocs (exit 9 = prune failure, 1 = scanner failure).
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn split_set(s: &str) -> HashSet<String> {
    s.split(':').filter(|x| !x.is_empty()).map(String::from).collect()
}

fn is_dir_nolink(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.file_type().is_dir()).unwrap_or(false)
}

fn first_component(rel: &str) -> &str {
    rel.split('/').next().unwrap_or("")
}

fn excluded(rel: &str, skip: &HashSet<String>, post: &HashSet<String>) -> bool {
    // rel != "." and (first in skip or ("/" in rel and first in post))
    !rel.is_empty() && (skip.contains(first_component(rel)) || (rel.contains('/') && post.contains(first_component(rel))))
}

fn prune(dir: &Path, rel: &str, skip: &HashSet<String>, post: &HashSet<String>, removed: &mut Vec<String>) -> Result<(), String> {
    let rd = match fs::read_dir(dir) { Ok(r) => r, Err(_) => return Ok(()) };
    let mut subs: Vec<String> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if is_dir_nolink(&e.path()) { subs.push(name); }
    }
    for name in subs {
        let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        prune(&dir.join(&name), &r, skip, post, removed)?;
    }
    if rel.is_empty() || excluded(rel, skip, post) { return Ok(()); }
    let empty = fs::read_dir(dir).map_err(|e| format!("prune-failed {}: {e}", dir.display()))?.next().is_none();
    if empty {
        fs::remove_dir(dir).map_err(|e| format!("prune-failed {}: {e}", dir.display()))?;
        removed.push(rel.to_string());
    }
    Ok(())
}

fn cmd_prune(a: &[String]) -> i32 {
    if a.len() != 3 { eprintln!("usage: sub_runner prune-empty-dirs ROOT SKIP POST"); return 2; }
    let root = PathBuf::from(&a[0]);
    if !root.is_dir() { return 0; }
    let (skip, post) = (split_set(&a[1]), split_set(&a[2]));
    let mut removed = Vec::new();
    if let Err(m) = prune(&root, "", &skip, &post, &mut removed) {
        eprintln!("{m}");
        return 9;
    }
    println!("{}", removed.join("\n"));
    0
}

fn walk_names(dir: &Path, rel: &str, skip: &HashSet<String>, post: &HashSet<String>, hits: &mut Vec<String>) {
    let rd = match fs::read_dir(dir) { Ok(r) => r, Err(_) => return };
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        let name = e.file_name();
        if is_dir_nolink(&e.path()) { dirs.push(name); } else { files.push(name); }
    }
    if rel.is_empty() {
        dirs.retain(|d| !skip.contains(d.to_string_lossy().as_ref()));
    } else if post.contains(rel) {
        dirs.clear();
    }
    for name in dirs.iter().chain(files.iter()) {
        if rel.is_empty() && skip.contains(name.to_string_lossy().as_ref()) { continue; }
        if name.as_bytes().iter().any(|&b| b <= 32 || b == 127) {
            let lossy = name.to_string_lossy();
            hits.push(if rel.is_empty() { lossy.into_owned() } else { format!("{rel}/{lossy}") });
        }
    }
    for d in &dirs {
        let n = d.to_string_lossy();
        let r = if rel.is_empty() { n.into_owned() } else { format!("{rel}/{n}") };
        walk_names(&dir.join(d), &r, skip, post, hits);
    }
}

fn cmd_names(a: &[String]) -> i32 {
    if a.len() != 3 { eprintln!("usage: sub_runner validate-names ROOT SKIP POST"); return 2; }
    let root = PathBuf::from(&a[0]);
    if !root.is_dir() { return 0; }
    let (skip, post) = (split_set(&a[1]), split_set(&a[2]));
    let mut hits = Vec::new();
    walk_names(&root, "", &skip, &post, &mut hits);
    println!("{}", hits.join("\n"));
    0
}

fn is_chars(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-') }
fn is_before(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') }

fn rust_checks_path(p: &str) -> bool {
    // re.match(r"\.claude/(skills/[^/]+/)?rust/[^/]+/checks/", p)
    let Some(rest) = p.strip_prefix(".claude/") else { return false };
    let tail = match rest.strip_prefix("skills/") {
        Some(r) if r.split_once('/').map_or(false, |(s, _)| !s.is_empty()) && r.split_once('/').unwrap().1.starts_with("rust/") => r.split_once('/').unwrap().1,
        _ => rest,
    };
    let Some(t) = tail.strip_prefix("rust/") else { return false };
    matches!(t.split_once('/'), Some((s, r)) if !s.is_empty() && r.starts_with("checks/"))
}

/// Not consumers (plan_26111615 · the Sub now receives Rust crate sources): the parity tool's case/golden data is the same
/// fixture plane as a crate's `checks/`, and the renderer that DECLARES what the Sub must not receive has to name each
/// retired path to exclude it (the 2026-09-10 reverse-oracle: a declaration is not a call). Closed list — a new declaring
/// owner is added here by review, never by pattern.
const RETIREMENT_DECLARING_OWNERS: [&str; 2] = [
    ".claude/skills/terraforming_node/rust/src/render_sub_env.rs",
    ".claude/skills/terraforming_node/rust/src/render_sub_env/selftest.rs",
];

fn fixture_or_declaring_owner(p: &str) -> bool {
    p.starts_with(".claude/rust/parity/cases/") || p.starts_with(".claude/rust/parity/data/") || RETIREMENT_DECLARING_OWNERS.contains(&p)
}

fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, b) = (0usize, s.as_bytes());
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(&s[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' && b.get(next) == Some(&b'\n') { it.next(); next += 1; }
            start = next;
        }
    }
    if start < s.len() { out.push(&s[start..]); }
    out
}

fn cmd_scan(a: &[String]) -> i32 {
    if a.len() != 1 { eprintln!("usage: sub_runner retirement-scan STALE < surfaces"); return 2; }
    let stale: Vec<char> = a[0].chars().collect();
    let owner = format!(".claude/skills/upstream-version-watch/{}", a[0]);
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let surfaces: Vec<&str> = input.lines().filter(|x| !x.trim().is_empty()).collect();
    if surfaces.is_empty() { eprintln!("active surface list empty"); return 1; }
    let (mut n, mut hits) = (0usize, Vec::<String>::new());
    for rel in surfaces {
        let p = Path::new(rel);
        let Ok(md) = fs::symlink_metadata(p) else { continue };
        if md.file_type().is_symlink() {
            eprintln!("RuntimeError: active scanner refuses symlink: {rel}");
            return 1;
        }
        if !md.is_file() { continue; }
        n += 1;
        if rust_checks_path(rel) || fixture_or_declaring_owner(rel) { continue; }
        let data = fs::read(p).unwrap_or_default();
        let text = String::from_utf8_lossy(&data).into_owned();
        for (ln, line) in py_splitlines(&text).iter().enumerate() {
            let cs: Vec<char> = line.chars().collect();
            let mut i = 0;
            while !stale.is_empty() && i + stale.len() <= cs.len() {
                let ok = cs[i..i + stale.len()] == stale[..]
                    && (i == 0 || !is_before(cs[i - 1]))
                    && cs.get(i + stale.len()).map_or(true, |&c| !is_chars(c));
                if !ok { i += 1; continue; }
                let (mut lo, mut hi) = (i, i + stale.len());
                while lo > 0 && is_chars(cs[lo - 1]) { lo -= 1; }
                while hi < cs.len() && is_chars(cs[hi]) { hi += 1; }
                let mut token: String = cs[lo..hi].iter().collect();
                while let Some(t) = token.strip_prefix("./") { token = t.to_string(); }
                if token != owner { hits.push(format!("{rel}:{}:{line}", ln + 1)); }
                i += stale.len();
            }
        }
    }
    let mut out = std::io::stdout();
    let _ = write!(out, "{n}\n{}", hits.join("\n"));
    let _ = out.flush();
    0
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("prune-empty-dirs") => cmd_prune(&args[1..]),
        Some("validate-names") => cmd_names(&args[1..]),
        Some("retirement-scan") => cmd_scan(&args[1..]),
        _ => { eprintln!("usage: sub_runner {{prune-empty-dirs|validate-names|retirement-scan}} ..."); 2 }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
