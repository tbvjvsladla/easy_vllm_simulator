//! `push_branches --self-test` (Rust): isolated repo + temporary bare remotes, no network, no real token.
//!
//! The contract asserted (plan_26112012 #9 · #5d: a branch push runs over an ssh key only): a remote whose DECLARED push
//! address is http(s) (reserved `.invalid` TLD, redirected to a local bare via insteadOf so a slip would really land)
//! fails loud per branch with `BRANCH_PUSH_HTTPS_REFUSED` and pushes nothing; the declared push address is read in git's
//! order -- `pushurl`, else `url` -- and any one http(s) value among several refuses; token-free remotes push every branch,
//! never force, never carry tags along under `push.followTags=true`, and the target repo (not the CWD) is pushed. The scope
//! options are cross-checked against the tag push's `PUSH_SCOPE_ARGS` in hint-publisher `tag.rs` (read as text).

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

/// The owner is the Rust hintlib (`hint-publisher/rust/src/tag.rs`, the port of `hintlib/tag.py`); it is not a dependency of this
/// crate, so its `pub const PUSH_SCOPE_ARGS: [&str; N] = [...];` declaration is read as text (one anchor, else fail) — no `.py`.
fn hintlib_scope_args() -> Result<Vec<String>, String> {
    const REL: &str = ".claude/skills/hint-publisher/rust/src/tag.rs";
    const ANCHOR: &str = "pub const PUSH_SCOPE_ARGS: [&str; ";
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().map(Path::to_path_buf));
    }
    let path = roots.iter().map(|r| r.join(REL)).find(|p| p.is_file()).ok_or_else(|| format!("실물 hintlib 이 없다: {REL}"))?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let anchors: Vec<&str> = text.lines().filter(|l| l.starts_with(ANCHOR)).collect();
    if anchors.len() != 1 {
        return Err(format!("PUSH_SCOPE_ARGS 앵커 {}건(1건이어야 한다)", anchors.len()));
    }
    // `pub const PUSH_SCOPE_ARGS: [&str; 2] = ["--no-follow-tags", "--no-recurse-submodules"];`
    let (count, rest) = anchors[0][ANCHOR.len()..].split_once(']').ok_or("PUSH_SCOPE_ARGS 선언 모양이 아니다")?;
    let inner = rest.split_once('[').map(|(_, r)| r).ok_or("PUSH_SCOPE_ARGS 값 목록이 없다")?;
    let inner = inner.split_once(']').map(|(l, _)| l).ok_or("PUSH_SCOPE_ARGS 값 목록이 닫히지 않았다")?;
    let items: Vec<String> = inner.split(',').map(|x| x.trim().trim_matches('"')).filter(|x| !x.is_empty()).map(String::from).collect();
    if count.trim().parse::<usize>().ok() != Some(items.len()) {
        return Err(format!("PUSH_SCOPE_ARGS 길이 {count} ≠ 값 {}개", items.len()));
    }
    Ok(items)
}

fn scenario(fx: &mut Fx) -> Result<(), String> {
    let root = fx.root.clone();
    let scope = hintlib_scope_args();
    let want: Vec<String> = crate::push_branches::SCOPE_ARGS.iter().map(|s| s.to_string()).collect();
    fx.ck("push 범위 옵션 == hintlib::tag::PUSH_SCOPE_ARGS(교차 대조 · Rust tag.rs 선언 파싱)", scope.as_ref().map(|s| *s == want).unwrap_or(false), &format!("{scope:?}"));
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

    // A: https-declared remote → fail loud per branch (BRANCH_PUSH_HTTPS_REFUSED), both branches reported, remote untouched.
    let refused = format!("\"{}\"", crate::push_branches::HTTPS_REFUSED);
    let all_refused = |out: &str| ["single-node", "multi-node"].iter().all(|b| field_of(out, b, "code").as_deref() == Some(refused.as_str()));
    let (rc, out, _) = fx.run(&repo, "fx", &[], &root);
    fx.ck("★S5 https 선언 원격 → 두 브랜치 모두 BRANCH_PUSH_HTTPS_REFUSED · rc=5 (첫 실패에서 멈추지 않는다)", rc == 5 && all_refused(&out), &format!("{rc} {out}"));
    let untouched = |fx: &Fx| -> Result<bool, String> {
        Ok(fx.g(&remote, &["rev-parse", "refs/heads/multi-node"])? == multi_before && fx.g(&remote, &["rev-parse", "refs/heads/single-node"])? == single_before)
    };
    fx.ck("https fail-loud 에서는 원격이 그대로다", untouched(fx)?, "");

    // #9: the declared push address is pushurl, else url (git's push order) -- the judgement reads what git would push to.
    // D: url token-free + pushurl https (redirected by the same insteadOf onto `remote`) → refused, nothing pushed.
    let spare = fx.bare("url-only.git")?;
    fx.g(&repo, &["remote", "add", "fxp", &spare.to_string_lossy()])?;
    fx.g(&repo, &["config", "remote.fxp.pushurl", https_url])?;
    let (rc, out, _) = fx.run(&repo, "fxp", &[], &root);
    fx.ck("★#9 url 토큰 불요 + pushurl https → 두 브랜치 모두 BRANCH_PUSH_HTTPS_REFUSED · rc=5", rc == 5 && all_refused(&out), &format!("{rc} {out}"));
    let r = fx.refs(&spare, "refs/heads");
    fx.ck("#9 거부 때 push 0: pushurl 이 가리키는 원격과 url 원격이 모두 그대로다", untouched(fx)? && r.is_empty(), &format!("{r:?}"));
    // E: url https + pushurl token-free → git pushes to the pushurl, so this is not refused and the url remote is untouched.
    let first = fx.bare("pushurl-first.git")?;
    fx.g(&repo, &["remote", "add", "fxu", https_url])?;
    fx.g(&repo, &["config", "remote.fxu.pushurl", &first.to_string_lossy()])?;
    let (rc, out, _) = fx.run(&repo, "fxu", &[], &root);
    let both = ["single-node", "multi-node"].iter().all(|b| field_of(&out, b, "returncode").as_deref() == Some("0") && field_of(&out, b, "code").as_deref() == Some("null"));
    let r = fx.refs(&first, "refs/heads");
    fx.ck("★#9 url https + pushurl 토큰 불요 → pushurl 로 두 브랜치 push · rc=0 · url 쪽 원격 그대로", rc == 0 && both && r.len() == 2 && untouched(fx)?, &format!("{rc} {r:?} {out}"));
    // F: several pushurl values, one of them https → refused before any push (no partial push to the token-free one).
    let partial = fx.bare("pushurl-many.git")?;
    fx.g(&repo, &["remote", "add", "fxm", &partial.to_string_lossy()])?;
    fx.g(&repo, &["config", "--add", "remote.fxm.pushurl", &partial.to_string_lossy()])?;
    fx.g(&repo, &["config", "--add", "remote.fxm.pushurl", https_url])?;
    let (rc, out, _) = fx.run(&repo, "fxm", &[], &root);
    let r = fx.refs(&partial, "refs/heads");
    fx.ck("★#9 pushurl 여럿 중 하나라도 https → BRANCH_PUSH_HTTPS_REFUSED · 토큰 불요 쪽에도 push 0", rc == 5 && all_refused(&out) && r.is_empty() && untouched(fx)?, &format!("{rc} {r:?} {out}"));

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

    // Mutation control (replaces the Python source-copy mutation): the same push with the scope options emptied MUST leak the rollback tag
    // under push.followTags=true — proving the "no tag carried" checks above are not vacuous.
    let mutated = fx.bare("mutated.git")?;
    let (mrc, _) = crate::push_branches::push_local(&repo.to_string_lossy(), &mutated.to_string_lossy(), "refs/heads/multi-node:refs/heads/multi-node", false, &[])?;
    let leaked = fx.refs(&mutated, "refs/tags");
    fx.ck("★변이 음성대조: 범위 옵션을 비우면 followTags 로 롤백 태그가 새어 나간다(결함 재현)", mrc == 0 && leaked == ["refs/tags/last-good-fixture"], &format!("{mrc} {leaked:?}"));
    eprintln!("  N/A  자격증명 경로 변이 대조: 이 도구는 PAT 경로(hint-publisher tag.rs git_push_authenticated)를 쓰지 않는다 — 정책(브랜치 push = ssh 키만) · https push 주소는 위 ★S5·★#9 가 BRANCH_PUSH_HTTPS_REFUSED 로 가른다");
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
