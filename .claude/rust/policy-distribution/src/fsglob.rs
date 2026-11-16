//! `pathlib.Path.glob`/`rglob` of CPython 3.12 for the `*`-only patterns the original uses: hidden names match, the
//! recursive walk does not descend into symlinked directories, entries of any type (broken symlinks included) are
//! yielded when their name matches, unreadable directories are skipped silently (pathlib swallows PermissionError).
use std::fs;
use std::path::{Path, PathBuf};

/// `fnmatch.fnmatchcase(name, pattern)` for patterns made of literal characters and `*`.
pub fn star_match(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while i < n.len() {
        if j < p.len() && p[j] != '*' && p[j] == n[i] {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '*' {
            star = Some(j);
            mark = i;
            j += 1;
        } else if let Some(s) = star {
            j = s + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '*' {
        j += 1;
    }
    j == p.len()
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    }
}

/// `dir.glob(pattern)` for a single-component `*` pattern.
pub fn glob(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    entries(dir)
        .into_iter()
        .filter(|p| p.file_name().map(|n| star_match(&n.to_string_lossy(), pattern)).unwrap_or(false))
        .collect()
}

/// `dir.glob("*/<literal>")`: children (directories, symlinks followed) holding an existing `<literal>`.
pub fn glob_child_literal(dir: &Path, literal: &str) -> Vec<PathBuf> {
    entries(dir)
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| p.join(literal))
        .filter(|p| p.exists())
        .collect()
}

/// `dir.rglob(pattern)`.
pub fn rglob(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for p in entries(&d) {
            if p.file_name().map(|n| star_match(&n.to_string_lossy(), pattern)).unwrap_or(false) {
                out.push(p.clone());
            }
            let is_link = fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
            if !is_link && p.is_dir() {
                stack.push(p);
            }
        }
    }
    out
}
