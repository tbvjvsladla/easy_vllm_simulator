//! The original's command line: `argparse.ArgumentParser()` with `--json-out`, its usage/help/
//! error text (help wrapped for argparse's default 80-column width), then `verify()`, the indented JSON on stdout,
//! the optional `--json-out` copy and the exit code (0 PASS · 1 FAIL · 2 usage).
use crate::checks::verify;
use std::io::Write;
use std::path::{Path, PathBuf};

const OPTIONS: [&str; 2] = ["--help", "--json-out"];

fn usage(prog: &str) -> String {
    format!("usage: {prog} [-h] [--json-out JSON_OUT]\n")
}

fn help(prog: &str) -> String {
    format!(
        "{}\noptions:\n  -h, --help           show this help message and exit\n  --json-out JSON_OUT\n",
        usage(prog)
    )
}

fn fail(prog: &str, message: &str) -> i32 {
    eprint!("{}{prog}: error: {message}\n", usage(prog));
    2
}

/// Resolve an option string as argparse does: exact name, else a unique prefix of a long option.
fn resolve(arg: &str) -> Result<Option<&'static str>, String> {
    if arg == "-h" {
        return Ok(Some("--help"));
    }
    if !arg.starts_with("--") || arg == "--" {
        return Ok(None);
    }
    let name = arg.split('=').next().unwrap();
    if let Some(o) = OPTIONS.iter().find(|o| **o == name) {
        return Ok(Some(o));
    }
    let hits: Vec<&&str> = OPTIONS.iter().filter(|o| o.starts_with(name)).collect();
    match hits.len() {
        0 => Ok(None),
        1 => Ok(Some(hits[0])),
        _ => Err(format!(
            "ambiguous option: {name} could match {}",
            hits.iter().map(|h| h.to_string()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// An argument argparse would read as an option (so not as an option's value).
fn looks_like_option(arg: &str) -> bool {
    if !arg.starts_with('-') || arg == "-" {
        return false;
    }
    // `_parse_optional`: a negative number (`^-\d+$|^-\d*\.\d+$`, no option looks like one) or a string holding a
    // space is a positional value.
    let rest = &arg[1..];
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let negative = digits(rest)
        || rest.split_once('.').map(|(a, b)| (a.is_empty() || digits(a)) && digits(b)).unwrap_or(false);
    !negative && !arg.contains(' ')
}

/// The repository root: `Path(__file__).resolve().parents[3]` of the public `.py` path the launcher passes as argv[0]
/// (`exec -a`), else the ancestor of the native executable that holds `.claude/policies/runtime`.
pub fn repo_root(argv0: &str) -> PathBuf {
    let p = Path::new(argv0);
    if p.file_name().map(|n| n == "verify_distribution.sh").unwrap_or(false) {
        let abs = std::fs::canonicalize(p).unwrap_or_else(|_| {
            std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf())
        });
        if let Some(r) = abs.ancestors().nth(4) {
            return r.to_path_buf();
        }
    }
    if let Ok(exe) = std::env::current_exe().and_then(std::fs::canonicalize) {
        for a in exe.ancestors() {
            if a.join(".claude/policies/runtime").is_dir() {
                return a.to_path_buf();
            }
        }
    }
    std::env::current_dir().unwrap_or_default()
}

pub fn main_with(argv: Vec<String>) -> i32 {
    let argv0 = argv.first().cloned().unwrap_or_else(|| "verify_distribution.sh".into());
    let prog = Path::new(&argv0).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(argv0.clone());
    let mut json_out: Option<String> = None;
    let mut extras: Vec<String> = Vec::new();
    let mut i = 1;
    let mut positional_only = false;
    while i < argv.len() {
        let a = &argv[i];
        if positional_only {
            extras.push(a.clone());
            i += 1;
            continue;
        }
        if a == "--" {
            // CPython 3.12 with no positional parameter: `--` itself is left among the extras.
            positional_only = true;
            extras.push(a.clone());
            i += 1;
            continue;
        }
        if a.starts_with("-h") && a.len() > 2 && !a.starts_with("--") {
            let explicit = a.strip_prefix("-h=").unwrap_or(&a[2..]);
            return fail(&prog, &format!("argument -h/--help: ignored explicit argument {}", crate::pytext::py_repr(explicit)));
        }
        let opt = match resolve(a) {
            Ok(o) => o,
            Err(m) => return fail(&prog, &m),
        };
        match opt {
            Some("--help") => {
                if a.contains('=') {
                    return fail(&prog, &format!("argument -h/--help: ignored explicit argument {}", crate::pytext::py_repr(a.split_once('=').unwrap().1)));
                }
                print!("{}", help(&prog));
                let _ = std::io::stdout().flush();
                return 0;
            }
            Some(_) => {
                let value = if let Some((_, v)) = a.split_once('=') {
                    v.to_string()
                } else if i + 1 < argv.len() && !looks_like_option(&argv[i + 1]) {
                    i += 1;
                    argv[i].clone()
                } else {
                    return fail(&prog, "argument --json-out: expected one argument");
                };
                json_out = Some(value);
            }
            None => extras.push(a.clone()),
        }
        i += 1;
    }
    if !extras.is_empty() {
        return fail(&prog, &format!("unrecognized arguments: {}", extras.join(" ")));
    }
    let repo = repo_root(&argv0);
    let result = match verify(&repo) {
        Err(uncaught) => {
            eprintln!("Traceback (most recent call last):\n  (native verify_distribution)\n{}", uncaught.0);
            return 1;
        }
        Ok(v) => v,
    };
    let text = match result.dumps_pretty() {
        Ok(t) => t.to_utf8().unwrap_or_default(),
        Err(_) => {
            eprintln!("[verify_distribution] report is not JSON-serialisable");
            return 1;
        }
    };
    println!("{text}");
    let _ = std::io::stdout().flush();
    if let Some(path) = json_out {
        if let Err(e) = std::fs::write(&path, format!("{text}\n")) {
            let pe = crate::pytext::os_error(&e, &path);
            eprintln!("Traceback (most recent call last):\n  (native verify_distribution)\n{}: {}", pe.type_name, pe.message);
            return 1;
        }
    }
    let pass = crate::get(&result, "verdict").map(|v| matches!(v.peel(), crate::PyValue::Str(s) if s.equals("PASS"))).unwrap_or(false);
    if pass { 0 } else { 1 }
}
