//! `regen_build_patches_src` — the `unbundle`, `derive` and `snapshot-resolutions` subcommands.
//!
//! `unbundle` extracts a `bundle` tar.gz (gzip member + ustar/PAX, the shape `bundle` writes) into a staging directory, verifies it against
//! `PROVENANCE.json` and replaces the payload as a whole. `derive` re-derives the payload from the upstream coordinates (clone/fetch, a detached
//! worktree at the base commit, the PR-scope delta applied with `git apply --3way`, fork-verbatim blobs, the residual patch) and verifies the
//! result; `snapshot-resolutions` classifies every file the other way round and re-issues the resolutions spec and patch. Git and `diff` are
//! external programs, called as the original did. The spec JSON is written the way Python's `json.dump(ensure_ascii=False, indent=1)` does.

use crate::regen_patches_src::{as_dict, as_text, dict_get, fail, log, sha256_bytes, verify, walk, Fail, Opts, Provenance};
use agent_runtime_support::pyjson::{decode_file_text, loads_text};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

const EXEC_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

struct Ran {
    code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Ran {
    fn out_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    fn err_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

fn io_fail<T>(what: &str, err: std::io::Error) -> Result<T, Fail> {
    fail(format!("{what}: {err}"))
}

/// `run(cmd, cwd, check, capture)`: a non-zero status with `check` is the original's `명령 실패` failure.
fn run(cmd: &[&str], cwd: Option<&str>, check: bool, capture: bool) -> Result<Ran, Fail> {
    let mut command = Command::new(cmd[0]);
    command.args(&cmd[1..]);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let ran = if capture {
        let out = command.output().map_err(|e| Fail(format!("명령 실행 불가: {}: {e}", cmd.join(" "))))?;
        Ran { code: out.status.code().unwrap_or(-1), stdout: out.stdout, stderr: out.stderr }
    } else {
        let status = command.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status().map_err(|e| Fail(format!("명령 실행 불가: {}: {e}", cmd.join(" "))))?;
        Ran { code: status.code().unwrap_or(-1), stdout: Vec::new(), stderr: Vec::new() }
    };
    if check && ran.code != 0 {
        let err = ran.err_text();
        let detail = if err.is_empty() { ran.out_text() } else { err };
        return fail(format!("명령 실패(rc={}): {}\n    {}", ran.code, cmd.join(" "), detail.trim()));
    }
    Ok(ran)
}

fn sha256_file(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|d| sha256_bytes(&d))
}

fn chmod(path: &Path, mode: u32) -> Result<(), Fail> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| Fail(format!("{}: {e}", path.display())))
}

fn text_field(d: &PyDict, key: &str) -> Result<String, Fail> {
    match dict_get(d, key) {
        Some(v) => Ok(as_text(v).unwrap_or_else(|| v.display_text().to_utf8().unwrap_or_default())),
        None => fail(format!("PROVENANCE.source.{key} 부재")),
    }
}

struct Source {
    upstream_repo: String,
    upstream_base_sha: String,
    pr_base_sha: String,
    fork_head_sha: String,
    fork_pr: Option<String>,
}

fn source_of(man: &PyDict) -> Result<Source, Fail> {
    let Some(src) = dict_get(man, "source").and_then(as_dict) else { return fail("PROVENANCE.source 부재") };
    Ok(Source {
        upstream_repo: text_field(src, "upstream_repo")?,
        upstream_base_sha: text_field(src, "upstream_base_sha")?,
        pr_base_sha: text_field(src, "pr_base_sha")?,
        fork_head_sha: text_field(src, "fork_head_sha")?,
        fork_pr: dict_get(src, "fork_pr").filter(|v| !matches!(v, PyValue::None)).map(|v| v.display_text().to_utf8().unwrap_or_default()),
    })
}

// ── bundle reader: gzip member, ustar header, PAX `path` records ──

fn gunzip(data: &[u8]) -> Result<Vec<u8>, Fail> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return fail("번들이 gzip 이 아니다");
    }
    let flags = data[3];
    let mut at = 10usize;
    if flags & 4 != 0 {
        let n = data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(|| Fail("gzip 헤더 손상".into()))?;
        at += 2 + n;
    }
    for bit in [8u8, 16u8] {
        if flags & bit != 0 {
            while *data.get(at).ok_or_else(|| Fail("gzip 헤더 손상".into()))? != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    if at + 8 > data.len() {
        return fail("gzip 헤더 손상");
    }
    let body = &data[at..data.len() - 8];
    let out = miniz_oxide::inflate::decompress_to_vec(body).map_err(|e| Fail(format!("gzip 해제 실패: {e:?}")))?;
    let tail = &data[data.len() - 8..];
    let crc = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]);
    let size = u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]);
    if crc != crc32fast::hash(&out) || size != out.len() as u32 {
        return fail("gzip 무결성(CRC/크기) 불일치");
    }
    Ok(out)
}

fn field_text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn field_octal(bytes: &[u8]) -> Result<u64, Fail> {
    let text = field_text(bytes);
    let text = text.trim();
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8).map_err(|_| Fail(format!("tar 헤더 숫자 손상: {text:?}")))
}

fn pax_path(body: &[u8]) -> Option<String> {
    let mut rest = body;
    let mut found = None;
    while !rest.is_empty() {
        let space = rest.iter().position(|b| *b == b' ')?;
        let len: usize = std::str::from_utf8(&rest[..space]).ok()?.parse().ok()?;
        let record = rest.get(..len)?;
        let content = &record[space + 1..record.len().saturating_sub(1)];
        if let Some(eq) = content.iter().position(|b| *b == b'=') {
            if &content[..eq] == b"path" {
                found = Some(String::from_utf8_lossy(&content[eq + 1..]).into_owned());
            }
        }
        rest = &rest[len..];
    }
    found
}

struct Member {
    name: String,
    mode: u64,
    data: Vec<u8>,
}

fn read_members(tar: &[u8]) -> Result<Vec<Member>, Fail> {
    let mut members = Vec::new();
    let mut at = 0usize;
    let mut pending_path: Option<String> = None;
    while at + 512 <= tar.len() {
        let header = &tar[at..at + 512];
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let size = field_octal(&header[124..136])? as usize;
        let mode = field_octal(&header[100..108])?;
        let flag = header[156];
        let mut name = field_text(&header[0..100]);
        let prefix = field_text(&header[345..500]);
        if !prefix.is_empty() {
            name = format!("{prefix}/{name}");
        }
        let start = at + 512;
        let end = start + size;
        if end > tar.len() {
            return fail("tar 멤버가 잘렸다");
        }
        let body = &tar[start..end];
        at = start + size.div_ceil(512) * 512;
        match flag {
            b'x' => pending_path = pax_path(body).or(pending_path),
            b'g' => {}
            b'0' | 0 | b'7' => {
                let name = pending_path.take().unwrap_or(name);
                members.push(Member { name, mode, data: body.to_vec() });
            }
            other => {
                let name = pending_path.take().unwrap_or(name);
                return fail(format!("번들에 비정규 멤버: {name} (type={:?})", [other]));
            }
        }
    }
    Ok(members)
}

/// `extract_bundle`: `dest_root` is rebuilt; every member path is checked fail-closed.
fn extract_bundle(bundle: &Path, dest_root: &Path) -> Result<usize, Fail> {
    if dest_root.exists() {
        std::fs::remove_dir_all(dest_root).or_else(|e| io_fail(&dest_root.display().to_string(), e))?;
    }
    std::fs::create_dir_all(dest_root).or_else(|e| io_fail(&dest_root.display().to_string(), e))?;
    let raw = std::fs::read(bundle).or_else(|e| io_fail(&bundle.display().to_string(), e))?;
    let members = read_members(&gunzip(&raw)?)?;
    let mut count = 0;
    for member in members {
        let name = &member.name;
        let Some(rel) = name.strip_prefix("files/") else { return fail(format!("번들 멤버가 files/ 밖이다: {name}")) };
        if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|p| p.is_empty() || p == "..") {
            return fail(format!("번들 멤버 경로 거부: {name}"));
        }
        let target = dest_root.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).or_else(|e| io_fail(&parent.display().to_string(), e))?;
        }
        std::fs::write(&target, &member.data).or_else(|e| io_fail(&target.display().to_string(), e))?;
        chmod(&target, if member.mode & 0o100 != 0 { EXEC_MODE } else { FILE_MODE })?;
        count += 1;
    }
    Ok(count)
}

fn file_set(root: &Path) -> BTreeSet<String> {
    let mut found = Vec::new();
    if root.is_dir() {
        walk(root, root, &mut found);
    }
    found.into_iter().collect()
}

fn declared_files(prov: &Provenance) -> Vec<(String, String)> {
    prov.files()
}

fn require_files_map(prov: &Provenance, operation: &str) -> Result<(), Fail> {
    if prov.script_modules {
        return fail(format!("{operation} is not applicable to script-modules: authored modules are verified and delivered directly; no files/ derivation or bundle"));
    }
    Ok(())
}

fn cmd_unbundle(o: &Opts, prov: &Provenance, path: &str) -> Result<i32, Fail> {
    require_files_map(prov, "unbundle")?;
    let files_root = o.files_root.clone().unwrap_or_else(|| format!("{}/files", o.root));
    let files = declared_files(prov);
    let bundle = o.bundle.clone().unwrap_or_default();
    if !Path::new(&bundle).is_file() {
        return fail(format!("번들 부재: {bundle}"));
    }
    let raw = std::fs::read(&bundle).or_else(|e| io_fail(&bundle, e))?;
    let digest = sha256_bytes(&raw);
    if let Some(expect) = &o.expect_sha256 {
        if !expect.is_empty() && *expect != digest {
            return fail(format!("번들 전송 무결성 실패: expect={expect} actual={digest}"));
        }
    }
    let abs_files_root = std::path::absolute(&files_root).or_else(|e| io_fail(&files_root, e))?;
    let staging = abs_files_root.parent().unwrap_or(Path::new("/")).join(".files.incoming");
    let count = extract_bundle(Path::new(&bundle), &staging)?;
    if verify(&staging.to_string_lossy(), &files, true) != 0 {
        let _ = std::fs::remove_dir_all(&staging);
        return fail("해체분이 PROVENANCE 와 불일치 — 기존 payload 를 건드리지 않고 중단한다");
    }
    // Replacing the payload as a whole removes the previous port's leftovers: say how many, never delete silently.
    let previous = file_set(Path::new(&files_root));
    let declared: BTreeSet<String> = files.iter().map(|(r, _)| r.clone()).collect();
    let evicted: Vec<&String> = previous.difference(&declared).collect();
    let added: Vec<&String> = declared.difference(&previous).collect();
    if Path::new(&files_root).exists() {
        std::fs::remove_dir_all(&files_root).or_else(|e| io_fail(&files_root, e))?;
    }
    std::fs::rename(&staging, &files_root).or_else(|e| io_fail(&files_root, e))?;
    if !previous.is_empty() {
        log(&format!(
            "payload 교체 — 이전 {} 파일 → 선언 {} 파일 (제거 {} · 신규 {} · 유지 {})",
            previous.len(),
            declared.len(),
            evicted.len(),
            added.len(),
            previous.intersection(&declared).count()
        ));
        for rel in evicted.iter().take(20) {
            log(&format!("  evicted {rel}"));
        }
        if evicted.len() > 20 {
            log(&format!("  evicted … 외 {}건", evicted.len() - 20));
        }
    }
    log(&format!("해체·검증 OK — {count}파일 → {files_root} (bundle sha256={digest})"));
    log(&format!("authority={path}"));
    Ok(verify(&files_root, &files, false))
}

// ── re-derivation ──

fn resolutions_paths(man: &PyDict, repo: &str, must_exist: bool) -> Result<(String, String), Fail> {
    let Some(res) = dict_get(man, "resolutions").filter(|v| !matches!(v, PyValue::None)).and_then(as_dict) else {
        return fail("PROVENANCE.json 에 resolutions 포인터가 없다 — 잔차 자산 없이는 재파생이 결정론이 아니다");
    };
    let spec = format!("{repo}/{}", text_field(res, "spec")?);
    let patch = format!("{repo}/{}", text_field(res, "patch")?);
    if must_exist && !Path::new(&spec).is_file() {
        return fail(format!("resolutions 스펙 부재: {spec} — `snapshot-resolutions` 로 먼저 발행하라"));
    }
    Ok((spec, patch))
}

fn commit_present(repo_dir: &str, sha: &str) -> Result<bool, Fail> {
    Ok(run(&["git", "cat-file", "-e", &format!("{sha}^{{commit}}")], Some(repo_dir), false, true)?.code == 0)
}

fn ensure_source_clone(work: &str, src: &Source) -> Result<String, Fail> {
    let repo_dir = format!("{work}/vllm-src");
    let shas = [&src.upstream_base_sha, &src.pr_base_sha, &src.fork_head_sha];
    if !Path::new(&format!("{repo_dir}/.git")).is_dir() {
        std::fs::create_dir_all(work).or_else(|e| io_fail(work, e))?;
        log(&format!("clone {} → {repo_dir} (수백 MB · 최초 1회)", src.upstream_repo));
        run(&["git", "clone", "--quiet", &src.upstream_repo, &repo_dir], None, true, false)?;
    }
    let mut have = true;
    for s in shas {
        if !commit_present(&repo_dir, s)? {
            have = false;
            break;
        }
    }
    if !have {
        let pr = src.fork_pr.clone().unwrap_or_else(|| "None".to_string());
        log(&format!("fetch refs/pull/{pr}/head + 태그(좌표 SHA 확보)"));
        run(&["git", "fetch", "--quiet", "origin", &format!("+refs/pull/{pr}/head:refs/regen/pr{pr}")], Some(&repo_dir), true, true)?;
        run(&["git", "fetch", "--quiet", "--tags", "origin"], Some(&repo_dir), true, true)?;
    }
    for s in shas {
        if !commit_present(&repo_dir, s)? {
            return fail(format!("좌표 SHA 를 상류에서 찾을 수 없다: {s} — PROVENANCE.source 를 확인하라"));
        }
    }
    Ok(repo_dir)
}

fn measurement(man: &PyDict, path: &[&str]) -> Option<PyValue> {
    let mut cur = dict_get(man, path[0])?.clone();
    for key in &path[1..] {
        cur = dict_get(as_dict(&cur)?, key)?.clone();
    }
    if matches!(cur, PyValue::None) {
        None
    } else {
        Some(cur)
    }
}

fn mechanical_tree(work: &str, repo_dir: &str, man: &PyDict, files: &[(String, String)], src: &Source, tree_name: &str) -> Result<(String, Vec<String>), Fail> {
    let tree = format!("{work}/{tree_name}");
    if Path::new(&tree).exists() {
        run(&["git", "worktree", "remove", "--force", &tree], Some(repo_dir), false, true)?;
        let _ = std::fs::remove_dir_all(&tree);
    }
    run(&["git", "worktree", "prune"], Some(repo_dir), false, true)?;
    run(&["git", "worktree", "add", "--detach", "--quiet", &tree, &src.upstream_base_sha], Some(repo_dir), true, true)?;

    let additions: BTreeSet<String> = dict_get(man, "scope_additions_beyond_pr_delta")
        .and_then(as_dict)
        .map(|d| d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect())
        .unwrap_or_default();
    let scope: Vec<String> = files.iter().map(|(r, _)| r.clone()).filter(|r| !additions.contains(r)).collect();
    if let Some(declared) = measurement(man, &["measurements", "port_scope", "files"]) {
        if !declared.equals_int(&scope.len().into()) {
            return fail(format!(
                "스코프 자기모순: measurements.port_scope.files={} 인데 유도값은 {}",
                declared.display_text().to_utf8().unwrap_or_default(),
                scope.len()
            ));
        }
    }
    log(&format!("PR 스코프 {}파일 = files({}) − 폐포/드리프트추출({})", scope.len(), files.len(), additions.len()));

    let patch_path = format!("{work}/pr_scope.regen.patch");
    let range = format!("{}..{}", src.pr_base_sha, src.fork_head_sha);
    let mut diff: Vec<&str> = vec!["git", "diff", &range, "--"];
    diff.extend(scope.iter().map(String::as_str));
    let proc = run(&diff, Some(repo_dir), true, true)?;
    std::fs::write(&patch_path, &proc.stdout).or_else(|e| io_fail(&patch_path, e))?;

    let apply = run(&["git", "apply", "--3way", &patch_path], Some(&tree), false, true)?;
    let status = run(&["git", "status", "--porcelain"], Some(&tree), true, true)?;
    let mut conflicts: Vec<String> = status
        .out_text()
        .lines()
        .filter(|l| l.starts_with("UU ") || l.starts_with("AA "))
        .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
        .collect();
    conflicts.sort();
    if apply.code != 0 && conflicts.is_empty() {
        return fail(format!("3-way 적용이 충돌이 아닌 이유로 실패했다:\n    {}", apply.err_text().trim()));
    }
    match measurement(man, &["measurements", "conflict_files"]) {
        Some(declared) if !declared.equals_int(&conflicts.len().into()) => log(&format!(
            "⚠ 충돌 파일 수 불일치: PROVENANCE={} 실측={} (잔차 패치 적용이 실패할 수 있다)",
            declared.display_text().to_utf8().unwrap_or_default(),
            conflicts.len()
        )),
        _ => log(&format!("3-way 적용 완료 — 충돌 {}파일(예상대로)", conflicts.len())),
    }
    Ok((tree, conflicts))
}

fn git_version() -> Result<String, Fail> {
    Ok(run(&["git", "--version"], None, false, true)?.out_text().trim().to_string())
}

/// `spec["files"]` as `(path, strategy)` sorted by path.
fn spec_strategies(spec: &PyValue) -> Result<Vec<(String, String)>, Fail> {
    let Some(files) = as_dict(spec).and_then(|d| dict_get(d, "files")).and_then(as_dict) else { return fail("resolutions 스펙에 files 가 없다") };
    let mut out = Vec::new();
    for (rel, entry) in files.iter() {
        let strategy = as_dict(entry).and_then(|e| dict_get(e, "strategy")).and_then(as_text).unwrap_or_default();
        out.push((rel.to_utf8().unwrap_or_default(), strategy));
    }
    out.sort();
    Ok(out)
}

fn place_fork_blob(tree: &str, repo_dir: &str, fork: &str, rel: &str) -> Result<(), Fail> {
    let blob = run(&["git", "show", &format!("{fork}:{rel}")], Some(repo_dir), true, true)?;
    let target = Path::new(tree).join(rel);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).or_else(|e| io_fail(&parent.display().to_string(), e))?;
    }
    std::fs::write(&target, &blob.stdout).or_else(|e| io_fail(&target.display().to_string(), e))?;
    Ok(())
}

fn apply_fork_verbatim(tree: &str, repo_dir: &str, fork: &str, spec: &PyValue) -> Result<usize, Fail> {
    let rels: Vec<String> = spec_strategies(spec)?.into_iter().filter(|(_, s)| s == "fork-verbatim").map(|(r, _)| r).collect();
    for rel in &rels {
        place_fork_blob(tree, repo_dir, fork, rel)?;
        let listing = run(&["git", "ls-tree", fork, "--", rel], Some(repo_dir), true, true)?.out_text();
        let mode = listing.split_whitespace().next().unwrap_or("");
        chmod(&Path::new(tree).join(rel), if mode == "100755" { EXEC_MODE } else { FILE_MODE })?;
    }
    log(&format!("fork-verbatim {}파일 배치(바이트 아닌 **결정**을 기록한다)", rels.len()));
    Ok(rels.len())
}

fn apply_residual(tree: &str, patch_path: &str, spec: &PyValue) -> Result<(), Fail> {
    let rels: Vec<String> = spec_strategies(spec)?.into_iter().filter(|(_, s)| s == "residual-patch").map(|(r, _)| r).collect();
    if rels.is_empty() {
        return Ok(());
    }
    if !Path::new(patch_path).is_file() {
        return fail(format!("잔차 패치 부재: {patch_path} (스펙은 {}파일을 요구한다)", rels.len()));
    }
    let proc = run(&["git", "apply", "-p1", patch_path], Some(tree), false, true)?;
    if proc.code != 0 {
        let recorded = as_dict(spec).and_then(|d| dict_get(d, "derived_with_git")).and_then(as_text).unwrap_or_else(|| "unknown".to_string());
        return fail(format!(
            "잔차 패치 적용 실패 — 대개 `git apply --3way` 의 충돌 마커가 달라진 경우다.\n    스펙 생성 git: {recorded} · 현재 git: {}\n    {}\n    → 같은 git 으로 재시도하거나 `snapshot-resolutions` 로 잔차를 재발행하라.",
            git_version()?,
            proc.err_text().trim()
        ));
    }
    log(&format!("잔차 패치 적용 {}파일(사람 판단이 든 유일한 부분)", rels.len()));
    Ok(())
}

fn materialize(tree: &str, files_root: &str, files: &[(String, String)]) -> Result<(), Fail> {
    if Path::new(files_root).exists() {
        std::fs::remove_dir_all(files_root).or_else(|e| io_fail(files_root, e))?;
    }
    for (rel, _) in files {
        let src = Path::new(tree).join(rel);
        if !src.is_file() {
            return fail(format!("재파생 트리에 파일이 없다: {rel}"));
        }
        let dst = Path::new(files_root).join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).or_else(|e| io_fail(&parent.display().to_string(), e))?;
        }
        std::fs::copy(&src, &dst).or_else(|e| io_fail(&dst.display().to_string(), e))?;
        let exec = std::fs::metadata(&src).map(|m| m.permissions().mode() & 0o100 != 0).unwrap_or(false);
        chmod(&dst, if exec { EXEC_MODE } else { FILE_MODE })?;
    }
    Ok(())
}

fn load_spec(spec_path: &str) -> Result<PyValue, Fail> {
    let bytes = std::fs::read(spec_path).or_else(|e| io_fail(spec_path, e))?;
    let text = decode_file_text(&bytes).map_err(|e| Fail(format!("{spec_path}: {e:?}")))?;
    loads_text(&text).map_err(|e| Fail(format!("{spec_path}: {e}")))
}

fn cmd_derive(o: &Opts, prov: &Provenance, path: &str) -> Result<i32, Fail> {
    require_files_map(prov, "derive")?;
    let man = prov.dict();
    let files_root = o.files_root.clone().unwrap_or_else(|| format!("{}/files", o.root));
    let files = declared_files(prov);
    let repo = repo_root(o);
    let (spec_path, patch_path) = resolutions_paths(man, &repo, true)?;
    let spec = load_spec(&spec_path)?;
    let work = abs_text(o.work.as_deref().unwrap_or_default())?;
    let src = source_of(man)?;
    let repo_dir = ensure_source_clone(&work, &src)?;
    let (tree, _conflicts) = mechanical_tree(&work, &repo_dir, man, &files, &src, "derive-tree")?;
    apply_fork_verbatim(&tree, &repo_dir, &src.fork_head_sha, &spec)?;
    apply_residual(&tree, &patch_path, &spec)?;
    materialize(&tree, &files_root, &files)?;
    let rc = verify(&files_root, &files, false);
    if rc == 0 {
        log(&format!("재파생 완료 — {}파일이 좌표에서 결정론적으로 복원됐다", files.len()));
        log(&format!("authority={path}"));
    } else {
        return fail("재파생 결과가 PROVENANCE 와 불일치 — 좌표·잔차·git 버전 중 하나가 어긋났다");
    }
    if !o.keep {
        run(&["git", "worktree", "remove", "--force", &tree], Some(&repo_dir), false, true)?;
    }
    Ok(rc)
}

fn abs_text(path: &str) -> Result<String, Fail> {
    Ok(std::path::absolute(path).or_else(|e| io_fail(path, e))?.to_string_lossy().into_owned())
}

/// `repo_root_default()`: `--repo`, else four levels above the script the launcher names in argv[0] (when that is a real file), else the cwd.
fn repo_root(o: &Opts) -> String {
    if let Some(repo) = &o.repo {
        return repo.clone();
    }
    if let Some(argv0) = std::env::args().next() {
        let script = Path::new(&argv0);
        if script.is_file() {
            if let Some(here) = script.parent().and_then(|p| std::path::absolute(p).ok()) {
                let cand = here.join("../../../..");
                if cand.join(".claude").is_dir() {
                    if let Ok(real) = std::fs::canonicalize(&cand) {
                        return real.to_string_lossy().into_owned();
                    }
                }
            }
        }
    }
    std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| ".".to_string())
}

// ── Python `json.dump(obj, ensure_ascii=False, indent=1)` ──

enum Json {
    Int(u64),
    Str(String),
    List(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn dump_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn dump(value: &Json, level: usize, out: &mut String) {
    let pad = " ".repeat(level + 1);
    match value {
        Json::Int(n) => out.push_str(&n.to_string()),
        Json::Str(s) => dump_str(s, out),
        Json::List(items) if items.is_empty() => out.push_str("[]"),
        Json::List(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad);
                dump(item, level + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&" ".repeat(level));
            out.push(']');
        }
        Json::Obj(items) if items.is_empty() => out.push_str("{}"),
        Json::Obj(items) => {
            out.push_str("{\n");
            for (i, (key, item)) in items.iter().enumerate() {
                out.push_str(&pad);
                dump_str(key, out);
                out.push_str(": ");
                dump(item, level + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&" ".repeat(level));
            out.push('}');
        }
    }
}

fn s(text: &str) -> Json {
    Json::Str(text.to_string())
}

fn cmd_snapshot_resolutions(o: &Opts, prov: &Provenance) -> Result<i32, Fail> {
    require_files_map(prov, "snapshot-resolutions")?;
    let man = prov.dict();
    let files_root = o.files_root.clone().unwrap_or_else(|| format!("{}/files", o.root));
    let files = declared_files(prov);
    if verify(&files_root, &files, true) != 0 {
        return fail("현재 payload 가 PROVENANCE 와 불일치 — 잔차를 뜰 기준이 없다");
    }
    let repo = repo_root(o);
    let (spec_path, patch_path) = resolutions_paths(man, &repo, false)?;
    let work = abs_text(o.work.as_deref().unwrap_or_default())?;
    let src = source_of(man)?;
    let repo_dir = ensure_source_clone(&work, &src)?;
    let (tree, conflicts) = mechanical_tree(&work, &repo_dir, man, &files, &src, "snapshot-tree")?;
    let fork = src.fork_head_sha.clone();

    let mut classes: Vec<(String, Json)> = Vec::new();
    let (mut fork_rels, mut residual_rels) = (Vec::<String>::new(), Vec::<String>::new());
    for (rel, want) in &files {
        let cur = Path::new(&tree).join(rel);
        let cur_sha = if cur.is_file() { sha256_file(&cur) } else { None };
        if cur_sha.as_deref() == Some(want.as_str()) {
            classes.push((rel.clone(), Json::Obj(vec![("strategy".into(), s("mechanical"))])));
            continue;
        }
        let blob = run(&["git", "show", &format!("{fork}:{rel}")], Some(&repo_dir), false, true)?;
        if blob.code == 0 && sha256_bytes(&blob.stdout) == *want {
            classes.push((
                rel.clone(),
                Json::Obj(vec![("strategy".into(), s("fork-verbatim")), ("reason".into(), s("충돌 판정 = PR본 통째 채택(드리프트가 PR 재작성 대비 미미)"))]),
            ));
            fork_rels.push(rel.clone());
            continue;
        }
        classes.push((
            rel.clone(),
            Json::Obj(vec![("strategy".into(), s("residual-patch")), ("reason".into(), s("헝크 판정·드리프트 되돌림·폐포 최소추출 등 사람 판단"))]),
        ));
        residual_rels.push(rel.clone());
    }
    let mechanical = files.len() - fork_rels.len() - residual_rels.len();
    let variant = dict_get(man, "resolutions")
        .and_then(as_dict)
        .and_then(|r| dict_get(r, "variant_id"))
        .and_then(as_text)
        .unwrap_or_else(|| "unknown".to_string());
    let spec = Json::Obj(vec![
        ("schema_version".into(), Json::Int(1)),
        ("variant_id".into(), Json::Str(variant)),
        (
            "_note".into(),
            s("① mechanical=유도식 그대로 · ② fork-verbatim=결정만 기록(바이트 ✗) · ③ residual-patch=사람 판단이 든 유일한 손작성 잔차. 최종 게이트는 PROVENANCE sha256."),
        ),
        ("derived_with_git".into(), Json::Str(git_version()?)),
        ("mechanical_conflict_files".into(), Json::List(conflicts.iter().map(|c| Json::Str(c.clone())).collect())),
        (
            "counts".into(),
            Json::Obj(vec![
                ("mechanical".into(), Json::Int(mechanical as u64)),
                ("fork_verbatim".into(), Json::Int(fork_rels.len() as u64)),
                ("residual_patch".into(), Json::Int(residual_rels.len() as u64)),
            ]),
        ),
        ("files".into(), Json::Obj(classes)),
    ]);

    // fork-verbatim goes first so the residual carries only the difference on top of it.
    for rel in &fork_rels {
        place_fork_blob(&tree, &repo_dir, &fork, rel)?;
    }
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    for rel in &residual_rels {
        let cur = Path::new(&tree).join(rel);
        if !cur.is_file() {
            return fail(format!("잔차 기준 파일이 재파생 트리에 없다(폐포 누락?): {rel}"));
        }
        let final_path = Path::new(&files_root).join(rel);
        let proc = run(
            &["diff", "-u", "--label", &format!("a/{rel}"), "--label", &format!("b/{rel}"), &cur.to_string_lossy(), &final_path.to_string_lossy()],
            None,
            false,
            true,
        )?;
        if proc.code != 0 && proc.code != 1 {
            return fail(format!("diff 실패: {rel}"));
        }
        chunks.push(proc.stdout);
    }

    if let Some(parent) = Path::new(&spec_path).parent() {
        std::fs::create_dir_all(parent).or_else(|e| io_fail(&parent.display().to_string(), e))?;
    }
    let mut text = String::new();
    dump(&spec, 0, &mut text);
    text.push('\n');
    std::fs::write(&spec_path, text).or_else(|e| io_fail(&spec_path, e))?;
    std::fs::write(&patch_path, chunks.concat()).or_else(|e| io_fail(&patch_path, e))?;
    let lines: usize = chunks.iter().map(|c| c.iter().filter(|b| **b == b'\n').count()).sum();
    log(&format!("잔차 자산 발행 — mechanical={mechanical} fork-verbatim={} residual={}", fork_rels.len(), residual_rels.len()));
    log(&format!("  {spec_path}"));
    log(&format!("  {patch_path} ({lines}줄)"));
    if !o.keep {
        run(&["git", "worktree", "remove", "--force", &tree], Some(&repo_dir), false, true)?;
    }
    Ok(0)
}

pub(crate) fn run_cmd(cmd: &str, o: &Opts, prov: &Provenance, path: &str) -> Result<i32, Fail> {
    match cmd {
        "unbundle" => cmd_unbundle(o, prov, path),
        "derive" => cmd_derive(o, prov, path),
        _ => cmd_snapshot_resolutions(o, prov),
    }
}
