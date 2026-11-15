//! `push_branches.py` (non-self-test part): branch-by-branch `git push` of the closing sequence, one JSON document on stdout.
//!
//! Port boundary: the Python tool loads `hintlib.tag.git_push_authenticated` (hint-publisher, Python) for the credential path. That path is not
//! reproducible without Python, so this port pushes only to remotes that need no token (ssh, scp-like, local path) with the same scope options
//! as the Python fallback (`--no-follow-tags --no-recurse-submodules`, `GIT_TERMINAL_PROMPT=0`). A remote whose DECLARED url is http(s) fails
//! loud per branch with code `HTTPS_NEEDS_HINTLIB` (no silent degrade). `--self-test` is not ported (exit 2).

use std::process::{Command, Stdio};

const OPERATIONAL: [&str; 2] = ["single-node", "multi-node"];
const SCOPE_ARGS: [&str; 2] = ["--no-follow-tags", "--no-recurse-submodules"];

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    o.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    let t: String = if c.len() > n { c[c.len() - n..].iter().collect() } else { s.to_string() };
    t.trim().to_string()
}

fn declared_url(repo: &str, remote: &str) -> String {
    Command::new("git")
        .args(["-C", repo, "config", "--get", &format!("remote.{remote}.url")])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn run(args: &[String]) -> i32 {
    let (mut repo, mut remote, mut branches, mut dry) = (".".to_string(), None::<String>, Vec::new(), false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--self-test" => {
                eprintln!("[push_branches] self-test not ported to Rust (use push_branches.py --self-test)");
                return 2;
            }
            "--dry-run" => dry = true,
            "--repo" | "--remote" | "--branch" if i + 1 < args.len() => {
                let v = args[i + 1].clone();
                match args[i].as_str() {
                    "--repo" => repo = v,
                    "--remote" => remote = Some(v),
                    _ => branches.push(v),
                }
                i += 1;
            }
            other => {
                eprintln!("[push_branches] FAIL: unrecognized or incomplete argument: {other}");
                return 2;
            }
        }
        i += 1;
    }
    let remote = match remote {
        Some(r) if !branches.is_empty() => r,
        _ => {
            eprintln!("[push_branches] FAIL: --remote 와 --branch 가 필요하다");
            return 2;
        }
    };
    if remote.starts_with('-') {
        eprintln!("[push_branches] FAIL: 원격 인자가 옵션처럼 보인다: {remote:?}");
        return 2;
    }
    for br in &branches {
        if !OPERATIONAL.contains(&br.as_str()) {
            eprintln!("[push_branches] FAIL: 운영 refs 는 single-node·multi-node 뿐이다: '{br}'");
            return 2;
        }
    }
    let decl = declared_url(&repo, &remote);
    let probe = if decl.is_empty() { remote.clone() } else { decl };
    let https = probe.starts_with("https://") || probe.starts_with("http://");
    let mut items = Vec::new();
    let mut ok = true;
    for br in &branches {
        let refspec = format!("refs/heads/{br}:refs/heads/{br}");
        let (rc, code, path, err): (i32, Option<&str>, String, String) = if https {
            (
                1,
                Some("HTTPS_NEEDS_HINTLIB"),
                "rust-native (https remote needs hintlib credential path; not ported)".to_string(),
                "https remote: authenticated push is owned by hintlib (Python); use push_branches.py".to_string(),
            )
        } else {
            let mut a = vec!["-C".to_string(), repo.clone(), "push".into()];
            a.extend(SCOPE_ARGS.iter().map(|s| s.to_string()));
            if dry {
                a.push("--dry-run".into());
            }
            a.push(remote.clone());
            a.push(refspec.clone());
            match Command::new("git").args(&a).env("GIT_TERMINAL_PROMPT", "0").output() {
                Ok(o) => (
                    o.status.code().unwrap_or(1),
                    None,
                    "git push (rust-native; token-free remote)".to_string(),
                    tail(&String::from_utf8_lossy(&o.stderr), 400),
                ),
                Err(e) => (1, None, "git push (rust-native) → spawn failure".into(), format!("{e}")),
            }
        };
        if !err.is_empty() && rc != 0 {
            eprintln!("{err}");
        }
        if rc != 0 {
            ok = false;
        }
        let code_s = code.map(json_str).unwrap_or_else(|| "null".into());
        items.push(format!(
            "    {{\n      \"branch\": {},\n      \"refspec\": {},\n      \"returncode\": {rc},\n      \"code\": {code_s},\n      \"path\": {},\n      \"stderr_tail\": {}\n    }}",
            json_str(br), json_str(&refspec), json_str(&path), json_str(&err)
        ));
    }
    println!(
        "{{\n  \"remote\": {},\n  \"dry_run\": {},\n  \"ok\": {},\n  \"results\": [\n{}\n  ]\n}}",
        json_str(&remote), dry, ok, items.join(",\n")
    );
    if ok { 0 } else { 5 }
}
