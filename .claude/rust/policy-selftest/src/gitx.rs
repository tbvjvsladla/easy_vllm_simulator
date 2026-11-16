//! `subprocess.run(["git", ...], capture_output=True)` as the originals call it.
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct Out {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Out {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn err_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

/// Run a program with captured output; a spawn failure is `code = -1` with the error in stderr.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>, stdin: Option<&[u8]>) -> Out {
    let mut cmd = Command::new(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Out { code: -1, stdout: Vec::new(), stderr: e.to_string().into_bytes() },
    };
    if let Some(data) = stdin {
        if let Some(mut si) = child.stdin.take() {
            let data = data.to_vec();
            std::thread::spawn(move || {
                let _ = si.write_all(&data);
            });
        }
    }
    match child.wait_with_output() {
        Ok(o) => Out { code: o.status.code().unwrap_or(-1), stdout: o.stdout, stderr: o.stderr },
        Err(e) => Out { code: -1, stdout: Vec::new(), stderr: e.to_string().into_bytes() },
    }
}

/// `git -C <root> <args...>`.
pub fn git(root: &str, args: &[&str]) -> Out {
    let mut a = vec!["-C", root];
    a.extend_from_slice(args);
    run("git", &a, None, None)
}

pub fn git_stdin(root: &str, args: &[&str], stdin: &[u8]) -> Out {
    let mut a = vec!["-C", root];
    a.extend_from_slice(args);
    run("git", &a, None, Some(stdin))
}

/// `-z` output split on NUL, empty items dropped (surrogateescape-ish: lossy).
pub fn split_z(out: &[u8]) -> Vec<String> {
    out.split(|&b| b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
}
