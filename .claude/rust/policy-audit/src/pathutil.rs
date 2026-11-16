//! Path and OS-error semantics the original relies on: `Path.resolve()` (non-strict `os.path.realpath`), `Path.parts`,
//! `shutil.which`, and `str(OSError)` (`[Errno N] strerror: 'path'`).

use std::ffi::CStr;
use std::path::{Path, PathBuf};

/// `os.path.realpath(path, strict=False)` (POSIX): absolute, symlinks resolved as far as the path exists, `..` collapsed.
pub fn resolve(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(path)
    };
    let text = abs.to_string_lossy().into_owned();
    let mut resolved = String::new(); // "" means "/"
    let mut seen: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    realpath_join(&mut resolved, &text, &mut seen);
    if resolved.is_empty() {
        PathBuf::from("/")
    } else {
        PathBuf::from(resolved)
    }
}

fn realpath_join(path: &mut String, rest: &str, seen: &mut std::collections::HashMap<String, Option<String>>) {
    let mut rest = rest.to_string();
    if rest.starts_with('/') {
        path.clear();
        rest = rest.trim_start_matches('/').to_string();
    }
    let mut todo: Vec<String> = rest.split('/').map(|s| s.to_string()).collect();
    todo.reverse();
    while let Some(name) = todo.pop() {
        if name.is_empty() || name == "." {
            continue;
        }
        if name == ".." {
            if let Some(i) = path.rfind('/') {
                path.truncate(i);
            } else {
                path.clear();
            }
            continue;
        }
        let newpath = format!("{path}/{name}");
        match std::fs::symlink_metadata(&newpath) {
            Ok(m) if m.file_type().is_symlink() => {
                if let Some(cached) = seen.get(&newpath) {
                    match cached {
                        Some(p) => {
                            *path = p.clone();
                            continue;
                        }
                        None => {
                            // loop: keep the unresolved path (non-strict)
                            *path = newpath;
                            continue;
                        }
                    }
                }
                seen.insert(newpath.clone(), None);
                match std::fs::read_link(&newpath) {
                    Ok(target) => {
                        let t = target.to_string_lossy().into_owned();
                        realpath_join(path, &t, seen);
                        seen.insert(newpath, Some(path.clone()));
                    }
                    Err(_) => *path = newpath,
                }
            }
            _ => *path = newpath,
        }
    }
}

/// `pathlib.PurePosixPath(rel).parts` for a relative path: empty and `.` segments dropped.
pub fn parts(rel: &str) -> Vec<String> {
    let mut out = Vec::new();
    if rel.starts_with('/') {
        out.push("/".to_string());
    }
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        out.push(seg.to_string());
    }
    out
}

/// `shutil.which(name) is not None`
pub fn which(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else { return false };
    for dir in std::env::split_paths(&paths) {
        let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
        let p = dir.join(name);
        if let Ok(m) = std::fs::metadata(&p) {
            use std::os::unix::fs::PermissionsExt;
            if m.is_file() && m.permissions().mode() & 0o111 != 0 {
                return true;
            }
        }
    }
    false
}

pub fn strerror(errno: i32) -> String {
    // SAFETY: strerror returns a pointer to a static NUL-terminated message for any errno.
    unsafe { CStr::from_ptr(libc::strerror(errno)).to_string_lossy().into_owned() }
}

/// `str(OSError)` for an error raised by `open(path)` / `path.read_bytes()`.
pub fn os_error_text(err: &std::io::Error, path: &Path) -> String {
    match err.raw_os_error() {
        Some(n) => format!("[Errno {n}] {}: {}", strerror(n), crate::value::r(&path.to_string_lossy())),
        None => err.to_string(),
    }
}
