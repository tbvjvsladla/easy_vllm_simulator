//! `push_branches --self-test` (Rust): isolated repo + temporary bare remotes, no network, no real token.
//!
//! Boundary vs the Python self-test: the hintlib credential path (module load, `PUSH_SCOPE_ARGS` cross-check, HintError mutation controls)
//! is Python-plane and is not exercised here. Instead the Rust-owned contract is asserted: an https-DECLARED remote (reserved `.invalid`
//! TLD, redirected to a local bare via insteadOf) fails loud per branch with `HTTPS_NEEDS_HINTLIB`; token-free remotes push every branch,
//! never force, never carry tags along under `push.followTags=true`, and the target repo (not the CWD) is pushed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fx {
    root: PathBuf,
    env: Vec<(String, String)>,
    failures: Vec<String>,
}

impl Fx {
    fn g(&self, cwd: &Path, args: &[&str]) -> Result<String, String> {
        let mut c = Command::new("git");
        c.arg("-C").arg(cwd).args(args);
        self.apply(&mut c);
        let o = c.output().map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(format!("fixture git {args:?} failed: {}", String::from_utf8_lossy(&o.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    }
    fn apply(&self, c: &mut Command) {
        for k in ["GITHUB_TOKEN", "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX"] {
            c.env_remove(k);
        }
        for (k, v) in &self.env {
            c.env(k, v);
        }
    }
    fn bare(&self, name: &str) -> Result<PathBuf, String> {
        let p = self.root.join(name);
        self.g(&self.root, &["init", "-q", "--bare", &p.to_string_lossy()])?;
        Ok(p)
    }
    fn refs(&self, b: &Path, prefix: &str) -> Vec<String> {
        self.g(b, &["for-each-ref", "--format=%(refname)", prefix]).unwrap_or_default().lines().map(str::to_string).collect()
    }
    fn ck(&mut self, name: &str, cond: bool, got: &str) {
        eprintln!("  {} {name}", if cond { "ok  " } else { "FAIL" });
        if !cond {
            let mut s = format!("{name} (got={got})");
            s.truncate(600);
            self.failures.push(s);
        }
    }
    /// Run the push_branches binary itself; returns (rc, stdout, stderr).
    fn run(&self, repo: &Path, remote: &str, extra: &[(&str, &str)], cwd: &Path) -> (i32, String, String) {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("push_branches"));
        let mut c = Command::new(exe);
        c.args(["--repo", &repo.to_string_lossy(), "--remote", remote, "--branch", "single-node", "--branch", "multi-node"]).current_dir(cwd);
        self.apply(&mut c);
        for (k, v) in extra {
            c.env(k, v);
        }
        match c.output() {
            Ok(o) => (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()),
            Err(e) => (-1, String::new(), e.to_string()),
        }
    }
}

fn field_of(doc: &str, branch: &str, key: &str) -> Option<String> {
    // The document is the fixed pretty-print of this crate; locate the branch block and read the key line.
    let at = doc.find(&format!("\"branch\": \"{branch}\""))?;
    let block = &doc[at..];
    let end = block.find("}").unwrap_or(block.len());
    let block = &block[..end];
    let line = block.lines().find(|l| l.trim_start().starts_with(&format!("\"{key}\":")))?;
    Some(line.split_once(':')?.1.trim().trim_end_matches(',').to_string())
}

fn scenario(fx: &mut Fx) -> Result<(), String> {
    let root = fx.root.clone();
    let gcfg = root.join("gitconfig");
    fs::write(&gcfg, "[user]\n\tname = selftest\n\temail = t@t\n[commit]\n\tgpgsign = false\n[tag]\n\tgpgSign = false\n[core]\n\thooksPath = /dev/null\n").map_err(|e| e.to_string())?;
    fx.env = vec![
        ("GIT_CONFIG_GLOBAL".into(), gcfg.to_string_lossy().into_owned()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
    ];
    let remote = fx.bare("remote.git")?;
    let repo = root.join("repo");
    fs::create_dir_all(&repo).map_err(|e| e.to_string())?;
    let w = |p: &str, t: &str| fs::write(repo.join(p), t).map_err(|e| e.to_string());
    fx.g(&repo, &["init", "-q", "-b", "multi-node"])?;
    w("f.txt", "1\n")?;
    fx.g(&repo, &["add", "-A"])?;
    fx.g(&repo, &["commit", "-qm", "base"])?;
    fx.g(&repo, &["branch", "single-node"])?;
    let rs = remote.to_string_lossy().into_owned();
    fx.g(&repo, &["push", "-q", &rs, "refs/heads/multi-node:refs/heads/multi-node", "refs/heads/single-node:refs/heads/single-node"])?;
    w("f.txt", "2\n")?;
    fx.g(&repo, &["commit", "-qam", "multi ahead"])?;
    fx.g(&repo, &["checkout", "-q", "single-node"])?;
    w("g.txt", "remote-only\n")?;
    fx.g(&repo, &["add", "-A"])?;
    fx.g(&repo, &["commit", "-qm", "remote-only"])?;
    fx.g(&repo, &["push", "-q", &rs, "refs/heads/single-node:refs/heads/single-node"])?;
    fx.g(&repo, &["reset", "-q", "--hard", "HEAD~1"])?;
    w("h.txt", "local-diverged\n")?;
    fx.g(&repo, &["add", "-A"])?;
    fx.g(&repo, &["commit", "-qm", "local diverged"])?;
    fx.g(&repo, &["checkout", "-q", "multi-node"])?;
    let single_before = fx.g(&remote, &["rev-parse", "refs/heads/single-node"])?;
    let multi_before = fx.g(&remote, &["rev-parse", "refs/heads/multi-node"])?;
    let local_multi = fx.g(&repo, &["rev-parse", "multi-node"])?;

    let https_url = "https://fixture.invalid/remote.git";
    fx.g(&repo, &["remote", "add", "fx", https_url])?;
    fx.g(&repo, &["config", &format!("url.{rs}.insteadOf"), https_url])?;
    fx.g(&repo, &["config", "push.followTags", "true"])?;
    fx.g(&repo, &["tag", "-a", "-m", "local rollback anchor", "last-good-fixture", "multi-node"])?;
    let ctrl = fx.bare("ctrl-follow.git")?;
    fx.g(&repo, &["push", "-q", &ctrl.to_string_lossy(), "refs/heads/multi-node:refs/heads/multi-node"])?;
    let r = fx.refs(&ctrl, "refs/tags");
    fx.ck("대조: 옵션 없는 git push 는 followTags 로 태그를 딸려 보낸다(픽스처가 살아 있다)", r == ["refs/tags/last-good-fixture"], &format!("{r:?}"));

    let decoy = root.join("decoy");
    fs::create_dir_all(&decoy).map_err(|e| e.to_string())?;
    fx.g(&decoy, &["init", "-q", "-b", "multi-node"])?;
    fs::write(decoy.join("d.txt"), "decoy\n").map_err(|e| e.to_string())?;
    fx.g(&decoy, &["add", "-A"])?;
    fx.g(&decoy, &["commit", "-qm", "decoy"])?;

    // A: https-declared remote → fail loud per branch (HTTPS_NEEDS_HINTLIB), both branches reported, remote untouched.
    let (rc, out, _) = fx.run(&repo, "fx", &[], &root);
    let codes: Vec<_> = ["single-node", "multi-node"].iter().map(|b| field_of(&out, b, "code")).collect();
    fx.ck("★S5 https 선언 원격 → 두 브랜치 모두 HTTPS_NEEDS_HINTLIB · rc=5 (첫 실패에서 멈추지 않는다)",
        rc == 5 && codes.iter().all(|c| c.as_deref() == Some("\"HTTPS_NEEDS_HINTLIB\"")), &format!("{rc} {out}"));
    let same = fx.g(&remote, &["rev-parse", "refs/heads/multi-node"])? == multi_before && fx.g(&remote, &["rev-parse", "refs/heads/single-node"])? == single_before;
    fx.ck("https fail-loud 에서는 원격이 그대로다", same, "");

    // B: token-free local remote; single-node non-ff rejected, multi-node still pushed, no force, no tags; CWD is another repo.
    let (rc, out, _) = fx.run(&repo, &rs, &[], &decoy);
    let s_rc = field_of(&out, "single-node", "returncode").unwrap_or_default();
    let m_rc = field_of(&out, "multi-node", "returncode").unwrap_or_default();
    fx.ck("★S5 non-fast-forward 거부 → 그 브랜치만 실패 · 다음 브랜치는 시도해 성공 · rc=5", rc == 5 && s_rc != "0" && !s_rc.is_empty() && m_rc == "0", &format!("{rc} {out}"));
    fx.ck("★강제 push 없음: 원격 single-node 는 그대로", fx.g(&remote, &["rev-parse", "refs/heads/single-node"])? == single_before, "");
    fx.ck("★대상 저장소 기준: CWD 가 다른 저장소여도 --repo 의 커밋을 민다", fx.g(&remote, &["rev-parse", "refs/heads/multi-node"])? == local_multi, "");
    let r = fx.refs(&remote, "refs/tags");
    fx.ck("★followTags=true 에서도 브랜치 push 가 태그를 딸려 보내지 않는다(last-good 유출 ✗)", r.is_empty(), &format!("{r:?}"));

    // C: a clean local remote takes both branches with no token.
    let local_remote = fx.bare("local.git")?;
    let (rc, out, _) = fx.run(&repo, &local_remote.to_string_lossy(), &[], &root);
    let both = ["single-node", "multi-node"].iter().all(|b| field_of(&out, b, "returncode").as_deref() == Some("0") && field_of(&out, b, "code").as_deref() == Some("null"));
    fx.ck("★H7 로컬(비-https) 원격 · 토큰 없음 → 두 브랜치 모두 push 성공 · rc=0", rc == 0 && both, &format!("{rc} {out}"));
    let r = fx.refs(&local_remote, "refs/tags");
    fx.ck("C: 태그 딸림 없음", r.is_empty(), &format!("{r:?}"));
    Ok(())
}

pub fn run() -> i32 {
    let root = std::env::temp_dir().join(format!("push_branches_selftest_{}", std::process::id()));
    if let Err(e) = fs::create_dir_all(&root) {
        eprintln!("[push_branches] self-test FAIL: {e}");
        return 1;
    }
    let mut fx = Fx { root: root.clone(), env: Vec::new(), failures: Vec::new() };
    if let Err(e) = scenario(&mut fx) {
        fx.failures.push(format!("fixture error: {e}"));
    }
    let _ = fs::remove_dir_all(&root);
    if !fx.failures.is_empty() {
        for f in &fx.failures {
            eprintln!("[push_branches] self-test FAIL: {f}");
        }
        return 1;
    }
    eprintln!("[push_branches] self-test PASS");
    0
}
