//! `pathlib.PurePosixPath` text rules and Python `OSError` texts the ported programs print.

/// `str(Path(text))`: repeated separators collapsed, `.` components and a trailing separator dropped (`..` kept); an
/// initial `//` (exactly two) is kept, as POSIX allows; the empty path is `.`.
pub fn py_path(text: &str) -> String {
    if text.is_empty() {
        return ".".into();
    }
    let root = if text.starts_with("//") && !text.starts_with("///") {
        "//"
    } else if text.starts_with('/') {
        "/"
    } else {
        ""
    };
    let parts: Vec<&str> = text
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let body = parts.join("/");
    if root.is_empty() && body.is_empty() {
        return ".".into();
    }
    format!("{root}{body}")
}

/// `Path(text).name`.
pub fn py_name(text: &str) -> String {
    let p = py_path(text);
    let root_only = p == "/" || p == "//" || p == ".";
    if root_only {
        return String::new();
    }
    p.rsplit('/').next().unwrap_or("").to_string()
}

/// The last line of the traceback Python prints for an `OSError` raised by `open(path)` (exit status 1).
pub fn os_error(e: &std::io::Error, path: &str) -> String {
    let (class, errno, msg) = match e.raw_os_error() {
        Some(2) => ("FileNotFoundError", 2, "No such file or directory"),
        Some(13) => ("PermissionError", 13, "Permission denied"),
        Some(21) => ("IsADirectoryError", 21, "Is a directory"),
        Some(20) => ("NotADirectoryError", 20, "Not a directory"),
        Some(n) => {
            return format!(
                "OSError: [Errno {n}] {}: {}",
                e,
                agent_runtime_support::pystr::PyStr::new(path).repr()
            )
        }
        None => return format!("OSError: {e}"),
    };
    format!(
        "{class}: [Errno {errno}] {msg}: {}",
        agent_runtime_support::pystr::PyStr::new(path).repr()
    )
}
