//! `push_branches` (non-self-test part): branch-by-branch `git push` of the closing sequence, one JSON document on stdout.
//!
//! Policy (plan_26112012 #5d, h53; the rule's authority is ENV_SECRET_KEY_LEDGER C3): a branch push runs on main over an
//! ssh key only -- the PAT (`GITHUB_TOKEN`) authenticates the hint tag push and nothing else (its credential path is
//! `hint-publisher/rust/src/tag.rs::git_push_authenticated`). So this tool pushes only to remotes that need no token (ssh,
//! scp-like, local path), with the scope options of the tag push (`--no-follow-tags --no-recurse-submodules`,
//! `GIT_TERMINAL_PROMPT=0`). The judgement reads the DECLARED push addresses (before insteadOf) in git's own order (#9):
//! `remote.<r>.pushurl`, else `remote.<r>.url`, else the argument itself -- the order of `tag.rs::_declared_urls`. When any
//! of them is http(s), every branch fails loud with code [`HTTPS_REFUSED`] (no silent degrade, nothing is pushed).
//! `--self-test` is `push_branches_selftest`.

use std::process::{Command, Stdio};

const OPERATIONAL: [&str; 2] = ["single-node", "multi-node"];
pub(crate) const SCOPE_ARGS: [&str; 2] = ["--no-follow-tags", "--no-recurse-submodules"];
/// The reason code of a branch push refused because a declared push address is http(s) (plan_26112012 #5d: the name
/// states the policy, not a porting gap).
pub const HTTPS_REFUSED: &str = "BRANCH_PUSH_HTTPS_REFUSED";

/// One token-free `git push` with the given scope options; the self-test passes an emptied list to prove its tag-leak control is not vacuous.
pub(crate) fn push_local(repo: &str, remote: &str, refspec: &str, dry: bool, scope: &[&str]) -> Result<(i32, String), String> {
    let mut a = vec!["-C".to_string(), repo.to_string(), "push".into()];
    a.extend(scope.iter().map(|s| s.to_string()));
    if dry {
        a.push("--dry-run".into());
    }
    a.push(remote.to_string());
    a.push(refspec.to_string());
    match Command::new("git").args(&a).env("GIT_TERMINAL_PROMPT", "0").output() {
        Ok(o) => Ok((o.status.code().unwrap_or(1), tail(&String::from_utf8_lossy(&o.stderr), 400))),
        Err(e) => Err(format!("{e}")),
    }
}

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

/// git's remote-name rule (`remote.c` valid_remote_nick), as `tag.rs::_is_remote_nick` has it: anything else is a URL or a
/// path given directly.
fn is_remote_nick(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

/// Every value of one git config key (`--get-all`), trimmed, empty values dropped.
fn config_values(repo: &str, key: &str) -> Vec<String> {
    Command::new("git")
        .args(["-C", repo, "config", "--get-all", key])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// The DECLARED push addresses of `remote` (insteadOf not applied), in the order git pushes (plan_26112012 #9): every
/// `remote.<r>.pushurl`, else every `remote.<r>.url`, else the argument itself.
fn declared_push_urls(repo: &str, remote: &str) -> Vec<String> {
    if is_remote_nick(remote) {
        for key in [format!("remote.{remote}.pushurl"), format!("remote.{remote}.url")] {
            let values = config_values(repo, &key);
            if !values.is_empty() {
                return values;
            }
        }
    }
    vec![remote.to_string()]
}

/// Whether a branch push to `remote` is refused: one of its declared push addresses ([`declared_push_urls`]) is http(s). [`run`]
/// refuses every branch with [`HTTPS_REFUSED`] on it; the predicate of ENV_SECRET_KEY_LEDGER C3 asks the same judgement in process.
pub fn refuses_http_push_address(repo: &str, remote: &str) -> bool {
    declared_push_urls(repo, remote).iter().any(|u| u.starts_with("https://") || u.starts_with("http://"))
}

pub fn run(args: &[String]) -> i32 {
    let (mut repo, mut remote, mut branches, mut dry) = (".".to_string(), None::<String>, Vec::new(), false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--self-test" => {
                return crate::push_branches_selftest::run();
            }
            "-h" | "--help" => {
                // the original's argparse page, captured from it (golden text)
                print!("{}", include_str!("help/push_branches.txt"));
                return 0;
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
    let https = refuses_http_push_address(&repo, &remote);
    let mut items = Vec::new();
    let mut ok = true;
    for br in &branches {
        let refspec = format!("refs/heads/{br}:refs/heads/{br}");
        let (rc, code, path, err): (i32, Option<&str>, String, String) = if https {
            (
                1,
                Some(HTTPS_REFUSED),
                "refused (a declared push address is http(s); a branch push runs over an ssh key only)".to_string(),
                "http(s) push address: a branch push runs on main over an ssh key only (ENV_SECRET_KEY_LEDGER C3) -- declare an ssh, scp-like or local push address (remote.<r>.pushurl, else url)".to_string(),
            )
        } else {
            match push_local(&repo, &remote, &refspec, dry, &SCOPE_ARGS) {
                Ok((rc, err)) => (rc, None, "git push (rust-native; token-free remote)".to_string(), err),
                Err(e) => (1, None, "git push (rust-native) → spawn failure".into(), e),
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
