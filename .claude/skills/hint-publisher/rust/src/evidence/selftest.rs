//! `evidence.selftest()` — port of `scripts/hintlib/evidence.py` 7134–9997 (`_selftest_*` · `_git_fixture` ·
//! `selftest_stage_manifest_contract` · `selftest`). Fixture helpers (`_fixture` · `_fake_docker_run` · `_tree` · `_code` ·
//! `_OWNER_LINKS`) are e4's. ★ = 음성대조.
//!
//! Translations (no Rust meaning for the Python form → the closest Rust fact):
//! - `builtins.open` trap on `campaigns/ACTIVE` → the decoy stays mode 0 for a non-root process (a read would fail
//!   closed); the final check records the proof as vacuous when running as root.
//! - `docker=` callable → the typed `DockerRunner` hooks (`from_campaign_r` · `_local_image_digest_r` …) over one shared
//!   fake-docker state (Python shares the `fx["docker"]` dict the same way).
//! - owner modules the Python imports/runs in-process → their native launchers; the scratch checkouts get the node-local
//!   build outputs linked (`_link_native`) the way the fixture links the owner sources.
//! - emitted-missing-code tripwire scans this crate's Rust sources instead of `hintlib/*.py`.
#![allow(non_snake_case)]
#![allow(clippy::too_many_lines)]
use super::*;
use crate::core::{self, Proc, HR};
use crate::value::{dumps_default, dumps_unicode, Map, J};
use crate::{jd, jl};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

struct Ck(Vec<String>);
impl Ck {
    fn ck(&mut self, name: &str, cond: bool) {
        if !cond {
            self.0.push(format!("evidence: {name}"));
        }
    }
    fn exc(&mut self, e: core::HintError) {
        self.0.push(format!("evidence: 자체검사 중 예외 {}: {}", e.code, core::head_chars(&e.message, 600)));
    }
}

// ── small helpers ───────────────────────────────────────────────────────────────────────────────
fn py(e: impl std::fmt::Display) -> core::HintError {
    core::HintError::py("OSError", e.to_string())
}
fn has(j: &J, sub: &str) -> bool {
    j.as_str().map_or(false, |x| x.contains(sub))
}
fn eqs(j: &J, v: &str) -> bool {
    j.as_str() == Some(v)
}
fn sw_(j: &J, p: &str) -> bool {
    j.as_str().map_or(false, |x| x.starts_with(p))
}
fn ew_(j: &J, p: &str) -> bool {
    j.as_str().map_or(false, |x| x.ends_with(p))
}
fn st(j: &J) -> String {
    j.as_str().unwrap_or("").to_string()
}
fn ud(j: &J) -> String {
    dumps_unicode(j)
}
fn ad(j: &J) -> String {
    dumps_default(j)
}
fn pj(p: &Path) -> J {
    J::from(core::posix(p))
}
/// Python `==` on JSON-shaped values: dict equality ignores key order · int == float by value.
fn peq(a: &J, b: &J) -> bool {
    match (a, b) {
        (J::Dict(x), J::Dict(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map_or(false, |w| peq(v, w))),
        (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(v, w)| peq(v, w)),
        (J::Int(_) | J::Float(_), J::Int(_) | J::Float(_)) => a.as_f64() == b.as_f64(),
        _ => a == b,
    }
}
/// `dataclasses.replace(ev, k=v, ...)` / `{**d, k: v}`.
fn rep(ev: &J, kv: &[(&str, J)]) -> J {
    let mut e = if ev.is_dict() { ev.clone() } else { J::dict() };
    for (k, v) in kv {
        e.set(*k, v.clone());
    }
    e
}
fn without(d: &J, k: &str) -> J {
    let mut e = if d.is_dict() { d.clone() } else { J::dict() };
    if let Some(m) = e.as_dict_mut() {
        m.remove(k);
    }
    e
}
/// `x in container` (list of str / dict keys / str).
fn inn(c: &J, x: &str) -> bool {
    match c {
        J::List(l) => l.iter().any(|i| i.as_str() == Some(x)),
        J::Dict(m) => m.contains_key(x),
        J::Str(s) => s.contains(x),
        _ => false,
    }
}
fn keys(j: &J) -> Vec<String> {
    j.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default()
}
/// nested mutable access by keys (numeric = list index); the fixture shape is known (panics = KeyError).
fn pm<'a>(j: &'a mut J, path: &[&str]) -> &'a mut J {
    let mut cur = j;
    for k in path {
        cur = match k.parse::<usize>() {
            Ok(i) if cur.is_list() => &mut cur.as_list_mut().expect("list")[i],
            _ => cur.get_mut(k).unwrap_or_else(|| panic!("selftest fixture key {k}")),
        };
    }
    cur
}
fn mkd(p: &Path) -> HR<()> {
    std::fs::create_dir_all(p).map_err(|e| core::io_err(&e, p))
}
fn wr(p: &Path, t: &str) -> HR<()> {
    if let Some(d) = p.parent() {
        mkd(d)?;
    }
    std::fs::write(p, t).map_err(|e| core::io_err(&e, p))
}
fn wrb(p: &Path, b: &[u8]) -> HR<()> {
    std::fs::write(p, b).map_err(|e| core::io_err(&e, p))
}
fn wr0(p: &Path, t: &str) -> HR<()> {
    std::fs::write(p, t).map_err(|e| core::io_err(&e, p))
}
fn app(p: &Path, t: &str) -> HR<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(p).map_err(|e| core::io_err(&e, p))?;
    f.write_all(t.as_bytes()).map_err(|e| core::io_err(&e, p))
}
fn rd(p: &Path) -> HR<String> {
    std::fs::read_to_string(p).map_err(|e| core::io_err(&e, p))
}
fn rdb(p: &Path) -> HR<Vec<u8>> {
    std::fs::read(p).map_err(|e| core::io_err(&e, p))
}
fn rm(p: &Path) -> HR<()> {
    std::fs::remove_file(p).map_err(|e| core::io_err(&e, p))
}
fn rmdir(p: &Path) -> HR<()> {
    std::fs::remove_dir(p).map_err(|e| core::io_err(&e, p))
}
fn rmtree(p: &Path) {
    let _ = std::fs::remove_dir_all(p);
}
fn mv(a: &Path, b: &Path) -> HR<()> {
    std::fs::rename(a, b).map_err(|e| core::io_err(&e, a))
}
fn symlink(src: &Path, dst: &Path) -> HR<()> {
    std::os::unix::fs::symlink(src, dst).map_err(|e| core::io_err(&e, dst))
}
fn wj(p: &Path, j: &J) -> HR<()> {
    core::write_json(p, j)
}
fn lj(t: &str) -> HR<J> {
    crate::value::loads(t).map_err(|e| core::HintError::py("JSONDecodeError", e))
}
fn rj(p: &Path) -> HR<J> {
    lj(&rd(p)?)
}
fn jsonl(rows: &[J]) -> String {
    rows.iter().map(|r| ad(r) + "\n").collect()
}
fn utime(p: &Path, secs: u64) -> HR<()> {
    let f = std::fs::OpenOptions::new().write(true).open(p).map_err(|e| core::io_err(&e, p))?;
    f.set_times(std::fs::FileTimes::new().set_accessed(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs)))
        .map_err(|e| core::io_err(&e, p))
}
/// every file under `d` (recursive · sorted).
fn rglob(d: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                out.push(p.clone());
                if p.is_dir() && !p.is_symlink() {
                    walk(&p, out);
                }
            }
        }
    }
    let mut v = Vec::new();
    walk(d, &mut v);
    v.sort();
    v
}
fn code_root() -> PathBuf {
    core::skill_dir().parent().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default()
}
fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn genv(utc: Option<&str>) -> HR<Vec<(String, String)>> {
    let mut v: Vec<(String, String)> = vec![
        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("GIT_AUTHOR_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_AUTHOR_EMAIL".into(), core::synthetic_email()),
        ("GIT_COMMITTER_NAME".into(), core::SYNTHETIC_NAME.into()),
        ("GIT_COMMITTER_EMAIL".into(), core::synthetic_email()),
    ];
    if let Some(u) = utc {
        v.push(("GIT_AUTHOR_DATE".into(), core::git_date(u)?));
        v.push(("GIT_COMMITTER_DATE".into(), core::git_date(u)?));
    }
    Ok(v)
}
fn git_e(g: &Path, args: &[&str], env: &[(String, String)]) -> HR<Proc> {
    core::git_full(g, args, true, None, env)
}

/// Rust analogue of the in-process owner imports: the node-local build outputs that the owner launchers resolve relative
/// to their own directory are linked into a scratch checkout (absent source = not linked; the launcher then fails closed).
fn _link_native(repo: &Path) -> HR<()> {
    let root = code_root();
    // The policy owners (completion_gate · evidence_publisher) resolve their own repository from the launcher's physical
    // directory, exactly as the Python resolved `Path(__file__).resolve()` of the linked .py to the code checkout. The
    // fixture's runtime directory holds only links to those owner files, so it becomes one link to the owner directory.
    let rt = repo.join(".claude/policies/runtime");
    let rt_src = root.join(".claude/policies/runtime");
    if rt.is_dir() && !rt.is_symlink() && rt_src.is_dir() {
        let only_links = std::fs::read_dir(&rt).map_err(py)?.flatten().all(|e| e.path().is_symlink());
        if only_links {
            std::fs::remove_dir_all(&rt).map_err(|e| core::io_err(&e, &rt))?;
            symlink(&rt_src, &rt)?;
        }
    }
    for rel in [
        ".claude/skills/adversarial-benchmark/rust/target",
        ".claude/skills/upstream-version-watch/rust/target",
        ".claude/skills/upstream-version-watch/rust/runtime/target",
        ".claude/rust/policy-gate/target",
        ".claude/rust/policy-publisher/target",
        ".claude/rust/runtime-support/target",
        ".claude/skills/hint-publisher/rust/target",
        ".claude/skills/terraforming_node/rust/target",
        // the owners' own runtime files (Python reached them through `Path(__file__).resolve()` of the linked .py; the native
        // launchers locate them from the launcher's directory, i.e. this scratch checkout).
        ".claude/schemas",
    ] {
        let (src, dst) = (root.join(rel), repo.join(rel));
        if src.exists() && !dst.exists() && dst.symlink_metadata().is_err() {
            mkd(dst.parent().unwrap())?;
            symlink(&src, &dst)?;
        }
    }
    for rel in [
        ".claude/skills/terraforming_node/scripts/campaign_init.sh",
        ".claude/policies/runtime/completion_gate.sh",
        ".claude/policies/runtime/evidence_publisher.sh",
        ".claude/skills/terraforming_node/scripts/campaign_template_validator.sh",
        ".claude/skills/adversarial-benchmark/scripts/parse_guidellm.sh",
    ] {
        let (src, dst) = (root.join(rel), repo.join(rel));
        if src.is_file() && dst.symlink_metadata().is_err() {
            mkd(dst.parent().unwrap())?;
            symlink(&src, &dst)?;
        }
    }
    Ok(())
}

type Dk<'a> = super::DockerRunner<'a>;

fn fc(repo: &Path, cell: &str, node: Option<&str>, dk: Option<Dk>) -> HR<J> {
    from_campaign_r(repo, &J::from("c1"), &J::from(cell), &J::from(node), dk)
}

/// `selftest_stage_manifest_contract(own, scratch)` — the REAL owner adapter, its runtime files and the built executable
/// (bytes and mode) copied into a scratch checkout. A missing source file raises (nothing is skipped).
pub fn selftest_stage_manifest_contract(own: &Path, scratch: &Path) -> HR<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut rels = vec![REL_MANIFEST_CONTRACT];
    rels.extend(REL_MANIFEST_CONTRACT_RUNTIME.iter().copied());
    for rel in rels {
        let (source, dest) = (own.join(rel), scratch.join(rel));
        mkd(dest.parent().unwrap())?;
        wrb(&dest, &rdb(&source)?)?;
        let mode = std::fs::metadata(&source).map_err(|e| core::io_err(&e, &source))?.permissions().mode() & 0o777;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode)).map_err(|e| core::io_err(&e, &dest))?;
    }
    Ok(())
}

/// 실패 메시지 목록(빈 목록 = 통과). 격리 저장소 · 라이브 태그/브랜치/캠페인 비의존 · ★ = 음성대조.
pub fn selftest() -> Vec<String> {
    let mut c = Ck(Vec::new());
    {
        let td = core::TempDir::new("hint-ev-st-");
        if let Err(e) = _selftest_custom_sessions(&mut c, td.path()) {
            c.exc(e);
        }
    }
    {
        let td = core::TempDir::new("hint-ev-st-");
        if let Err(e) = _selftest_runtime_epochs(&mut c, td.path()) {
            c.exc(e);
        }
    }
    {
        let td = core::TempDir::new("hint-ev-st-");
        if let Err(e) = st_preserved_runs(&mut c, td.path()) {
            c.exc(e);
        }
    }
    if let Err(e) = st_windows(&mut c) {
        c.exc(e);
    }
    let prev = std::env::var_os("QUANT_MODEL_PATH");
    std::env::remove_var("QUANT_MODEL_PATH");
    {
        let td = core::TempDir::new("hint-ev-st-");
        if let Err(e) = st_main(&mut c, td.path()) {
            c.exc(e);
        }
    }
    if let Some(p) = prev {
        std::env::set_var("QUANT_MODEL_PATH", p);
    }
    // trap_open 의 Rust 사실: 미끼는 모드 0 — root 가 아니면 읽기는 실패로 드러난다(root 면 증명이 공허하다고 적는다).
    let euid_root = std::fs::metadata("/proc/self").map(|m| std::os::unix::fs::MetadataExt::uid(&m) == 0).unwrap_or(false);
    let active_hits: Vec<String> = if euid_root { vec!["root: 모드 0 미끼가 증명하지 못한다".into()] } else { vec![] };
    c.ck(&format!("★campaigns/ACTIVE 는 이 모듈에서 한 번도 열리지 않았다({})", J::from(active_hits.iter().take(2).cloned().collect::<Vec<_>>()).repr()),
        active_hits.is_empty());
    c.0
}

fn st_preserved_runs(c: &mut Ck, repo: &Path) -> HR<()> {
    let base = repo.join("output/single/benchlog");
    let old = base.join("sweep_cell-a");
    let fresh = base.join("sweep_cell-a_fresh");
    mkd(&old)?;
    mkd(&fresh)?;
    let (old_utc, fresh_utc) = ("2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z");
    for (p, u) in [(&old, old_utc), (&fresh, fresh_utc)] {
        wr(&p.join("sweep_index.json"), &ad(&jd! {"generated_utc" => u, "meta" => jd! {"config_name" => "cell-a", "tensor_parallel_size" => 1}, "levels" => J::list()}))?;
    }
    let old_bytes = rdb(&old.join("sweep_index.json"))?;
    let fresh_rel = core::posix(fresh.strip_prefix(repo).unwrap());
    let old_rel = core::posix(old.strip_prefix(repo).unwrap());
    let n = J::Null;
    let l = J::list();
    let bs = |cell: &str, mu: &str, gu: &J, dir: &J, cfg: &J| _bind_sweep(repo, "single", cell, Some(mu), &l, dir, cfg, gu);
    let fr = J::from(fresh_rel.as_str());
    let fu = J::from(fresh_utc);
    let (got, missing, _) = bs("cell-a", fresh_utc, &fu, &fr, &n)?;
    c.ck("fresh output pointer binds the matching run", got.as_ref().map_or(false, |g| eqs(&g["dir"], &fresh_rel)) && missing.is_empty());
    let (got, missing, _) = bs("cell-a", fresh_utc, &n, &n, &n)?;
    c.ck("replay finds the preserved custom run by config and UTC", got.as_ref().map_or(false, |g| eqs(&g["dir"], &fresh_rel)) && missing.is_empty());
    c.ck("publication cell comes from custom run metadata", _cell_of_publication(repo, "single", Some(fresh_utc), &l)?.0 == "cell-a");
    c.ck("old sweep bytes and original path survive reads", rdb(&old.join("sweep_index.json"))? == old_bytes);
    c.ck("legacy measured UTC still binds the old run", bs("cell-a", old_utc, &n, &n, &n)?.0.map_or(false, |g| eqs(&g["dir"], &old_rel)));
    c.ck("explicit stale pointer cannot silently select a different run",
        _code(bs("cell-a", fresh_utc, &fu, &J::from(old_rel.as_str()), &n))?.as_deref() == Some("HINT_SWEEP_POINTER_MISMATCH"));
    c.ck("explicit wrong cell pointer is rejected", _code(bs("cell-b", fresh_utc, &fu, &fr, &n))?.as_deref() == Some("HINT_SWEEP_POINTER_MISMATCH"));
    c.ck("official sweep config permits a distinct campaign cell identifier",
        bs("campaign-cell", fresh_utc, &fu, &fr, &J::from("cell-a"))?.0.map_or(false, |g| eqs(&g["dir"], &fresh_rel)));
    c.ck("explicit missing pointer does not fall back to a matching run",
        bs("cell-a", fresh_utc, &fu, &J::from("output/single/benchlog/sweep_missing"), &n)?.0.is_none());
    for wrong in [core::posix(&fresh), "output/multi/benchlog/sweep_cell-a".into(), "output/single/benchlog/../sweep_cell-a".into()] {
        c.ck(&format!("unsafe output pointer rejected: {wrong}"),
            _code(bs("cell-a", fresh_utc, &fu, &J::from(wrong.as_str()), &n))?.as_deref() == Some("HINT_SWEEP_POINTER_INVALID"));
    }
    let alias = base.join("sweep_alias");
    symlink(&fresh, &alias)?;
    c.ck("symlink pointer rejected",
        _code(bs("cell-a", fresh_utc, &fu, &J::from(core::posix(alias.strip_prefix(repo).unwrap())), &n))?.as_deref() == Some("HINT_SWEEP_POINTER_INVALID"));
    let duplicate = base.join("sweep_cell-a_duplicate");
    mkd(&duplicate)?;
    wrb(&duplicate.join("sweep_index.json"), &rdb(&fresh.join("sweep_index.json"))?)?;
    c.ck("ambiguous replay does not choose a run", _code(bs("cell-a", fresh_utc, &n, &n, &n))?.as_deref() == Some("HINT_SWEEP_AMBIGUOUS"));
    c.ck("official exact pointer disambiguates the preserved runs", bs("cell-a", fresh_utc, &fu, &fr, &n)?.0.map_or(false, |g| eqs(&g["dir"], &fresh_rel)));
    c.ck("writer UTC cannot be replaced by a later matching index and report",
        _code(bs("cell-a", fresh_utc, &J::from(old_utc), &fr, &n))?.as_deref() == Some("HINT_SWEEP_POINTER_MISMATCH"));
    c.ck("explicit pointer requires the producer UTC", _code(bs("cell-a", fresh_utc, &n, &fr, &n))?.as_deref() == Some("HINT_SWEEP_POINTER_INVALID"));
    c.ck("TP discovery honors the exact pointer despite duplicate candidates",
        _tp_signal(repo, "single", "cell-a", &n, Some(fresh_utc), None, &l,
            &jd! {"sweep_dir" => fresh_rel.as_str(), "sweep_config" => "cell-a", "sweep_generated_utc" => fresh_utc})?.0 == Some(1));
    Ok(())
}

fn st_windows(c: &mut Ck) -> HR<()> {
    let wd = jl![jl!["2026-01-01T00:00:00Z", 1, jd! {"kind" => "budget_honored", "label" => "A"}],
        jl!["2026-01-01T00:05:00Z", 2, jd! {"kind" => "watchdog_highrate_hold"}]];
    let opens = jl![jl!["2026-01-01T00:00:00Z", "A"], jl!["2026-01-01T00:04:00Z", "B"]];
    let w = _event_windows(&wd, &opens)?.into_iter().next().unwrap_or_default();
    c.ck("★다른 스트림의 다른 라벨 선언 뒤 라벨 없는 행은 앞 창 밖", w["rows"] == jl![0] && w["end_ts"].is_null());
    c.ck("교차 상한 없으면 종전 그대로(hold 가 열린 창 안)", _event_windows(&wd, &J::Null)?.first().map_or(false, |x| x["rows"] == jl![0, 1]));
    let clamp = J::Int(37580963840);
    c.ck("★선언 클램프와 같은 KV 는 '되읽은 값(측정 아님)'", _kv_declared_note(&J::from("35.0"), &clamp, "y")?.unwrap_or_default().contains("측정 아님"));
    c.ck("선언과 다른 KV 는 '다르다(엔진 보고)'", _kv_declared_note(&J::from("30.2"), &clamp, "y")?.unwrap_or_default().contains("다르다"));
    c.ck("파싱 불가 = None(지어내지 않는다)", _kv_declared_note(&J::Null, &clamp, "y")?.is_none());
    c.ck("같은 라벨 재선언은 창을 자르지 않는다",
        _event_windows(&wd, &jl![jl!["2026-01-01T00:00:00Z", "A"], jl!["2026-01-01T00:04:00Z", "A"]])?.first().map_or(false, |x| x["rows"] == jl![0, 1]));
    Ok(())
}

fn st_main(c: &mut Ck, td: &Path) -> HR<()> {
    let (repo, fx) = _fixture(td)?;
    _link_native(&repo)?;
    let state = Rc::new(RefCell::new(fx["docker"].clone()));
    let st2 = state.clone();
    let dkf = move |argv: &[String]| -> Result<Proc, String> { Ok(_fake_docker_run(&mut st2.borrow_mut(), argv)) };
    let dk: Dk = &dkf;
    let set_hist = |h: &str| {
        state.borrow_mut().get_mut("history").and_then(J::as_dict_mut).expect("history").insert(_DIGEST, h);
    };
    let tds = core::posix(td);
    // ── from_campaign · ★ACTIVE 미독 ──
    let ev = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("명시 id 만 읽는다(★ACTIVE=decoy·권한 0 — 읽었다면 죽었다)", eqs(&ev["campaign_id"], "c1") && eqs(&ev["cell"], "c1-a"));
    c.ck("노드 축 = 인증서 measured_node(cluster)", eqs(&ev["node"], "cluster") && eqs(&ev["topology"], "multi"));
    c.ck("셀 필터 — 다른 셀 포인터 제외", ev["pointers"].items().iter().all(|p| eqs(&p["cell_id"], "c1-a")) && ev["pointers"].len() == 4);
    c.ck("인증서 파싱(게이트 파서)", eqs(&ev["certificate"]["measured_utc"], _M_UTC));
    c.ck("스윕 조인(generated_utc = measured_utc)", eqs(&ev["sweep"]["binding"], "output"));
    let bi = &ev["build_identity"];
    c.ck("빌드 정체 = history build-arg · digest 관측", eqs(&bi["vllm_ref"], "v0.9.0")
        && eqs(&bi["source"]["vllm_ref"], "docker history build-arg VLLM_REF") && eqs(&bi["torch"], "2.9.0a0") && eqs(&bi["track"], "source-build"));
    let mut hk = keys(&bi["history_build_args"]);
    hk.sort();
    c.ck("★베이스 이미지 build-arg 는 싣지 않는다(쓰인 Dockerfile 의 ARG 만)", hk == strs(&["SM12X_PORT", "VLLM_REF", "VLLM_REPO"]));
    c.ck("resolve 바인딩 → vllm_sha", eqs(&bi["vllm_sha"], _SHA_A));
    c.ck("attestation 셀 이름 바인딩", eqs(&ev["attestation"]["config"], "c1-a") && !inn(&ev["missing"], "HINT_MISSING_SLAVE_ATTESTATION"));
    c.ck("phases 셀 필터(artifacts 소비 모양)", eqs(&ev["phases"]["serve"]["ended_utc"], "2026-01-02T01:30:00Z"));
    c.ck("docker 는 읽기만(inspect·history)", state.borrow()["calls"].items().iter().all(|x| eqs(&x[1], "image") || eqs(&x[1], "history")));
    c.ck("결손 코드는 전부 등재", ev["missing"].items().iter().all(|m| MISSING_CODES().get(&st(m)).is_some()));
    c.ck("lineage 시드 예약 키", eqs(&ev["lineage_seeds"]["cell_key"], "c1-a") && ev["lineage_seeds"]["identity"]["tp"] == J::Int(2)
        && ev["lineage_seeds"]["journey"].truthy());
    c.ck("모든 dict 필드에 출처", ["sweep", "manifest", "attestation", "resolve", "node"].iter().all(|k| inn(&ev["sources"], k)));
    let q = qualification(&ev)?;
    c.ck("자격 = post_health + bench(관측)", eqs(&q["method"], "post_health+bench") && q["sources"].len() == 2);
    let lv = repo.join(st(&ev["sweep"]["dir"])).join("level_01");
    let bj_a = rdb(&lv.join("bench_c1-a.json"))?;
    rm(&lv.join("bench_c1-a.json"))?;
    let r = (|| -> HR<(J, Option<String>)> {
        wj(&lv.join("guidellm_c1-a.json"), &jd! {"benchmarks" => jl![jd! {"metrics" => jd! {"request_totals" => jd! {"successful" => 3, "errored" => 0, "incomplete" => 0, "total" => 3}}}]})?;
        let qg = qualification(&ev)?;
        wj(&lv.join("guidellm_c1-a.json"), &jd! {"benchmarks" => jl![jd! {"metrics" => jd! {"request_totals" => jd! {"successful" => 0, "errored" => 3}}}]})?;
        Ok((qg, _code(qualification(&ev))?))
    })();
    let _ = std::fs::remove_file(lv.join("guidellm_c1-a.json"));
    wrb(&lv.join("bench_c1-a.json"), &bj_a)?;
    let (qg, qg0) = r?;
    c.ck("★결함 A 자격 = post_health + guidellm(successful ≥1 · parse_guidellm 규칙)",
        sw_(&qg["method"], "post_health+guidellm") && ew_(&qg["sources"][1], "level_01/guidellm_c1-a.json"));
    c.ck("결함 A 음성대조: GuideLLM 성공 요청 0 = 자격 차단(HINT_QUALIFICATION_UNOBSERVED)", qg0.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    let vocab = crate::naming::load_vocab(&repo)?;
    let facts = naming_facts(&repo, &ev)?;
    let dn = crate::naming::derive_name(&facts, &vocab)?;
    c.ck(&format!("이름 전량 파생 ({})", dn.tag), dn.tag == "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len4096-kvauto");
    c.ck("출처에 호스트 경로 없음(PII)", !ud(&facts).contains(&tds));
    c.ck("D-h naming facts 가 vllm_repo 를 키로 싣는다(업스트림 판정 입력)", inn(&facts["vllm"], "vllm_repo")
        && crate::naming::vllm_repo_kind(&facts["vllm"]["vllm_repo"])?.as_deref() == Some("upstream"));
    let hist0 = st(&state.borrow()["history"][_DIGEST]);
    set_hist(&hist0.replace("VLLM_REPO=https://github.com/vllm-project/vllm.git", "VLLM_REPO=https://example.invalid/fork/vllm.git"));
    let ev_fork = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★D-h 포크 VLLM_REPO 면 resolved.json(업스트림 해소)의 to_sha 를 vllm_sha 로 쓰지 않는다",
        ev_fork["build_identity"]["vllm_sha"].is_null() && has(&ev_fork["build_identity"]["source"]["vllm_sha"], "포크"));
    let nf_fork = naming_facts(&repo, &ev_fork)?;
    c.ck("★D-h 포크 + 릴리스 모양 ref + 원장 SHA 없음 = 이름 차단(업스트림 릴리스 이름 ✗)",
        _code(crate::naming::derive_name(&nf_fork, &vocab))?.as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    set_hist(&hist0);
    let envp = repo.join("output/multi/envs/.env.c1-a");
    let env_text = rd(&envp)?;
    wr0(&envp, &env_text.replace("VLLM_REF=v0.9.0", "VLLM_REF=v0.9.1"))?;
    let e_ = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★셀 env 와 이미지 history 가 다르면 이름을 짓지 않는다", _code(naming_facts(&repo, &e_))?.as_deref() == Some("HINT_VLLM_INPUT_CONFLICT"));
    wr0(&envp, &env_text)?;
    let y = repo.join("output/multi/configs/c1-a.yaml");
    let y_text = rd(&y)?;
    wr0(&y, &y_text.replace("max-model-len: 4096", "max-model-len: 8192"))?;
    c.ck("★측정 뒤 바뀐 트리플렛으로 이름을 짓지 않는다", _code(naming_facts(&repo, &ev))?.as_deref() == Some("HINT_TRIPLET_DRIFT"));
    c.ck("★스윕 meta 가 없어도 인증서 기록과 대조한다", _code(naming_facts(&repo, &rep(&ev, &[("sweep", J::Null)])))?.as_deref() == Some("HINT_TRIPLET_DRIFT"));
    for (new_text, what) in [
        (y_text.replace("kv-cache-dtype: auto", "kv-cache-dtype: fp8"), "kv dtype"),
        (y_text.replace("speculative-config:", "# speculative-config:"), "spec 줄 삭제 · 인증서 spec_on"),
        (y_text.replace("tensor-parallel-size: 2", "tensor-parallel-size: 1"), "TP"),
        (y_text.replace("Fixture-Model-NVFP4", "Fixture-Model-FP8"), "모델 경로"),
    ] {
        wr0(&y, &new_text)?;
        c.ck(&format!("★측정 기록과 다른 트리플렛({what})으로 이름을 짓지 않는다"), _code(naming_facts(&repo, &ev))?.as_deref() == Some("HINT_TRIPLET_DRIFT"));
    }
    wr0(&y, &y_text.replace("kv-cache-dtype: auto\n", ""))?;
    c.ck("kv dtype 줄이 없어도 실효값(auto)이 기록과 같으면 드리프트가 아니다", _code(naming_facts(&repo, &ev))?.is_none());
    wr0(&y, &y_text)?;
    let cfgp = td.join("models/Org/Fixture-Model-NVFP4/config.json");
    let cfg_text = rd(&cfgp)?;
    let mut cfg_doc = lj(&cfg_text)?;
    cfg_doc.as_dict_mut().unwrap().remove("text_config");
    wr0(&cfgp, &ad(&cfg_doc))?;
    c.ck("★PLE 키 없는 모델에 VLLM_PLE_MMAP=1 = 모순(조용히 plenone 으로 접지 않는다)", _code(naming_facts(&repo, &ev))?.as_deref() == Some("HINT_PLE_CONFLICT"));
    wr0(&cfgp, &cfg_text)?;
    let ev_nt = rep(&ev, &[("cell_config", J::dict()), ("declaration", rep(&fx["decl"], &[("control_variables", jd! {"topology" => "multi", "target_gpu" => "<<FILL>>"})]))]);
    c.ck("★캠페인 모드에서 타겟 선언을 읽지 못하면 roofline 으로 대체하지 않는다(target_gpu 키 부재 = 파생 불가)", !inn(&_arch_facts(&repo, &ev_nt)?, "target_gpu"));
    let mb = if ev["manifest"].is_dict() { ev["manifest"].clone() } else { J::dict() };
    let ms = rep(&mb, &[("self_role", J::from("sub")), ("nodes", J::list()), ("gpus_per_node", J::Int(8))]);
    let a_s = _arch_facts(&repo, &rep(&ev, &[("node", J::from("sub")), ("manifest", ms)]))?;
    c.ck("★서브 자기 manifest 의 sub 축 = 최상위 gpu_model(nodes[role=sub] 부재여도 파생)", a_s["gpu_model"] == mb["gpu_model"] && !mb["gpu_model"].is_null());
    let mm = rep(&mb, &[("nodes", jl![jd! {"role" => "main"}, jd! {"role" => "sub"}]), ("gpus_per_node", J::Int(8))]);
    let a_m = _arch_facts(&repo, &rep(&ev, &[("node", J::from("sub")), ("manifest", mm)]))?;
    c.ck("메인 manifest 의 sub 축은 여전히 nodes[role=sub] 항목에서만(최상위 = 메인 사실로 대체 ✗)", a_m["gpu_model"].is_null());
    let ptrp = repo.join("campaigns/c1/evidence_pointers.json");
    let ptr_text = rd(&ptrp)?;
    let mut ptr_doc = lj(&ptr_text)?;
    pm(&mut ptr_doc, &["pointers"]).push(jd! {"kind" => "testlog", "path" => fx["testlog"].clone(), "cell_id" => "c1-a", "node_id" => J::Null});
    wj(&ptrp, &ptr_doc)?;
    c.ck("★다노드 캠페인의 노드 태그 없는 포인터는 싣지 않는다(validator P2 · 귀속 불명)", fc(&repo, "c1-a", None, Some(dk))?["pointers"].len() == 4);
    let other_rep = "docs/benchmark/bench_report_26010111_fixture-model-nvfp4_GB10_0.9.0.md";
    wr0(&repo.join(other_rep), &rd(&repo.join(st(&fx["report"])))?.replace(_M_UTC, "2026-01-01T02:00:00Z"))?;
    let mut ptr_doc = lj(&ptr_text)?;
    let mut ps: Vec<J> = ptr_doc["pointers"].items().iter().filter(|p| !eqs(&p["kind"], "bench_report")).cloned().collect();
    ps.push(jd! {"kind" => "bench_report", "path" => other_rep, "cell_id" => "c1-a", "node_id" => "cluster"});
    ptr_doc.set("pointers", J::List(ps));
    wj(&ptrp, &ptr_doc)?;
    c.ck("★인증서와 다른 측정의 리포트 포인터 = 섞지 않고 죽는다(생성일 ≠ measured_utc)",
        _code(fc(&repo, "c1-a", None, Some(dk)))?.as_deref() == Some("HINT_BENCH_REPORT_CERT_MISMATCH"));
    rm(&repo.join(other_rep))?;
    wr0(&ptrp, &ptr_text)?;
    // attestation v2
    let attp = repo.join("output/multi/benchlog/attestation_c1-a.json");
    let att_text = rd(&attp)?;
    let mut v2 = jd! {"schema_version" => 2, "config" => "c1-a", "image_tag" => "fixture:0.9.0-source", "phase" => "serve", "checks" => J::list(),
        "nodes" => jd! {"main" => jd! {"image_id" => _DIGEST, "build_ledger" => jd! {"vllm" => jd! {"describe" => "v0.9.0"}}},
                        "sub" => jd! {"image_id" => format!("sha256:{}", "e".repeat(64))}},
        "parity" => jd! {"vllm_sha" => jd! {"verdict" => "equal", "main" => _SHA_A, "sub" => _SHA_A}, "torch" => jd! {"verdict" => "unobservable"}}};
    wj(&attp, &v2)?;
    let ev_v2 = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("v2 attestation(checks 비어 있음 · parity 관측) = 대조 행 결손 아님", !ev_v2["attestation"].is_null() && !inn(&ev_v2["missing"], "HINT_MISSING_ATTESTATION_PARITY_ROWS"));
    c.ck("artifacts 이음매 — 묶인 attestation 경로 = lineage_seeds.attestation", eqs(&ev_v2["lineage_seeds"]["attestation"], "output/multi/benchlog/attestation_c1-a.json"));
    let sha_l = "d".repeat(40);
    let mut v2l = v2.clone();
    pm(&mut v2l, &["nodes", "main", "build_ledger", "vllm"]).set("git_sha", sha_l.as_str());
    pm(&mut v2l, &["parity"]).set("vllm_sha", jd! {"verdict" => "equal", "main" => sha_l.as_str(), "sub" => sha_l.as_str()});
    wj(&attp, &v2l)?;
    let rsv_p = repo.join("output/multi/resolved.json");
    let rsv_b = rdb(&rsv_p)?;
    let ev_bound = fc(&repo, "c1-a", None, Some(dk))?;
    rm(&rsv_p)?;
    let r = (|| -> HR<(J, J)> {
        let ev_led = fc(&repo, "c1-a", None, Some(dk))?;
        let mut v2n = v2l.clone();
        pm(&mut v2n, &["nodes", "main", "build_ledger", "vllm"]).as_dict_mut().unwrap().remove("git_sha");
        wj(&attp, &v2n)?;
        Ok((ev_led, fc(&repo, "c1-a", None, Some(dk))?))
    })();
    wrb(&rsv_p, &rsv_b)?;
    let (ev_led, ev_none) = r?;
    let sv = &ev_led["build_identity"]["source"]["vllm_sha"];
    c.ck("★F1 resolved.json 미바인딩 + 원장 vllm.git_sha = vllm_sha(출처 = 빌드 원장 · describe · parity 판정)",
        eqs(&ev_led["build_identity"]["vllm_sha"], &sha_l) && inn(&ev_led["missing"], "HINT_MISSING_RESOLVE")
        && has(sv, "빌드 원장 vllm.git_sha") && has(sv, "describe=v0.9.0") && has(sv, "parity.vllm_sha=equal"));
    c.ck("★F1 음성대조: resolved.json 이 묶이면 해소값 유지(원장이 덮지 않는다) · 원장 SHA 도 없으면 미관측(지어내지 않는다)",
        eqs(&ev_bound["build_identity"]["vllm_sha"], _SHA_A) && ev_none["build_identity"]["vllm_sha"].is_null());
    let res_from = rep(&ev_bound["resolve"], &[("from_ref", J::from("v0.8.0"))]);
    let mut ev_img = rep(&ev_none, &[("attestation", J::Null), ("image_ledger", J::Null), ("resolve", res_from)]);
    let view = jd! {"vllm" => jd! {"dist_version" => "0.9.1.dev0+gdddd.cu133", "git_sha" => "d".repeat(40), "describe" => "v0.9.0"},
        "source" => "docker-image(main-local):fixture:0.9.0-source:/opt/easy-vllm/build_ledger.json", "provenance" => "measured(build-time, in-image)"};
    let wm0 = _wheel_meta(&repo, &ev_img)?;
    bind_image_ledger_mut(&mut ev_img, &view)?;
    let wm1 = _wheel_meta(&repo, &ev_img)?;
    let bi = &ev_img["build_identity"];
    c.ck("★F7 이미지 원장 vllm.dist_version = wheel 메타(출처 = 이미지 원장 · 빌드 시점) · 바인딩 전엔 미관측",
        wm0.0.is_none() && wm1.0.as_deref() == Some("0.9.1.dev0+gdddd.cu133") && wm1.1.contains("docker-image(main-local)"));
    c.ck("★F7·F9 이미지 원장 git_sha(빈 칸만) · describe · resolved from_ref(이전 핀) = 출처와 함께 · 음성대조: attestation 원장이 있으면 그것",
        eqs(&bi["vllm_sha"], &"d".repeat(40)) && eqs(&bi["vllm_describe"], "v0.9.0") && eqs(&bi["vllm_prev_pin"], "v0.8.0")
        && has(&bi["source"]["vllm_prev_pin"], "from_ref") && has(&bi["source"]["vllm_describe"], "docker-image") && !ev_img["image_ledger"].is_null());
    let mut ev_att = rep(&ev_led, &[("image_ledger", J::Null)]);
    bind_image_ledger_mut(&mut ev_att, &view)?;
    c.ck("F7 음성대조: attestation 원장이 있으면 이미지 원장 뷰로 바꾸지 않는다", ev_att["image_ledger"].is_null()
        && eqs(&ev_att["build_identity"]["vllm_describe"], "v0.9.0") && has(&ev_att["build_identity"]["source"]["vllm_describe"], "attestation"));
    wj(&attp, &v2)?;
    v2.set("parity", jd! {"vllm_sha" => jd! {"verdict" => "unobservable"}, "torch" => jd! {"verdict" => "unobservable"}});
    wj(&attp, &v2)?;
    c.ck("★v2 의 모든 축 unobservable = 대조 행 결손", inn(&fc(&repo, "c1-a", None, Some(dk))?["missing"], "HINT_MISSING_ATTESTATION_PARITY_ROWS"));
    pm(&mut v2, &["nodes", "main"]).set("image_id", format!("sha256:{}", "f".repeat(64)));
    wj(&attp, &v2)?;
    let ev_x = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★노드 image_id 에 측정 digest 가 없는 attestation(같은 태그 · 다른 빌드)은 묶지 않는다",
        ev_x["attestation"].is_null() && inn(&ev_x["missing"], "HINT_MISSING_SLAVE_ATTESTATION"));
    rm(&attp)?;
    let att_build = repo.join("output/multi/benchlog/attestation_c1-build.json");
    wj(&att_build, &jd! {"config" => "c1-build", "image_tag" => "fixture:0.9.0-source", "checks" => jl![jd! {"axis" => "torch", "node" => "sub", "observed" => "x"}]})?;
    let ev_lbl = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("attestation — 셀 이름이 없으면 이미지 compose 라벨의 빌드 config 이름으로",
        eqs(&ev_lbl["lineage_seeds"]["attestation"], "output/multi/benchlog/attestation_c1-build.json") && has(&ev_lbl["attestation"]["_source"], "compose 라벨"));
    rm(&att_build)?;
    wr0(&attp, &att_text)?;
    // 커밋 핀
    let sha_c = "c".repeat(40);
    set_hist(&hist0.replace("VLLM_REF=v0.9.0", &format!("VLLM_REF={sha_c}")));
    wr0(&envp, &env_text.replace("VLLM_REF=v0.9.0", &format!("VLLM_REF={sha_c}")))?;
    let ev_pin = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("커밋 핀 = vllm_sha 는 핀 그 자체(resolved.json 미바인딩이어도 · 결손은 기재)",
        eqs(&ev_pin["build_identity"]["vllm_sha"], &sha_c) && inn(&ev_pin["missing"], "HINT_MISSING_RESOLVE"));
    c.ck("resolved.json — to_sha = 핀이면 묶고 ★다른 핀이면 묶지 않는다",
        _bind_resolve(&repo, "multi", &jd! {"vllm_ref" => _SHA_A})?.0.is_some() && _bind_resolve(&repo, "multi", &jd! {"vllm_ref" => sha_c.as_str()})?.0.is_none());
    set_hist("RUN |2 VLLM_VERSION=0.8.0 CUDA_VERSION=130 /bin/sh -c pip install\n");
    let e_ = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★선택자 트랙과 다른 Dockerfile 로 지은 이미지 = 이름을 짓지 않는다(env 선언으로 폴백 ✗)",
        _code(naming_facts(&repo, &e_))?.as_deref() == Some("HINT_VLLM_INPUT_CONFLICT"));
    set_hist(&hist0);
    wr0(&envp, &env_text)?;
    wr0(&envp, &env_text.replace("BUILD_DOCKERFILE=Dockerfile.source-build\n", ""))?;
    let ev_hist = fc(&repo, "c1-a", None, Some(dk))?;
    let hb = &ev_hist["build_identity"];
    c.ck("★결함 B 선택자 부재 + history VLLM_REF 만 = Dockerfile.source-build(출처 = history 관측 표지 · 태그 휴리스틱 ✗)",
        eqs(&hb["dockerfile"], "Dockerfile.source-build") && eqs(&hb["track"], "source-build")
        && has(&hb["source"]["dockerfile"], HISTORY_TRACK_MARK) && has(&hb["source"]["track"], HISTORY_TRACK_MARK));
    set_hist("RUN |2 VLLM_VERSION=0.8.0 CUDA_VERSION=130 /bin/sh -c pip install\n");
    let ev_hw = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★결함 B history VLLM_VERSION 만 = Dockerfile(wheel)", eqs(&ev_hw["build_identity"]["dockerfile"], "Dockerfile") && eqs(&ev_hw["build_identity"]["track"], "wheel"));
    set_hist("RUN |1 CUDA_VERSION=130 /bin/sh -c true\n");
    let ev_nosel = fc(&repo, "c1-a", None, Some(dk))?;
    let nd_code = match naming_facts(&repo, &ev_nosel) {
        Ok(f) => _code(crate::naming::derive_name(&f, &vocab))?,
        Err(e) => _code::<()>(Err(e))?,
    };
    c.ck("★선택자 미관측 + history 에 트랙 인자 없음이면 트랙을 추측하지 않는다(이름 파생 불가)",
        ev_nosel["build_identity"]["track"].is_null() && nd_code.as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    set_hist(&(hist0.clone() + "RUN |1 VLLM_VERSION=0.8.0 /bin/sh -c true\n"));
    c.ck("결함 B history 에 VLLM_REF · VLLM_VERSION 둘 다 = 판정 불가 유지", fc(&repo, "c1-a", None, Some(dk))?["build_identity"]["track"].is_null());
    set_hist(&hist0);
    let comp = repo.join("output/multi/docker-compose.yaml");
    let comp_text = rd(&comp)?;
    wr0(&comp, &(comp_text.clone() + "    build:\n      dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n"))?;
    let ev_def = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("셀 env 선택자 부재 → compose 기본값(slave_forward.compose_default) · history 관측과 일치하면 기본값 출처 유지",
        eqs(&ev_def["build_identity"]["track"], "source-build") && has(&ev_def["build_identity"]["source"]["dockerfile"], "기본값")
        && !has(&ev_def["build_identity"]["source"]["dockerfile"], HISTORY_TRACK_MARK));
    wr0(&comp, &(comp_text.clone() + "    build:\n      dockerfile: ${BUILD_DOCKERFILE:-Dockerfile}\n"))?;
    let ev_cx = fc(&repo, "c1-a", None, Some(dk))?;
    c.ck("★결함 B compose 기본값(Dockerfile)이 이미지 관측(VLLM_REF)과 모순 = 관측이 이긴다(근거 기재)",
        eqs(&ev_cx["build_identity"]["dockerfile"], "Dockerfile.source-build") && has(&ev_cx["build_identity"]["source"]["dockerfile"], "모순 · 관측이 이긴다"));
    wr0(&comp, &(comp_text.clone() + "    build:\n      dockerfile: Dockerfile   # 트랙별 주석\n"))?;
    let ev_lit = fc(&repo, "c1-a", None, Some(dk))?;
    let s_lit = st(&ev_lit["build_identity"]["source"]["dockerfile"]);
    let cbd = |p: &Path| compose_build_dockerfile(&pj(p));
    c.ck("★F2 compose 리터럴 Dockerfile(주석 제외) · history 관측 VLLM_REF → 값 = 관측 · 출처에 리터럴 + '≠ 관측(compose build 로 지어지지 않았다)' · '모두 부재' ✗",
        cbd(&comp)? == (Some("literal".into()), Some("Dockerfile".into())) && eqs(&ev_lit["build_identity"]["dockerfile"], "Dockerfile.source-build")
        && s_lit.contains("리터럴 build.dockerfile: Dockerfile(") && s_lit.contains("이 파일만 단독 적용하고 override 가 없을 때")
        && s_lit.contains("과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측")
        && !s_lit.contains("compose build 는 늘") && !s_lit.contains("모두 부재"));
    let f2a = cbd(&td.join("nope.yaml"))? == (None, None);
    wr0(&comp, &(comp_text.clone() + "    build:\n      dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.x}\n"))?;
    c.ck("F2 compose_build_dockerfile: 기본값 꼴 = selector · 블록 없음 = (None, None)", f2a && cbd(&comp)? == (Some("selector".into()), Some("Dockerfile.x".into())));
    wr0(&comp, &comp_text)?;
    wr0(&envp, &env_text)?;
    let ident = identity(&ev)?;
    c.ck("identity 6키 + base_model/hf_repo", eqs(&ident["hf_repo"], "Org/Fixture-Model-NVFP4") && eqs(&ident["base_model"], "Org/Fixture-Model") && eqs(&ident["base_slug"], "fixture-model"));
    c.ck("X7 토폴로지 라벨", topology_label(&ident)? == "multi TP=2(Ray)" && topology_label(&jd! {"topology" => "single", "tp" => 1})? == "single TP=1");
    c.ck("★라벨 파생 불가", _code(topology_label(&jd! {"topology" => "multi", "tp" => "2"}))?.as_deref() == Some("HINT_TOPOLOGY_UNDERIVABLE"));
    _selftest_round2(c, &repo, td, &ev, &ident)?;
    _selftest_round3(c, &repo, td, &ev, &fx, dk)?;
    _selftest_v7(c, &repo, td, &ev, &fx, dk)?;
    _selftest_fix2(c, td, &ev)?;
    st_main2(c, &repo, td, &ev, &fx, dk, &dn.tag)
}

#[allow(clippy::too_many_arguments)]
fn st_main2(c: &mut Ck, repo: &Path, td: &Path, ev: &J, fx: &J, dk: Dk, tag: &str) -> HR<()> {
    let repo = repo.to_path_buf();
    let repo = repo.as_path();
    let m = measurement(ev)?;
    c.ck("measurement 수치만(절대경로 칸 제외)", eqs(&m["decode_tps_conc1"], "12.3") && !ad(&m).contains("raw_json"));
    let mc = measurement_config(repo, ev)?;
    let mut want: Vec<String> = DISTRIBUTED_MEASUREMENT_KEYS.iter().map(|s| s.to_string()).collect();
    want.push("source".into());
    want.sort();
    let mut got = keys(&mc);
    got.sort();
    c.ck("measurement_config = 리포트 표 + 배포 키만", eqs(&mc["bench_mode"], "full") && mc["repeats"] == J::Int(3) && got == want);
    // ── 승인(X3) ──
    let ap = approval_for(repo, &J::from("c1"), &J::from("c1-a"), &J::from("cluster"))?;
    c.ck("승인 — cluster 축을 배정 노드로 해소", ap.as_ref().map_or(false, |a| eqs(&a["node_id"], "main") && eqs(&a["source"], "campaign:hint_targets")));
    c.ck("★승인 없는 셀 = None", approval_for(repo, &J::from("c1"), &J::from("c1-b"), &J::from("sub"))?.is_none());
    let decl_p = repo.join("campaigns/c1/campaign.yaml");
    let mut partial = fx["decl"].clone();
    pm(&mut partial, &["hint_targets", "0", "approval"]).as_dict_mut().unwrap().remove("approved_utc");
    wj(&decl_p, &partial)?;
    c.ck("★반쪽 승인 = 죽는다", _code(approval_for(repo, &J::from("c1"), &J::from("c1-a"), &J::from("cluster")))?.as_deref() == Some("HINT_APPROVAL_INVALID"));
    let mut partial = fx["decl"].clone();
    pm(&mut partial, &["hint_targets", "0"]).set("arch", "gb10-main-native");
    wj(&decl_p, &partial)?;
    c.ck("★폐기 키 arch = 죽는다", _code(approval_for(repo, &J::from("c1"), &J::from("c1-a"), &J::from("cluster")))?.as_deref() == Some("HINT_APPROVAL_INVALID"));
    wj(&decl_p, &fx["decl"])?;
    c.ck("★_bootstrap 발행 거부", _code(from_campaign_r(repo, &J::from("_bootstrap"), &J::from("c1-a"), &J::Null, None))?.as_deref() == Some("HINT_CAMPAIGN_ID_RESERVED"));
    c.ck("★경로 셀 거부", _code(from_campaign_r(repo, &J::from("c1"), &J::from("../x"), &J::Null, None))?.as_deref() == Some("HINT_CELL_INVALID"));
    c.ck("★측정 노드와 다른 축 거부", _code(fc(repo, "c1-a", Some("main"), Some(dk)))?.as_deref() == Some("HINT_ARCH_NODE_AXIS_CERT_MISMATCH"));
    // ── 인증서 파서 ★ ──
    let bad_cert = repo.join("docs/benchmark/benchmark_26010213_bad_GB10_0.9.0.yaml");
    wr0(&bad_cert, "# 주석\nschema_version: 1\nmodel: demo\ngpu_model: \"NVIDIA GB10\"\ndecode_tps_conc1: 34.55\n")?;
    let bc_rel = J::from(_rel(repo, &bad_cert)?);
    let pc = parse_certificate(repo, &bc_rel)?;
    c.ck("인증서 파싱(게이트 파서 한 벌) — 따옴표 제거 · 전행 주석 무시 · 값은 원문 문자열",
        eqs(&pc["gpu_model"], "NVIDIA GB10") && eqs(&pc["decode_tps_conc1"], "34.55") && !ud(&pc).contains("주석"));
    for (body, what) in [("model: x\n  nested: y\n", "들여쓰기(중첩)"), ("a: 1\nb:\n  c: 2\n", "중첩 사전"), ("# 주석뿐\n", "빈 인증서"), ("model: a\nmodel: b\n", "중복 키")] {
        wr0(&bad_cert, body)?;
        c.ck(&format!("★{what} 인증서 = 파싱 거부"), _code(parse_certificate(repo, &bc_rel))?.as_deref() == Some("HINT_CERTIFICATE_UNPARSEABLE"));
    }
    rm(&bad_cert)?;
    // ── no_cert_binding_source ──
    let r_ = Some(repo);
    c.ck("map_only 는 클래스로 연다", no_cert_binding_source(&jd! {"task_class" => "hint_map_only", "benchmark" => J::dict()}, r_)?.as_deref() == Some("hint_map_only"));
    c.ck("explore REFUTE 는 계약 통과로 연다", no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => jd! {
        "rubric_authority" => "explore", "rubric_source" => "verdict_json", "floor_tps" => 1.0, "ratio_M_over_primary" => 0.9,
        "primary_source" => "x", "verdict" => "REFUTE"}}, r_)?.as_deref() == Some("explore"));
    let mut all_none = true;
    for a in ["explore", "weak", "explicit"] {
        all_none &= no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => jd! {
            "rubric_authority" => a, "rubric_source" => "verdict_json", "floor_tps" => 0, "ratio_M_over_primary" => 0.9,
            "primary_source" => "x", "verdict" => "REFUTE"}}, r_)?.is_none();
    }
    c.ck("★공허 floor 의 REFUTE 는 닫힌다(권한 무관)", all_none);
    let obs = jd! {"rubric_authority" => "weak", "rubric_source" => "verdict_json", "floor_tps" => J::Null,
        "ratio_M_over_primary" => J::Null, "primary_source" => J::Null, "verdict" => "OBSERVATION-ONLY"};
    c.ck("OBSERVATION-ONLY full 셀은 bench_report 로 연다(기준 부재 · 판정 주장 ✗)",
        no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => obs.clone()}, r_)?.as_deref() == Some("observation_only")
        && binding_artifact_path(&jd! {"task_class" => "full_benchmark", "benchmark" => obs.clone(), "evidence" => jd! {"bench_report" => jd! {"path" => "r"}}}, r_)?.as_deref() == Some("r"));
    c.ck("★문턱이 실리거나 출처 표시가 없는 OBSERVATION-ONLY 는 닫힌다",
        no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => rep(&obs, &[("floor_tps", J::Float(1.0))])}, r_)?.is_none()
        && no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => rep(&obs, &[("rubric_source", J::Null)])}, r_)?.is_none());
    c.ck("인증서가 map_only 여도 이긴다", binding_artifact_path(&jd! {"task_class" => "hint_map_only", "benchmark" => J::dict(),
        "evidence" => jd! {"certificate" => jd! {"path" => "c"}, "bench_report" => jd! {"path" => "r"}}}, r_)?.as_deref() == Some("c"));
    // ── publication_records = lineage 규칙 ──
    let evd_dir = repo.join(core::REL_EVIDENCE_DIR);
    mkd(&evd_dir)?;
    wr0(&evd_dir.join("broken.json"), "{")?;
    c.ck("publication_records ≡ lineage.load_publication_records", publication_records(repo)? == crate::lineage::load_publication_records(repo)?);
    rm(&evd_dir.join("broken.json"))?;
    // ── drive_publisher ──
    let draft = repo.join("hints/.drafts/d1");
    let before = _tree(repo)?;
    let topic = topic_for(ev)?;
    c.ck(&format!("토픽은 셀 키로 끝난다({topic})"), topic == "c1__cluster__c1_a");
    let sp_ = crate::lineage::seeds_from_publications(repo, &jl![jd! {"_id" => "c0__cluster__c1_a", "identity" => ev["lineage_seeds"]["identity"].clone(),
        "generated_utc" => "2026-01-01T00:00:00Z", "scaffolded" => J::dict()}], Some(&topic), &ev["lineage_seeds"]["identity"], ev["cell"].as_str(), None)?;
    c.ck("이음매 — 다른 캠페인의 같은 셀 발행이 lineage 과거 발행 시드로 잡힌다(토픽 끝 = 셀 키)", inn(&sp_["prior"], "c0__cluster__c1_a"));
    let jt = J::from(topic.as_str());
    let g5 = J::from("2026-01-02T05:00:00Z");
    let man = drive_publisher(repo, ev, &jt, &g5, &draft)?;
    let after = _tree(repo)?;
    let bk = keys(&before);
    let ak = keys(&after);
    let allowed = |p: &str| p.starts_with("hints/.drafts/d1/inputs/") || p.starts_with("docs/_evidence/") || p.starts_with("docs/simlog/");
    let new: Vec<String> = { let mut v: Vec<String> = ak.iter().filter(|k| !bk.contains(k)).cloned().collect(); v.sort(); v };
    let changed: Vec<&String> = bk.iter().filter(|k| ak.contains(k) && before[k.as_str()] != after[k.as_str()]).collect();
    let bad_new: Vec<String> = new.iter().filter(|p| !allowed(p)).cloned().collect();
    c.ck(&format!("★발행기 구동은 inputs/·발행기 소유 평면 밖에 쓰지 않는다(new={})", J::from(bad_new.clone()).repr()), bad_new.is_empty() && changed.is_empty());
    let mut names: Vec<String> = std::fs::read_dir(draft.join("inputs")).map_err(py)?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    c.ck("inputs/ 는 도구가 쓴 JSON 뿐", names == strs(&["identity.json", "pii.json", "runtime.json", "runtime.provenance.json"]));
    c.ck("★발행기 identity.json 의 quant 는 측정값(N/A → None) — PAYLOAD 표시용 이름 축 대체값이 아니다", rj(&draft.join("inputs/identity.json"))?["quant"].is_null());
    let mdoc = rj(&man)?;
    c.ck("manifest runtime.identity = 스윕 meta 관측", mdoc["runtime"]["identity"]["tp"] == J::Int(2));
    let _ = cgate(repo)?;
    c.ck("X5 simlog 면제 기재", mdoc["pii_scan"]["exempt_paths"].items().iter().any(|e| eqs(&e["reason"], CGate::PII_MACHINE_RAW_EXEMPTION_REASON)));
    let auth = authorize(repo, &pj(&man), "hint_finalize")?;
    c.ck("게이트 허가(격리 저장소 · promotion-ready)", auth["allowed"] == J::Bool(true));
    let topo_l = J::from("multi TP=2(Ray)");
    let g6 = J::from("2026-01-02T06:00:00Z");
    c.ck("★draft 없이 목표 기록 거부", _code(set_promotion_target(repo, &jt, &J::from("hint/x"), &topo_l, &J::from(_SHA_A), &g6, None))?.as_deref() == Some("HINT_PUBLISHER_INPUTS_ABSENT"));
    let jtag = J::from(tag);
    let man2 = set_promotion_target(repo, &jt, &jtag, &topo_l, &J::from(_SHA_A), &g6, Some(&draft))?;
    let mdoc2 = rj(&man2)?;
    c.ck("promotion_target 방출(writer = finalize)", eqs(&mdoc2["promotion_target"]["anchor"], _SHA_A));
    c.ck("봉인 대상 = manifest 목표(X7 라벨 · H3 vLLM 비대조)", promotion_binding_problems(&mdoc2, tag, "multi TP=2(Ray)", _SHA_A)?.is_empty());
    let mut codes: Vec<String> = promotion_binding_problems(&mdoc2, tag, "multi TP=2", &"b".repeat(40))?.into_iter().map(|(c_, _)| c_).collect();
    codes.sort();
    codes.dedup();
    c.ck("★다른 앵커·라벨은 불일치", codes == strs(&["HINT_IDENTITY_TOPOLOGY_LABEL_MISMATCH", "HINT_PROMOTION_TARGET_ANCHOR_MISMATCH", "HINT_PROMOTION_TARGET_TOPOLOGY_MISMATCH"]));
    c.ck("★목표 없는 manifest", promotion_binding_problems(&J::dict(), tag, "x", "y")?.first().map(|x| x.0.as_str()) == Some("HINT_PROMOTION_TARGET_MISSING"));
    c.ck("★짧은 앵커 = 발행기 거부", _code(set_promotion_target(repo, &jt, &jtag, &topo_l, &J::from("abc"), &g6, Some(&draft)))?.as_deref() == Some("HINT_PUBLISHER_FAILED"));
    let man_re = drive_publisher(repo, ev, &jt, &g5, &draft)?;
    c.ck("★발행기 재구동(full)은 멱등이고 기록된 승격 목표를 말없이 떨어뜨리지 않는다", man_re == man && eqs(&rj(&man_re)?["promotion_target"]["anchor"], _SHA_A));
    // ── rebind_campaign_narratives ──
    let recp = evd_dir.join(format!("{topic}.json"));
    let piip = draft.join("inputs/pii.json");
    let (rec0, pii0, man0) = (rdb(&recp)?, rdb(&piip)?, rdb(&man)?);
    let rb0 = rebind_campaign_narratives(repo, ev, &jt, &g5, &draft)?;
    c.ck("rebind 멱등 — 바인딩 = 캠페인 포인터면 set-narrative 없음 · 기록 · pii.json · manifest 바이트 불변",
        rb0["rebound"] == J::list() && crate::api::decode_path(&rb0["manifest"]) == man && rdb(&recp)? == rec0 && rdb(&piip)? == pii0 && rdb(&man)? == man0);
    let ptrp_rb = repo.join("campaigns/c1/evidence_pointers.json");
    let ptr_rb0 = rd(&ptrp_rb)?;
    let (tl_old, tl_new) = (st(&fx["testlog"]), "docs/testlog/testlog_26010213_05_00_fixture.md".to_string());
    mv(&repo.join(&tl_old), &repo.join(&tl_new))?;
    wr0(&ptrp_rb, &ptr_rb0.replace(&tl_old, &tl_new))?;
    let rb1 = rebind_campaign_narratives(repo, &fc(repo, "c1-a", None, Some(dk))?, &jt, &g5, &draft)?;
    let rec1 = rj(&recp)?;
    let pii1 = rj(&piip)?;
    c.ck("rebind 양성 — 옛 경로 부재(개명) → 새 경로로 재바인딩(작성자 표지)",
        peq(&rb1["rebound"], &jl![jd! {"kind" => "testlog", "from" => tl_old.as_str(), "to" => tl_new.as_str()}]) && eqs(&rec1["scaffolded"]["testlog"], &tl_new)
        && eqs(&rec1["narrative_status"]["testlog"]["author"], "hint.py(evidence.rebind_campaign_narratives)"));
    let nm = |p: &str| Path::new(p).file_name().unwrap().to_string_lossy().into_owned();
    c.ck("rebind 양성 — inputs/pii.json scanned_paths 가 새 경로를 들고 옛 경로를 버린다",
        pii1["scanned_paths"].items().iter().any(|p| ew_(p, &nm(&tl_new))) && !pii1["scanned_paths"].items().iter().any(|p| ew_(p, &nm(&tl_old))));
    c.ck("rebind 양성 — 재생성 manifest 가 게이트 허가(promotion-ready)", authorize(repo, &rb1["manifest"], "hint_finalize")?["allowed"] == J::Bool(true));
    wr0(&repo.join(&tl_old), "# testlog\n다른 문서.\n")?;
    wr0(&ptrp_rb, &ptr_rb0)?;
    let rec_c = rdb(&recp)?;
    let e_ = fc(repo, "c1-a", None, Some(dk))?;
    c.ck("★rebind 음성 — 바인딩된 문서가 실재하는데 포인터가 다르면 차단(조용한 교체 ✗ · 기록 불변)",
        _code(rebind_campaign_narratives(repo, &e_, &jt, &g5, &draft))?.as_deref() == Some("HINT_NARRATIVE_REBIND_CONFLICT") && rdb(&recp)? == rec_c);
    rm(&repo.join(&tl_old))?;
    mv(&repo.join(&tl_new), &repo.join(&tl_old))?;
    let rb2 = rebind_campaign_narratives(repo, ev, &jt, &g5, &draft)?;
    c.ck("rebind 역개명 복원 — 기록 · pii.json 이 원 경로로 돌아온다",
        peq(&rb2["rebound"], &jl![jd! {"kind" => "testlog", "from" => tl_new.as_str(), "to" => tl_old.as_str()}]) && rdb(&piip)? == pii0
        && eqs(&rj(&recp)?["scaffolded"]["testlog"], &tl_old));
    let replay_ns = jd! {"mode" => "publication-replay"};
    c.ck("★rebind 재생 모드 = 쓰기 거부", _code(rebind_campaign_narratives(repo, &replay_ns, &jt, &g5, &draft))?.as_deref() == Some("HINT_REPLAY_READ_ONLY"));
    // ★ runtime identity 는 관측이다
    let sweep = crate::api::decode_path(&fx["sweep"]);
    let mut idx = rj(&sweep.join("sweep_index.json"))?;
    pm(&mut idx, &["meta"]).set("vllm_version", "0.9.0-observed");
    wj(&sweep.join("sweep_index.json"), &idx)?;
    let ev2 = fc(repo, "c1-a", None, Some(dk))?;
    let man3 = drive_publisher(repo, &ev2, &J::from(format!("{topic}_rt")), &J::from("2026-01-02T07:00:00Z"), &repo.join("hints/.drafts/d2"))?;
    let rt = rj(&repo.join("hints/.drafts/d2/inputs/runtime.json"))?;
    c.ck("★runtime.identity 는 identity 복사가 아니다(스윕 meta 관측)", eqs(&rt["identity"]["vllm"], "0.9.0-observed"));
    let denied = authorize(repo, &pj(&man3), "hint_finalize").err();
    c.ck("★관측이 identity 와 다르면 게이트 거부(원문 JSON · 종료코드)", denied.as_ref().map_or(false, |e| e.code == "HINT_GATE_DENIED"
        && e.message.contains("RUNTIME_IDENTITY_MISMATCH") && e.exit_code != 0));
    pm(&mut idx, &["meta"]).set("vllm_version", "0.9.0");
    idx.set("generated_utc", "2026-01-09T00:00:00Z");
    wj(&sweep.join("sweep_index.json"), &idx)?;
    let ev3 = fc(repo, "c1-a", None, Some(dk))?;
    c.ck("★generated_utc ≠ measured_utc = 스윕 미바인딩", ev3["sweep"].is_null() && inn(&ev3["missing"], "HINT_MISSING_SWEEP"));
    c.ck("★스윕 미바인딩 → 자격 관측 불가 차단", _code(qualification(&ev3))?.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    let spp = repo.join("output/multi/benchlog/serve_proof_c1-a.json");
    let sp_base = jd! {"schema_version" => 1, "kind" => "multinode_serve_proof", "config" => "c1-a", "topology" => "multi", "ready" => true,
        "health_http" => 200, "inference_verdict" => "pass", "evidence" => "chat.content", "content_len" => 12,
        "completion_text_len" => J::Null, "image_tag" => "fixture:0.9.0-source", "provenance" => "measured(multinode_serve_smoke)"};
    wj(&spp, &sp_base)?;
    c.ck("serve_proof 가 1순위 자격", eqs(&qualification(&fc(repo, "c1-a", None, Some(dk))?)?["method"], "serve_proof"));
    wj(&spp, &rep(&sp_base, &[("evidence", J::from("v1.completions")), ("content_len", J::Int(0)), ("completion_text_len", J::Int(5))]))?;
    c.ck("serve_proof — /v1/completions 로 확증한 pass 도 자격(chat content_len=0)", eqs(&qualification(&fc(repo, "c1-a", None, Some(dk))?)?["method"], "serve_proof"));
    let nroot = repo.join("native-evidence");
    mkd(&nroot)?;
    wr0(&nroot.join("native-install.sh"), "python -m pip install vllm\n")?;
    wr0(&nroot.join("pip-freeze-main.txt"), "vllm==0.9.0\n")?;
    wr0(&nroot.join("pip-freeze-sub.txt"), "vllm==0.9.0\n")?;
    let cleanup = nroot.join("cleanup.json");
    let node_ok = jd! {"root_absent" => true, "owned_processes" => J::list(), "owned_gpu_processes" => J::list(), "unknown" => J::list()};
    let clean_base = jd! {"schema_version" => 1, "kind" => "native_multinode_cleanup_attestation", "plane" => "native",
        "run_id" => "native-fixture", "status" => "PASS", "errors" => J::list(), "evidence_preserved" => true,
        "nodes" => jd! {"main" => node_ok.clone(), "sub" => node_ok}};
    wj(&cleanup, &clean_base)?;
    let native_base = jd! {"schema_version" => 1, "kind" => "native_multinode_serve_proof", "plane" => "native", "status" => "PASS",
        "cell" => "c1-a", "topology" => "multi", "run_id" => "native-fixture",
        "health" => jd! {"ok" => true}, "inference" => jd! {"ok" => true, "completion_text_len" => 3},
        "endpoints" => jd! {"health" => jd! {"http_status" => 200}, "inference" => jd! {"http_status" => 200}},
        "native_install_path" => "native-evidence/native-install.sh",
        "pip_freeze_paths" => jd! {"main" => "native-evidence/pip-freeze-main.txt", "sub" => "native-evidence/pip-freeze-sub.txt"},
        "cleanup_attestation_path" => "native-evidence/cleanup.json"};
    wj(&spp, &native_base)?;
    let native_ev = fc(repo, "c1-a", None, Some(dk))?;
    c.ck("native proof는 explicit plane을 CellEvidence로 전달", eqs(&native_ev["plane"], "native")
        && eqs(&native_ev["native_install_path"], "native-evidence/native-install.sh") && eqs(&native_ev["cleanup_attestation_path"], "native-evidence/cleanup.json"));
    c.ck("native proof + cleanup PASS가 자격", eqs(&qualification(&native_ev)?["method"], "native_serve_proof+cleanup_attestation"));
    c.ck("native 기동 기록: 옛 proof(필드 없음) = None · 사유 없음", native_ev["native_launch_path"].is_null()
        && native_ev["native_launch_error"].is_null() && has(&native_ev["sources"]["native_launch_path"], "옛 proof"));
    wr0(&nroot.join("native-launch.md"), "# native 기동 기록\n")?;
    wj(&spp, &rep(&native_base, &[("native_launch_path", J::from("native-evidence/native-launch.md"))]))?;
    let nl_ev = fc(repo, "c1-a", None, Some(dk))?;
    c.ck("native 기동 기록: proof native_launch_path → CellEvidence(저장소 상대) · artifacts 가 serve_proof 로도 읽는다",
        eqs(&nl_ev["native_launch_path"], "native-evidence/native-launch.md") && nl_ev["native_launch_error"].is_null()
        && eqs(&nl_ev["serve_proof"]["native_launch_path"], "native-evidence/native-launch.md"));
    wj(&spp, &rep(&native_base, &[("native_launch_error", J::from("OSError: disk full"))]))?;
    let ne_ev = fc(repo, "c1-a", None, Some(dk))?;
    c.ck("native 기동 기록 부재 + producer 사유 = native_launch_error 로 옮긴다(결손 사유)",
        ne_ev["native_launch_path"].is_null() && eqs(&ne_ev["native_launch_error"], "OSError: disk full") && has(&ne_ev["sources"]["native_launch_path"], "disk full"));
    wj(&spp, &rep(&native_base, &[("native_launch_path", J::from("/abs/native-launch.md"))]))?;
    c.ck("★native 기동 기록 절대경로 = HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE(보존 파일 계약)",
        _code(fc(repo, "c1-a", None, Some(dk)))?.as_deref() == Some("HINT_NATIVE_EVIDENCE_PATH_ABSOLUTE"));
    wj(&spp, &native_base)?;
    wj(&cleanup, &rep(&clean_base, &[("status", J::from("FAIL"))]))?;
    c.ck("★native cleanup nonpass 차단", _code(fc(repo, "c1-a", None, Some(dk)))?.as_deref() == Some("HINT_NATIVE_CLEANUP_NONPASS"));
    wj(&cleanup, &clean_base)?;
    wj(&spp, &without(&native_base, "cleanup_attestation_path"))?;
    c.ck("★native cleanup signal 없음 차단", _code(fc(repo, "c1-a", None, Some(dk)))?.as_deref() == Some("HINT_NATIVE_EVIDENCE_MISSING"));
    wj(&spp, &sp_base)?;
    let patches: Vec<(J, &str)> = vec![
        (jd! {"inference_verdict" => "fail", "evidence" => J::Null, "content_len" => 0}, "빈 응답 fail"),
        (jd! {"inference_verdict" => "relaxed-reasoning-only", "evidence" => "chat.reasoning(relaxed)", "content_len" => 0, "reasoning_len" => 40}, "완화 PASS(reasoning-only)"),
        (jd! {"inference_verdict" => "not-attempted", "ready" => false, "health_http" => J::Null, "evidence" => J::Null, "content_len" => J::Null}, "READY 미도달"),
        (jd! {"image_tag" => "fixture:0.8.0-source"}, "다른 이미지 태그의 스모크"),
        (jd! {"inference_verdict" => "passed-somehow"}, "모르는 판정어"),
    ];
    for (patch, what) in patches {
        let mut d = sp_base.clone();
        for (k, v) in patch.as_dict().unwrap().iter() {
            d.set(k, v.clone());
        }
        wj(&spp, &d)?;
        c.ck(&format!("★serve_proof 불성립 — {what}"), _code(qualification(&fc(repo, "c1-a", None, Some(dk))?))?.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    }
    c.ck("★재생 모드 쓰기 거부", _code(drive_publisher(repo, &replay_ns, &J::from("t"), &g5, &draft))?.as_deref() == Some("HINT_REPLAY_READ_ONLY"));
    // ── from_publication ──
    let evp = from_publication_r(repo, &jt, &J::Null, Some(dk))?;
    c.ck("재생 — 셀은 simlog 사본 meta 에서", eqs(&evp["cell"], "c1-a") && eqs(&evp["mode"], "publication-replay"));
    c.ck("재생 — 출력 스윕이 덮였으면 simlog 사본만", eqs(&evp["sweep"]["binding"], "simlog-copy")
        && inn(&evp["missing"], "HINT_MISSING_SWEEP_RAW") && inn(&evp["missing"], "HINT_MISSING_CAMPAIGN_INSTANCE"));
    c.ck("재생 — 계보 시드 this_topic", eqs(&evp["lineage_seeds"]["this_topic"], &topic));
    c.ck("★재생 — simlog 사본만 묶이면 roofline 도 없다 → 타겟 미관측(파생 불가)", !inn(&_arch_facts(repo, &evp)?, "target_gpu"));
    {
        let gtd = core::TempDir::new("hint-ev-tgt-");
        let g = gtd.path();
        let ident = ["-c", "user.name=fx", "-c", "user.email=fx\u{40}fixture.invalid"];
        core::git(g, &["init", "-q"])?;
        let mut sha_of: Vec<(String, String)> = Vec::new();
        for tv in ["native", "sim-h100"] {
            wj(&g.join("PAYLOAD.json"), &jd! {"naming" => jd! {"axes" => jd! {"target" => jd! {"value" => tv, "source" => "fx"}}}})?;
            core::git(g, &["add", "PAYLOAD.json"])?;
            let mut a: Vec<&str> = ident.to_vec();
            a.extend(["commit", "-q", "-m", tv]);
            core::git(g, &a)?;
            sha_of.push((tv.into(), core::git(g, &["rev-parse", "HEAD"])?.stdout.trim().to_string()));
        }
        let ev_ = |anchor: &str| jd! {"cell_config" => J::Null, "declaration" => J::Null, "mode" => "publication-replay", "sweep" => J::Null,
            "publication" => jd! {"record" => jd! {"promotion_target" => jd! {"anchor" => anchor}}}};
        let (t0, s0) = _target_gpu(&ev_(&sha_of[0].1), Some(g))?;
        c.ck("재생 — 봉인된 페이로드 target=native → 선언 없음(출처가 봉인 페이로드를 말한다)", t0.is_none() && s0.as_deref().map_or(false, |s| s.contains("봉인된 발행 페이로드")));
        c.ck("★재생 — 봉인된 페이로드 target=sim-* 는 원문 hw 를 되살리지 않는다(대체 ✗)", _target_gpu(&ev_(&sha_of[1].1), Some(g))? == (None, None));
        c.ck("★재생 — repo 없이는 대체 ✗", _target_gpu(&ev_(&sha_of[0].1), None)? == (None, None));
        wj(&g.join("PAYLOAD.json"), &jd! {"campaign" => jd! {"id" => "camp-fx", "cell" => "c1-a", "node" => "cluster", "mode" => "campaign"},
            "approval" => jd! {"approved_by" => "사용자(fx)", "approved_utc" => "2026-01-01T00:00:00Z", "source" => "campaign:hint_targets"}})?;
        core::git(g, &["add", "PAYLOAD.json"])?;
        let mut a: Vec<&str> = ident.to_vec();
        a.extend(["commit", "-q", "-m", "camp"]);
        core::git(g, &a)?;
        let a4 = core::git(g, &["rev-parse", "HEAD"])?.stdout.trim().to_string();
        let ev4 = |anchor: &str, cell: &str, mode: &str| jd! {"mode" => mode, "cell" => cell, "node" => "cluster",
            "publication" => jd! {"record" => jd! {"promotion_target" => jd! {"anchor" => anchor}}}};
        let sp4 = sealed_publication(g, &ev4(&a4, "c1-a", "publication-replay"))?;
        c.ck("F4 재생 — 봉인 페이로드의 campaign.id · approval 을 출처와 함께(재생 전용 대체)", sp4.as_ref().map_or(false, |s| eqs(&s["campaign"]["id"], "camp-fx")
            && eqs(&s["approval"]["approved_utc"], "2026-01-01T00:00:00Z") && has(&s["source"], "재생 전용 대체") && has(&s["source"], &a4[..12])));
        c.ck("★F4 음성대조: 다른 셀의 봉인 기록 · 캠페인 모드 · 캠페인 없는 봉인 페이로드는 대체 ✗",
            sealed_publication(g, &ev4(&a4, "other", "publication-replay"))?.is_none()
            && sealed_publication(g, &ev4(&a4, "c1-a", "campaign"))?.is_none()
            && sealed_publication(g, &ev4(&sha_of[0].1, "c1-a", "publication-replay"))?.map_or(J::Null, |s| s["campaign"].clone()).is_null());
    }
    idx.set("generated_utc", _M_UTC);
    wj(&sweep.join("sweep_index.json"), &idx)?;
    let arp = _arch_facts(repo, &from_publication_r(repo, &jt, &J::Null, Some(dk))?)?;
    c.ck("재생 — 출력 스윕이 묶이면 타겟은 roofline 대체이고 출처가 그렇게 말한다(선언 아님)", eqs(&arp["target_gpu"], "NVIDIA GB10") && has(&arp["source"], "재생 전용"));
    idx.set("generated_utc", "2026-01-09T00:00:00Z");
    wj(&sweep.join("sweep_index.json"), &idx)?;
    // ── phase_set_publish ──
    let calls: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
    let runner = |a: &[String]| -> HR<Proc> {
        calls.borrow_mut().push(a.to_vec());
        Ok(Proc { returncode: 0, stdout: String::new(), stderr: String::new() })
    };
    let bsha = "b".repeat(40);
    let (jc1, jca, jcl, g8) = (J::from("c1"), J::from("c1-a"), J::from("cluster"), J::from("2026-01-02T08:00:00Z"));
    phase_set_publish_with(repo, &jc1, &jca, &jcl, &jtag, &J::from(bsha.as_str()), &g8, Some(&runner))?;
    let a = calls.borrow().first().cloned().unwrap_or_default();
    let ni = a.iter().position(|x| x == "--node");
    c.ck("publish 위상 — 명시 --campaign-id · 배정 노드 · 원격 SHA 출처 · ★시작 시각을 지어내지 않는다",
        a.len() >= 2 && a[0] == "--campaign-id" && a[1] == "c1" && ni.and_then(|i| a.get(i + 1)).map(String::as_str) == Some("main")
        && a.contains(&format!("refs/tags/{tag}@{bsha}")) && !a.iter().any(|x| x == "--started-utc"));
    phase_set_publish_with(repo, &jc1, &jca, &jcl, &jtag, &J::from(bsha.as_str()), &g8, None)?;
    let pdoc = _read_json_opt(&repo.join("campaigns/c1/phases/main/publish.status.json"), "publish status")?;
    c.ck("publish 위상 — 실물 campaign_init CLI(격리 사본)가 진행표를 쓴다",
        eqs(&pdoc["state"], "done") && eqs(&pdoc["proof"]["source"], &format!("refs/tags/{tag}@{bsha}")) && eqs(&pdoc["cell_id"], "c1-a")
        && eqs(&pdoc["ended_utc"], "2026-01-02T08:00:00Z") && pdoc["started_utc"].is_null() && eqs(&pdoc["authored_by"], "main"));
    let noop = |_a: &[String]| -> HR<Proc> { Ok(Proc { returncode: 0, stdout: String::new(), stderr: String::new() }) };
    c.ck("★40자 아닌 원격 SHA 거부", _code(phase_set_publish_with(repo, &jc1, &jca, &jcl, &jtag, &J::from("abc"), &g8, Some(&noop)))?.as_deref() == Some("HINT_REMOTE_SHA_INVALID"));
    st_main3(c, repo, td, fx, dk, &idx)
}

fn st_main3(c: &mut Ck, repo: &Path, td: &Path, fx: &J, dk: Dk, idx: &J) -> HR<()> {
    let _ = td;
    let out = repo.join("output/multi");
    wr0(&out.join("configs/c1-b.yaml"), "model: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 2048\n")?;
    wr0(&out.join("configs/c1-b.sh"), "vllm serve\n")?;
    wr0(&out.join("envs/.env.c1-b"), "IMAGE_TAG=fixture:0.9.0-source\n")?;
    let lite_rel = "docs/benchmark/bench_report_26010214_fixture-model-nvfp4_GB10_0.9.0.md";
    let _ = _rbs(repo)?;
    use crate::render_bench_section as rbs;
    wr0(&repo.join(lite_rel), &[
        "# 경량", "", rbs::REPORT_LITE_MODE_LINE, "", "> 생성일 2026-01-02T14:00:00Z.", "", rbs::MEASUREMENT_CONFIG_TITLE,
        "", "| 키 | 값 |", "|---|---|", "| bench_mode | lite |", "| bench_mode_kind | declared-lite |", "| repeats | 1 |", "",
        "## lite 지표 (서빙 성공 직후)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
        "| gen tokens/sec (warm) [master] | 9.00 t/s | — |", "", "## 측정 환경 스냅샷", "", "| 키 | 값 |", "|---|---|",
        "| model | fixture-model-nvfp4 |", "| gpu_model | NVIDIA GB10 |", "| vllm_version | 0.9.0 |", "| topology | multi |", ""].join("\n"))?;
    let ptrs_p = repo.join("campaigns/c1/evidence_pointers.json");
    let mut ptrs = rj(&ptrs_p)?;
    pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => "bench_report", "path" => lite_rel, "cell_id" => "c1-b", "node_id" => "sub"});
    wj(&ptrs_p, &ptrs)?;
    let stale = out.join("benchlog/sweep_c1-b");
    mkd(&stale)?;
    wj(&stale.join("sweep_index.json"), &jd! {"generated_utc" => "2025-01-01T00:00:00Z", "meta" => J::dict()})?;
    let eva = fc(repo, "c1-b", None, Some(dk))?;
    c.ck("★lite 셀 — `--node` 미지정 + 측정 TP 2 > 노드당 GPU 1 → 자동 cluster(배정 sub 를 이긴다)",
        eqs(&eva["node"], "cluster") && has(&eva["sources"]["node"], "자동 파생") && has(&eva["sources"]["node"], "선언"));
    let evl = fc(repo, "c1-b", Some("sub"), Some(dk))?;
    c.ck("★lite 셀은 옛 스윕을 묶지 않고 스윕 결손도 적지 않는다", evl["sweep"].is_null() && !inn(&evl["missing"], "HINT_MISSING_SWEEP"));
    c.ck("★멀티 체크아웃의 단일 노드(sub) 실행은 노드 정합 대상 아님", !inn(&evl["missing"], "HINT_MISSING_SLAVE_ATTESTATION") && evl["attestation"].is_null());
    c.ck("lite 셀 — 노드 축 = 명시(sub) · report_kind lite", eqs(&evl["node"], "sub") && eqs(&evl["report_kind"], "lite"));
    c.ck("lite 셀 — BENCH_MODE_LITE 기재 · ★인증서 결손 아님(비발행 판정)", inn(&evl["missing"], "BENCH_MODE_LITE")
        && !inn(&evl["missing"], "HINT_MISSING_CERTIFICATE") && !inn(&evl["missing"], "HINT_MISSING_LITE") && has(&evl["sources"]["certificate_due"], "lite-only"));
    let ml = measurement(&evl)?;
    c.ck("lite 셀 measurement — verdict OBSERVATION-ONLY · lite 수치(리포트 표) · 측정 시각 = 리포트 생성일",
        eqs(&ml["verdict"], "OBSERVATION-ONLY") && eqs(&ml["benchmark_mode"], "lite") && peq(&ml["lite"]["gen_tps"], &J::Float(9.0))
        && eqs(&ml["measured_utc"], "2026-01-02T14:00:00Z") && ml["decode_tps_conc1"].is_null() && has(&ml["sources"]["verdict"], "lite-only"));
    c.ck("★lite_warm 없으면 자격 차단", _code(qualification(&evl))?.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    let warm = out.join("benchlog/lite_warm_c1-b.json");
    wj(&warm, &jd! {"completed" => 3, "failed" => 0})?;
    c.ck("★조인 키(lite_raw) 없는 lite_warm 은 자격이 아니다", _code(qualification(&evl))?.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    let rawp = out.join("benchlog/lite_raw_c1-b.json");
    wj(&rawp, &jd! {"config_name" => "c1-b", "measured_utc" => "2025-12-31T00:00:00Z", "bench_warm_json" => core::posix(&warm)})?;
    c.ck("★다른 실행의 lite_raw(measured_utc ≠ 리포트 생성일)는 자격이 아니다", _code(qualification(&evl))?.as_deref() == Some("HINT_QUALIFICATION_UNOBSERVED"));
    wj(&rawp, &jd! {"config_name" => "c1-b", "measured_utc" => "2026-01-02T14:00:00Z", "bench_warm_json" => core::posix(&warm)})?;
    c.ck("lite 자격 = lite_raw 조인(measured_utc = 생성일) + lite_warm", sw_(&qualification(&evl)?["method"], "lite_raw+lite_warm"));
    c.ck("★TP=2 측정을 단일 노드(sub) 형상으로 이름 짓지 않는다", _code(naming_facts(repo, &evl))?.as_deref() == Some("HINT_ARCH_TP_CONFLICT"));
    c.ck("같은 셀을 cluster 축으로 발행하면 2노드 형상", _arch_facts(repo, &fc(repo, "c1-b", Some("cluster"), Some(dk))?)?["nodes"] == J::Int(2));
    let before_l = _tree(repo)?;
    let d3 = repo.join("hints/.drafts/d3");
    let g15 = J::from("2026-01-02T15:00:00Z");
    c.ck("★셀 서사 증거가 없으면 쓰기 전에 차단", _code(drive_publisher(repo, &evl, &J::from(topic_for(&evl)?), &g15, &d3))?.as_deref() == Some("HINT_NARRATIVE_EVIDENCE_ABSENT")
        && _tree(repo)? == before_l);
    for (kind, rel_doc) in [("devlog", "docs/devlog/devlog_26010215_lite.md"), ("testlog", "docs/testlog/testlog_26010215_lite.md")] {
        wr0(&repo.join(rel_doc), &format!("# {kind}\nlite 셀 서사.\n"))?;
        pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => kind, "path" => rel_doc, "cell_id" => "c1-b", "node_id" => "sub"});
    }
    wj(&ptrs_p, &ptrs)?;
    let evl = fc(repo, "c1-b", Some("sub"), Some(dk))?;
    let manl = drive_publisher(repo, &evl, &J::from(topic_for(&evl)?), &g15, &d3)?;
    let mdl = rj(&manl)?;
    c.ck("lite 발행 = hint_map_only + 경량 리포트 바인딩", eqs(&mdl["task_class"], "hint_map_only") && eqs(&mdl["benchmark"]["mode"], "lite")
        && mdl["evidence"]["bench_report"]["path"].truthy());
    c.ck("★lite 등급 인증서 없는 lite 셀 → 게이트 거부(HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE)", _code(authorize(repo, &pj(&manl), "hint_finalize"))?.as_deref() == Some("HINT_GATE_DENIED"));
    let lite_cert_rel = "docs/benchmark/benchmark_26010214_fixture-model-nvfp4_GB10_0.9.0.yaml";
    wr0(&repo.join(lite_cert_rel), "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: not_applicable\n\
        model: fixture-model-nvfp4\ngpu_model: NVIDIA GB10\nvllm_version: 0.9.0\nquantization: N/A\n\
        topology: multi\ntensor_parallel_size: 2\nbenchmark_mode: lite\nlite_verdict: pass\n\
        entry_path: beta_lite_only_cell\nlite_included: true\nlite_gen_tps_warm: 9.0\n\
        measured_utc: \"2026-01-02T14:00:00Z\"\nmeasured_node: sub\n")?;
    pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => "certificate", "path" => lite_cert_rel, "cell_id" => "c1-b", "node_id" => "sub"});
    wj(&ptrs_p, &ptrs)?;
    let evl = fc(repo, "c1-b", Some("sub"), Some(dk))?;
    let ml2 = measurement(&evl)?;
    c.ck("lite 등급 인증서 셀 measurement — verdict OBSERVATION-ONLY(not_applicable 을 판정 칸에 옮기지 않는다) · mode lite",
        eqs(&ml2["verdict"], "OBSERVATION-ONLY") && eqs(&ml2["benchmark_mode"], "lite") && evl["sweep"].is_null());
    let manl = drive_publisher(repo, &evl, &J::from(topic_for(&evl)?), &g15, &d3)?;
    let mdl = rj(&manl)?;
    c.ck("lite 발행(β) = 경량 리포트 + lite 등급 인증서 바인딩", ew_(&mdl["evidence"]["certificate"]["path"], "benchmark_26010214_fixture-model-nvfp4_GB10_0.9.0.yaml"));
    c.ck("lite 발행 게이트 허가(lite 통과)", authorize(repo, &pj(&manl), "hint_finalize")?["allowed"] == J::Bool(true));
    // ── 강등 셀 ──
    let decl_p = repo.join("campaigns/c1/campaign.yaml");
    let mut decl_d = fx["decl"].clone();
    pm(&mut decl_d, &["assignments", "main"]).push(jd! {"cell" => "c1-d", "mode" => "AUTO"});
    wj(&decl_p, &decl_d)?;
    for suffix in ["configs/{}.yaml", "configs/{}.sh", "envs/.env.{}"] {
        let src_t = rd(&out.join(suffix.replace("{}", "c1-a")))?;
        wr0(&out.join(suffix.replace("{}", "c1-d")), &src_t.replace("c1-a", "c1-d"))?;
    }
    let d_utc = "2026-01-03T01:00:00Z";
    let sw_d = out.join("benchlog/sweep_c1-d");
    mkd(&sw_d.join("level_01"))?;
    let mut idx_d = idx.clone();
    idx_d.set("config", "c1-d");
    idx_d.set("generated_utc", d_utc);
    pm(&mut idx_d, &["meta"]).set("config_name", "c1-d");
    pm(&mut idx_d, &["meta"]).set("vllm_version", "0.9.0");
    wj(&sw_d.join("sweep_index.json"), &idx_d)?;
    wj(&sw_d.join("level_01/measured.json"), &jd! {"decode_tps" => 12.0, "measurement_ok" => true})?;
    wj(&sw_d.join("level_01/bench_c1-d.json"), &jd! {"completed" => 4, "failed" => 0})?;
    wj(&sw_d.join("level_01/post_health_c1-d.json"), &jd! {"health_http_code" => "200", "container_running" => "true", "container_oom_killed" => "false"})?;
    wj(&sw_d.join("verdict.json"), &jd! {"verdict" => "PASS"})?;
    let rep_d = "docs/benchmark/bench_report_26010310_fixture-model-nvfp4_GB10_0.9.0.md";
    let op_path = format!("/{}/op-fixture/ws/docs/logs/main/events/2026-01.jsonl", "home");
    let bms = format!("| bench_mode_source | derived(classify_cell · events {op_path}) |");
    let drs = format!("| downgrade_reason_source | events({op_path}) |");
    let gen_d = format!("> 생성일 {d_utc}.");
    wr0(&repo.join(rep_d), &[
        "# 성능 보고서", "", gen_d.as_str(), "", rbs::MEASUREMENT_CONFIG_TITLE, "", "| 키 | 값 |", "|---|---|",
        "| bench_mode | lite |", "| bench_mode_kind | downgraded-lite |", bms.as_str(),
        "| downgrade_reason | run_failed |", drs.as_str(), "| repeats | 3 |", "",
        "## lite 지표 (full ⊇ lite)", "", "| 메트릭 | Main | Sub |", "|---|---|---|",
        "| gen tokens/sec (warm) [master] | 10.00 t/s | — |", "",
        "## 부하 스윕 곡선", "", rbs::REPORT_HEADER, "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 12.0 | 12 | 60 | 100 | 20 | 4/0 |", "| 2 | 20.0 | 20 | 90 | 120 | 25 | 4/0 |", ""].join("\n"))?;
    for (kind, rel_doc) in [("devlog", "docs/devlog/devlog_26010311_down.md"), ("testlog", "docs/testlog/testlog_26010311_down.md")] {
        wr0(&repo.join(rel_doc), &format!("# {kind}\n강등 셀 서사.\n"))?;
        pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => kind, "path" => rel_doc, "cell_id" => "c1-d", "node_id" => "main"});
    }
    pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => "bench_report", "path" => rep_d, "cell_id" => "c1-d", "node_id" => "main"});
    let cert_d = "docs/benchmark/benchmark_26010310_fixture-model-nvfp4_GB10_0.9.0.yaml";
    wr0(&repo.join(cert_d), &format!("schema_version: 1\nrecord_type: benchmark_certificate\nverdict: not_applicable\n\
        model: fixture-model-nvfp4\ngpu_model: NVIDIA GB10\nvllm_version: 0.9.0\nquantization: N/A\n\
        topology: multi\ntensor_parallel_size: 2\nbenchmark_mode: lite\nlite_verdict: pass\n\
        entry_path: gamma_demoted_lite\nlite_included: true\nlite_gen_tps_warm: 10.0\n\
        measured_utc: \"{d_utc}\"\nmeasured_node: cluster\n"))?;
    pm(&mut ptrs, &["pointers"]).push(jd! {"kind" => "certificate", "path" => cert_d, "cell_id" => "c1-d", "node_id" => "cluster"});
    wj(&ptrs_p, &ptrs)?;
    let evd = fc(repo, "c1-d", Some("cluster"), Some(dk))?;
    let mcd = measurement_config(repo, &evd)?;
    c.ck("강등 셀 — 측정 구성 표가 downgraded-lite 를 말한다 · BENCH_MODE_LITE 기재",
        eqs(&mcd["bench_mode_kind"], "downgraded-lite") && eqs(&mcd["downgrade_reason"], "run_failed") && inn(&evd["missing"], "BENCH_MODE_LITE"));
    c.ck("★PII 회귀 — 자유 서술 *_source 의 운영자 절대경로가 배포 평면(measurement_config·measurement)에 0건",
        !ud(&jl![mcd.clone(), measurement(&evd)?]).contains("op-fixture") && !keys(&mcd).iter().any(|k| k.ends_with("_source")));
    let dp0 = dataset_provenance(repo, &evd)?;
    c.ck("데이터셋 출처 — dataset_* 부재 = synthetic **부재 추론**(status inferred-synthetic · source 가 추론이라 말한다)",
        eqs(&dp0["dataset_kind"], "synthetic") && eqs(&dp0["status"], DATASET_STATUS_INFERRED) && sw_(&dp0["source"], "inferred(")
        && DATASET_KEYS.iter().all(|k| inn(&dp0, k)));
    let ds_host = format!("/{}/llm/dataset/abisee__cnn_dailymail/{}/test-00000-of-00001.parquet", "mnt", "a".repeat(40));
    let tok_host = format!("/{}/op-fixture/tok/granite", "home");
    let rep_ds = "docs/benchmark/bench_report_26010311_fixture-model-nvfp4_GB10_0.9.0.md";
    let l_rev = format!("| dataset_revision | {} |", "a".repeat(40));
    let l_sha = format!("| dataset_source_sha256 | {} |", "b".repeat(64));
    let l_tok = format!("| dataset_tokenizer | {tok_host} |");
    let l_col = format!("| dataset_column_map | {ds_host} |");
    wr0(&repo.join(rep_ds), &[
        "# 성능 보고서", "", gen_d.as_str(), "", rbs::MEASUREMENT_CONFIG_TITLE, "", "| 키 | 값 |", "|---|---|",
        "| bench_mode | full |", "| dataset_kind | parquet_file |", "| dataset_source | managed |",
        "| dataset_id | abisee/cnn_dailymail |", l_rev.as_str(), l_sha.as_str(), l_tok.as_str(), l_col.as_str(), "| dataset_seed | 42 |", ""].join("\n"))?;
    let dp1 = dataset_provenance(repo, &rep(&evd, &[("bench_report_path", J::from(rep_ds))]))?;
    c.ck("데이터셋 출처 — 리포트 표 우선 · status recorded · source = bench_report 측정 구성 표 · id · revision 원문",
        eqs(&dp1["status"], DATASET_STATUS_RECORDED) && has(&dp1["source"], "bench_report(") && eqs(&dp1["dataset_id"], "abisee/cnn_dailymail")
        && eqs(&dp1["dataset_revision"], &"a".repeat(40)) && eqs(&dp1["dataset_kind"], "parquet_file"));
    let dp1_txt = ud(&dp1);
    let scan = |t: &str| crate::pii::scan_text(t, Some(&[]), "deploy", &[], false, true, None);
    c.ck("★PII 음성대조 — 호스트 절대경로(/mnt · /home)는 이름만(`<abs-path>/<이름>`) · deploy 스캔 0건",
        !dp1_txt.contains("op-fixture") && !dp1_txt.contains(&format!("/{}/", "mnt")) && eqs(&dp1["dataset_tokenizer"], "<abs-path>/granite")
        && eqs(&dp1["dataset_column_map"], "<abs-path>/test-00000-of-00001.parquet") && scan(&dp1_txt)?.is_empty());
    c.ck("★음성대조 — 정화 전 원문은 deploy 스캔에 걸린다(위 0건이 공허하지 않다)", !scan(&ad(&jd! {"t" => tok_host.as_str(), "c" => ds_host.as_str()}))?.is_empty());
    let cert2 = rep(&evd["certificate"], &[("dataset_kind", J::from("parquet_file")), ("dataset_tokenizer", J::from("/app/models/IBM/granite")),
        ("dataset_seed", J::from("N/A")), ("dataset_provenance", J::from("measured(런별 사이드카 · 3 런)"))]);
    let dp2 = dataset_provenance(repo, &rep(&evd, &[("bench_report_path", J::Null), ("certificate", cert2)]))?;
    c.ck("데이터셋 출처 — 표 없으면 인증서 소프트 지문(N/A = 미기재) · /app 경로는 그대로 · writer 출처 문구 승계",
        eqs(&dp2["status"], DATASET_STATUS_RECORDED) && has(&dp2["source"], "certificate(") && eqs(&dp2["dataset_tokenizer"], "/app/models/IBM/granite")
        && dp2["dataset_seed"].is_null() && eqs(&dp2["dataset_provenance"], "measured(런별 사이드카 · 3 런)"));
    let d4 = repo.join("hints/.drafts/d4");
    let g2 = J::from("2026-01-03T02:00:00Z");
    let mand = drive_publisher(repo, &evd, &J::from(topic_for(&evd)?), &g2, &d4)?;
    let mdd = rj(&mand)?;
    let recd = _read_json_opt(&repo.join(core::REL_EVIDENCE_DIR).join(format!("{}.json", topic_for(&evd)?)), "강등 기록")?;
    c.ck("강등 셀 발행 = hint_map_only 재분류(사유 run_failed · 판정 보존 · 리포트 바인딩)",
        eqs(&mdd["task_class"], "hint_map_only") && eqs(&mdd["benchmark"]["mode"], "lite") && eqs(&recd["reclassification"]["reason"], "run_failed")
        && ew_(&mdd["evidence"]["bench_report"]["path"], "bench_report_26010310_fixture-model-nvfp4_GB10_0.9.0.md")
        && ew_(&mdd["evidence"]["certificate"]["path"], "benchmark_26010310_fixture-model-nvfp4_GB10_0.9.0.yaml"));
    c.ck("강등 셀 발행 게이트 허가", authorize(repo, &pj(&mand), "hint_finalize")?["allowed"] == J::Bool(true));
    c.ck("강등 셀 재실행은 멱등(같은 manifest · 재분류 재시도 ✗)", drive_publisher(repo, &evd, &J::from(topic_for(&evd)?), &g2, &d4)? == mand);
    wj(&decl_p, &fx["decl"])?;
    // 결손 코드 tripwire — 이 크레이트 Rust 소스가 실제로 발행하는 코드(`missing.push("…")` · `"HINT_MISSING_…"` 리터럴)는 전부 이 표에.
    let mut emitted: Vec<String> = Vec::new();
    let src_root = core::skill_dir().join("rust/src");
    for f in rglob(&src_root) {
        if f.extension().and_then(|x| x.to_str()) != Some("rs") || f.file_name().and_then(|x| x.to_str()) == Some("selftest.rs") && f.parent().map_or(false, |p| p.ends_with("evidence")) {
            continue;
        }
        let t = rd(&f)?;
        // Python: `missing\.append\("([A-Z_]+)"\)` · `\["(HINT_MISSING_[A-Z_]+)"\]` — Rust: `missing.push("X"` · one-element `["HINT_MISSING_X"]`.
        for (pfx, sfx) in [("missing.push(\"", "\""), ("[\"HINT_MISSING_", "\"]")] {
            let mut rest = t.as_str();
            while let Some(i) = rest.find(pfx) {
                let after = &rest[i + pfx.len()..];
                let end = after.find('"').unwrap_or(after.len());
                let body = &after[..end];
                let ok_tail = after[end..].starts_with(sfx);
                let code = if pfx.starts_with('[') { format!("HINT_MISSING_{body}") } else { body.to_string() };
                if ok_tail && !body.is_empty() && code.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_') && !emitted.contains(&code) {
                    emitted.push(code);
                }
                rest = after;
            }
        }
    }
    emitted.sort();
    let codes = MISSING_CODES();
    let unreg: Vec<String> = emitted.iter().filter(|x| codes.get(x).is_none()).cloned().collect();
    c.ck(&format!("결손 코드 tripwire — 발행되는 코드 전부 등재(미등재 {})", J::from(unreg.clone()).repr()),
        emitted.iter().any(|x| x == "HINT_MISSING_APPLIED_PATCH_FILE") && emitted.iter().any(|x| x == "HINT_MISSING_ENV_SHAPE") && unreg.is_empty());
    let mut leaked: Vec<String> = Vec::new();
    for (k, m) in codes.as_dict().map(|m| m.iter().map(|(a, b)| (a.to_string(), b.clone())).collect::<Vec<_>>()).unwrap_or_default() {
        if !_internal_doc_refs(&m.display())?.is_empty() {
            leaked.push(k);
        }
    }
    leaked.sort();
    c.ck(&format!("결손 코드 뜻 문구에 수신자가 풀 수 없는 내부 좌표 없음({})", J::from(leaked.clone()).repr()), leaked.is_empty());
    c.ck("★음성대조: 옛 ATTESTATION_PARITY_ROWS 문구(코드맵 campaign_plane TL;DR 6)는 내부 좌표로 잡힌다",
        !_internal_doc_refs("대조 행이 없던 스모크 판본 · 코드맵 campaign_plane TL;DR 6")?.is_empty());
    Ok(())
}

fn kinds<'a>(tl: &'a [J], k: &str) -> Vec<&'a J> {
    tl.iter().filter(|r| eqs(&r["kind"], k)).collect()
}

fn _selftest_round2(c: &mut Ck, repo: &Path, td: &Path, ev: &J, ident: &J) -> HR<()> {
    let tds = core::posix(td);
    let hs = &ident["source"]["hf_revision"];
    c.ck("hf_revision = 체크포인트 HEAD → loose ref · origin 일치", eqs(&ident["hf_revision"], _SHA_H) && has(hs, "loose ref") && has(hs, "일치") && !has(hs, &tds));
    let gd = td.join("gitprobe");
    let cases: Vec<(&str, String, Vec<(&str, String)>, Option<&str>)> = vec![
        ("packed", "ref: refs/heads/dev\n".into(), vec![("packed-refs", format!("{_SHA_H} refs/heads/dev\n"))], Some(_SHA_H)),
        ("detached", format!("{_SHA_A}\n"), vec![], Some(_SHA_A)),
        ("dangling", "ref: refs/heads/none\n".into(), vec![], None),
        ("traversal", "ref: refs/../../escape\n".into(), vec![("../escape", format!("{_SHA_A}\n"))], None),
    ];
    for (name, head, files, want) in cases {
        let g = gd.join(name).join(".git");
        mkd(&g.join("refs"))?;
        wr0(&g.join("HEAD"), &head)?;
        for (rel_f, body) in files {
            wr(&g.join(rel_f), &body)?;
        }
        let (got, why) = _hf_revision(&gd.join(name), "L", None)?;
        let w60: String = why.chars().take(60).collect();
        c.ck(&format!("{}hf_revision 해소 — {name}({w60})", if want.is_none() { "★" } else { "" }), got.as_deref() == want);
    }
    c.ck("★.git 부재 = revision 미관측(None · 지어내지 않는다)", _hf_revision(&td.join("no-such-ckpt"), "L", None)?.0.is_none());
    let fx_mail = format!("op{}fixture.invalid", "\u{40}");
    let rl: Vec<(&str, Option<Vec<(&str, &str, i64)>>, Option<&str>, String)> = vec![
        ("rl-same", Some(vec![(_SHA0, _SHA_H, 1767139200)]), Some(_SHA_H), "측정 시점 checkout = 지금 HEAD".into()),
        ("rl-moved-same", Some(vec![(_SHA0, _SHA_H, 1767139200), (_SHA_H, _SHA_H, 1767398400)]), Some(_SHA_H), "측정 뒤 이동 1회".into()),
        ("rl-moved", Some(vec![(_SHA0, _SHA_A, 1767139200), (_SHA_A, _SHA_H, 1767398400)]), None, _SHA_A[..12].to_string()),
        ("rl-cloned-after", Some(vec![(_SHA0, _SHA_H, 1767398400)]), None, "측정 뒤 받아 온".into()),
        ("rl-none", None, Some(_SHA_H), "reflog(logs/HEAD) 부재".into()),
    ];
    for (name, lines, want, why_has) in rl {
        let g = gd.join(name).join(".git");
        mkd(&g.join("refs/heads"))?;
        wr0(&g.join("HEAD"), "ref: refs/heads/main\n")?;
        wr0(&g.join("refs/heads/main"), &format!("{_SHA_H}\n"))?;
        if let Some(ls) = &lines {
            mkd(&g.join("logs"))?;
            wr0(&g.join("logs/HEAD"), &ls.iter().map(|(a, b, t)| format!("{a} {b} Op Name <{fx_mail}> {t} +0900\tclone: from x\n")).collect::<String>())?;
        }
        let (got, why) = _hf_revision(&gd.join(name), "L", Some(_M_UTC))?;
        c.ck(&format!("{}hf_revision 측정 시점 대조 — {name}", if want.is_none() { "★" } else { "" }),
            got.as_deref() == want && why.contains(&why_has) && !why.contains(&fx_mail) && !why.contains("Op Name"));
    }
    let rg = gd.join("remote-diff/.git");
    mkd(&rg.join("refs/heads"))?;
    mkd(&rg.join("refs/remotes/origin"))?;
    wr0(&rg.join("HEAD"), "ref: refs/heads/main\n")?;
    wr0(&rg.join("refs/heads/main"), &format!("{_SHA_H}\n"))?;
    wr0(&rg.join("refs/remotes/origin/main"), &format!("{_SHA_A}\n"))?;
    let (got_rd, why_rd) = _hf_revision(&gd.join("remote-diff"), "L", None)?;
    c.ck("★origin 이 다른 커밋이면 '일치' 라 적지 않는다(로컬 커밋일 수 있다)", got_rd.as_deref() == Some(_SHA_H) && why_rd.contains("다른 커밋") && !why_rd.contains("와 일치"));
    let rm_ = _hf_revision(&gd.join("rl-moved"), "L", None)?;
    c.ck("★측정 시각이 없으면 값은 두되 '미검증' 을 적는다", rm_.0.as_deref() == Some(_SHA_H) && rm_.1.contains("미검증"));
    c.ck("identity.quant — 측정 N/A 면 이름 q 축(출처 naming-axis(q))", eqs(&ident["quant"], "nvfp4") && sw_(&ident["source"]["quant"], "naming-axis(q)"));
    let cert = if ev["certificate"].is_dict() { ev["certificate"].clone() } else { J::dict() };
    let i16 = identity(&rep(ev, &[("certificate", rep(&cert, &[("quantization", J::from("none")), ("quantization_source", J::from("measured(engine log)"))]))]))?;
    let i16n = identity(&rep(ev, &[("certificate", rep(&cert, &[("quantization", J::from("none"))]))]))?;
    c.ck("★F16 측정 quantization none(measured 출처) = 관측 표지 · '미관측' ✗ · 값(이름 q 축)은 그대로 · 출처 없는 none 은 종전(미관측)",
        eqs(&i16["quant"], "nvfp4") && has(&i16["source"]["quant"], QUANT_NONE_OBSERVED_MARK) && !has(&i16["source"]["quant"], "미관측(")
        && has(&i16["source"]["quant"], "measured(engine log)") && has(&i16n["source"]["quant"], "측정 identity quantization 미관측"));
    let iq = identity(&rep(ev, &[("certificate", rep(&cert, &[("quantization", J::from("fp8"))]))]))?;
    c.ck("★인증서가 quantization 을 들면 그 측정값(이름 축 대체 ✗)", eqs(&iq["quant"], "fp8") && !sw_(&iq["source"]["quant"], "naming-axis"));
    c.ck("★체크포인트 미관측이면 q 대체 없음(None)", identity(&rep(ev, &[("triplet", J::dict())]))?["quant"].is_null());
    let b = &ev["build_identity"];
    c.ck("★드라이버 원천이 없으면 None + 미관측(지어내지 않는다)", b["driver"].is_null() && has(&b["source"]["driver"], "미관측") && b["os"].is_null() && has(&b["source"]["os"], "미관측"));
    let meta = if ev["sweep"]["index"]["meta"].is_dict() { ev["sweep"]["index"]["meta"].clone() } else { J::dict() };
    let host = |cert_extra: &[(&str, J)], meta_extra: &[(&str, J)]| -> HR<J> {
        let mut sw = if ev["sweep"].is_dict() { ev["sweep"].clone() } else { J::dict() };
        let idx = rep(&sw["index"], &[("meta", rep(&meta, meta_extra))]);
        sw.set("index", idx);
        let mut e2 = rep(ev, &[("certificate", rep(&cert, cert_extra)), ("sweep", sw)]);
        _host_facts(&mut e2, None)?;
        Ok(e2["build_identity"].clone())
    };
    let h1 = host(&[("driver_version", J::from("N/A"))], &[("driver_version", J::from("580.1")), ("kernel_release", J::from("6.8.0-fx"))])?;
    c.ck("★인증서 N/A 는 건너뛰고 스윕 meta driver · OS 는 실제 키가 있을 때만", eqs(&h1["driver"], "580.1")
        && has(&h1["source"]["driver"], "sweep_index.meta") && eqs(&h1["os"], "6.8.0-fx") && has(&h1["source"]["os"], "kernel_release"));
    let h2 = host(&[("driver_version", J::from("999.9"))], &[("driver_version", J::from("580.1"))])?;
    c.ck("드라이버 우선순위 — 스윕 meta > 인증서(옮김) · 출처에 (manifest 선언 옮김)", eqs(&h2["driver"], "580.1")
        && sw_(&h2["source"]["driver"], "sweep_index.meta") && has(&h2["source"]["driver"], _DECLARED_COPY) && !inn(&h2, "driver_conflict"));
    // ── F6 원장 타임라인 ──
    let tl = event_timeline(repo, ev)?;
    let dump = ud(&J::List(tl.clone()));
    let decl = kinds(&tl, "budget_declare");
    let dd = |i: usize| decl.get(i).map(|r| st(&r["detail"])).unwrap_or_default();
    c.ck("타임라인 — 이 셀 라벨의 선언 창 2개(1차 renew 0 · 2차 renew 1 · 측정 포함)", decl.len() == 2
        && dd(0).contains("기동 창 1/2") && dd(0).contains("budget_renew 0회") && dd(0).contains("이 창 밖")
        && dd(1).contains("기동 창 2/2") && dd(1).contains("budget_renew 1회") && dd(1).contains("이 창 안"));
    c.ck("★FC-15 선언 행 머리 = '단 mem_total_mib 는 러너 측정'(그 칸이 있을 때만) · 칸이 없는 선언 행은 종전 머리",
        dd(1).contains("선언값 — 측정 아님 · 단 mem_total_mib 는 러너 측정") && dd(1).contains(_MEM_TOTAL_NOTE) && dd(0).contains("선언값 — 측정 아님)"));
    c.ck("타임라인 — 출처 = 파일:줄(`\\n` 기준)", decl.len() == 2 && eqs(&decl[0]["source"], "docs/logs/main/events/2026-01.jsonl:2")
        && eqs(&decl[1]["source"], "docs/logs/main/events/2026-01.jsonl:9"));
    let tl_early = event_timeline(repo, &rep(ev, &[("certificate", rep(&cert, &[("measured_utc", J::from("2026-01-02T00:30:00Z"))]))]))?;
    c.ck("★측정 창 밖 기동은 측정 전·뒤를 적는다(뒤 캠페인의 재기동이 이 측정의 여정으로 읽히지 않게)",
        dd(0).contains("측정 전)") && kinds(&tl_early, "budget_declare").iter().all(|r| has(&r["detail"], "측정 뒤 — 이 측정과 무관할 수 있다")));
    let pairs: Vec<(String, String)> = tl.iter().map(|r| (st(&r["kind"]), st(&r["utc"]))).collect();
    let need = [("budget_clear", "2026-01-02T01:30:00Z"), ("budget_clear", "2026-01-02T03:10:00Z"), ("budget_none", "2026-01-02T03:10:01Z"),
        ("watchdog_highrate_hold", "2026-01-02T02:31:00Z"), ("thermal_trip", "2026-01-02T02:45:00Z")];
    c.ck("라벨 없는 clear·arm 해제·보류·thermal 은 열린 창에 귀속", need.iter().all(|(k, u)| pairs.iter().any(|(a, b)| a == k && b == u)));
    let fd = _event_fields(&jd! {"kind" => "budget_declare", "source" => "blackbox_session", "floor_mib" => 12368, "weights_mib" => 73104, "mem_total_mib" => 124608})?;
    let fb = _event_fields(&jd! {"kind" => "budget_blocked", "source" => "blackbox_session", "mem_total_mib" => 124608})?;
    let bs_p = core::skill_dir().parent().unwrap().join("terraforming_node/scripts/node_blackbox/blackbox_session.py");
    let bs_src = rd(&bs_p)?;
    c.ck("★FC-15 budget_declare 의 mem_total_mib = 러너 측정 표지(선언값 아님) · floor·weights 는 그대로 · budget_blocked(자유 detail)는 종전 · 계약 원문(declare-budget --mem-total-mib help = /proc/meminfo MemTotal) 교차대조",
        fd.contains(&format!("mem_total_mib=124608({_MEM_TOTAL_NOTE})")) && fd.contains("floor_mib=12368 ·") && fd.contains("weights_mib=73104")
        && fb == "mem_total_mib=124608" && bs_src.contains("b.add_argument(\"--mem-total-mib\", type=int, required=True, help=\"/proc/meminfo MemTotal\")"));
    c.ck("★F4 라벨 없는 워치독 budget_rejected(창 닫힘 직전) = 열린 창에 귀속 · 행 라벨 None · 출처 watchdog.jsonl",
        tl.iter().any(|r| eqs(&r["kind"], "budget_rejected") && eqs(&r["utc"], "2026-01-02T03:10:00Z") && r["label"].is_null()
            && has(&r["detail"], "라벨 없음 → smoke-c1-a 선언 창 귀속") && has(&r["source"], "watchdog.jsonl")));
    c.ck("★접두만 같은 다른 셀 라벨(smoke-c1-a-kv8g)의 창과 그 창의 사살·clear·thermal 은 싣지 않는다",
        !dump.contains("kv8g") && !tl.iter().any(|r| eqs(&r["utc"], "2026-01-02T01:45:00Z") || eqs(&r["utc"], "2026-01-02T01:50:00Z")));
    c.ck("★nomatch 판정·다른 셀 세션·창 밖 pre-clear 는 싣지 않는다", !dump.contains("nomatch") && !dump.contains("c1-ab")
        && !tl.iter().any(|r| eqs(&r["kind"], "budget_clear") && eqs(&r["utc"], "2026-01-02T01:00:00Z")) && tl.iter().any(|r| eqs(&r["kind"], "serve_start")));
    c.ck("★운영자 경로 칸(decl_path)은 detail 로 새지 않는다(양성 목록)", !dump.contains("op-fixture") && !dump.contains("decl_path"));
    c.ck("★IPv4 모양 값은 detail 로 새지 않는다(값 필터)", !dump.contains(&["10", "1", "2", "3"].join("."))
        && tl.iter().any(|r| eqs(&r["utc"], "2026-01-02T02:52:00Z") && has(&r["detail"], "mem_avail_mib=7") && !has(&r["detail"], "target=")));
    c.ck("★label = 그 행의 라벨(라벨 없는 clear·보류·thermal 은 None · 창 귀속은 detail 이 말한다)",
        tl.iter().filter(|r| ["budget_clear", "budget_none", "watchdog_highrate_hold", "thermal_trip"].iter().any(|k| eqs(&r["kind"], k)))
            .all(|r| r["label"].is_null() && has(&r["detail"], "라벨 없음 →"))
        && decl.iter().all(|r| eqs(&r["label"], "smoke-c1-a")));
    let th: Vec<&J> = tl.iter().filter(|r| eqs(&r["kind"], "thermal_trip") && eqs(&r["utc"], "2026-01-02T02:45:00Z")).collect();
    c.ck("★다른 스트림 파일의 같은 사건(ts·kind)은 한 행으로 싣고 사본 출처를 적는다(월 원장 journald 사본)", th.len() == 1
        && eqs(&th[0]["source"], "docs/logs/main/events/2026-01.jsonl:14") && has(&th[0]["detail"], "thermal.jsonl:2"));
    c.ck("★같은 파일 안의 같은 (ts·kind) 두 행은 합치지 않는다(같은 초의 서로 다른 사건)",
        tl.iter().filter(|r| eqs(&r["kind"], "watchdog_highrate_hold") && eqs(&r["utc"], "2026-01-02T02:31:00Z")).count() == 2);
    c.ck("★다른 셀 라벨의 clear 는 이 셀 창을 닫지 않고 싣지도 않는다", !dump.contains("smoke-zz-other") && dd(1).contains("닫힘 budget_clear 2026-01-02T03:10:00Z"));
    c.ck("파손 줄은 판독 불가로 기재(부재 ≠ 실패)", tl.iter().any(|r| eqs(&r["kind"], "ledger_unparsable_lines") && has(&r["detail"], "1줄")));
    let us: Vec<String> = tl.iter().filter(|r| r["utc"].truthy()).map(|r| st(&r["utc"])).collect();
    let mut us_s = us.clone();
    us_s.sort();
    c.ck("타임라인 utc 순", us == us_s);
    // ── 캠페인 경계 ──
    let d0u = decl.first().map(|r| st(&r["utc"])).unwrap_or_default();
    let d1u = decl.get(1).map(|r| st(&r["utc"])).unwrap_or_default();
    let late_decl = rep(&ev["declaration"], &[("declared_utc", J::from("2026-01-02T23:00:00Z"))]);
    let evb = rep(ev, &[("mode", J::from("campaign")), ("declaration", late_decl.clone())]);
    let bnd = event_campaign_boundary(repo, &evb)?;
    let tlb = event_timeline(repo, &evb)?;
    let db = kinds(&tlb, "budget_declare");
    c.ck("캠페인 경계 = min(declared_utc, 측정 선언 창 시작) · 이전 선언 창 = 같은 셀 이름 · 앞선 캠페인(행은 남는다)",
        eqs(&bnd["utc"], &d1u) && bnd["observed"].truthy() && has(&bnd["source"], "declared_utc 2026-01-02T23:00:00Z")
        && db.len() == 2 && eqs(&db[0]["campaign_scope"], CAMPAIGN_SCOPE_PRIOR) && has(&db[0]["detail"], PRIOR_CAMPAIGN_NOTE)
        && eqs(&db[1]["campaign_scope"], CAMPAIGN_SCOPE_THIS) && !has(&db[1]["detail"], PRIOR_CAMPAIGN_NOTE) && tlb.len() == tl.len());
    let early = rep(&evb, &[("declaration", rep(&late_decl, &[("declared_utc", J::from("2025-12-31T00:00:00Z"))]))]);
    c.ck("★음성대조: 선언 시각이 모든 행보다 앞이면 앞선 캠페인 행 0(경계가 과하게 자르지 않는다)",
        eqs(&event_campaign_boundary(repo, &early)?["utc"], "2025-12-31T00:00:00Z")
        && !event_timeline(repo, &early)?.iter().any(|r| eqs(&r["campaign_scope"], CAMPAIGN_SCOPE_PRIOR)));
    let rp = rep(&evb, &[("mode", J::from("publication-replay")), ("campaign_id", J::Null)]);
    let bnr = event_campaign_boundary(repo, &rp)?;
    c.ck("★재생 모드(캠페인 purge) = 경계 미관측 표지 · 행에 scope 없음(현 규칙: 라벨 전부) · 행 수 같음",
        bnr["utc"].is_null() && bnr["observed"] == J::Bool(false) && has(&bnr["source"], "경계 미관측")
        && !event_timeline(repo, &rp)?.iter().any(|r| inn(r, "campaign_scope")) && d0u < d1u);
    // F1 월 경계
    let ledger_p = repo.join("docs/logs/main/events/2026-01.jsonl");
    let ledger_b = rdb(&ledger_p)?;
    let prev_m = repo.join("docs/logs/main/events/2025-12.jsonl");
    let r = (|| -> HR<(Vec<J>, J)> {
        wr0(&prev_m, &(ad(&jd! {"kind" => "budget_declare", "label" => "smoke-c1-a", "floor_mib" => 100, "ts" => "2025-12-31T23:58:00Z"}) + "\n"))?;
        let mut nb = jsonl(&[jd! {"kind" => "budget_renew", "label" => "smoke-c1-a", "ttl_s" => 900, "ts" => "2026-01-01T00:10:00Z"},
            jd! {"kind" => "budget_renew", "label" => "smoke-c1-a", "ttl_s" => 900, "ts" => "2026-01-01T00:20:00Z"},
            jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-01T00:30:00Z"}]).into_bytes();
        nb.extend_from_slice(&ledger_b);
        wrb(&ledger_p, &nb)?;
        Ok((event_timeline(repo, ev)?, event_campaign_boundary(repo, &evb)?))
    })();
    let _ = std::fs::remove_file(&prev_m);
    wrb(&ledger_p, &ledger_b)?;
    let (tlm, bm) = r?;
    let dm = kinds(&tlm, "budget_declare");
    let rn: Vec<&J> = tlm.iter().filter(|r| eqs(&r["kind"], "budget_renew") && st(&r["utc"]).as_str() < "2026-01-01T01:00:00Z").collect();
    let srv_m = tlm.iter().find(|r| eqs(&r["kind"], "server_starting")).cloned().unwrap_or_default();
    let dmd = |i: usize| dm.get(i).map(|r| st(&r["detail"])).unwrap_or_default();
    c.ck("★F1 월 경계를 넘는 앞선 창 = 다음 달 renew 2회 · clear 로 닫힘 · '측정 전'(열린 채 원장이 끝난다 ✗ · 이 측정의 기동 ✗)",
        dm.len() == 3 && eqs(&dm[0]["source"], "docs/logs/main/events/2025-12.jsonl:1") && dmd(0).contains("기동 창 1/3")
        && dmd(0).contains("budget_renew 2회") && dmd(0).contains("닫힘 budget_clear 2026-01-01T00:30:00Z") && dmd(0).contains("측정 전)") && !dmd(0).contains("닫힘 미관측"));
    let inside: Vec<usize> = dm.iter().enumerate().filter(|(_, r)| has(&r["detail"], "이 창 안")).map(|(i, _)| i).collect();
    c.ck("★F1 측정을 품은 창만 '이 측정의 기동'(한 창 · 3/3) · 앞선 창의 renew 출처 = 실제 파일:줄 · 상대시각 기준 = 그 창",
        inside == vec![2] && dmd(2).contains("기동 창 3/3")
        && rn.iter().map(|r| st(&r["source"])).collect::<Vec<_>>() == strs(&["docs/logs/main/events/2026-01.jsonl:1", "docs/logs/main/events/2026-01.jsonl:2"])
        && rn.iter().all(|r| has(&r["detail"], "창 시작 2025-12-31T23:58:00Z 후")) && has(&srv_m["detail"], "후 28m00s"));
    c.ck("★F1 캠페인 경계 = 측정을 품은 창 시작(앞선 회차 창 ✗)", eqs(&bm["utc"], &d1u));
    let wh = jd! {"start_ts" => "2026-01-01T00:00:00Z", "end_ts" => J::Null, "stream_last" => "2026-01-01T01:00:00Z"};
    c.ck("★F1 닫힘 미관측 창은 스트림 마지막 행까지만 품는다(그 뒤 측정 ✗ · 그 안 ✓)",
        !_win_holds(&wh, "2026-01-02T00:00:00Z")? && _win_holds(&wh, "2026-01-01T00:30:00Z")?);
    // F5 다른 라벨 창
    let evd5 = repo.join("docs/logs/main/events");
    let mut keep5: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for f in std::fs::read_dir(&evd5).map_err(py)?.flatten() {
        if f.path().is_file() {
            keep5.push((f.path(), rdb(&f.path())?));
        }
    }
    let r = (|| -> HR<Vec<J>> {
        for (p, _) in &keep5 {
            rm(p)?;
        }
        let (sid1, sid2) = ("serve-x-35b-v030-20260101T000100Z", "serve-x-35b-v030-20260101T002100Z");
        wr0(&evd5.join("2026-01.jsonl"), &jsonl(&[
            jd! {"kind" => "budget_declare", "label" => "serve-x-v030", "kv_mib" => 16384, "ts" => "2026-01-01T00:00:00Z"},
            jd! {"kind" => "serve_start", "session_id" => sid1, "model" => "x", "ts" => "2026-01-01T00:01:00Z"},
            jd! {"kind" => "watchdog_trip", "rule" => "absolute", "mem_avail_mib" => 900, "ts" => "2026-01-01T00:10:00Z"},
            jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-01T00:11:00Z"},
            jd! {"kind" => "serve_stop", "session_id" => sid1, "model" => "x", "ts" => "2026-01-01T00:12:00Z"},
            jd! {"kind" => "budget_declare", "label" => "serve-x-v030-r3", "kv_mib" => 8192, "ts" => "2026-01-01T00:20:00Z"},
            jd! {"kind" => "serve_start", "session_id" => sid2, "model" => "x", "ts" => "2026-01-01T00:21:00Z"},
            jd! {"kind" => "watchdog_trip", "rule" => "absolute", "mem_avail_mib" => 800, "ts" => "2026-01-01T00:30:00Z"},
            jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-01T00:31:00Z"},
            jd! {"kind" => "serve_stop", "session_id" => sid2, "model" => "x", "ts" => "2026-01-01T00:32:00Z"},
            jd! {"kind" => "budget_declare", "label" => "smoke-unrelated-model", "ts" => "2026-01-01T01:00:00Z"},
            jd! {"kind" => "watchdog_trip", "rule" => "absolute", "mem_avail_mib" => 700, "ts" => "2026-01-01T01:02:00Z"},
            jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-01T01:05:00Z"},
            jd! {"kind" => "budget_declare", "label" => "smoke-x-35b-v030", "kv_mib" => 8192, "ts" => "2026-01-02T02:30:00Z"},
            jd! {"kind" => "budget_renew", "label" => "smoke-x-35b-v030", "ttl_s" => 900, "ts" => "2026-01-02T02:59:00Z"},
            jd! {"kind" => "budget_clear", "existed" => true, "ts" => "2026-01-02T03:10:00Z"}]))?;
        let ev5 = rep(ev, &[("cell", J::from("x-35b-v030")), ("certificate", rep(&cert, &[("measured_utc", J::from(_M_UTC))]))]);
        event_timeline(repo, &ev5)
    })();
    for f in std::fs::read_dir(&evd5).map_err(py)?.flatten() {
        let _ = std::fs::remove_file(f.path());
    }
    for (p, b_) in &keep5 {
        wrb(p, b_)?;
    }
    let tl5 = r?;
    let d5 = kinds(&tl5, "budget_declare");
    let oth5: Vec<&J> = tl5.iter().filter(|r| eqs(&r["label_scope"], LABEL_SCOPE_OTHER)).collect();
    let mut fl_ok = true;
    for r in &oth5 {
        fl_ok &= has(&r["detail"], &foreign_label_note(&r["window_label"], "x-35b-v030")?);
    }
    c.ck("★F5 겹치는 다른 라벨 창의 선언 · 트립 · clear 를 싣는다(떨구지 않는다) · 모든 행에 label_scope · 창 라벨 · 문장 표지",
        d5.iter().map(|r| st(&r["label"])).collect::<Vec<_>>() == strs(&["serve-x-v030", "serve-x-v030-r3", "smoke-x-35b-v030"])
        && oth5.iter().filter(|r| eqs(&r["kind"], "watchdog_trip")).count() == 2 && oth5.iter().filter(|r| eqs(&r["kind"], "budget_clear")).count() == 2
        && fl_ok && d5.len() > 1 && has(&d5[1]["detail"], "라벨 다름: `serve-x-v030-r3` — 같은 노드 · 이 셀 라벨(`x-35b-v030`) 아님"));
    c.ck("★F5 다른 라벨 창은 이 셀의 기동 창 번호 · 측정 창 밖(이 셀 창 = 1/1 · 이 측정의 기동) · 세션 행은 이 셀 행 그대로",
        d5.len() == 3 && has(&d5[2]["detail"], "기동 창 1/1") && has(&d5[2]["detail"], "이 창 안(이 측정의 기동)") && !has(&d5[0]["detail"], "기동 창")
        && tl5.iter().filter(|r| eqs(&r["kind"], "serve_start") || eqs(&r["kind"], "serve_stop")).all(|r| !inn(r, "label_scope")));
    c.ck("★F5 음성대조: 이 셀 행과 시간이 겹치지 않는 다른 라벨 창(다른 모델)과 그 트립은 싣지 않는다",
        !ud(&J::List(tl5.clone())).contains("smoke-unrelated-model") && !tl5.iter().any(|r| eqs(&r["utc"], "2026-01-01T01:02:00Z")));
    // F14
    mkd(&repo.join("docs/simlog/fx_f14"))?;
    mkd(&repo.join("docs/simlog/fx_f14_other"))?;
    mkd(&repo.join("docs/testlog"))?;
    let tl14 = repo.join("docs/testlog/testlog_26010200_fx_f14.md");
    wr0(&tl14, "- 원시: `docs/simlog/fx_f14/`(a_serve.log)\n")?;
    let ptr14 = jl![jd! {"kind" => "testlog", "path" => _rel(repo, &tl14)?, "cell_id" => "c1-a", "node_id" => "main"}];
    let scope_at = |tl: &[J]| -> Map {
        let mut m = Map::new();
        for r in tl.iter().filter(|r| eqs(&r["utc"], "2026-01-02T02:00:00Z")) {
            m.insert(st(&r["kind"]), r["campaign_scope"].clone());
        }
        m
    };
    let r = (|| -> HR<(Map, J, Vec<J>, J, Map)> {
        wr0(&repo.join("docs/simlog/fx_f14_other/x_serve.log"), "[single-up] READY session=serve-c1-a-20260102T020000Z\n")?;
        let ev14n = rep(&evb, &[("pointers", ptr14.clone())]);
        let s14n = scope_at(&event_timeline(repo, &ev14n)?);
        wr0(&repo.join("docs/simlog/fx_f14/a_serve.log"), "[single-up] 세션 시작\n[single-up] READY session=serve-c1-a-20260102T020000Z\n")?;
        let tl14y = event_timeline(repo, &ev14n)?;
        let s14 = tl14y.iter().find(|r| eqs(&r["kind"], "serve_start") && eqs(&r["utc"], "2026-01-02T02:00:00Z")).cloned().unwrap_or_default();
        let b14 = event_campaign_boundary(repo, &ev14n)?;
        let s14x = scope_at(&event_timeline(repo, &rep(&evb, &[("pointers", J::list())]))?);
        Ok((s14n, s14, tl14y, b14, s14x))
    })();
    rmtree(&repo.join("docs/simlog/fx_f14"));
    rmtree(&repo.join("docs/simlog/fx_f14_other"));
    rm(&tl14)?;
    let (s14n, s14, tl14y, b14, s14x) = r?;
    let d14 = kinds(&tl14y, "budget_declare");
    c.ck("★F14 경계 이전 serve_start 세션 id 가 이 셀 testlog 인용 simlog 로그에 관측 → same-cell-pre-boundary(이 셀 시도) · 출처 file:line",
        eqs(&s14["campaign_scope"], CAMPAIGN_SCOPE_SAME_CELL_PRE) && has(&s14["detail"], SAME_CELL_PRE_NOTE)
        && has(&s14["detail"], "docs/simlog/fx_f14/a_serve.log:2") && !has(&s14["detail"], PRIOR_CAMPAIGN_NOTE)
        && has(&b14["source"], SAME_CELL_PRE_NOTE) && b14["same_cell_sessions"].truthy());
    c.ck("★F14 귀속 구간 밖(그 앞의 01:00 선언 창)은 여전히 앞선 캠페인 · 측정 창은 this-campaign",
        d14.len() > 1 && eqs(&d14[0]["campaign_scope"], CAMPAIGN_SCOPE_PRIOR) && eqs(&d14[1]["campaign_scope"], CAMPAIGN_SCOPE_THIS));
    c.ck("★F14 음성대조: 인용되지 않은 simlog 의 같은 세션 id · 포인터 없음 → 앞선 캠페인(추측 ✗)",
        s14n.get("serve_start").map_or(false, |x| eqs(x, CAMPAIGN_SCOPE_PRIOR)) && s14x.get("serve_start").map_or(false, |x| eqs(x, CAMPAIGN_SCOPE_PRIOR)));
    let srv = tl.iter().find(|r| eqs(&r["kind"], "server_starting")).cloned().unwrap_or_default();
    c.ck("기동 표지 — API 서버 기동 = 측정 창 선언 후 28m00s · 출처 줄 = `\\n` 기준(★`\\r` 진행줄이 줄을 밀지 않는다)",
        has(&srv["detail"], "후 28m00s") && ew_(&srv["source"], "lite_engine_c1-a.log:5"));
    let ks: Vec<String> = tl.iter().map(|r| st(&r["kind"])).collect();
    c.ck("기동 표지 — ANSI 제거 후 첫 시각(1번 줄) · prefetch(CIFS) · KV", ks.iter().any(|k| k == "engine_prefetch_disabled") && ks.iter().any(|k| k == "engine_kv_cache_sized")
        && tl.iter().find(|r| eqs(&r["kind"], "engine_log_window_start")).map_or(false, |r| ew_(&r["source"], "lite_engine_c1-a.log:1")));
    let sdir = st(&ev["sweep"]["dir"]);
    let logp = repo.join(&sdir).join("lite_engine_c1-a.log");
    let log_bytes = rdb(&logp)?;
    wr0(&logp, &String::from_utf8_lossy(&log_bytes).replace("01-02 02:", "01-02 11:"))?;
    let tl_bad = event_timeline(repo, ev);
    wrb(&logp, &log_bytes)?;
    let tl_bad = tl_bad?;
    c.ck("★엔진 시계가 측정 창 밖이면 기동 표지를 싣지 않고 그 사실만 적는다",
        tl_bad.iter().filter(|r| sw_(&r["source"], &sdir)).map(|r| st(&r["kind"])).collect::<Vec<_>>() == strs(&["engine_log_clock_unverified"]));
    let ev_nb = rep(ev, &[("sweep", rep(&ev["sweep"], &[("binding", J::from("simlog-copy"))]))]);
    c.ck("★스윕이 출력 평면에 묶이지 않으면 엔진 표지 없음", !event_timeline(repo, &ev_nb)?.iter().any(|r| sw_(&r["kind"], "engine_") || sw_(&r["kind"], "server_")));
    c.ck("★원장 노드 = 셀 노드 축(main 셀은 sub 원장을 읽지 않는다 · cluster 는 선언 노드 전부)",
        _event_node_dirs(repo, &rep(ev, &[("node", J::from("main")), ("declaration", J::Null)]))? == strs(&["main"])
        && _event_node_dirs(repo, &rep(ev, &[("node", J::from("sub")), ("declaration", J::Null)]))? == strs(&["sub"])
        && _event_node_dirs(repo, &rep(ev, &[("node", J::from("cluster")), ("declaration", J::Null)]))? == strs(&["main", "sub"]));
    let ef = event_files(repo, ev)?;
    let mut cand: Vec<String> = ev["lineage_seeds"]["evidence_candidates"].items().iter().filter(|x| eqs(&x["kind"], "events_ledger")).map(|x| st(&x["path"])).collect();
    cand.sort();
    cand.dedup();
    let mut efs = ef.clone();
    efs.sort();
    c.ck("원장 파일 = 행을 낸 것만(★다른 노드·다른 셀만 든 파일 제외) · 계보 후보에 실린다",
        ef == strs(&["docs/logs/main/events/2026-01.jsonl", "docs/logs/main/events/thermal.jsonl", "docs/logs/main/events/watchdog.jsonl"]) && cand == efs);
    // ── F9 full 정의 · 벤치 명령 ──
    let bd0 = bench_definition(repo, ev)?;
    c.ck("★docs.md 부재 = 정의 미관측 → 충족 판정 None(모름 ≠ 미충족)", bd0["current_full_definition"].is_null() && bd0["meets_current_full"].is_null());
    let skill = repo.join(".claude/skills/adversarial-benchmark/SKILL.md");
    wr(&skill, "- **full bench 의 정의** — **`full = lite ∪ GuideLLM × 반복 ≥3`**\n")?;
    let mut sw_skill = ev["sweep"].clone();
    sw_skill.set("index", rep(&ev["sweep"]["index"], &[("lite", jd! {"lite_verdict" => "pass"})]));
    sw_skill.set("levels", jl![jd! {"level" => 1, "measured" => jd! {"repeats_completed" => 3}}, jd! {"level" => 2, "measured" => jd! {"repeats_completed" => 3}}]);
    let ev_skill = rep(ev, &[("certificate", rep(&cert, &[("bench_tool", J::from("guidellm"))])), ("sweep", sw_skill.clone())]);
    let fallback = bench_definition(repo, &ev_skill)?;
    c.ck("skill definition fallback: lite pass and all measured levels completed", ew_(&fallback["source"], "SKILL.md") && fallback["meets_current_full"] == J::Bool(true));
    let sw_missing = rep(&sw_skill, &[("index", rep(&sw_skill["index"], &[("lite", J::dict())]))]);
    c.ck("skill fallback does not infer missing lite pass", bench_definition(repo, &rep(&ev_skill, &[("sweep", sw_missing)]))?["meets_current_full"].is_null());
    rm(&skill)?;
    let rules = repo.join(REL_DOCS_RULES);
    let rule_old = "# docs\n\n| `benchmark/` | full 계측(`lite ∪ GuideLLM × 반복 ≥3`) · lite-only 셀의 경량 report | x |\n";
    wr(&rules, rule_old)?;
    let bd1 = bench_definition(repo, ev)?;
    c.ck("현행 full 정의 원문·줄 · 반복 하한 · 레그", eqs(&bd1["current_full_definition"], "full 계측(`lite ∪ GuideLLM × 반복 ≥3`)")
        && eqs(&bd1["source"], REL_DOCS_RULES) && bd1["source_line"] == J::Int(3) && bd1["required_repeats"] == J::Int(3) && bd1["required_legs"] == jl!["lite", "GuideLLM"]);
    let new_def = "full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)";
    wr0(&rules, &format!("# docs\n\n| `benchmark/` | {new_def} · lite-only 셀의 경량 report | x |\n"))?;
    let bd1n = bench_definition(repo, ev)?;
    c.ck("F4 현행(2026-09-29) 문구 — 원문·반복 하한 3·레그 [lite, GuideLLM](머리 `lite 게이트` = 레그)",
        eqs(&bd1n["current_full_definition"], new_def) && bd1n["required_repeats"] == J::Int(3) && bd1n["required_legs"] == jl!["lite", "GuideLLM"]
        && bd1n["meets_current_full"] == J::Bool(false));
    wr0(&rules, rule_old)?;
    c.ck("★요청 반복(repeats · 선언)은 this_repeats 가 아니다 — 완주 관측(단일 run 배치) 1 · 요청은 repeats_requested",
        bd1["this_repeats"] == J::Int(1) && bd1["repeats_requested"] == J::Int(3) && sw_(&bd1["repeats_source"], "observed(") && bd1["meets_current_full"] == J::Bool(false));
    let rep_p = repo.join(st(&ev["bench_report_path"]));
    let rep_text = rd(&rep_p)?;
    wr0(&rep_p, &rep_text.replace("| repeats | 3 |", "| repeats | 3 |\n| repeats_completed | 3 |"))?;
    let bd_c = bench_definition(repo, ev)?;
    c.ck("완주 수(repeats_completed) 가 있으면 그것 · ★도구 미관측이면 반복이 충분해도 판정하지 않는다",
        bd_c["this_repeats"] == J::Int(3) && has(&bd_c["repeats_source"], "repeats_completed") && bd_c["meets_current_full"].is_null());
    let bd_v = bench_definition(repo, &rep(ev, &[("certificate", rep(&cert, &[("bench_tool", J::from("vllm-bench-serve"))]))]))?;
    c.ck("★full 레그 도구가 GuideLLM 이 아니면 미충족(사유 기재) · 기록 라벨 full 과 갈림을 적는다", bd_v["meets_current_full"] == J::Bool(false)
        && bd_v["reasons"].items().iter().any(|r| has(r, "GuideLLM")) && bd_v["reasons"].items().iter().any(|r| has(r, "기록된 라벨")));
    let bd_g = bench_definition(repo, &rep(ev, &[("certificate", rep(&cert, &[("bench_tool", J::from("guidellm"))]))]))?;
    c.ck("GuideLLM × 완주 3 = 충족", bd_g["meets_current_full"] == J::Bool(true));
    wr0(&rep_p, &rep_text.replace("| repeats | 3 |", "| repeats | 3 |\n| repeats_completed | 3 |").replace("| bench_mode | full |", "| bench_mode | lite |"))?;
    c.ck("★측정 구성 bench_mode=lite 면 완주·도구가 맞아도 미충족", bench_definition(repo, &rep(ev, &[("certificate",
        rep(&cert, &[("bench_tool", J::from("guidellm")), ("benchmark_mode", J::from("lite"))]))]))?["meets_current_full"] == J::Bool(false));
    wr0(&rep_p, &rep_text)?;
    let sw0 = if ev["sweep"].is_dict() { ev["sweep"].clone() } else { J::dict() };
    let cert_g = rep(&cert, &[("bench_tool", J::from("guidellm"))]);
    let ev_nv = rep(ev, &[("certificate", cert_g.clone()), ("sweep", rep(&sw0, &[("index", without(&sw0["index"], "verdict_point_level"))]))]);
    let bd_r = bench_definition(repo, &ev_nv)?;
    c.ck("★완주 관측이 없으면(판정점 미기재) 요청 반복 3 으로 충족을 말하지 않는다(None · 요청은 사유로만)",
        bd_r["this_repeats"].is_null() && bd_r["verdict_level"].is_null() && bd_r["meets_current_full"].is_null() && has(&bd_r["repeats_source"], "요청 반복 3(선언"));
    let with_level = |extra: &[(&str, J)]| -> HR<J> {
        let mut lv: Vec<J> = Vec::new();
        for x in sw0["levels"].items() {
            if _as_int(&x["level"])? == Some(1) {
                lv.push(rep(x, &[("measured", rep(&x["measured"], extra))]));
            } else {
                lv.push(x.clone());
            }
        }
        Ok(rep(ev, &[("bench_report_path", J::Null), ("certificate", cert_g.clone()), ("sweep", rep(&sw0, &[("levels", J::List(lv))]))]))
    };
    let runs3 = jl![jd! {"run" => 1, "measurement_ok" => true}, jd! {"run" => 2, "measurement_ok" => true}, jd! {"run" => 3, "measurement_ok" => false}];
    let bd_u = bench_definition(repo, &with_level(&[("runs", runs3.clone())])?)?;
    c.ck("★레벨 runs[] 는 measurement_ok 인 run 만 센다(실패 run 포함 3 → 완주 2 < 3 미충족)", bd_u["this_repeats"] == J::Int(2)
        && has(&bd_u["repeats_source"], "runs[]") && bd_u["meets_current_full"] == J::Bool(false));
    let bd_uc = bench_definition(repo, &with_level(&[("runs", runs3), ("repeats_completed", J::Int(3))])?)?;
    c.ck("레벨 measured.repeats_completed 가 runs[] 보다 먼저", bd_uc["this_repeats"] == J::Int(3) && bd_uc["meets_current_full"] == J::Bool(true));
    let ev_n = rep(ev, &[("bench_report_path", J::Null), ("certificate", cert_g.clone())]);
    let bd_n = bench_definition(repo, &ev_n)?;
    c.ck("★반복 기록이 없으면 단일 run 배치 관측 — 1 < 3 미충족", bd_n["this_repeats"] == J::Int(1)
        && sw_(&bd_n["repeats_source"], "observed(") && bd_n["meets_current_full"] == J::Bool(false));
    let rdir = repo.join(&sdir).join("level_01/run_02");
    std::fs::create_dir(&rdir).map_err(|e| core::io_err(&e, &rdir))?;
    let bd_k = bench_definition(repo, &ev_n);
    rmdir(&rdir)?;
    let bd_k = bd_k?;
    c.ck("★run_KK/ 가 있는데 집계가 없으면 판정 불가(파일 수로 세지 않는다)", bd_k["this_repeats"].is_null()
        && has(&bd_k["repeats_source"], "판정 불가") && bd_k["meets_current_full"].is_null());
    let bc0 = bench_command(repo, ev)?;
    c.ck("★vllm bench serve 결과 모양이 아니면 재구성하지 않는다", bc0["command"].is_null() && has(&bc0["source"], "모양이 아니다"));
    let bj = repo.join(&sdir).join("level_01/bench_c1-a.json");
    let bj_text = rd(&bj)?;
    let full = jd! {"backend" => "openai", "endpoint_type" => "openai", "label" => J::Null, "model_id" => "fixture-served", "tokenizer_id" => "/app/x y",
        "num_prompts" => 4, "completed" => 4, "failed" => 0, "total_input_tokens" => 4096, "total_output_tokens" => 1000,
        "max_concurrency" => 1, "request_rate" => "inf", "burstiness" => 1.0};
    wj(&bj, &full)?;
    let bc = bench_command(repo, ev)?;
    let cmd = st(&bc["command"]);
    c.ck("벤치 명령 재구성 — 결과 JSON 필드만 · 길이는 나누어떨어질 때 파생(출처 표시)",
        cmd.starts_with("vllm bench serve --backend openai --model fixture-served --tokenizer '/app/x y' --num-prompts 4")
        && cmd.contains("--random-input-len 1024") && cmd.contains("--random-output-len 250")
        && sw_(&bc["source"], "reconstructed(bench json)") && eqs(&bc["command_source"], "reconstructed(bench json)")
        && !(cmd.clone() + &st(&bc["source"])).contains('§')
        && bc["fields"].items().iter().any(|f| eqs(&f["flag"], "--random-input-len") && sw_(&f["source"], "derived("))
        && inn(&bc["unrecorded"], "--ignore-eos") && !cmd.contains("--label"));
    wj(&bj, &rep(&full, &[("total_output_tokens", J::Int(1001))]))?;
    let bc_nd = bench_command(repo, ev)?;
    c.ck("★나누어떨어지지 않으면 출력 길이 플래그를 싣지 않고 unrecorded 에 사유를 적는다",
        !st(&bc_nd["command"]).contains("--random-output-len") && bc_nd["unrecorded"].items().iter().any(|u| sw_(u, "--random-output-len(")));
    let sw_il = rep(&ev["sweep"], &[("index", rep(&ev["sweep"]["index"], &[("input_len", J::Int(2048))]))]);
    let bc_il = bench_command(repo, &rep(ev, &[("sweep", sw_il)]))?;
    c.ck("★sweep_index.input_len 과 어긋나면 입력 길이 플래그를 싣지 않는다(고르지 않는다 · 어긋남을 적는다)",
        !st(&bc_il["command"]).contains("--random-input-len") && bc_il["unrecorded"].items().iter().any(|u| has(u, "≠ sweep_index.input_len 2048")));
    let bc_nb = bench_command(repo, &ev_nb)?;
    c.ck("★스윕이 출력 평면에 묶이지 않으면 재구성하지 않는다(같은 모양 · command_source None)",
        bc_nb["command"].is_null() && inn(&bc_nb, "command_source") && bc_nb["command_source"].is_null());
    wj(&bj, &rep(&full, &[("ignore_eos", J::Bool(true)), ("trust_remote_code", J::Bool(false))]))?;
    let bc_b = bench_command(repo, ev)?;
    c.ck("★불리언 플래그는 참일 때만(ignore_eos=true → --ignore-eos · false 는 싣지 않는다)", st(&bc_b["command"]).contains(" --ignore-eos")
        && !st(&bc_b["command"]).contains("--trust-remote-code") && !inn(&bc_b["unrecorded"], "--ignore-eos"));
    wr0(&bj, &bj_text)?;
    Ok(())
}

/// `_git_fixture(td)` — 측정 도구 판본 선택용 격리 git 저장소(시각 전부 주입). → (g, {c1, c2, c3, c4, rb_v1, lb_side}).
pub fn _git_fixture(td: &Path) -> HR<(PathBuf, J)> {
    let g = td.join("gitrepo");
    let d = g.join(REL_BENCH_TOOLS_DIR);
    mkd(&d)?;
    let commit = |utc: &str, msg: &str| -> HR<String> {
        let e = genv(Some(utc))?;
        git_e(&g, &["add", "-A"], &e)?;
        git_e(&g, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", msg], &e)?;
        Ok(core::git(&g, &["rev-parse", "HEAD"])?.stdout.trim().to_string())
    };
    let sweep_v1 = [
        "#!/bin/bash", "# fixture sweep_bench", "bash \"$SDIR/lite_bench.sh\" \"$CONFIG\"", "bash \"$SDIR/run_bench.sh\" \"${RB_ARGS[@]}\"",
        "python3 - <<'PY'", "_img = grep_env(envtext, \"IMAGE_TAG\") or \"\"", "_m = re.search(r\"\\bv([0-9.]+)\", _elog)",
        "vllm_build = _m.group(1) if _m else \"NA\"", "vllm = os.environ.get(\"VLLM_VER\") or os.environ.get(\"EASY_VLLM_VERSION\") or None",
        "if not vllm:", "    _mi = re.search(r\"easy-vllm:([0-9]+\\.[0-9]+\\.[0-9]+)\", _img)", "PY", ""].join("\n");
    let rb_v1 = "# run_bench v1\n# tail -1 > \"$ELOG\" 는 주석이라 세지 않는다\ndocker logs \"$CTR\" 2>&1 | tail -1 | grep -q ready\n";
    let lb_side = "# lite_bench side\ndocker logs \"$CTR\" 2>&1 | tail -3 > \"$ELOG\" || true\n";
    let files: [(&str, String); 5] = [("sweep_bench.sh", sweep_v1), ("run_bench.sh", rb_v1.into()), ("lite_bench.sh", "# lite_bench v1\n".into()),
        ("broad_search.sh", "SB_ARGS=(\"$CONFIG\" --topology \"$TOPO\" --tool guidellm)\n# 사용: --tool vllm 은 주석이라 세지 않는다\nbash \"$SDIR/sweep_bench.sh\" \"${SB_ARGS[@]}\"\n".into()),
        ("other.sh", "# sweep_bench 를 부르지 않는다\n".into())];
    for (n, body) in &files {
        wr0(&d.join(n), body)?;
    }
    let e0 = genv(Some("2026-01-01T00:00:00Z"))?;
    git_e(&g, &["init", "-q"], &e0)?;
    git_e(&g, &["symbolic-ref", "HEAD", "refs/heads/main"], &e0)?;
    let c1 = commit("2026-01-01T00:00:00Z", "c1")?;
    wr0(&d.join("run_bench.sh"), "# run_bench v2\n")?;
    let c2 = commit("2026-01-02T00:00:00Z", "c2")?;
    git_e(&g, &["checkout", "-q", "-b", "side", &c1], &genv(Some("2026-01-02T01:00:00Z"))?)?;
    wr0(&d.join("lite_bench.sh"), lb_side)?;
    let c4 = commit("2026-01-02T02:00:00Z", "c4")?;
    git_e(&g, &["checkout", "-q", "main"], &genv(Some("2026-01-02T05:00:00Z"))?)?;
    wr0(&d.join("run_bench.sh"), "# run_bench v3\n")?;
    let c3 = commit("2026-01-03T00:00:00Z", "c3")?;
    Ok((g.clone(), jd! {"c1" => c1, "c2" => c2, "c3" => c3, "c4" => c4, "rb_v1" => rb_v1, "lb_side" => lb_side}))
}

fn by_key(env: &[J]) -> Vec<((String, String, Option<String>), J)> {
    env.iter().map(|r| ((st(&r["kind"]), st(&r["key"]), r["node"].as_str().map(str::to_string)), r.clone())).collect()
}
fn by_get(by: &[((String, String, Option<String>), J)], k: &str, key: &str, node: Option<&str>) -> J {
    by.iter().find(|(t, _)| t.0 == k && t.1 == key && t.2.as_deref() == node).map(|(_, r)| r.clone()).unwrap_or_default()
}
fn keyed(env: &[J]) -> Map {
    let mut m = Map::new();
    for r in env {
        m.insert(st(&r["key"]), r.clone());
    }
    m
}

fn _selftest_round3(c: &mut Ck, repo: &Path, td: &Path, ev: &J, fx: &J, dk: Dk) -> HR<()> {
    let sweep = crate::api::decode_path(&fx["sweep"]);
    let ip_sub = ["10", "9", "8", "7"].join(".");
    let man = jd! {"nodes" => jl![jd! {"role" => "main", "hostname" => "fixhost-m1"}, jd! {"role" => "sub", "hostname" => "fixhost-s2", "host" => ip_sub.as_str()}],
        "interconnect" => jd! {"hca_devices" => jl!["mlx5_7", "mlx5_8"]}};
    let evm = rep(ev, &[("manifest", man.clone())]);
    let lite = sweep.join("lite_engine_c1-a.log");
    let lite0 = rdb(&lite)?;
    let sub_p = format!("(EngineCore pid=1) (RayWorkerProc pid=402, ip={ip_sub}) fixhost-s2:402:534 [0] NCCL INFO ");
    let main_p = "(EngineCore pid=1) \x1b[36m(RayWorkerProc pid=7)\x1b[0m fixhost-m1:7:9 [0] NCCL INFO ";
    let extra = [
        format!("{sub_p}NCCL_IB_GID_INDEX set by environment to 3."),
        format!("{main_p}NCCL_IB_GID_INDEX set by environment to 3."),
        "progress 1%\rprogress 2%".into(),
        format!("{sub_p}NCCL_IB_HCA set to =mlx5_7,mlx5_8 [repeated 2x across cluster] (Ray deduplicates logs by default.)"),
        format!("{sub_p}Using network IB [repeated 3x across cluster]"),
        format!("{sub_p}NET/IB : Using [0]mlx5_7:1/RoCE [1]mlx5_8:1/RoCE [RO]; OOB mlx5_7:{ip_sub}<0> [repeated 2x across cluster] (Ray x)"),
        "(EngineCore pid=1) otherhost-9:1:1 [0] NCCL INFO NCCL_NET_PLUGIN set by environment to spcx".into(),
        format!("{main_p}P2P Chunksize set to 262144"),
        format!("{main_p}NCCL_CROSS_NIC set by environment to 1.\x1b[0m"),
        format!("{main_p}NCCL_DEBUG_SUBSYS set by environment to fixturesecretterm"),
        "(APIServer pid=186) \x1b[33mWARNING\x1b[0m 10-25 00:56:53 [interface.py:1355] Unknown vLLM environment variable detected: VLLM_PLE_MMAP".into(),
        "(APIServer pid=186) WARNING 10-25 00:56:53 [interface.py:1355] Unknown vLLM environment variable detected: VLLM_REF".into(),
        "(APIServer pid=186) WARNING 10-25 00:56:54 [interface.py:1355] Unknown vLLM environment variable detected: VLLM_PLE_MMAP".into(),
        String::new(),
    ];
    let mut nb = lite0.clone();
    nb.extend_from_slice(extra.join("\n").as_bytes());
    wrb(&lite, &nb)?;
    let lv1 = sweep.join("level_01/engine_c1-a.log");
    wr0(&lv1, &format!("{sub_p}NCCL_IB_GID_INDEX set by environment to 3.\n"))?;
    mkd(&sweep.join("level_02"))?;
    wr0(&sweep.join("level_02/engine_c1-a.log"), &format!("{sub_p}NCCL_IB_TIMEOUT set by environment to 22.\n"))?;
    let l0s = String::from_utf8_lossy(&lite0).into_owned();
    let base_ln = l0s.matches('\n').count() + if lite0.ends_with(b"\n") { 0 } else { 1 };
    let env = measurement_env_observed(repo, &evm)?;
    let by = by_key(&env);
    let lrel = _rel(repo, &lite)?;
    let gid_sub = by_get(&by, "env-echo", "NCCL_IB_GID_INDEX", Some("sub"));
    c.ck("env 되읊음 — 키·값(정수 끝 마침표 제거)·노드(Ray ip → manifest 치환표 <node:sub>)·출처 줄(`\\n` 기준)",
        eqs(&gid_sub["value"], "3") && eqs(&gid_sub["source"], &format!("{lrel}:L{}", base_ln + 1)) && gid_sub["seen_in_logs"] == J::Int(2));
    c.ck("같은 키·값이라도 노드가 다르면 다른 행(호스트 → <node:main> · ANSI 제거)",
        eqs(&by_get(&by, "env-echo", "NCCL_IB_GID_INDEX", Some("main"))["source"], &format!("{lrel}:L{}", base_ln + 2)));
    let hca = by_get(&by, "param-echo", "NCCL_IB_HCA", Some("sub"));
    c.ck("★장치 이름은 기계 치환(<nic:cluster>) · Ray 중복 제거 수 · `\\r` 진행줄이 줄 번호를 밀지 않는다",
        eqs(&hca["value"], "=<nic:cluster>,<nic:cluster>") && hca["ray_repeated"] == J::Int(2) && inn(&hca, "value_note")
        && ew_(&hca["source"], &format!(":L{}", base_ln + 4)) && !ad(&J::List(env.clone())).contains("mlx5_7"));
    c.ck("NCCL 런타임 관측(nccl:network — env 이름과 섞이지 않는 소문자 키)", eqs(&by_get(&by, "runtime", "nccl:network", Some("sub"))["value"], "IB"));
    c.ck("★NET/IB 장치 목록 — Ray 중복 제거 꼬리는 값이 아니다 · 장치·주소 치환",
        eqs(&by_get(&by, "runtime", "nccl:net_ib_devices", Some("sub"))["value"], "[0]<nic:cluster>:1/RoCE [1]<nic:cluster>:1/RoCE [RO]; OOB <nic:cluster>:<node:sub><0>"));
    c.ck("★치환표가 모르는 호스트 = 노드 None(추측 ✗)", eqs(&by_get(&by, "env-echo", "NCCL_NET_PLUGIN", None)["value"], "spcx"));
    c.ck("★NCCL_ 이 아닌 'set to'(P2P Chunksize)는 env 가 아니다", !env.iter().any(|r| has(&r["key"], "Chunksize") || eqs(&r["value"], "262144")));
    c.ck("★PII 용어는 값으로 새지 않는다(치환 <redacted>)", !ud(&J::List(env.clone())).contains("fixturesecretterm")
        && eqs(&by_get(&by, "env-echo", "NCCL_DEBUG_SUBSYS", Some("main"))["value"], "<redacted>"));
    c.ck("★색인에 없는 레벨 디렉터리(level_02)의 로그는 이 측정이 아니다", !env.iter().any(|r| eqs(&r["key"], "NCCL_IB_TIMEOUT")));
    c.ck("★주소 리터럴 비유출", !ad(&J::List(env.clone())).contains(&ip_sub));
    c.ck("★줄 끝 ANSI 리셋은 값이 아니다(제거 후 정수 마침표도 뗀다)", eqs(&by_get(&by, "env-echo", "NCCL_CROSS_NIC", Some("main"))["value"], "1"));
    let man2 = jd! {"nodes" => jl![jd! {"role" => "main", "hostname" => "fixhost-m1"}, jd! {"role" => "sub", "hostname" => "fixhost-s1"},
        jd! {"role" => "sub", "hostname" => "fixhost-s2", "host" => ip_sub.as_str()}], "interconnect" => man["interconnect"].clone()};
    let env2 = measurement_env_observed(repo, &rep(ev, &[("manifest", man2)]))?;
    let g2 = env2.iter().find(|r| eqs(&r["key"], "NCCL_IB_GID_INDEX") && eqs(&r["node_label"], "sub2")).cloned().unwrap_or_default();
    c.ck("★계약 밖 노드 라벨(sub2 — 같은 role 둘)은 node None + node_label 로(main|sub 로 접지 않는다)",
        g2.truthy() && g2["node"].is_null() && !env2.iter().any(|r| !(r["node"].is_null() || eqs(&r["node"], "main") || eqs(&r["node"], "sub"))));
    let kp: Vec<&J> = env.iter().filter(|r| eqs(&r["kind"], "key-present")).collect();
    c.ck("★FC-14 값 없는 키 존재 경고 = kind key-present · value None · 값 미관측 주 · 키당 한 행(첫 줄) — '관측 없음' 으로 접지 않는다",
        kp.iter().map(|r| st(&r["key"])).collect::<Vec<_>>() == strs(&["VLLM_PLE_MMAP", "VLLM_REF"])
        && kp.iter().all(|r| r["value"].is_null() && has(&r["value_note"], "값 미관측")) && !kp.is_empty() && eqs(&kp[0]["source"], &format!("{lrel}:L{}", base_ln + 11)));
    c.ck("★스윕이 출력 평면에 묶이지 않으면 [] (그 로그는 이 측정의 것이 아니다)",
        measurement_env_observed(repo, &rep(&evm, &[("sweep", rep(&evm["sweep"], &[("binding", J::from("simlog-copy"))]))]))?.is_empty());
    // ── 계보 후보 ──
    let ev_c = fc(repo, "c1-a", None, Some(dk))?;
    let cands: Vec<(String, String)> = ev_c["lineage_seeds"]["evidence_candidates"].items().iter().map(|x| (st(&x["path"]), st(&x["kind"]))).collect();
    let cget = |p: &str| cands.iter().rev().find(|(a, _)| a == p).map(|(_, k)| k.clone());
    c.ck("계보 후보 — 이 측정 엔진 로그(lite · 색인 레벨) · 묶인 resolved.json",
        cget(&lrel).as_deref() == Some("engine_log") && cget(&_rel(repo, &lv1)?).as_deref() == Some("engine_log")
        && cget("output/multi/resolved.json").as_deref() == Some("resolve") && !cands.iter().any(|(p, _)| p.contains("level_02")));
    let rsv = repo.join("output/multi/resolved.json");
    let rsv_text = rd(&rsv)?;
    wr0(&rsv, &rsv_text.replace("\"vllm_version\": \"0.9.0\"", "\"vllm_version\": \"0.8.0\"").replace(_SHA_A, &"b".repeat(40)))?;
    let ev_nr = fc(repo, "c1-a", None, Some(dk));
    wr0(&rsv, &rsv_text)?;
    let ev_nr = ev_nr?;
    c.ck("★다른 버전의 해소값(묶이지 않음)은 계보 후보가 아니다", ev_nr["resolve"].is_null()
        && !ev_nr["lineage_seeds"]["evidence_candidates"].items().iter().any(|x| eqs(&x["path"], "output/multi/resolved.json")));
    wrb(&lite, &lite0)?;
    rm(&lv1)?;
    rm(&sweep.join("level_02/engine_c1-a.log"))?;
    rmdir(&sweep.join("level_02"))?;
    // ── attestation 범위 ──
    let att = repo.join("output/multi/benchlog/attestation_c1-a.json");
    utime(&att, 1767225600)?;
    let sc = attestation_scope(repo, &ev_c)?.unwrap_or_default();
    c.ck("attestation 범위 — 셀 이름 바인딩 · 같은 config · 실행 동일성 미검증 · written_utc = mtime(출처가 그렇게 말한다)",
        sc.truthy() && eqs(&sc["config"], "c1-a") && eqs(&sc["bound_by"], "cell-name") && sw_(&sc["scope"], "same config(c1-a)")
        && has(&sc["scope"], "미검증") && eqs(&sc["written_utc"], "2026-01-01T00:00:00Z") && has(&sc["written_utc_source"], "mtime"));
    let att_cap = |u: &str| rep(&ev_c, &[("attestation", rep(&ev_c["attestation"], &[("captured_utc", J::from(u))]))]);
    c.ck("attestation 범위 — 문서 시각 칸이 있으면 그것", attestation_scope(repo, &att_cap("2026-01-02T00:00:00Z"))?.map_or(false, |s| eqs(&s["written_utc_source"], "attestation captured_utc")));
    let mk_ = _measured_key(repo, &ev_c)?;
    let sc7 = attestation_scope(repo, &att_cap("2099-01-01T00:00:00Z"))?.unwrap_or_default();
    c.ck("★F7 작성 > 측정 끝 = 후작성 관측 · 시각만으로 재기동/다른 실행을 단정하지 않는다", mk_.is_some() && eqs(&sc7["timing"], "after-measurement")
        && has(&sc7["scope"], "측정 뒤 작성된 관측") && has(&sc7["timing_note"], "미검증") && !has(&sc7["scope"], "재기동") && !has(&sc7["timing_note"], "재기동")
        && sc7["measure_window"]["end_utc"].as_str() == mk_.as_deref());
    c.ck("F7 음성대조: 작성 ≤ 측정 끝 = 측정 뒤 ✗(측정 전 또는 끝 이전)", (eqs(&sc["timing"], "before-measurement") || eqs(&sc["timing"], "before-end")) && !has(&sc["scope"], "측정 뒤"));
    c.ck("F7 호스트 사실 출처에도 같은 문장(드라이버 · wheel 메타 출처가 창을 말한다)",
        _attestation_timing_note(&att_cap("2099-01-01T00:00:00Z"))?.unwrap_or_default().ends_with("측정 뒤 작성된 관측(측정 시점·같은 기동 여부는 시각만으로 미검증)"));
    let dk2 = |created: &str| -> Rc<RefCell<J>> {
        let mut images = Map::new();
        images.insert("fx:tag", jd! {"Id" => format!("sha256:{}", "e".repeat(64)), "Created" => created, "Config" => J::dict()});
        Rc::new(RefCell::new(jd! {"images" => J::Dict(images), "history" => J::dict()}))
    };
    let run_with = |st_: Rc<RefCell<J>>, e: &mut J| -> HR<()> {
        let f = move |argv: &[String]| -> Result<Proc, String> { Ok(_fake_docker_run(&mut st_.borrow_mut(), argv)) };
        _local_image_digest_r(repo, e, Some(&f))
    };
    let mut e2d = rep(&ev_c, &[("build_identity", rep(&ev_c["build_identity"], &[("image_digest", J::Null), ("image_tag", J::from("fx:tag"))]))]);
    run_with(dk2("2025-12-31T00:00:00.123456789+09:00"), &mut e2d)?;
    let loc = e2d["build_identity"]["image_digest_local"].clone();
    c.ck("F2 로컬 이미지 Created < 측정 → same_build · Id · 출처(태그는 가변 포인터 · 측정 전 빌드)",
        loc["same_build"] == J::Bool(true) && eqs(&loc["id"], &format!("sha256:{}", "e".repeat(64))) && has(&loc["source"], "측정 전 빌드")
        && has(&loc["source"], "태그는 가변 포인터") && e2d["build_identity"]["image_digest"].is_null());
    run_with(dk2("2099-01-01T00:00:00Z"), &mut e2d)?;
    let loc2 = e2d["build_identity"]["image_digest_local"].clone();
    let mut ev_c = ev_c;
    run_with(dk2("2025-12-31T00:00:00Z"), &mut ev_c)?;
    c.ck("★F2 음성대조: Created ≥ 측정 → same_build ✗(측정 이미지 아님) · 측정 digest 가 있으면 관측하지 않는다",
        loc2["same_build"] == J::Bool(false) && has(&loc2["source"], "측정 뒤 빌드") && !inn(&ev_c["build_identity"], "image_digest_local"));
    // F9 native
    let nd = repo.join("docs/simlog/fx_native/logs");
    mkd(&nd)?;
    wr0(&nd.join("main-vllm-serve.log"), "INFO boot\nfxhost:1:1 [0] NCCL INFO NCCL_IB_DISABLE set by environment to 1.\n")?;
    let ev9 = rep(&ev_c, &[("plane", J::from("native")), ("serve_proof", rep(&ev_c["serve_proof"], &[("evidence_dir", J::from("docs/simlog/fx_native"))]))]);
    let lg9 = _measured_engine_logs(repo, &ev9)?;
    let rows9 = measurement_env_observed(repo, &ev9)?;
    let mlog = nd.join("main-vllm-serve.log");
    c.ck("F9 native — 보존 엔진 로그가 측정 로그 · env 관측 원천(창 표지 없음)",
        lg9.0.contains(&mlog) && lg9.1.contains("evidence_dir")
        && rows9.iter().any(|r| eqs(&r["key"], "NCCL_IB_DISABLE") && sw_(&r["source"], "docs/simlog/fx_native/logs/main-vllm-serve.log") && !inn(r, "log_window")));
    c.ck("★F9 음성대조: docker 평면 · 절대경로 · 저장소 밖 evidence_dir 은 읽지 않는다",
        _native_preserved_engine_logs(repo, &rep(&ev9, &[("plane", J::from("docker"))]))?.is_empty()
        && _native_preserved_engine_logs(repo, &rep(&ev9, &[("serve_proof", jd! {"evidence_dir" => core::posix(nd.parent().unwrap())})]))?.is_empty()
        && _native_preserved_engine_logs(repo, &rep(&ev9, &[("serve_proof", jd! {"evidence_dir" => "docs/../../x"})]))?.is_empty()
        && !_measured_engine_logs(repo, &ev_c)?.0.contains(&mlog));
    rmtree(&repo.join("docs/simlog/fx_native"));
    let att_text = rd(&att)?;
    rm(&att)?;
    let att_build = repo.join("output/multi/benchlog/attestation_c1-build.json");
    wj(&att_build, &jd! {"config" => "c1-build", "image_tag" => "fixture:0.9.0-source", "checks" => jl![jd! {"axis" => "torch", "node" => "sub", "observed" => "x"}]})?;
    let ev_lb = fc(repo, "c1-a", None, Some(dk))?;
    let sc2 = attestation_scope(repo, &ev_lb)?.unwrap_or_default();
    c.ck("★compose 라벨 대체로 묶인 다른 config = 이 셀 실행이 아니다(계약 문자열 그대로)",
        sc2.truthy() && eqs(&sc2["scope"], ATTESTATION_SCOPE_OTHER_RUN) && eqs(&sc2["bound_by"], "compose-label") && eqs(&sc2["config"], "c1-build"));
    rm(&att_build)?;
    c.ck("★묶인 attestation 이 없으면 None", attestation_scope(repo, &fc(repo, "c1-a", None, Some(dk))?)?.is_none());
    wr0(&att, &att_text)?;
    round3b(c, repo, td, ev, &ev_c, dk, &sweep, &env)
}

#[allow(clippy::too_many_arguments)]
fn round3b(c: &mut Ck, repo: &Path, td: &Path, ev: &J, ev_c: &J, dk: Dk, sweep: &Path, env: &[J]) -> HR<()> {
    // ── 같은 셀의 다른 측정 창 ──
    let ident = ev_c["lineage_seeds"]["identity"].clone();
    let mut made: Vec<PathBuf> = Vec::new();
    let mut rec = |rid: &str, mu: &str, cell: &str, identity: Option<J>, node: &str| -> HR<()> {
        let sl = format!("docs/simlog/26010100_{rid}");
        wj(&repo.join(&sl).join("sweep_index.json"), &jd! {"generated_utc" => mu, "meta" => jd! {"config_name" => cell, "measured_node" => node}})?;
        wj(&repo.join(core::REL_EVIDENCE_DIR).join(format!("{rid}.json")), &jd! {"identity" => identity.unwrap_or_else(|| ident.clone()),
            "benchmark" => jd! {"measured_utc" => mu, "verdict" => "PASS"}, "raw_log_paths" => jd! {"simlog" => sl.as_str()}})?;
        made.extend([repo.join(&sl).join("sweep_index.json"), repo.join(&sl), repo.join(core::REL_EVIDENCE_DIR).join(format!("{rid}.json"))]);
        Ok(())
    };
    rec("old_c1_a", "2026-01-01T05:00:00Z", "c1-a", None, "cluster")?;
    rec("gap_c1_a", "2026-01-02T01:35:00Z", "c1-a", None, "cluster")?;
    rec("kv_c1_a", "2026-01-02T01:45:00Z", "c1-a", None, "cluster")?;
    rec("win_c1_a", "2026-01-02T02:40:00Z", "c1-a", None, "cluster")?;
    rec("late_c1_a", "2026-01-02T03:30:00Z", "c1-a", None, "cluster")?;
    rec("same_c1_a", _M_UTC, "c1-a", None, "cluster")?;
    rec("othercell_c1_a", "2026-01-01T06:00:00Z", "c1-b", None, "cluster")?;
    rec("othermodel_c1_a", "2026-01-01T07:00:00Z", "c1-a", Some(rep(&ident, &[("model", J::from("different-model"))])), "cluster")?;
    let meas = |tl: &[J]| -> Vec<(String, J)> { tl.iter().filter(|r| eqs(&r["kind"], "measurement")).map(|r| (st(&r["utc"]), r.clone())).collect() };
    let mget = |ms: &[(String, J)], u: &str| ms.iter().rev().find(|(a, _)| a == u).map(|(_, r)| r.clone()).unwrap_or_default();
    let ms = meas(&event_timeline(repo, ev_c)?);
    let gap = mget(&ms, "2026-01-02T01:35:00Z");
    c.ck("과거 측정 — 원장 창 밖 = 'measurement (no budget event recorded)' · 출처 = 발행 기록 measured_utc 줄 · node None(축은 detail)",
        eqs(&gap["label"], "measurement (no budget event recorded)") && gap["node"].is_null() && has(&gap["detail"], "측정 노드 축 cluster")
        && sw_(&gap["source"], "docs/_evidence/gap_c1_a.json:") && has(&gap["detail"], "측정 2026-01-02T03:04:05Z 전")
        && has(&gap["detail"], "직후 예산 행 budget_declare(smoke-c1-a-kv8g)"));
    c.ck("★그 시각을 관측하지 않은 원장(sub · 기록 02:30 한 점)은 '선언 없음' 의 증인이 아니다 — 관측 밖이라 적는다",
        has(&gap["detail"], "이 시각을 관측한 원장(main)") && has(&gap["detail"], "이 시각을 관측하지 않은 원장: sub(기록 2026-01-02T02:30:00Z~"));
    let old = mget(&ms, "2026-01-01T05:00:00Z");
    c.ck("★원장 기록 범위 밖(모든 노드 첫 행 이전)의 측정 = 'ledger unobserved'(선언 없음이라 말하지 않는다)",
        eqs(&old["label"], "measurement (ledger unobserved)") && has(&old["detail"], "기록 범위 밖"));
    c.ck("★원장이 끝난 뒤의 측정 = 'ledger unobserved'(sub 의 열린 채 끝난 창이 덮는다고 읽지 않는다)",
        eqs(&mget(&ms, "2026-01-02T03:30:00Z")["label"], "measurement (ledger unobserved)"));
    let src = st(&gap["source"]);
    let (src_f, src_ln) = src.rsplit_once(':').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    let ln = _as_int(&J::from(src_ln.as_str()))?;
    let line_ok = match ln {
        Some(n) if n >= 1 => rd(&repo.join(&src_f)).ok().and_then(|t| t.split('\n').nth((n - 1) as usize).map(|l| l.contains("2026-01-02T01:35:00Z"))).unwrap_or(false),
        _ => false,
    };
    c.ck("과거 측정 출처 줄이 실제로 measured_utc 를 든다", ln.is_some() && line_ok);
    let kv = mget(&ms, "2026-01-02T01:45:00Z");
    c.ck("★다른 라벨 선언 창 안의 측정 = 'other label's budget window'(이 셀 선언 없음)",
        eqs(&kv["label"], "measurement (other label's budget window)") && has(&kv["detail"], "smoke-c1-a-kv8g"));
    c.ck("이 셀 선언 창 안의 다른 측정 = 'budget window recorded'", eqs(&mget(&ms, "2026-01-02T02:40:00Z")["label"], "measurement (budget window recorded)"));
    let mut mk: Vec<String> = ms.iter().map(|(u, _)| u.clone()).collect();
    mk.sort();
    mk.dedup();
    c.ck("★이 측정 자신(measured_utc 같음) · 다른 셀 · 다른 모델의 기록은 행이 아니다",
        mk == strs(&["2026-01-01T05:00:00Z", "2026-01-02T01:35:00Z", "2026-01-02T01:45:00Z", "2026-01-02T02:40:00Z", "2026-01-02T03:30:00Z"]));
    let ev_dir = repo.join("docs/logs/main/events");
    let extra = ev_dir.join("zz-extra.jsonl");
    wr0(&extra, &(ad(&jd! {"kind" => "thermal_trip", "soc_temp_c" => 90, "ts" => "2026-01-02T04:00:00Z"}) + "\n"))?;
    let late = mget(&meas(&event_timeline(repo, ev_c)?), "2026-01-02T03:30:00Z");
    c.ck("★다른 노드 스트림이 열린 채 끝난 창(sub smoke-c1-b · 마지막 행 02:30)은 그 뒤 시각을 덮지 않는다(main 관측 · 창 없음 = 선언 없음)",
        eqs(&late["label"], "measurement (no budget event recorded)") && !has(&late["detail"], "smoke-c1-b") && has(&late["detail"], "이 시각을 관측하지 않은 원장: sub"));
    let dang = ev_dir.join("zz-dangling.jsonl");
    wr0(&dang, &(ad(&jd! {"kind" => "budget_declare", "label" => "smoke-zz-dangling", "ts" => "2026-01-02T01:31:00Z"}) + "\n"))?;
    let g2 = mget(&meas(&event_timeline(repo, ev_c)?), "2026-01-02T01:35:00Z");
    c.ck("★스트림 끝에서 열린 채 끝난 창 뒤의 시각 = 'budget window closure unobserved'(덮었다고도 · 선언 없었다고도 말하지 않는다)",
        eqs(&g2["label"], "measurement (budget window closure unobserved)") && has(&g2["detail"], "smoke-zz-dangling")
        && has(&g2["detail"], "닫힘 미관측(스트림 마지막 행 2026-01-02T01:31:00Z)"));
    rm(&dang)?;
    rm(&extra)?;
    let ef = event_files(repo, ev_c)?;
    c.ck("과거 측정의 발행 기록·simlog 색인이 계보 후보 입력", ef.iter().any(|x| x == "docs/_evidence/old_c1_a.json")
        && ef.iter().any(|x| x == "docs/simlog/26010100_old_c1_a/sweep_index.json"));
    let ev_nl = rep(ev_c, &[("node", J::from("main")), ("declaration", jd! {"nodes" => jl![jd! {"node_id" => "nope", "role" => "main"}]})]);
    let labs: Vec<String> = { let mut v: Vec<String> = meas(&event_timeline(repo, &ev_nl)?).iter().map(|(_, r)| st(&r["label"])).collect(); v.sort(); v.dedup(); v };
    c.ck("★원장 노드가 없으면 'ledger unobserved'(선언 없음이라 말하지 않는다)", labs == strs(&["measurement (ledger unobserved)"]));
    for p in &made {
        if p.is_file() {
            rm(p)?;
        }
    }
    for p in &made {
        if p.is_dir() {
            rmdir(p)?;
        }
    }
    // ── 엔진 자기보고(배너) 3계층 ──
    let evb = fc(repo, "c1-a", None, Some(dk))?;
    let vo0 = vllm_observed(&evb, None)?;
    c.ck("★배너 원천이 없으면 unobserved(인증서 값을 자기보고로 옮기지 않는다)", vo0["engine_self_report"].is_null()
        && eqs(&vo0["engine_self_report_basis"], "unobserved") && eqs(&vo0["certificate_vllm_version"], "0.9.0"));
    let bl = repo.join("output/multi/benchlog");
    let mut made_b: Vec<PathBuf> = Vec::new();
    let sweep_dir = |made_b: &mut Vec<PathBuf>, name: &str, gu: &str, digest: &str, tag: &str, build: &str| -> HR<PathBuf> {
        let d = bl.join(format!("sweep_{name}"));
        mkd(&d.join("level_01"))?;
        wj(&d.join("sweep_index.json"), &jd! {"config" => name, "generated_utc" => gu, "levels" => jl![jd! {"level" => 1}], "meta" => jd! {
            "config_name" => name, "image_digest" => digest, "image_digest_source" => "measured(docker inspect .Image)",
            "image_tag" => tag, "image_tag_source" => "measured(docker inspect)", "vllm_build" => build}})?;
        made_b.push(d.clone());
        Ok(d)
    };
    let fail_log = |made_b: &mut Vec<PathBuf>, cfg: &str, stamp: &str, ver: &str, tag: &str, pre: &str| -> HR<()> {
        let d = bl.join(format!("serve_fail_{cfg}"));
        std::fs::create_dir(&d).map_err(|e| core::io_err(&e, &d))?;
        wr0(&d.join("master_exited.log"), &format!("== banner ==\n{pre}(APIServer pid=1) INFO {stamp} [api_utils.py:1]  ▄▄ █  version {ver}\n"))?;
        wr0(&repo.join(format!("output/multi/envs/.env.{cfg}")), &format!("IMAGE_TAG={tag}\n"))?;
        made_b.extend([d, repo.join(format!("output/multi/envs/.env.{cfg}"))]);
        Ok(())
    };
    let tg = "fixture:0.9.0-source";
    sweep_dir(&mut made_b, "c1-z", "2026-01-01T12:00:00Z", _DIGEST, tg, "NA")?;
    fail_log(&mut made_b, "c1-y", "01-02 01:00:00", "0.9.0rc2.dev5+gfix", tg, "(Worker pid=2) INFO 12-30 00:00:00 [p.py:1] load 5%\r")?;
    fail_log(&mut made_b, "c1-late", "01-03 01:00:00", "0.9.9-late", tg, "")?;
    fail_log(&mut made_b, "c1-othertag", "01-02 01:30:00", "0.9.8-other", "fixture:0.8.0-source", "")?;
    let vo = vllm_observed(&evb, None)?;
    c.ck("digest 미기록 로그 = 같은 태그 + 앞뒤 측정 스윕 괄호로 **추론**(basis · 출처가 추론이라 말한다)",
        eqs(&vo["engine_self_report"], "0.9.0rc2.dev5+gfix") && eqs(&vo["engine_self_report_basis"], "same-digest-inferred")
        && has(&vo["engine_self_report_source"], "추론") && has(&vo["engine_self_report_source"], "serve_fail_c1-y/master_exited.log:2")
        && has(&vo["engine_self_report_source"], "컨테이너 시계 01-02 01:00:00"));
    c.ck("★괄호 밖(뒤 스윕 없음) · 다른 IMAGE_TAG 의 로그는 쓰지 않는다", !ad(&vo).contains("0.9.9-late") && !ad(&vo).contains("0.9.8-other"));
    c.ck("추론 배너 로그는 계보 후보", fc(repo, "c1-a", None, Some(dk))?["lineage_seeds"]["evidence_candidates"].items().iter()
        .any(|x| eqs(&x["path"], "output/multi/benchlog/serve_fail_c1-y/master_exited.log")));
    let br1 = _bracketed("12-31 18:00:00", &jl![jd! {"utc" => "2025-12-31T12:00:00Z"}, jd! {"utc" => _M_UTC}], Some(_M_UTC))?;
    let br2 = _bracketed("12-31 18:00:00", &jl![jd! {"utc" => "2026-01-01T12:00:00Z"}, jd! {"utc" => _M_UTC}], Some(_M_UTC))?;
    c.ck("★연도 없는 컨테이너 시각 — 측정 연도로 괄호 밖이면 전해로 읽는다(연말 로그 · 연초 측정)",
        br1.map(|x| x.0).as_deref() == Some("2025-12-31T18:00:00Z") && br2.is_none());
    let z_idx = bl.join("sweep_c1-z/sweep_index.json");
    let z_text = rd(&z_idx)?;
    let mut z_doc = lj(&z_text)?;
    pm(&mut z_doc, &["meta"]).set("image_tag", "fixture:0.8.0-source");
    wj(&z_idx, &z_doc)?;
    let vz = vllm_observed(&evb, None)?;
    c.ck("★괄호 스윕은 같은 **태그**로 그 digest 를 잰 것만(같은 digest · 다른 태그 = 괄호 ✗ → 추론 ✗)",
        vz["engine_self_report"].is_null() && eqs(&vz["engine_self_report_basis"], "unobserved"));
    wr0(&z_idx, &z_text)?;
    fail_log(&mut made_b, "c1-y2", "01-02 02:00:00", "0.9.0rc2.dev6+gother", tg, "")?;
    let vcf = vllm_observed(&evb, None)?;
    c.ck("★같은 계층에서 값이 둘이면 고르지 않는다(conflict)", vcf["engine_self_report"].is_null() && eqs(&vcf["engine_self_report_basis"], "conflict"));
    let z2 = sweep_dir(&mut made_b, "c1-w", "2026-01-01T13:00:00Z", _DIGEST, tg, "NA")?;
    wr0(&z2.join("level_01/engine_c1-w.log"), "x\n(APIServer pid=1) INFO 01-01 12:59:00 [a.py:1] vLLM API server version 0.9.0rc2.dev7+grec\n")?;
    let vr = vllm_observed(&evb, None)?;
    c.ck("digest 를 **기록한** 다른 스윕의 로그가 추론보다 먼저(same-digest-recorded)", eqs(&vr["engine_self_report"], "0.9.0rc2.dev7+grec")
        && eqs(&vr["engine_self_report_basis"], "same-digest-recorded") && has(&vr["engine_self_report_source"], "sweep_c1-w/level_01/engine_c1-w.log:2"));
    sweep_dir(&mut made_b, "c1-v", "2026-01-01T14:00:00Z", &format!("sha256:{}", "9".repeat(64)), tg, "7.7.7-otherimage")?;
    mkd(&bl.join("sweep_c1-broken"))?;
    wr0(&bl.join("sweep_c1-broken/sweep_index.json"), "{")?;
    made_b.push(bl.join("sweep_c1-broken"));
    let vbk = vllm_observed(&evb, None);
    let vbk_code = _code(vllm_observed(&evb, None))?;
    c.ck("★이웃 셀의 깨진 스윕 색인은 이 셀 발행을 막지 않는다(건너뛴다 · 이 측정 자신의 증거는 fail-closed 유지)",
        vbk_code.is_none() && vbk.map_or(false, |v| eqs(&v["engine_self_report_basis"], "same-digest-recorded")));
    c.ck("★다른 digest 스윕의 vllm_build 는 쓰지 않는다", !ad(&vllm_observed(&evb, None)?).contains("7.7.7"));
    let idx_p = sweep.join("sweep_index.json");
    let idx_text = rd(&idx_p)?;
    let mut idx_doc = lj(&idx_text)?;
    pm(&mut idx_doc, &["meta"]).set("vllm_build", "0.9.0rc2.dev8+gthis");
    wj(&idx_p, &idx_doc)?;
    let vt = vllm_observed(&fc(repo, "c1-a", None, Some(dk))?, None);
    wr0(&idx_p, &idx_text)?;
    let vt = vt?;
    c.ck("스윕 meta의 값만 있고 serve 경계가 없으면 epoch 귀속은 미검증", eqs(&vt["engine_self_report"], "0.9.0rc2.dev8+gthis")
        && eqs(&vt["engine_self_report_basis"], "measurement-capture-unbound") && has(&vt["engine_self_report_source"], "sweep_c1-a/sweep_index.json:"));
    made_b.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for p in &made_b {
        if p.is_dir() {
            rmtree(p);
        } else if p.is_file() {
            rm(p)?;
        }
    }
    c.ck("wheel_meta 는 관측자가 없다(엔진 로그 값을 wheel 메타로 부르지 않는다)", vt["wheel_meta"].is_null() && has(&vt["wheel_meta_source"], "엔진 로그에서"));
    c.ck("★git 이 아닌 저장소 = 인증서 producer 미검증(지어내지 않는다)", eqs(&vt["certificate_vllm_version_producer"], "unverified"));
    round3c(c, repo, td, ev, env)
}

fn round3c(c: &mut Ck, repo: &Path, td: &Path, ev: &J, env: &[J]) -> HR<()> {
    let (g, revs) = _git_fixture(td)?;
    let rv = |k: &str| st(&revs[k]);
    let sw_g = jd! {"dir" => "output/multi/benchlog/sweep_c1-a", "binding" => "output", "generated_utc" => _M_UTC, "levels" => jl![jd! {"level" => 1}],
        "index" => jd! {"lite" => jd! {"gen_tps" => 1.0}, "meta" => jd! {"bench_tool" => "vllm-bench-serve", "vllm_version" => "0.9.0", "vllm_build" => "NA",
            "image_tag_declared" => "easy-vllm:0.9.0rc2-cu130-source"}}};
    let mut evg = rep(ev, &[("repo", pj(&g)), ("sweep", sw_g.clone()), ("certificate", jd! {"measured_utc" => _M_UTC, "vllm_version" => "0.9.0"}),
        ("lineage_seeds", jd! {"evidence_candidates" => jl![jd! {"path" => "x.json", "kind" => "raw_json"}]})]);
    let draft = td.join("draft-g");
    let snaps = tool_snapshots_mut(&g, &mut evg, Some(&draft))?;
    let sn = |name: &str| snaps.iter().find(|r| eqs(&r["name"], name)).cloned().unwrap_or_default();
    let mut names: Vec<String> = snaps.iter().map(|r| st(&r["name"])).collect();
    names.sort();
    c.ck("도구 스냅숏 — 측정 시각의 체크아웃(reflog → side 브랜치 c4) · 이름·경로·rev·draft 상대 경로(계약 키)",
        names == strs(&["lite_bench.sh", "run_bench.sh", "sweep_bench.sh"]) && snaps.iter().all(|r| eqs(&r["head_at_measurement"], &rv("c4")))
        && eqs(&sn("lite_bench.sh")["git_rev"], &rv("c4")) && eqs(&sn("run_bench.sh")["git_rev"], &rv("c1"))
        && eqs(&sn("run_bench.sh")["snapshot_rel"], &format!("inputs/sources/run_bench.sh@{}", &rv("c1")[..12]))
        && snaps.iter().all(|r| ["name", "repo_path", "git_rev", "snapshot_rel"].iter().all(|k| inn(r, k))) && !snaps.is_empty() && has(&snaps[0]["rev_method"], "reflog"));
    c.ck("★committer date 로 고르면 main 의 c2(run_bench v2)였다 — reflog 가 실제 체크아웃(side)을 고른다",
        rd(&draft.join(st(&sn("run_bench.sh")["snapshot_rel"]))).ok().as_deref() == Some(st(&revs["rb_v1"]).as_str())
        && rd(&draft.join(st(&sn("lite_bench.sh")["snapshot_rel"]))).ok().as_deref() == Some(st(&revs["lb_side"]).as_str()));
    let mut bytes_ok = true;
    for r in &snaps {
        let spec = format!("{}:{}", st(&r["git_rev"]), st(&r["repo_path"]));
        bytes_ok &= rdb(&draft.join(st(&r["snapshot_rel"]))).ok() == core::git_bytes(&g, &["show", &spec], true)?;
    }
    c.ck("스냅숏 바이트 = git show <rev>:<path>", bytes_ok);
    let rb = sn("run_bench.sh");
    c.ck("★다른 브랜치의 측정 시점 판본이면 다음 변경을 말하지 않는다(at..HEAD 는 무관 커밋) · '변경 없음' 이 아니라 미관측이라 적는다",
        rb["next_rev"].is_null() && sw_(&rb["next_rev_basis"], "미관측") && has(&rb["next_rev_basis"], "후손이 아니다"));
    let dn = st(&sn("sweep_bench.sh")["driver_note"]);
    c.ck("★드라이버 배제 — 그 판본 broad_search 는 --tool guidellm(주석의 --tool 은 세지 않는다) ≠ 이 측정 vllm-bench-serve",
        !names.iter().any(|n| n == "broad_search.sh") && dn.contains("broad_search.sh 배제") && dn.contains("배제로 추론") && dn.contains("저장소 밖 호출자"));
    let cands: Vec<J> = evg["lineage_seeds"]["evidence_candidates"].items().to_vec();
    let cand = |p: &str| cands.iter().find(|x| eqs(&x["path"], p)).cloned().unwrap_or_default();
    let mut lin_ok = true;
    for r in &snaps {
        let cd = cand(&st(&r["snapshot_rel"]));
        lin_ok &= eqs(&cd["origin"], &format!("git:{}:{}", st(&r["git_rev"]), st(&r["repo_path"])))
            && crate::lineage::_tool_candidate_problem(&g, &cd)?.is_none();
    }
    c.ck("계보 후보 = tool-source@rev · origin git:<rev>:<path> · 기존 후보 보존 · lineage 검증 통과", lin_ok && cand("x.json").truthy());
    let n_before = evg["lineage_seeds"]["evidence_candidates"].len();
    tool_snapshots_mut(&g, &mut evg, Some(&draft))?;
    c.ck("재호출 멱등(후보 중복 ✗)", evg["lineage_seeds"]["evidence_candidates"].len() == n_before);
    let parts: Vec<Vec<String>> = rglob(&draft).into_iter().filter(|p| p.is_file()).map(|p| {
        p.strip_prefix(&draft).unwrap().components().take(2).map(|x| x.as_os_str().to_string_lossy().into_owned()).collect()
    }).collect();
    c.ck("★draft 밖에 쓰지 않는다(inputs/sources 만)", parts == vec![strs(&["inputs", "sources"]); snaps.len()]);
    // ── 엔진 로그 꼬리 창 ──
    let sdir = g.join("output/multi/benchlog/sweep_c1-a");
    mkd(&sdir.join("level_01"))?;
    let nccl = "(EngineCore pid=1) fixhost-m1:7:9 [0] NCCL INFO NCCL_IB_GID_INDEX set by environment to 3.\n";
    wr0(&sdir.join("lite_engine_c1-a.log"), &format!("x\ny\n{nccl}"))?;
    wr0(&sdir.join("level_01/engine_c1-a.log"), &nccl.replace("GID_INDEX", "TC"))?;
    let wg = keyed(&measurement_env_observed(&g, &evg)?);
    let c4s = rv("c4")[..12].to_string();
    c.ck("log_window — lite 로그가 측정 시점 lite_bench.sh 의 `tail -3 > \"$ELOG\"` 상한에 닿았다(도구·rev·줄 출처)",
        wg.get("NCCL_IB_GID_INDEX").map_or(false, |r| eqs(&r["log_window"], &format!("tail-3(lite_bench.sh@{c4s} L2)"))));
    c.ck("★상한 줄이 없는 도구(주석의 tail · ELOG 로 가지 않는 tail 은 세지 않는다)의 로그는 log_window 없음",
        wg.get("NCCL_IB_TC").map_or(false, |r| !inn(r, "log_window")));
    wr0(&sdir.join("lite_engine_c1-a.log"), &format!("x\n{nccl}"))?;
    c.ck("★상한에 닿지 않은 로그(줄 수 < N)는 log_window 없음(전체 출력일 수 있다)",
        keyed(&measurement_env_observed(&g, &evg)?).get("NCCL_IB_GID_INDEX").map_or(false, |r| !inn(r, "log_window")));
    c.ck("★git 판본을 못 고르면(git 아님) log_window 없음 — 상한을 지어내지 않는다", !env.iter().any(|r| inn(r, "log_window")));
    wr0(&sdir.join("lite_engine_c1-a.log"), &format!("x\ny\n{nccl}"))?;
    rm(&sdir.join("level_01/engine_c1-a.log"))?;
    let bw = _engine_banner(&g, &evg)?;
    c.ck("F1 배너 미관측 + 읽은 로그 전부 꼬리 창 → 출처에 '보존 꼬리 창 tail-3(도구@rev L줄)' · 머리 캡처 없는 측정 명시",
        eqs(&bw["basis"], "unobserved") && has(&bw["source"], &format!("보존 꼬리 창 tail-3(lite_bench.sh@{c4s} L2)")) && has(&bw["source"], "머리 캡처"));
    wr0(&sdir.join("lite_engine_c1-a.log"), &format!("x\n{nccl}"))?;
    c.ck("★F1 음성대조: 상한에 닿지 않은 로그(전체 출력일 수 있다)면 창 범위를 붙이지 않는다", !has(&_engine_banner(&g, &evg)?["source"], "보존 꼬리 창"));
    wr0(&sdir.join("lite_engine_c1-a.log"), &format!("x\ny\n{nccl}"))?;
    wr0(&sdir.join("lite_engine_head_c1-a.log"), "INFO 01-02 00:10:00 Initializing a V1 LLM engine (v0.9.0rc2.dev9+ghead)\n")?;
    let bh = _engine_banner(&g, &evg)?;
    c.ck("★F1 머리 캡처 배너는 읽되 serve 경계 없는 기동 epoch를 현재 실행으로 승격하지 않는다", eqs(&bh["value"], "0.9.0rc2.dev9+ghead")
        && eqs(&bh["basis"], "measurement-capture-unbound") && has(&bh["source"], "lite_engine_head_c1-a.log:1"));
    wr0(&sdir.join("lite_engine_head_c1-a.log"), &format!("x\n{}", nccl.replace("GID_INDEX", "HEADKEY")))?;
    wj(&sdir.join("lite_raw_c1-a.json"), &jd! {"config_name" => "c1-a", "measured_utc" => _M_UTC, "engine_log_head_lines" => 2,
        "engine_log_head" => "/abs/x/lite_engine_head_c1-a.log"})?;
    let wh = keyed(&measurement_env_observed(&g, &evg)?);
    let hk = wh.get("NCCL_IB_HEADKEY").cloned().unwrap_or_default();
    let gk = wh.get("NCCL_IB_GID_INDEX").cloned().unwrap_or_default();
    c.ck("★P15 머리 캡처 행 = log_head_window head-N(lite_raw engine_log_head_lines) · 꼬리 창 표지 없음 · 꼬리 행은 log_window 만",
        eqs(&hk["log_head_window"], "head-2(output/multi/benchlog/sweep_c1-a/lite_raw_c1-a.json engine_log_head_lines)")
        && !inn(&hk, "log_window") && inn(&gk, "log_window") && !inn(&gk, "log_head_window"));
    wj(&sdir.join("lite_raw_c1-a.json"), &jd! {"config_name" => "c1-a", "measured_utc" => _M_UTC, "engine_log_head_lines" => 600,
        "engine_log_head" => "/abs/x/lite_engine_head_c1-a.log"})?;
    c.ck("P15 음성대조: 머리 캡처가 상한에 닿지 않으면(줄 수 < N · 전체 출력일 수 있다) log_head_window 없음",
        keyed(&measurement_env_observed(&g, &evg)?).get("NCCL_IB_HEADKEY").map_or(false, |r| !inn(r, "log_head_window")));
    rmtree(&g.join("output"));
    let ev_t2 = rep(&evg, &[("certificate", jd! {"measured_utc" => "2026-01-02T00:30:00Z", "vllm_version" => "0.9.0"}),
        ("sweep", rep(&sw_g, &[("generated_utc", J::from("2026-01-02T00:30:00Z"))])), ("lineage_seeds", J::dict())]);
    let ts = |e: &J, d: &str| -> HR<Vec<J>> { let mut e = e.clone(); tool_snapshots_mut(&g, &mut e, Some(&td.join(d))) };
    let sn2 = ts(&ev_t2, "draft-g2")?;
    let s2 = |n: &str| sn2.iter().find(|r| eqs(&r["name"], n)).cloned().unwrap_or_default();
    c.ck("main 체크아웃 시각의 측정 — run_bench v2(c2) · 다음 변경 c3 · 측정 뒤 커밋 v3 는 싣지 않는다 · 변경 없는 도구는 '변경 없음'",
        eqs(&s2("run_bench.sh")["git_rev"], &rv("c2")) && eqs(&s2("run_bench.sh")["next_rev"], &rv("c3"))
        && has(&s2("run_bench.sh")["next_rev_basis"], "첫 변경") && s2("sweep_bench.sh")["next_rev"].is_null()
        && has(&s2("sweep_bench.sh")["next_rev_basis"], "변경 없음") && s2("run_bench.sh")["git_rev_note"].is_null()
        && rd(&td.join("draft-g2").join(st(&s2("run_bench.sh")["snapshot_rel"]))).ok().as_deref() == Some("# run_bench v2\n"));
    let ev_gl = rep(&ev_t2, &[("sweep", rep(&ev_t2["sweep"], &[("index", rep(&sw_g["index"], &[("meta", rep(&sw_g["index"]["meta"], &[("bench_tool", J::from("guidellm"))]))]))]))]);
    let gl_rows = ts(&ev_gl, "draft-g3")?;
    c.ck("★bench_tool 이 드라이버 --tool 과 맞으면 드라이버 후보로 싣는다(미검증이라 적는다 · '직접 불렀다' 추론 ✗)",
        gl_rows.iter().any(|r| eqs(&r["name"], "broad_search.sh") && eqs(&r["role"], "driver-candidate") && has(&r["why"], "미검증"))
        && !gl_rows.iter().any(|r| has(&r["driver_note"], "배제로 추론")));
    let nolite = rep(&ev_t2, &[("sweep", rep(&ev_t2["sweep"], &[("index", jd! {"meta" => sw_g["index"]["meta"].clone()})]))]);
    c.ck("★색인에 lite 기록이 없으면 lite_bench.sh 를 싣지 않는다", !ts(&nolite, "draft-g4")?.iter().any(|r| eqs(&r["name"], "lite_bench.sh")));
    let pre = rep(&ev_t2, &[("certificate", jd! {"measured_utc" => "2025-01-01T00:00:00Z"}), ("sweep", rep(&sw_g, &[("generated_utc", J::Null)]))]);
    c.ck("★측정 시각 이전 커밋이 없으면 []", ts(&pre, "draft-g5")?.is_empty());
    let mut ev_r = rep(ev, &[("lineage_seeds", J::dict())]);
    c.ck("★git 이 아니면 []", tool_snapshots_mut(repo, &mut ev_r, Some(&td.join("draft-g6")))?.is_empty());
    let mut evg2 = evg.clone();
    c.ck("★draft 없이 부르면 차단", _code(tool_snapshots_mut(&g, &mut evg2, None))?.as_deref() == Some("HINT_DRAFT_REQUIRED"));
    let lh = g.join(".git/logs/HEAD");
    let lh_text = rdb(&lh)?;
    rm(&lh)?;
    let fbk = _head_at(&g, Some(_M_UTC));
    wrb(&lh, &lh_text)?;
    let (at_fb, why_fb) = fbk?;
    c.ck("★reflog 가 없으면 committer date 대체 — 방법이 그렇게 말한다(브랜치 미검증)", at_fb.as_deref() == Some(rv("c2").as_str())
        && why_fb.contains("committer date") && why_fb.contains("미검증"));
    let gone = "e".repeat(40);
    let ep = core::parse_utc("2026-01-02T03:00:00Z")?.epoch();
    let mut nb = lh_text.clone();
    nb.extend_from_slice(format!("{} {gone} {} <{}> {ep} +0000\tcheckout: moving from side to gone\n", rv("c4"), core::SYNTHETIC_NAME, core::synthetic_email()).as_bytes());
    wrb(&lh, &nb)?;
    let gr = _head_at(&g, Some(_M_UTC));
    wrb(&lh, &lh_text)?;
    let (at_gone, why_gone) = gr?;
    c.ck("★reflog 항목의 커밋이 없으면 그 전 항목으로 조용히 넘어가지 않는다 — committer date 대체 · 사유 = 오브젝트 부재",
        at_gone.as_deref() == Some(rv("c2").as_str()) && why_gone.contains("오브젝트 부재") && why_gone.contains("committer date"));
    let cvp = |e: &J| _cert_vllm_producer(&g, e, &J::Null);
    let (lab, psrc) = cvp(&evg)?;
    c.ck("인증서 vllm_version producer = 측정 시점 sweep_bench 판본의 이미지 태그 규칙(검증 · 줄 번호 · 엔진 자기보고 아님)",
        lab == "image-tag-line" && psrc.contains("L6·L11") && psrc.contains("엔진 자기보고가 아니다") && psrc.contains("L9"));
    c.ck("★규칙 결과가 인증서와 다르면 미검증(덮어쓰기였을 수 있다)",
        cvp(&rep(&evg, &[("certificate", rep(&evg["certificate"], &[("vllm_version", J::from("0.9.1"))]))]))?.0 == "unverified");
    let with_meta = |kv: &[(&str, &str)]| -> J {
        let extra: Vec<(&str, J)> = kv.iter().map(|(k, v)| (*k, J::from(*v))).collect();
        rep(&evg, &[("sweep", rep(&sw_g, &[("index", rep(&sw_g["index"], &[("meta", rep(&sw_g["index"]["meta"], &extra))]))]))])
    };
    c.ck("태그 규칙이 값을 못 내면 배너 x.y.z 접두(그 판본 우선순위)",
        cvp(&with_meta(&[("image_tag_declared", "other:tag"), ("vllm_build", "0.9.0rc2.dev1+gx")]))?.0 == "engine-banner-prefix");
    c.ck("★태그 규칙이 다른 값(0.9.1)을 냈으면 배너 접두가 맞아도 미검증(덮어쓰기 신호)",
        cvp(&with_meta(&[("image_tag_declared", "easy-vllm:0.9.1rc2-cu130-source"), ("vllm_build", "0.9.0rc2.dev1+gx")]))?.0 == "unverified");
    c.ck("★배너 접두는 x.y.z 추출 일치만(0.9.01… 은 0.9.0 이 아니다)",
        cvp(&with_meta(&[("image_tag_declared", "other:tag"), ("vllm_build", "0.9.01rc1")]))?.0 == "unverified");
    c.ck("★저장소 없이(repo·ev.repo 둘 다 없음) vllm_observed = 차단(cwd 폴백 ✗)",
        _code(vllm_observed(&rep(&evg, &[("repo", J::Null)]), None))?.as_deref() == Some("HINT_EVIDENCE_REPO_REQUIRED"));
    let ev_old = rep(&evg, &[("certificate", jd! {"measured_utc" => "2025-06-01T00:00:00Z", "vllm_version" => "0.9.0"})]);
    c.ck("★측정 시점 판본이 없으면 미검증", cvp(&ev_old)?.0 == "unverified");
    wr0(&g.join(REL_BENCH_TOOLS_DIR).join("sweep_bench.sh"), "# 규칙 표지 없는 판본\n")?;
    core::git(&g, &["add", "-A"])?;
    git_e(&g, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "c5"], &genv(Some("2026-01-04T00:00:00Z"))?)?;
    let ev_new = rep(&evg, &[("certificate", jd! {"measured_utc" => "2026-01-05T00:00:00Z", "vllm_version" => "0.9.0"})]);
    let pn = cvp(&ev_new)?;
    c.ck("★판본에 규칙 표지가 없으면 미검증(tripwire)", pn.0 == "unverified" && pn.1.contains("표지"));
    let lrep = "docs/benchmark/bench_report_26010205_fx-lite_GB10_0.9.0.md";
    mkd(g.join(lrep).parent().unwrap())?;
    let lite_rep = |v: Option<&str>, src: Option<&str>| -> HR<J> {
        let rows: String = [("vllm_version", v), ("vllm_version_source", src)].iter().filter_map(|(k, x)| x.map(|x| format!("| {k} | {x} |\n"))).collect();
        wr0(&g.join(lrep), &format!("# fx\n\nmode: lite\n\n## 측정 환경 스냅샷\n\n| 키 | 값 |\n|---|---|\n| model | fx-lite |\n{rows}\n## 다른 절\n\n| vllm_version | 9.9.9 |\n"))?;
        Ok(rep(&evg, &[("sweep", J::Null), ("report_kind", J::from("lite")), ("bench_report_path", J::from(lrep)),
            ("certificate", rep(&evg["certificate"], &[("benchmark_mode", J::from("lite")), ("vllm_version", J::from("0.9.0"))]))]))
    };
    let (lab8, src8) = cvp(&lite_rep(Some("0.9.0"), Some("derived(envfile IMAGE_TAG)"))?)?;
    c.ck("F8 lite 인증서 + 경량 리포트 기재 일치 = lite-report-identity(리포트 출처 그대로 · 엔진 자기보고 아님)",
        lab8 == "lite-report-identity" && src8.contains("derived(envfile IMAGE_TAG)") && src8.contains("엔진 자기보고가 아니다")
        && src8.contains("bench_report_26010205_fx-lite_GB10_0.9.0.md") && src8.contains("render_report.lite_identity"));
    let (lab8d, src8d) = cvp(&lite_rep(Some("0.9.0"), Some("declared(env EASY_VLLM_VERSION)"))?)?;
    c.ck("F8 리포트 출처가 env 선언이면 그 출처를 옮긴다(IMAGE_TAG 파생이라 적지 않는다)",
        lab8d == "lite-report-identity" && src8d.contains("declared(env EASY_VLLM_VERSION)") && !src8d.contains("envfile IMAGE_TAG("));
    c.ck("★F8 리포트 값 ≠ 인증서 값 = 미검증(고르지 않는다 · 다른 절의 같은 키는 읽지 않는다)",
        cvp(&lite_rep(Some("0.9.1"), Some("derived(envfile IMAGE_TAG)"))?)?.0 == "unverified");
    c.ck("★F8 리포트에 기재 없음 = 기존 규칙(스윕 없음 → 미검증 · 지어내지 않음)", cvp(&lite_rep(None, None)?)?.0 == "unverified");
    let full_c = rep(&lite_rep(Some("0.9.0"), Some("derived(envfile IMAGE_TAG)"))?, &[("certificate", evg["certificate"].clone())]);
    c.ck("★F8 full 인증서면 경량 리포트가 있어도 lite 분기를 타지 않는다", cvp(&full_c)?.0 != "lite-report-identity");
    rm(&g.join(lrep))?;
    Ok(())
}

fn _selftest_v7(c: &mut Ck, repo: &Path, td: &Path, ev: &J, fx: &J, dk: Dk) -> HR<()> {
    let _ = (fx, dk);
    let sw = if ev["sweep"].is_dict() { ev["sweep"].clone() } else { J::dict() };
    let vj0 = if sw["verdict"].is_dict() { sw["verdict"].clone() } else { J::dict() };
    let with_v = |e: &J, v: J| rep(e, &[("sweep", rep(&sw, &[("verdict", v)]))]);
    // ── V1 ──
    let m = measurement(ev)?;
    c.ck("measurement — 인증서 셀: 판정·출처 키별(sources) · 측정 시각", eqs(&m["verdict"], "PASS") && sw_(&m["sources"]["verdict"], "certificate(")
        && eqs(&m["measured_utc"], _M_UTC) && eqs(&m["rubric_authority"], "weak") && eqs(&m["floor_tps"], "8.5"));
    c.ck("★인증서와 같은 측정의 verdict.json 이 다른 값 = 죽는다(고르지 않는다)",
        _code(measurement(&with_v(ev, rep(&vj0, &[("measured_decode_tps", J::Float(99.0))]))))?.as_deref() == Some("HINT_MEASUREMENT_VERDICT_MISMATCH"));
    c.ck("음성대조: 같은 값(12.3)이면 대조 통과", _code(measurement(&with_v(ev, rep(&vj0, &[("measured_decode_tps", J::Float(12.3))]))))?.is_none());
    let op_line = format!("see /{}/op-fixture/x.json", "home");
    let refute = jd! {"verdict" => "REFUTE", "measured_decode_tps" => 5.5, "measured_accept_len" => 1.5, "measured_spec_on" => true,
        "rubric" => jd! {"authority" => "explore", "floor" => 8.5, "ratio_M_over_primary" => 0.647, "primary" => 10.0, "tolerance" => 0.15,
            "source" => "expected_achievable(roofline×MBU)"},
        "reasons" => jl!["M(5.5) < 10.0×(1−0.15)=8.5 → 기대 이하", op_line.as_str()]};
    let e_ref = rep(ev, &[("certificate", J::Null), ("certificate_path", J::Null), ("sweep", rep(&sw, &[("verdict", refute.clone())]))]);
    let mr = measurement(&e_ref)?;
    c.ck("★REFUTE 셀(인증서 없음) — 판정·수치가 verdict.json 에서 채워진다(옛 5키 결함 · V1)",
        eqs(&mr["verdict"], "REFUTE") && eqs(&mr["decode_tps_conc1"], "5.5") && eqs(&mr["floor_tps"], "8.5") && eqs(&mr["ratio_M_over_primary"], "0.647")
        && eqs(&mr["rubric_authority"], "explore") && eqs(&mr["accept_len"], "1.5") && eqs(&mr["spec_on"], "true") && eqs(&mr["benchmark_mode"], "full")
        && has(&mr["sources"]["verdict"], "verdict.json") && eqs(&mr["measured_utc"], _M_UTC) && has(&mr["sources"]["measured_utc"], "sweep_index.generated_utc"));
    c.ck("★REFUTE 사유는 싣되 운영자 절대경로 줄은 싣지 않는다(배포 평면)", mr["verdict_reasons"] == jl![refute["reasons"][0].clone()]);
    let mut off = rep(&refute, &[("measured_accept_len", J::Null), ("measured_spec_on", J::Bool(false))]);
    off.set("rubric", rep(&refute["rubric"], &[("spec_axis", jd! {"declared" => "off", "accept_len" => 1.0, "accept_len_source" => "declared-absent", "missing" => false})]));
    let m17 = measurement(&rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", off.clone())]))]))?;
    let m17n = measurement(&rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", without(&off, "measured_accept_len"))]))]))?;
    let a17 = &m17["sources"]["accept_len"];
    c.ck("★F17 measured_accept_len null + spec_axis → 값 None(1.0 은 선언값 · 옮기지 않음) · 출처 = 측정 null · declared-absent · 결손 아님",
        m17["accept_len"].is_null() && has(a17, "measured_accept_len = null") && has(a17, "accept_len_source=declared-absent") && has(a17, "결손 아님")
        && !has(a17, "칸이 없다") && has(&m17n["sources"]["accept_len"], "칸이 없다"));
    c.ck("REFUTE = 인증서 비발행 판정(결손 아님)", _certificate_not_due(repo, &e_ref)?.unwrap_or_default().starts_with("REFUTE"));
    let obs = jd! {"verdict" => "NEEDS_RUBRIC", "failure_axis" => "establish", "reason_code" => J::Null, "measured_decode_tps" => 7.7,
        "rubric" => jd! {"authority" => "weak", "primary" => J::Null, "source" => J::Null, "floor" => J::Null, "ratio_M_over_primary" => J::Null}};
    let e_obs = rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", obs)]))]);
    let mo = measurement(&e_obs)?;
    c.ck("★기준 부재 full 셀 — measurement.verdict = OBSERVATION-ONLY(원천 NEEDS_RUBRIC 은 출처 칸) · 실측은 싣고 루브릭 칸은 '루브릭이 서지 않았다'",
        eqs(&mo["verdict"], OBSERVATION_ONLY_VERDICT) && has(&mo["sources"]["verdict"], "NEEDS_RUBRIC") && eqs(&mo["decode_tps_conc1"], "7.7")
        && mo["floor_tps"].is_null() && eqs(&mo["benchmark_mode"], "full") && has(&mo["sources"]["floor_tps"], "루브릭이 서지 않았다") && !has(&mo["sources"]["floor_tps"], "lite-only"));
    c.ck("OBSERVATION-ONLY = 인증서 비발행(결손 아님)", _certificate_not_due(repo, &e_obs)?.unwrap_or_default().starts_with("OBSERVATION-ONLY"));
    let _ = cgate(repo)?;
    c.ck("측정 표면 리터럴 ↔ 게이트 대응 상수 교차검증(두 자리 tripwire)",
        (OBSERVATION_ONLY_VERDICT, _JUDGE_NO_BASELINE_VERDICT) == (CGate::OBSERVATION_ONLY_VERDICT, CGate::OBSERVATION_ONLY_JUDGE_VERDICT));
    let pass_rub = |a: &str| jd! {"verdict" => "PASS", "measured_decode_tps" => 12.3, "rubric" => jd! {"authority" => a, "floor" => 8.5, "ratio_M_over_primary" => 1.23}};
    let e_ex = rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", pass_rub("explicit"))]))]);
    c.ck("★explicit PASS 인데 인증서 없음 = 결손(발행되는 판정)", _certificate_not_due(repo, &e_ex)?.is_none());
    c.ck("weak PASS = 비발행 권한(결손 아님)", _certificate_not_due(repo, &rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", pass_rub("weak"))]))]))?.unwrap_or_default().contains("weak"));
    let e_unk = rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", jd! {"verdict" => "PASS"})]))]);
    c.ck("★권한 미관측 PASS = 결손(모름을 면제로 접지 않는다)", _certificate_not_due(repo, &e_unk)?.is_none());
    c.ck("★판정 어휘 밖 = 죽는다(tripwire)", _code(measurement(&rep(&e_ref, &[("sweep", rep(&sw, &[("verdict", jd! {"verdict" => "MAYBE"})]))])))?.as_deref() == Some("HINT_MEASUREMENT_VERDICT_UNKNOWN"));
    let rep2 = "docs/benchmark/bench_report_26010215_fixture-model-nvfp4_GB10_0.9.0.md";
    wr0(&repo.join(rep2), &["# 성능 보고서", "", "> 생성일 2026-01-02T15:00:00Z.", "", "## 판정 (표시만 — verdict_rule.py 결과)", "", "| 항목 | 값 |",
        "|---|---|", "| verdict | **REFUTE** |", "| 측정 decode t/s (동시성1) | 20.98 t/s |", "| 루브릭 권한 | explore |",
        "| 루브릭 primary | 29.8 t/s (expected_achievable(roofline×MBU)) |", "| floor (primary×(1−tol)) | 25.33 t/s |",
        "| ratio (M/primary) | 0.704 |", "| tolerance | 0.15 |", "", "## 다음", ""].join("\n"))?;
    let mt = measurement(&rep(ev, &[("certificate", J::Null), ("certificate_path", J::Null), ("sweep", J::Null), ("bench_report_path", J::from(rep2))]));
    rm(&repo.join(rep2))?;
    let mt = mt?;
    c.ck("리포트 `## 판정` 표 — 굵게·단위 벗김 · primary 괄호 = primary_source · 측정 시각 = 생성일",
        eqs(&mt["verdict"], "REFUTE") && eqs(&mt["decode_tps_conc1"], "20.98") && eqs(&mt["floor_tps"], "25.33") && eqs(&mt["primary_tps"], "29.8")
        && eqs(&mt["primary_source"], "expected_achievable(roofline×MBU)") && has(&mt["sources"]["verdict"], "`## 판정` 표") && eqs(&mt["measured_utc"], "2026-01-02T15:00:00Z"));
    let mn = measurement(&rep(ev, &[("certificate", J::Null), ("certificate_path", J::Null), ("sweep", J::Null), ("bench_report_path", J::Null), ("report_kind", J::Null)]))?;
    c.ck("★판정 원천이 하나도 없으면 verdict None + 미관측(지어내지 않는다)", mn["verdict"].is_null() && has(&mn["sources"]["verdict"], "미관측") && mn["decode_tps_conc1"].is_null());
    // ── V3 ──
    let meta = if sw["index"]["meta"].is_dict() { sw["index"]["meta"].clone() } else { J::dict() };
    let host = |att: J, meta_extra: &[(&str, J)], man_extra: &[(&str, J)]| -> HR<J> {
        let s2 = rep(&sw, &[("index", rep(&sw["index"], &[("meta", rep(&meta, meta_extra))]))]);
        let mut e2 = rep(ev, &[("attestation", att), ("sweep", s2), ("manifest", rep(&ev["manifest"], man_extra))]);
        _host_facts(&mut e2, None)?;
        Ok(e2["build_identity"].clone())
    };
    let att = jd! {"_path" => "output/multi/benchlog/attestation_c1-a.json", "parity" => jd! {"driver" => jd! {"main" => "580.178.04", "sub" => "580.178.04", "verdict" => "equal"}}};
    let dv = |v: &str| vec![("driver_version", J::from(v))];
    let hb = host(att.clone(), &dv("580.173.02"), &[])?;
    let cf = &hb["driver_conflict"];
    c.ck("★attestation 노드별 관측이 선언 옮김(스윕 meta)을 이긴다 + driver_conflict 에 둘 다(V3)",
        eqs(&hb["driver"], "580.178.04") && sw_(&hb["source"]["driver"], "attestation parity.driver") && eqs(&cf["observed"], "580.178.04")
        && eqs(&cf["declared"], "580.173.02") && has(&cf["sources"]["declared"], _DECLARED_COPY)
        && peq(&hb["driver_by_node"], &jd! {"main" => "580.178.04", "sub" => "580.178.04"}));
    let mc_of = |bk: Option<J>| -> HR<J> {
        let idx = match bk { Some(b) => rep(&sw["index"], &[("bench_knobs", b)]), None => sw["index"].clone() };
        measurement_campaign(&rep(ev, &[("sweep", rep(&sw, &[("index", idx)]))]))
    };
    let out_bk = jd! {"campaign" => J::Null, "campaign_pins_note" => "활성 캠페인 없음(_bootstrap)"};
    let in_bk = jd! {"campaign" => "camp-fx", "campaign_pins_note" => J::Null};
    let mc_out = mc_of(Some(out_bk.clone()))?;
    let mc_in = mc_of(Some(in_bk.clone()))?;
    let mc_un = mc_of(None)?;
    let mc_odd = mc_of(Some(jd! {"campaign" => J::Null, "campaign_pins_note" => "캠페인 해소자 부재: x — 선언을 읽지 못했다"}))?;
    let ra_src = rd(&core::skill_dir().parent().unwrap().join("adversarial-benchmark/scripts/repeat_axis.py"))?;
    c.ck("★F13 측정 시점 캠페인 소속: 캠페인 밖(관측) · 캠페인 id · 기록 없음/해소 실패 = 미관측(구분) · 사유 문구 = repeat_axis 원문(교차검증)",
        eqs(&mc_out["status"], "outside") && eqs(&mc_out["id"], "_bootstrap") && has(&mc_out["source"], "campaign_pins_note")
        && eqs(&mc_in["status"], "in-campaign") && eqs(&mc_in["id"], "camp-fx") && eqs(&mc_un["status"], "unobserved") && eqs(&mc_odd["status"], "unobserved")
        && ra_src.contains("\"활성 캠페인 없음(%s)\" % mod.BOOTSTRAP"));
    let ab_of = |bk: Option<J>, mode: &str, r: Option<&Path>, base: &J| -> HR<J> {
        let idx = match bk { Some(b) => rep(&sw["index"], &[("bench_knobs", b)]), None => sw["index"].clone() };
        campaign_absence(&rep(base, &[("mode", J::from(mode)), ("sweep", rep(&sw, &[("index", idx)]))]), r)
    };
    let ab_out = ab_of(Some(out_bk.clone()), "publication-replay", None, ev)?;
    let ab_in = ab_of(Some(in_bk.clone()), "publication-replay", None, ev)?;
    let ab_un = ab_of(None, "publication-replay", None, ev)?;
    c.ck("★F14 캠페인 인스턴스 부재 사유 단일 판정: 캠페인 밖 측정 = 'purge' ✗(인스턴스 없음) · 측정 캠페인 id 기록 = purge(사유에 id) · 기록 없음 = 종전 purge 문구(사유 없음) · 캠페인 모드 = none",
        eqs(&ab_out["kind"], "outside") && !has(&ab_out["phrase"], "purge") && has(&ab_out["reason"], "애초에 없다")
        && eqs(&ab_in["kind"], "purged") && has(&ab_in["reason"], "camp-fx") && eqs(&ab_un["phrase"], "캠페인 purge")
        && ab_un["reason"].is_null() && eqs(&ab_of(None, "campaign", None, ev)?["kind"], "none"));
    let (p_un, p_out, p_in, p_gone, p_abs);
    {
        let t25 = core::TempDir::new("hint-f25-");
        let r25 = t25.path();
        selftest_stage_manifest_contract(&code_root(), r25)?;
        _link_native(r25)?;
        let cd25 = r25.join("campaigns/camp-fx/cells/other-name");
        mkd(&cd25)?;
        wr0(&cd25.join("config.yaml"), &format!("serving:\n  config_name: {}\n", st(&ev["cell"])))?;
        wr0(&r25.join("campaigns/camp-fx/journey.jsonl"), &(ad(&jd! {"kind" => "campaign_closed"}) + "\n"))?;
        mkd(&r25.join("campaigns/_template/cells").join(st(&ev["cell"])))?;
        let evr = rep(ev, &[("mode", J::from("publication-replay"))]);
        p_un = ab_of(None, "publication-replay", Some(r25), &evr)?;
        p_out = ab_of(Some(out_bk.clone()), "publication-replay", Some(r25), &evr)?;
        p_in = ab_of(Some(in_bk.clone()), "publication-replay", Some(r25), &evr)?;
        p_gone = ab_of(Some(jd! {"campaign" => "camp-gone", "campaign_pins_note" => J::Null}), "publication-replay", Some(r25), &evr)?;
        rmtree(&r25.join("campaigns/camp-fx"));
        p_abs = ab_of(None, "publication-replay", Some(r25), &evr)?;
    }
    c.ck("★F25 인스턴스 실재 관측: 보존(config_name 일치 · journey 마지막) = preserved('purge' ✗ · 재생 모드는 읽지 않았다) · 캠페인 밖이면 앞선 인스턴스 보존을 함께 · 측정 캠페인 id 인스턴스 부재 = purged · 둘 다 없음 = absent(가르지 못함) · 예약 id 무시",
        eqs(&p_un["kind"], "preserved") && !has(&p_un["phrase"], "purge") && has(&p_un["reason"], "campaigns/camp-fx/cells/other-name")
        && has(&p_un["reason"], "campaign_closed") && eqs(&p_out["kind"], "outside") && has(&p_out["reason"], "보존돼 있으나")
        && eqs(&p_in["kind"], "preserved") && has(&p_in["phrase"], "camp-fx") && eqs(&p_gone["kind"], "purged") && has(&p_gone["phrase"], "camp-gone")
        && eqs(&p_abs["kind"], "absent") && has(&p_abs["phrase"], "가르지 못함"));
    let evo = rep(ev, &[("mode", J::from("publication-replay")), ("declaration", J::Null), ("sweep", rep(&sw, &[("index", rep(&sw["index"], &[("bench_knobs", out_bk.clone())]))]))]);
    let cbs = _campaign_boundary(&evo, &J::Null, None)?;
    c.ck("★F14 재생 경계 사유 = 같은 판정(캠페인 밖이면 'purge' 문구가 나오지 않는다)", has(&cbs["source"], "캠페인 밖 측정") && !has(&cbs["source"], "purge"));
    let h_eq = host(att.clone(), &dv("580.178.04"), &[])?;
    c.ck("음성대조: 관측 = 선언이면 conflict 없음", !inn(&h_eq, "driver_conflict"));
    let h_none = host(att.clone(), &[], &[])?;
    c.ck("★F12(2026-10-25 실물 모양: attestation 양 노드 580.178.04 = manifest 옮김 580.178.04) 대조 사실 = driver_check agree(두 출처)",
        eqs(&h_eq["driver_check"]["verdict"], "agree") && eqs(&h_eq["driver_check"]["declared"], "580.178.04")
        && sw_(&h_eq["driver_check"]["sources"]["observed"], "attestation parity.driver") && eqs(&hb["driver_check"]["verdict"], "conflict")
        && (eqs(&h_none["driver_check"]["verdict"], "unpaired") || eqs(&h_none["driver_check"]["verdict"], "agree")));
    let hm = host(J::Null, &[("driver_version", J::from("580.2")), ("driver_version_source", J::from("measured(nvidia-smi)"))], &[("driver_version", J::from("580.1"))])?;
    c.ck("스윕 meta 가 스스로 measured 를 말하면 관측 — 옮김 표지 없음 · manifest 선언과 다르면 conflict",
        eqs(&hm["driver"], "580.2") && !has(&hm["source"]["driver"], _DECLARED_COPY) && eqs(&hm["driver_conflict"]["declared"], "580.1"));
    let hd = host(jd! {"_path" => "a.json", "parity" => jd! {"driver" => jd! {"main" => "1.0", "sub" => "2.0", "verdict" => "differ"}}}, &[], &[])?;
    c.ck("★노드별 상이는 한 값으로 접지 않는다(main 값 + 출처가 상이를 말한다 · by_node 둘 다)",
        eqs(&hd["driver"], "1.0") && has(&hd["source"]["driver"], "노드별 상이") && peq(&hd["driver_by_node"], &jd! {"main" => "1.0", "sub" => "2.0"}));
    c.ck("F3 드라이버(노드별 관측) 행 출처 = attestation 출처(출처 미기재 ✗)", sw_(&hb["source"]["driver_by_node"], "attestation parity.driver"));
    let hn = host(rep(&att, &[("config", ev["cell"].clone())]), &dv("580.173.02"), &[])?;
    c.ck("F10 음성대조: 같은 config(같은 run 후보)의 attestation 은 관측 층 — 선언을 이긴다", eqs(&hn["driver"], "580.178.04") && inn(&hn, "driver_conflict"));
    let mut e_nat = rep(ev, &[("plane", J::from("native")), ("attestation", rep(&att, &[("config", J::from("src-cell"))])),
        ("sweep", rep(&sw, &[("index", rep(&sw["index"], &[("meta", rep(&meta, &dv("580.173.02")))]))]))]);
    _host_facts(&mut e_nat, None)?;
    let hn2 = &e_nat["build_identity"];
    c.ck("★F10 native(원천 셀 attestation = 다른 run) — 값 = 선언 · 원천 관측은 범위를 붙여 출처에 · conflict · 노드별 관측 행 없음",
        eqs(&hn2["driver"], "580.173.02") && has(&hn2["source"]["driver"], "원천 셀(src-cell) attestation 관측 580.178.04")
        && has(&hn2["source"]["driver"], "이 run 아님") && !inn(hn2, "driver_conflict") && !inn(hn2, "driver_by_node"));
    let ho = host(rep(&att, &[("config", J::from("other-cfg"))]), &[], &[])?;
    c.ck("★F10 다른 config attestation · 선언 없음 — 값은 옮기되 출처가 '다른 run' 범위를 말한다",
        eqs(&ho["driver"], "580.178.04") && has(&ho["source"]["driver"], "다른 run(other-cfg)") && !inn(&ho, "driver_by_node"));
    let lg1 = _measured_engine_logs(repo, ev)?.0;
    if !lg1.is_empty() {
        let mut lb1: Vec<Vec<u8>> = Vec::new();
        for p in &lg1 {
            lb1.push(rdb(p)?);
        }
        let banner: &[u8] = b"  Using CUDA 13.1 driver version 590.48.01 with kernel driver version 580.178.04.\n";
        let host_r = |e_: &J| -> HR<J> {
            let mut e2 = rep(e_, &[("attestation", J::Null), ("sweep", rep(&sw, &[("index", rep(&sw["index"], &[("meta", rep(&meta, &dv("580.173.02")))]))])),
                ("build_identity", ev["build_identity"].clone())]);
            _host_facts(&mut e2, Some(repo))?;
            Ok(e2["build_identity"].clone())
        };
        let r = (|| -> HR<(J, J, J, J)> {
            let mut b0 = banner.to_vec();
            b0.extend_from_slice(&lb1[0]);
            wrb(&lg1[0], &b0)?;
            let hf1 = host_r(&rep(ev, &[("node", J::from("main"))]))?;
            let hf1c = host_r(&rep(ev, &[("node", J::from("cluster"))]))?;
            let mut e0 = rep(ev, &[("attestation", J::Null), ("sweep", rep(&sw, &[("index", rep(&sw["index"], &[("meta", rep(&meta, &dv("580.173.02")))]))]))]);
            _host_facts(&mut e0, None)?;
            let hf1x = if lg1.len() > 1 {
                let mut b1 = String::from_utf8_lossy(banner).replace("580.178.04", "580.999.01").into_bytes();
                b1.extend_from_slice(&lb1[1]);
                wrb(&lg1[1], &b1)?;
                host_r(&rep(ev, &[("node", J::from("main"))]))?
            } else {
                jd! {"driver" => "580.173.02"}
            };
            Ok((hf1, hf1c, e0, hf1x))
        })();
        for (p, b_) in lg1.iter().zip(lb1.iter()) {
            wrb(p, b_)?;
        }
        let (hf1, hf1c, e0, hf1x) = r?;
        c.ck("★F1 attestation 부재 → 측정 엔진 로그 배너 kernel driver = 관측(선언 옮김을 이긴다) · driver_conflict 둘 다 · 출처 file:line",
            eqs(&hf1["driver"], "580.178.04") && has(&hf1["source"]["driver"], "kernel driver version")
            && has(&hf1["source"]["driver"], &format!("{}:1", _rel(repo, &lg1[0])?))
            && eqs(&hf1["driver_conflict"]["declared"], "580.173.02") && peq(&hf1["driver_by_node"], &jd! {"main" => "580.178.04"}));
        c.ck("★F1 cluster 축 = 노드별 관측 행 ✗(head 배너뿐) · repo 없이 부르면 종전(선언 옮김) · 로그끼리 갈리면 관측 ✗",
            eqs(&hf1c["driver"], "580.178.04") && !inn(&hf1c, "driver_by_node") && eqs(&e0["build_identity"]["driver"], "580.173.02") && eqs(&hf1x["driver"], "580.173.02"));
    } else {
        c.ck("F1 픽스처 — 측정 엔진 로그가 있어야 한다", false);
    }
    // ── U9 ──
    let ident = identity(ev)?;
    let qc = ident["quant_composition"].items().to_vec();
    c.ck("quant_composition — MIXED 층 알고리즘별 개수(해석 ✗) · 출처",
        qc.iter().map(|r| (st(&r["dtype"]), r["count"].clone())).collect::<Vec<_>>() == vec![("NVFP4".to_string(), J::Int(2)), ("FP8".to_string(), J::Int(1))]
        && qc.iter().all(|r| r["source"].truthy()) && !ud(&J::List(qc.clone())).contains(&core::posix(td)));
    let ds4 = jd! {"observed" => true, "host_label" => "L", "config" => jd! {"quantization_config" => jd! {"quant_method" => "fp8", "fmt" => "e4m3",
        "weight_block_size" => jl![128, 128]}, "expert_dtype" => "fp4", "torch_dtype" => "bfloat16"}};
    let (comp, _src) = _quant_composition(&ds4)?;
    let comp = comp.unwrap_or_default();
    c.ck("quant_composition — DS4F 형(선언 fp8 + routed expert fp4 + 기본 bf16) · q 축은 선언 방식 그대로(fp8)",
        comp.iter().map(|r| st(&r["dtype"]).split(' ').next().unwrap_or("").to_string()).collect::<Vec<_>>() == strs(&["fp8", "fp4", "bfloat16"])
        && comp.len() > 1 && eqs(&comp[1]["scope"], "routed experts") && eqs(&_quant_raw(&ds4)?["raw"], "fp8"));
    c.ck("★체크포인트 미관측 = 구성 None(빈 구성으로 접지 않는다)", _quant_composition(&jd! {"observed" => false, "reason" => "r"})?.0.is_none());
    // ── V11③ ──
    let bl = repo.join("output/multi/benchlog");
    let l = J::list();
    let n = J::Null;
    let an = |topo: &str, cell: &str, cert: &J| _auto_node(repo, topo, cell, cert, None, None, &l, &n);
    c.ck("자동 cluster — 측정 TP(2) > gpus_per_node(1)", an("multi", "c1-z", &jd! {"tensor_parallel_size" => "2"})?.0.as_deref() == Some("cluster"));
    c.ck("음성대조: TP ≤ gpus_per_node · 멀티 신호 없음 = 파생하지 않는다(배정으로 간다)", an("multi", "c1-z", &jd! {"tensor_parallel_size" => "1"})?.0.is_none());
    wj(&bl.join("serve_proof_c1-z.json"), &jd! {"kind" => "multinode_serve_proof", "config" => "c1-z"})?;
    let r = (|| -> HR<(Option<String>, Option<String>)> {
        Ok((_code(an("multi", "c1-z", &jd! {"tensor_parallel_size" => "1"}))?, an("multi", "c1-z", &J::Null)?.0))
    })();
    rm(&bl.join("serve_proof_c1-z.json"))?;
    let (amb, tpn) = r?;
    c.ck("★모호(TP=1 단일 노드 형상인데 멀티 serve proof) = 기존 오류로 죽는다", amb.as_deref() == Some("HINT_NODE_AXIS_UNDERIVABLE"));
    c.ck("TP 미관측 + 멀티 serve proof = cluster", tpn.as_deref() == Some("cluster"));
    c.ck("single 토폴로지는 자동 cluster 대상 아님", an("single", "c1-a", &jd! {"tensor_parallel_size" => "2"})?.0.is_none());
    let decl_n = strs(&["main", "sub"]);
    let auto = jl!["cluster", "why"];
    c.ck("자동 cluster 가 배정(main)을 이긴다 · 인증서 measured_node 는 자동을 이긴다",
        _resolve_node(&J::Null, &J::Null, "multi", Some("main"), &decl_n, "x", &auto)?.0 == "cluster"
        && _code(_resolve_node(&J::Null, &J::from("main"), "multi", Some("main"), &decl_n, "x", &auto))?.is_none());
    // ── U6 ──
    let ts = tail_sources(repo, ev)?;
    let (y_rel, e_rel, c_rel) = ("output/multi/configs/c1-a.yaml", "output/multi/envs/.env.c1-a", "campaigns/c1/cells/c1-a/config.yaml");
    c.ck("tail_sources — 서빙 yaml(중첩 `a.b` · 문자열 JSON 펼침) · 셀 env · 러너 · 셀 config",
        eqs(&ts[y_rel]["speculative-config.num_speculative_tokens"], "2") && eqs(&ts[y_rel]["enforce-eager"], "true")
        && eqs(&ts[e_rel]["VLLM_PLE_MMAP"], "1") && eqs(&ts[c_rel]["declared_axes.ple_mode"], "mmap") && eqs(&ts[c_rel]["target_gpu.gpu_model"], "NVIDIA GB10")
        && eqs(&ts["output/multi/configs/c1-a.sh"]["--config"], "/app/configs/${CONFIG_FILE}.yaml"));
    c.ck("재생(캠페인 밖)은 셀 config 를 싣지 않는다", !inn(&tail_sources(repo, &rep(ev, &[("mode", J::from("publication-replay"))]))?, c_rel));
    let nf = naming_facts(repo, ev)?;
    let tail = J::List([("mmap", "ple"), ("s2", "spec"), ("eager", "graph")].iter()
        .map(|(t, ax)| jd! {"token" => *t, "meaning" => "뜻", "evidence" => nf[*ax]["evidence"].clone()}).collect());
    c.ck("naming facts 구조화 근거(ple·spec·graph)가 tail_sources 와 정확히 맞는다(validate_tail 0건)",
        ["ple", "spec", "graph"].iter().all(|ax| nf[*ax]["evidence"].truthy()) && crate::naming::validate_tail(&tail, &ts)?.is_empty());
    let liar = jl![jd! {"token" => "spec9", "meaning" => "뜻", "evidence" => jd! {"file" => y_rel, "key" => "speculative-config.num_speculative_tokens", "value" => "9"}}];
    c.ck("★거짓 꼬리(값 불일치) = HINT_TAIL_UNGROUNDED", crate::naming::validate_tail(&liar, &ts)?.iter().any(|(c_, _)| c_ == "HINT_TAIL_UNGROUNDED"));
    // ── §4.5 ──
    let nodes_w = |a: &str, b: &str| jd! {"_path" => "a.json", "nodes" => jd! {"main" => jd! {"vllm_dist_version" => a}, "sub" => jd! {"vllm_dist_version" => b}}};
    let (wm, wsrc) = _wheel_meta(repo, &rep(ev, &[("attestation", nodes_w("0.9.0+gabc", "0.9.0+gabc"))]))?;
    c.ck("★wheel 메타 — attestation vllm_dist_version 이 있으면 관측(옛 '관측자 없음' 오판)", wm.as_deref() == Some("0.9.0+gabc") && wsrc.contains("일치"));
    c.ck("음성대조: 원천이 없으면 None + 미관측", _wheel_meta(repo, &rep(ev, &[("attestation", J::Null)]))?.0.is_none());
    c.ck("★노드별 상이 wheel 은 고르지 않는다", _wheel_meta(repo, &rep(ev, &[("attestation", nodes_w("1", "2"))]))?.0.is_none());
    let different_wheel = vllm_observed(&rep(ev, &[("plane", J::from("docker")), ("attestation", nodes_w("1", "2"))]), Some(repo))?;
    c.ck("노드별 배포메타 관측 차이는 공통값 None을 유지하되 미관측으로 접지 않는다",
        different_wheel["wheel_meta"].is_null() && eqs(&different_wheel["wheel_meta_basis"], "observed-node-difference")
        && has(&different_wheel["source"], "노드별 상이·공통값 미선택"));
    let fz = repo.join("docs/simlog/fx_native");
    mkd(&fz)?;
    wr0(&fz.join("pip-freeze-main.txt"), "torch==2.9\nvllm==0.9.0+native\n")?;
    wr0(&fz.join("pip-freeze-sub.txt"), "vllm==0.9.0+native\n")?;
    let (wn, wnsrc) = _wheel_meta(repo, &rep(ev, &[("plane", J::from("native")), ("attestation", jd! {"_path" => "a", "nodes" => jd! {"main" => jd! {"vllm_dist_version" => "other"}}}),
        ("pip_freeze_paths", jd! {"main" => "docs/simlog/fx_native/pip-freeze-main.txt", "sub" => "docs/simlog/fx_native/pip-freeze-sub.txt"})]))?;
    c.ck("native 셀 wheel 메타 = pip freeze `vllm==`(실행 평면 관측이 원천 이미지 attestation 을 이긴다)", wn.as_deref() == Some("0.9.0+native") && wnsrc.contains("pip freeze"));
    // ── V11④ ──
    let base = jd! {"rubric_source" => "verdict_json", "floor_tps" => 1.0, "ratio_M_over_primary" => 0.9, "primary_source" => "x"};
    let ncb = |b: J| no_cert_binding_source(&jd! {"task_class" => "full_benchmark", "benchmark" => b}, Some(repo));
    let ba = |a: &str, v: &str| rep(&base, &[("rubric_authority", J::from(a)), ("verdict", J::from(v))]);
    c.ck("weak PASS·REFUTE 는 bench_report 로 묶인다(비발행 권한 · 게이트 술어 한 벌)",
        ncb(ba("weak", "PASS"))?.as_deref() == Some("weak") && ncb(ba("weak", "REFUTE"))?.as_deref() == Some("weak"));
    c.ck("★explicit PASS 는 인증서가 필요하다(bench_report 로 열지 않는다)", ncb(ba("explicit", "PASS"))?.is_none());
    c.ck("explicit·weak REFUTE 는 bench_report 로 묶인다(발행 게이트 통일 · 권한 무관 · M1 모양)",
        ncb(ba("explicit", "REFUTE"))?.as_deref() == Some("explicit") && ncb(ba("weak", "REFUTE"))?.as_deref() == Some("weak"));
    c.ck("★출처 표시 없는 weak 는 열지 않는다(rubric_source 부재)", ncb(rep(&without(&base, "rubric_source"), &[("rubric_authority", J::from("weak")), ("verdict", J::from("PASS"))]))?.is_none());
    let vjp = repo.join(st(&sw["dir"])).join("verdict.json");
    let held = rdb(&vjp)?;
    rm(&vjp)?;
    let r = (|| -> HR<Option<String>> {
        _code(drive_publisher(repo, ev, &J::from("c1__vj_absent"), &J::from("2026-01-03T05:00:00Z"), &repo.join("hints/.drafts/vj")))
    })();
    wrb(&vjp, &held)?;
    c.ck("★full_benchmark 발행에 verdict.json 이 없으면 첫 쓰기 전에 명시 실패(권한 None → 인증서 요구 부활 방지)",
        r?.as_deref() == Some("HINT_VERDICT_JSON_ABSENT") && !repo.join(core::REL_EVIDENCE_DIR).join("c1__vj_absent.json").exists() && !repo.join("hints/.drafts/vj").exists());
    let codes = MISSING_CODES();
    c.ck("결손 코드 등재 — SUB_RECIPE · APPLIED_SET · NATIVE_LAUNCH",
        ["HINT_MISSING_SUB_RECIPE", "HINT_MISSING_APPLIED_SET", "HINT_MISSING_NATIVE_LAUNCH"].iter().all(|k| codes.get(k).is_some()));
    Ok(())
}

fn cache_put(key: &str, v: Option<(Option<String>, String)>) -> Option<(Option<String>, String)> {
    _self_node_cache_put(key, v)
}

fn _selftest_fix2(c: &mut Ck, td: &Path, ev: &J) -> HR<()> {
    let r = td.join("fix2");
    let bl = r.join("output/multi/benchlog");
    mkd(&bl)?;
    let root = code_root();
    for rel in _OWNER_LINKS() {
        mkd(r.join(rel).parent().unwrap())?;
        if !r.join(rel).exists() {
            symlink(&root.join(rel), &r.join(rel))?;
        }
    }
    _link_native(&r)?;
    let r = r.as_path();
    // ── G1 ──
    let sw_dir = bl.join("sweep_cx");
    mkd(&sw_dir)?;
    let tail = sw_dir.join("lite_engine_cx.log");
    let t_lines = ["(EngineCore pid=71) \x1b[36m(RayWorkerProc pid=9)\x1b[0m INFO 01-02 02:10:00 [x.py:1] tail start",
        "(EngineCore pid=71) (Worker_TP0 pid=9) INFO 01-02 02:20:00 [default_loader.py:430] Loading weights took 353.23 seconds"];
    wr0(&tail, &(t_lines.join("\n") + "\n"))?;
    let full_lines = ["INFO 01-02 02:00:00 [api_server.py:1] █ version 9.9.9rc1.dev0",
        "(EngineCore pid=71) fixm:9:9 [0] NCCL INFO NCCL_NET_PLUGIN set by environment to spcx",
        "(EngineCore pid=71) INFO 01-02 02:05:00 [x.py:1] boot",
        "(EngineCore pid=71) (Worker_TP0 pid=9) INFO 01-02 02:20:00 [default_loader.py:430] Loading weights took 353.23 seconds"];
    let full = bl.join("engine_roce_cx.log");
    wr0(&full, &(full_lines.join("\n") + "\n"))?;
    let ev1 = rep(ev, &[("repo", pj(r)), ("cell", J::from("cx")), ("topology", J::from("multi")), ("plane", J::from("docker")), ("serve_proof", J::Null),
        ("lineage_seeds", J::dict()), ("certificate", jd! {"measured_utc" => "2026-01-02T03:00:00Z"}), ("report_kind", J::Null), ("bench_report_path", J::Null),
        ("sweep", jd! {"dir" => "output/multi/benchlog/sweep_cx", "binding" => "output", "generated_utc" => "2026-01-02T03:00:00Z",
            "index" => jd! {"lite" => jd! {"gen_tps" => 1.0}}, "levels" => J::list()})]);
    let (logs1, why1, cp1) = measured_engine_logs_ex_t(r, &ev1)?;
    let env1 = measurement_env_observed(r, &ev1)?;
    let ban1 = _engine_banner(r, &ev1)?;
    let cpf = cp1.iter().find(|(p, _)| *p == full).map(|(_, w)| w.clone()).unwrap_or_default();
    c.ck("G1 같은 실행 전체 사본(pid · 로드 줄 · 첫 시각) = 원천(앞에 둔다) · 판별 규칙이 근거에 남는다",
        logs1.first() == Some(&full) && cp1.iter().any(|(p, _)| *p == full) && cpf.contains("EngineCore pid ['71'] 일치") && cpf.contains(SAME_RUN_RULE)
        && why1.contains("engine_roce_cx.log"));
    c.ck("G1 env 관측 · 엔진 자기보고가 사본에서 나온다(창 표지 ✗ · 배너 출처에 판별)",
        env1.iter().any(|x| eqs(&x["key"], "NCCL_NET_PLUGIN") && sw_(&x["source"], "output/multi/benchlog/engine_roce_cx.log") && !inn(x, "log_window"))
        && eqs(&ban1["value"], "9.9.9rc1.dev0") && has(&ban1["source"], "같은 실행의 전체 사본"));
    let cp_empty = |c_: &Ck| -> HR<bool> { let _ = c_; Ok(measured_engine_logs_ex_t(r, &ev1)?.2.is_empty()) };
    let mut fl = full_lines.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    fl[3] = fl[3].replace("353.23", "300.00");
    wr0(&full, &(fl.join("\n") + "\n"))?;
    c.ck("★G1 음성대조: 로드 줄(시각·초)이 다르면 같은 실행이 아니다(pid 만으로 묶지 않는다)", cp_empty(c)?);
    wr0(&full, &(full_lines.iter().map(|x| x.replace("pid=71", "pid=72")).collect::<Vec<_>>().join("\n") + "\n"))?;
    c.ck("★G1 음성대조: EngineCore pid 가 다르면 사본 ✗", cp_empty(c)?);
    wr0(&full, &([full_lines[0].replace("02:00:00", "02:15:00"), full_lines[1].to_string(), full_lines[2].replace("02:05:00", "02:15:00"), full_lines[3].to_string()].join("\n") + "\n"))?;
    c.ck("★G1 음성대조: 사본이 꼬리보다 늦게 시작하면(더 긴 머리가 아니다) 사본 ✗", cp_empty(c)?);
    wr0(&bl.join("watchdog_cx.log"), &(full_lines.join("\n") + "\n"))?;
    c.ck("★G1 음성대조: 워치독 로그는 후보가 아니다", cp_empty(c)?);
    rm(&full)?;
    rm(&bl.join("watchdog_cx.log"))?;
    // ── G6 ──
    let att2 = jd! {"_path" => "a.json", "nodes" => jd! {"main" => jd! {"image_id" => format!("sha256:{}", "a".repeat(64))}, "sub" => jd! {"image_id" => format!("sha256:{}", "b".repeat(64))}}};
    let n6 = _node_image_id_note(&att2, &format!("sha256:{}", "a".repeat(64)))?;
    c.ck("G6 노드별: main 같다 · sub 다르다(id) · 노드별 로컬 빌드 정상(동일 ABI ≠ 동일 digest)",
        n6.contains("main 같다") && n6.contains("sub 다르다(sha256:bbbbbbbbbbbb") && n6.contains("동일 ABI"));
    let n6b = _node_image_id_note(&jd! {"_path" => "a.json", "nodes" => jd! {"main" => jd! {"image_id" => "x"}, "sub" => jd! {"image_id" => "x"}}}, "x")?;
    c.ck("★G6 음성대조: 노드 id 가 같으면 '다르다' · 정상 주석 없음 · image_id 없으면 빈 문자열",
        !n6b.contains("다르다") && !n6b.contains("동일 ABI") && n6b.contains("main 같다 · sub 같다") && _node_image_id_note(&J::dict(), "x")?.is_empty());
    // ── G7 · G9 ──
    let (t0, t1) = ("2026-01-02T03:00:00Z", "2026-01-02T03:01:05Z");
    mkd(&r.join("docs/benchmark"))?;
    wr0(&r.join("docs/benchmark/bench_report_fx.md"), &format!("# lite\n생성일 {t0}\nmode: lite\n"))?;
    wj(&bl.join("lite_raw_cl.json"), &jd! {"config_name" => "cl", "measured_utc" => t0, "topology" => "multi", "backend" => "openai-chat",
        "bench_warm_json" => "/x/lite_warm_cl.json", "bench_cold_json" => "/x/lite_cold_cl.json"})?;
    let leg = jd! {"backend" => "openai-chat", "num_prompts" => 3, "completed" => 3, "total_input_tokens" => 1695, "total_output_tokens" => 384,
        "max_concurrency" => 1, "request_rate" => "inf"};
    wj(&bl.join("lite_cold_cl.json"), &rep(&leg, &[("date", J::from("20260102-030030")), ("spec_decode_acceptance_length", J::Float(2.0))]))?;
    wj(&bl.join("lite_warm_cl.json"), &rep(&leg, &[("date", J::from("20260102-030105")), ("spec_decode_acceptance_length", J::Float(1.81))]))?;
    let ev7 = rep(&ev1, &[("cell", J::from("cl")), ("sweep", J::Null), ("certificate", J::Null), ("certificate_path", J::Null), ("report_kind", J::from("lite")),
        ("bench_report_path", J::from("docs/benchmark/bench_report_fx.md"))]);
    let lw = lite_window(r, &ev7)?.unwrap_or_default();
    let mw = _measure_window(r, &ev7)?;
    c.ck("G7 lite 측정 창: 시작 = lite_raw measured_utc(부하 직전 · '시작') · 끝 = 마지막 레그 JSON date",
        lw.truthy() && eqs(&lw["start_utc"], t0) && eqs(&lw["end_utc"], t1) && has(&lw["start_source"], "시작") && has(&lw["end_source"], "lite_warm_cl.json")
        && (mw.0.as_deref(), mw.1.as_deref()) == (Some(t0), Some(t1)));
    c.ck("★G7 음성대조: 조인 불성립(생성일 ≠ measured_utc) · 스윕 셀 = lite_window 없음",
        lite_window(r, &rep(&ev7, &[("bench_report_path", J::Null)]))?.is_none() && lite_window(r, &rep(&ev7, &[("sweep", ev1["sweep"].clone())]))?.is_none());
    let m7 = measurement(&rep(&ev7, &[("repo", pj(r))]))?;
    c.ck("G9 lite-only accept_len = warm 레그 JSON 원문(판정 입력 아님 · cold 병기)",
        eqs(&m7["accept_len"], "1.81") && has(&m7["sources"]["accept_len"], "lite_warm_cl.json") && has(&m7["sources"]["accept_len"], "판정 입력 아님")
        && has(&m7["sources"]["accept_len"], "cold 2.0"));
    wj(&bl.join("lite_warm_cl.json"), &rep(&leg, &[("date", J::from("20260102-030105"))]))?;
    wj(&bl.join("lite_cold_cl.json"), &rep(&leg, &[("date", J::from("20260102-030030"))]))?;
    c.ck("★G9 음성대조: 레그 JSON 에 수용 길이가 없으면 미기재(지어내지 않는다)", measurement(&rep(&ev7, &[("repo", pj(r))]))?["accept_len"].is_null());
    // ── F1/F3/F5 ──
    let root_raw = rdb(&bl.join("lite_raw_cl.json"))?;
    rm(&bl.join("lite_raw_cl.json"))?;
    c.ck("★F1 음성대조: 루트 raw 가 없고 보존 자리도 없으면 조인 불성립", lite_join(r, &ev7)?.0.is_none() && lite_window(r, &ev7)?.is_none());
    let pubd = bl.join(LITE_PUBLISH_DIR);
    let (good, late, alien) = (pubd.join("cl_20260102T025959Z"), pubd.join("cl_20260102T050000Z"), pubd.join("cl-x_20260102T025959Z"));
    for (dd, mu) in [(&good, t0), (&late, "2026-01-02T05:00:10Z"), (&alien, t0)] {
        mkd(dd)?;
        let nm = dd.file_name().unwrap().to_string_lossy().split('_').next().unwrap().to_string();
        wj(&dd.join(format!("lite_raw_{nm}.json")), &jd! {"config_name" => nm.as_str(), "measured_utc" => mu, "topology" => "multi",
            "bench_warm_json" => format!("/x/lite_warm_{nm}.json")})?;
    }
    wj(&good.join("lite_warm_cl.json"), &rep(&leg, &[("date", J::from("20260102-030105")), ("spec_decode_acceptance_length", J::Float(2.0535))]))?;
    wj(&late.join("lite_warm_cl.json"), &rep(&leg, &[("date", J::from("20260102-050100")), ("spec_decode_acceptance_length", J::Float(9.9))]))?;
    wr0(&good.join("lite_engine_cl.log"), "(EngineCore pid=7) INFO 01-02 02:59:00 tail\n")?;
    let jd_ = lite_join(r, &ev7)?;
    let lw_p = lite_window(r, &ev7)?.unwrap_or_default();
    c.ck("F1 lite 조인 = measured_utc 가 맞는 보존 디렉터리(★더 늦은 보존본 · 접두 충돌 셀 cl-x 는 고르지 않는다)",
        jd_.0.as_ref() == Some(&good) && jd_.2.contains("lite_publish/cl_20260102T025959Z") && lw_p.truthy() && eqs(&lw_p["start_utc"], t0)
        && ew_(&lw_p["legs"]["warm"]["path"], "cl_20260102T025959Z/lite_warm_cl.json"));
    let m_p = measurement(&rep(&ev7, &[("repo", pj(r))]))?;
    c.ck("F5 lite accept_len = 바인딩 보존본 warm 원문 2.0535(★늦은 보존본 9.9 ✗)", eqs(&m_p["accept_len"], "2.0535")
        && has(&m_p["sources"]["accept_len"], "cl_20260102T025959Z/lite_warm_cl.json"));
    let lg_t = _measured_engine_logs_base(r, &ev7)?;
    c.ck("F1 엔진 로그 = 보존본 꼬리(머리 캡처 없으면 그 사실을 근거에 적는다)", lg_t.0 == vec![good.join("lite_engine_cl.log")] && lg_t.1.contains("꼬리 캡처만"));
    wr0(&good.join("lite_engine_head_cl.log"), "INFO 01-02 02:50:00 Initializing a V1 LLM engine (v0.0.1)\n")?;
    let lg_h = _measured_engine_logs_base(r, &ev7)?;
    c.ck("F1 머리 캡처가 있으면 꼬리 **앞**에 둔다(배너 · env 되읊음) · 배너가 머리에서 읽힌다",
        lg_h.0 == vec![good.join("lite_engine_head_cl.log"), good.join("lite_engine_cl.log")] && lg_h.1.contains("머리 캡처 포함")
        && eqs(&_engine_banner(r, &ev7)?["value"], "0.0.1"));
    wj(&late.join("lite_raw_cl.json"), &jd! {"config_name" => "cl", "measured_utc" => t0, "topology" => "multi"})?;
    let lj2 = lite_join(r, &ev7)?;
    c.ck("★F1 음성대조: 같은 measured_utc 의 보존본이 둘이면 고르지 않는다(모호 → None)", lj2.0.is_none() && lj2.2.contains("모호"));
    rmtree(&late);
    rm(&good.join("lite_warm_cl.json"))?;
    let m_c = measurement(&rep(&ev7, &[("repo", pj(r)), ("certificate", jd! {"benchmark_mode" => "lite", "verdict" => "not_applicable",
        "lite_accept_len" => "2.0535", "lite_accept_len_src" => "warm.spec_decode_acceptance_length"}), ("certificate_path", J::from("docs/benchmark/benchmark_fx.yaml"))]))?;
    c.ck("F5 레그 JSON 이 없으면 lite 인증서 lite_accept_len(출처 칸 병기)", eqs(&m_c["accept_len"], "2.0535")
        && has(&m_c["sources"]["accept_len"], "lite_accept_len(warm.spec_decode_acceptance_length)") && eqs(&m_c["lite_accept_len"], "2.0535"));
    rmtree(&pubd);
    wrb(&bl.join("lite_raw_cl.json"), &root_raw)?;
    // ── G2 ──
    let sp = bl.join("serve_proof_cl.json");
    wj(&sp, &jd! {"config" => "cl", "health_http" => 200})?;
    let ev2 = rep(&ev7, &[("serve_proof", jd! {"config" => "cl"}), ("sources", rep(&ev7["sources"], &[("serve_proof", J::from("output/multi/benchlog/serve_proof_cl.json"))]))]);
    utime(&sp, 1767326400)?;
    let sc2 = serve_proof_scope(r, &ev2)?.unwrap_or_default();
    c.ck("G2 serve_proof 후작성은 재기동 단정 없이 측정 시점 직접관측 미검증으로 남긴다",
        sc2.truthy() && eqs(&sc2["timing"], "after-measurement") && has(&sc2["timing_note"], "측정 뒤 작성된 스모크 산출물")
        && has(&sc2["timing_note"], "미검증") && !has(&sc2["timing_note"], "재기동"));
    utime(&sp, 1767322800)?;
    utime(&sp, 1767322700)?;
    let sc2b = serve_proof_scope(r, &ev2)?.unwrap_or_default();
    c.ck("★G2 음성대조: 측정 전 작성 = 측정 뒤 ✗ · serve_proof 없으면 None",
        sc2b.truthy() && eqs(&sc2b["timing"], "before-measurement") && !has(&sc2b["timing_note"], "측정 뒤")
        && serve_proof_scope(r, &rep(&ev2, &[("serve_proof", J::Null)]))?.is_none());
    // ── G8 ──
    let g8rows: [(&str, Vec<(&str, &str, Option<&str>)>); 2] = [
        ("main", vec![("2026-01-01T00:00:00Z", "budget_declare", Some("smoke-cl")), ("2026-01-02T02:50:00Z", "budget_declare", Some("smoke-cl")),
            ("2026-01-02T03:30:00Z", "budget_clear", None)]),
        ("sub", vec![("2025-12-01T00:00:00Z", "budget_declare", Some("smoke-old"))])];
    for (node, rows) in &g8rows {
        let d = r.join(REL_EVENTS_ROOT).join(node).join("events");
        mkd(&d)?;
        let lines: Vec<J> = rows.iter().map(|(ts, k, lb)| {
            let mut j = jd! {"ts" => *ts, "kind" => *k};
            if let Some(l) = lb { j.set("label", *l); }
            if *k == "budget_clear" { j.set("existed", true); }
            j
        }).collect();
        wr0(&d.join("2026-01.jsonl"), &jsonl(&lines))?;
    }
    let ev8 = rep(&ev2, &[("node", J::from("cluster")), ("declaration", jd! {"nodes" => jl![jd! {"node_id" => "main", "role" => "main"}, jd! {"node_id" => "sub", "role" => "sub"}]})]);
    let spans = |e: &J| -> HR<Vec<(String, J)>> { Ok(event_ledger_spans(r, e)?.into_iter().map(|x| (st(&x["node"]), x)).collect()) };
    let sget = |v: &[(String, J)], n: &str| v.iter().find(|(a, _)| a == n).map(|(_, x)| x.clone()).unwrap_or_default();
    let sp8 = spans(&ev8)?;
    c.ck("G8 원장 관측 범위: 노드별 첫·마지막 행 · 측정 끝보다 앞에서 끝난 노드 = covers_measurement False",
        eqs(&sget(&sp8, "main")["last_utc"], "2026-01-02T03:30:00Z") && sget(&sp8, "main")["covers_measurement"] == J::Bool(true)
        && eqs(&sget(&sp8, "sub")["last_utc"], "2025-12-01T00:00:00Z") && sget(&sp8, "sub")["covers_measurement"] == J::Bool(false));
    let sn_fx = td.join("self_node_fx");
    wr(&sn_fx.join("output/single/manifest.yaml"), "nodes:\n  - role: main\n")?;
    mkd(&sn_fx.join("output/multi"))?;
    c.ck("F9 이 체크아웃 노드 = node_identity.sh --resolve 실물(유일한 role: main)", _self_node(&sn_fx)?.0.as_deref() == Some("main"));
    wr0(&sn_fx.join("output/single/manifest.yaml"), "self_role: main\nnodes:\n  - role: main\n")?;
    wr0(&sn_fx.join("output/multi/manifest.yaml"), "self_role: sub\nnodes:\n  - role: main\n")?;
    cache_put(&core::posix(&core::resolve(&sn_fx)), None);
    let sfx = _self_node(&sn_fx)?;
    c.ck("★F9 해소기 fail-loud(manifest 간 self_role 불일치)면 None + 사유(hostname 대체 ✗)", sfx.0.is_none() && _self_node(&sn_fx)?.1.contains("rc=1"));
    let rkey = core::posix(&core::resolve(r));
    let saved = cache_put(&rkey, Some((Some("sub".into()), "fixture 주입".into())));
    let res = (|| -> HR<(Vec<(String, J)>, Vec<J>)> {
        let sp9 = spans(&ev8)?;
        cache_put(&rkey, Some((None, "fixture 미해소".into())));
        Ok((sp9, event_ledger_spans(r, &ev8)?))
    })();
    cache_put(&rkey, saved);
    let (sp9, unres) = res?;
    c.ck("F9 이 체크아웃 노드의 원장 = live_local True · 다른 노드 = False · covers_measurement 는 그대로",
        sget(&sp9, "sub")["live_local"] == J::Bool(true) && sget(&sp9, "main")["live_local"] == J::Bool(false)
        && sget(&sp9, "sub")["covers_measurement"] == J::Bool(false) && has(&sget(&sp9, "sub")["live_local_source"], "sub"));
    c.ck("★F9 노드 미해소면 어느 노드도 live_local 이 아니다(미해소 사유는 출처에)",
        unres.iter().all(|x| x["live_local"] == J::Bool(false) && has(&x["live_local_source"], "미해소")));
    // F8 회수 미러
    let mir = r.join(REL_EVENTS_RECOVERED_ROOT).join("sub/events");
    mkd(&mir)?;
    wr0(&mir.join("2026-01.jsonl"), &jsonl(&[jd! {"ts" => "2025-12-01T00:00:00Z", "kind" => "budget_declare", "label" => "smoke-old"},
        jd! {"ts" => "2026-01-01T12:00:00Z", "kind" => "budget_declare", "label" => "smoke-later"}]))?;
    wr0(&mir.join("watchdog.jsonl"), &(ad(&jd! {"ts" => "2026-01-02T00:02:35Z", "kind" => "budget_none"}) + "\n"))?;
    wr(&r.join(REL_EVENTS_RECOVERED_ROOT).join("main/events/2026-01.jsonl"), &(ad(&jd! {"ts" => "2027-01-01T00:00:00Z", "kind" => "budget_declare", "label" => "x"}) + "\n"))?;
    let saved = cache_put(&rkey, Some((Some("main".into()), "fixture 주입".into())));
    let res = (|| -> HR<(Vec<(String, J)>, Vec<J>)> { Ok((spans(&ev8)?, event_timeline(r, &ev8)?)) })();
    cache_put(&rkey, saved);
    rmtree(&r.join("sync_staging"));
    let (spm, tl_m) = res?;
    let ss = sget(&spm, "sub");
    c.ck("★F8 다른 노드 원장 = 회수 미러(이름마다 미러 우선 · 가린 로컬 사본 기재) · 관측 범위 끝 = 미러 마지막 행 · 측정 끝 전이면 covers False 유지",
        eqs(&ss["last_utc"], "2026-01-02T00:02:35Z") && ss["covers_measurement"] == J::Bool(false)
        && ss["files"] == jl![format!("{REL_EVENTS_RECOVERED_ROOT}/sub/events/2026-01.jsonl"), format!("{REL_EVENTS_RECOVERED_ROOT}/sub/events/watchdog.jsonl")]
        && ss["shadowed_local_copies"] == jl![format!("{REL_EVENTS_ROOT}/sub/events/2026-01.jsonl")] && has(&ss["location"], "회수 미러"));
    let sm = sget(&spm, "main");
    c.ck("★F8 이 체크아웃 노드(main) = 로컬 원장만(미러의 main 사본은 읽지 않는다) · 같은 사건 이중 적재 ✗",
        eqs(&sm["last_utc"], "2026-01-02T03:30:00Z") && sm["shadowed_local_copies"] == J::list()
        && !tl_m.iter().any(|x| x["utc"].display().contains("2027-01-01")) && tl_m.iter().filter(|x| eqs(&x["utc"], "2025-12-01T00:00:00Z")).count() <= 1);
    let cc = core::run_capture(&strs(&["bash", &core::posix(&core::skill_dir().parent().unwrap().join("adversarial-benchmark/scripts/classify_cell_api.sh")), "constants"]),
        None, &[], &[], None).map_err(py)?;
    if cc.returncode != 0 {
        return Err(core::HintError::py("CalledProcessError", format!("classify_cell_api.sh constants rc={}", cc.returncode)));
    }
    let ccb = lj(&String::from_utf8_lossy(&cc.stdout))?;
    c.ck("F8 미러 자리 = classify_cell.EVENTS_RECOVERED_PARTS(벤치 판정이 스캔하는 자리 · 교차검증)",
        ccb["EVENTS_RECOVERED_PARTS"] == J::from(REL_EVENTS_RECOVERED_ROOT.split('/').map(str::to_string).collect::<Vec<_>>()));
    let ev8b = rep(&ev8, &[("declaration", jd! {"nodes" => jl![jd! {"node_id" => "main"}, jd! {"node_id" => "ghost"}]})]);
    c.ck("★G8 음성대조: 선언 노드인데 원장 없음 = 범위 None(없음으로 단언 ✗)", sget(&spans(&ev8b)?, "ghost")["last_utc"].is_null());
    // ── G10 ──
    let att_rel = "output/multi/benchlog/attestation_cl.json";
    wj(&r.join(att_rel), &jd! {"config" => "cl", "phase" => "serve"})?;
    let ev10 = rep(&ev8, &[("attestation", jd! {"_path" => att_rel, "config" => "cl", "phase" => "serve"})]);
    let lec = bl.join("lite_engine_cl.log");
    wr0(&lec, "(APIServer pid=5) INFO 01-02 03:00:10 [x.py:1] POST\n")?;
    let sl = r.join("docs/simlog/fx_run");
    mkd(&sl)?;
    let good: Vec<String> = vec!["[mn] main: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:00Z\",\"kind\":\"budget_honored\"}".into(), "[mn] SMOKE PASS — ok".into(),
        format!("[mn] 노드 정합 attestation v2(serve) → {att_rel}"), "[mn] --keep-up: 워치독 유지".into()];
    let fxs = sl.join("fx_serve.log");
    let wl = |v: &[String]| wr0(&fxs, &(v.join("\n") + "\n"));
    wl(&good)?;
    let win = jd! {"start_utc" => t0, "end_utc" => t1};
    let w55 = Some("2026-01-02T02:55:00Z");
    let sb = attestation_same_boot(r, &ev10, w55, &win)?.unwrap_or_default();
    c.ck("G10 스모크 로그 순서 · 원장 단일 선언(재선언 ✗) · APIServer pid 하나 = 같은 기동의 스모크 직후 관측(근거 파일 · 줄)",
        sb.truthy() && has(&sb["basis"], "L2 SMOKE PASS → L3 attestation 작성 → L4 --keep-up") && has(&sb["basis"], "APIServer pid 5 하나")
        && eqs(&sb["files"][0], "docs/simlog/fx_run/fx_serve.log"));
    utime(&r.join(att_rel), 1767322500)?;
    let sc10 = attestation_scope(r, &ev10)?.unwrap_or_default();
    c.ck("G10 attestation 범위 scope 가 판별을 말한다('미검증' ✗) · 호스트 사실 출처에도 표지",
        sc10.truthy() && has(&sc10["scope"], SAME_BOOT_MARK) && !has(&sc10["scope"], "미검증")
        && _attestation_timing_note(&rep(&ev10, &[("repo", pj(r))]))?.unwrap_or_default().contains(SAME_BOOT_MARK));
    let reord = |v: &[String]| vec![v[0].clone(), v[1].clone(), v[3].clone(), v[2].clone()];
    wl(&reord(&good))?;
    c.ck("★G10 음성대조: 순서가 어긋나면(keep-up 이 attestation 앞) 판별 ✗ → scope '미검증' 유지",
        attestation_same_boot(r, &ev10, w55, &win)?.is_none() && attestation_scope(r, &ev10)?.map_or(false, |s| has(&s["scope"], "미검증")));
    wl(&good)?;
    wr0(&lec, "(APIServer pid=5) a\n(APIServer pid=6) b\n")?;
    c.ck("★G10 음성대조: 측정 로그에 APIServer pid 둘(서빙 프로세스 교체) = 판별 ✗", attestation_same_boot(r, &ev10, w55, &win)?.is_none());
    wr0(&lec, "(APIServer pid=5) a\n")?;
    let led = r.join(REL_EVENTS_ROOT).join("main/events/2026-01.jsonl");
    let led_bytes = rdb(&led)?;
    let mut off = good.clone();
    off[0] = good[0].replace("02:50:00Z", "02:50:01Z");
    wl(&off)?;
    c.ck("★G10 음성대조: 스모크 로그 honored ts 가 선언 · 원장 honored 어디에도 없으면 판별 ✗", attestation_same_boot(r, &ev10, w55, &win)?.is_none());
    app(&led, &(ad(&jd! {"ts" => "2026-01-02T02:50:01Z", "kind" => "budget_honored", "label" => "smoke-cl"}) + "\n"))?;
    let sb1 = attestation_same_boot(r, &ev10, w55, &win)?.unwrap_or_default();
    c.ck("G10 스모크 로그 honored ts = 원장의 같은 선언 budget_honored(1초 뒤)면 강한 판별",
        sb1.truthy() && eqs(&sb1["strength"], "strong") && has(&sb1["basis"], "02:50:01Z = 그 선언의 원장 budget_honored"));
    let sub_echo: Vec<String> = vec![
        "[mn]   sub: 선언 발행: /x/docs/logs/sub/serve_budget.env".into(),
        "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:04Z\",\"kind\":\"budget_honored\",\"label\":\"smoke-cl\"}".into(),
        "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:05Z\",\"kind\":\"budget_honored\",\"label\":\"smoke-other\"}".into(),
        "[mn]   sub: budget_honored ✓ {\"ts\":\"2026-01-02T02:50:06Z\",\"kind\":\"budget_clear\"}".into(),
        "[mn]   sub: mem_kill {\"ts\":\"2026-01-02T02:50:07Z\",\"kind\":\"mem_kill\"}".into()];
    let mut v = vec![off[0].clone()];
    v.extend(sub_echo.iter().cloned());
    v.extend(off[1..].iter().cloned());
    wl(&v)?;
    let sp10 = spans(&ev10)?;
    let se = sget(&sp10, "sub")["smoke_echo"].clone();
    let rows: Vec<(String, String, String)> = se["rows"].items().iter().map(|x| (st(&x["ts"]), st(&x["kind"]), st(&x["source"]))).collect();
    c.ck("F10 미러 밖 노드: 같은 기동 스모크 로그의 그 노드 원장 JSON echo(선언 · honored · 이 셀 라벨)만 보조 관측 · 출처 L 번호",
        sget(&sp10, "sub")["covers_measurement"] == J::Bool(false) && eqs(&se["log"], "docs/simlog/fx_run/fx_serve.log")
        && rows == vec![("2026-01-02T02:50:04Z".to_string(), "budget_honored".to_string(), "docs/simlog/fx_run/fx_serve.log:3".to_string())]);
    c.ck("★F10 음성대조: 측정 끝을 덮는 노드(main)는 보조 관측 ✗ · 문장 줄 · 다른 라벨 · kind 불일치 · 목록 밖 kind 는 싣지 않는다",
        !inn(&sget(&sp10, "main"), "smoke_echo") && rows.len() == 1);
    let mut v = vec![off[0].clone()];
    v.extend(sub_echo.iter().cloned());
    v.extend([off[1].clone(), off[3].clone(), off[2].clone()]);
    wl(&v)?;
    c.ck("★F10 음성대조: 같은 기동 판별 불성립(순서 어긋남)이면 스모크 로그 echo 를 싣지 않는다", !inn(&sget(&spans(&ev10)?, "sub"), "smoke_echo"));
    wl(&off)?;
    rm(&fxs)?;
    let outside_smoke = r.join("docs/logs/fx_serve.log");
    wr(&outside_smoke, &(good.join("\n") + "\n"))?;
    wr0(&lec, "(APIServer pid=5) INFO 01-02 02:52:00 [entry.py:1] Starting vLLM server on http://x\n")?;
    let sbw = attestation_same_boot(r, &ev10, w55, &win)?.unwrap_or_default();
    let scw = attestation_scope(r, &ev10)?.unwrap_or_default();
    c.ck("G10 약한 판별: 제한검색 밖 스모크 로그 보존을 부정하지 않으며 기존 원장 순서 판별 유지",
        sbw.truthy() && eqs(&sbw["strength"], "weak") && has(&sbw["basis"], "결속된 스모크 로그 미확인 · 제한검색")
        && has(&sbw["basis"], "다른 경로의 보존 여부는 판정하지 않음") && !has(&sbw["basis"], "미보존")
        && has(&sbw["basis"], "serve_proof 작성 2026-01-02T02:58:20Z") && has(&sbw["basis"], "API 서버 기동 2026-01-02T02:52:00Z")
        && has(&scw["scope"], WEAK_SAME_BOOT_MARK) && !has(&scw["scope"], SAME_BOOT_MARK) && !has(&scw["scope"], "미검증")
        && _attestation_timing_note(&rep(&ev10, &[("repo", pj(r))]))?.unwrap_or_default().contains(WEAK_SAME_BOOT_MARK));
    wr0(&lec, "(APIServer pid=5) INFO 01-02 02:57:00 [entry.py:1] Starting vLLM server on http://x\n")?;
    c.ck("★G10 약한 판별 음성대조: API 서버 기동이 attestation 작성 뒤(모순) = 판별 ✗ · scope '미검증'",
        attestation_same_boot(r, &ev10, w55, &win)?.is_none() && attestation_scope(r, &ev10)?.map_or(false, |s| has(&s["scope"], "미검증")));
    wr0(&lec, "(APIServer pid=5) a\n")?;
    utime(&sp, 1767322860)?;
    c.ck("★G10 약한 판별 음성대조: serve_proof 작성이 측정 시작 이후 = 판별 ✗", attestation_same_boot(r, &ev10, w55, &win)?.is_none());
    utime(&sp, 1767322700)?;
    c.ck("G10 약한 판별: API 서버 기동 미관측은 모순이 아니다(근거에 미관측을 적는다)",
        attestation_same_boot(r, &ev10, w55, &win)?.map_or(false, |s| has(&s["basis"], "API 서버 기동 시각 미관측")));
    wl(&reord(&good))?;
    c.ck("★G10 음성대조: 스모크 로그가 있는데 순서가 어긋나면 약한 판별로 내려가지 않는다(모순)", attestation_same_boot(r, &ev10, w55, &win)?.is_none());
    wl(&good)?;
    wrb(&led, &led_bytes)?;
    app(&led, &(ad(&jd! {"ts" => "2026-01-02T03:00:30Z", "kind" => "budget_declare", "label" => "smoke-other"}) + "\n"))?;
    c.ck("★G10 음성대조: 측정 끝 전 재선언(다른 기동) = 판별 ✗", attestation_same_boot(r, &ev10, w55, &win)?.is_none());
    _selftest_lite_tool_args(c, td)
}

fn _selftest_lite_tool_args(c: &mut Ck, td: &Path) -> HR<()> {
    let g = td.join("lite-tool-args");
    let tdir = g.join(REL_BENCH_TOOLS_DIR);
    mkd(&tdir)?;
    wr0(&tdir.join("lite_bench.sh"), &["#!/bin/bash",
        "BLOG=\"$OUTDIR/lite_client.log\" # vllm bench serve --random-input-len 9999 --random-output-len 4096",
        "_bench() {", "  docker exec \"$CTR\" bash -lc \"cd /tmp && vllm bench serve \\",
        "      --backend $BACKEND --base-url $BASE_URL --model '$MODEL_NAME' --trust-remote-code \\",
        "      --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \\",
        "      --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \\",
        "      --save-result --result-dir /tmp\"", "}", "MEASURED_UTC=\"$(date -u +%FT%TZ)\"", ""].join("\n"))?;
    wr0(&tdir.join("sweep_bench.sh"), "#!/bin/bash\nbash \"$SDIR/lite_bench.sh\" \"$CONFIG\"\nbash \"$SDIR/run_bench.sh\" \"$CONFIG\"\n")?;
    let ge = genv(Some("2026-01-01T00:00:00Z"))?;
    git_e(&g, &["init", "-q"], &ge)?;
    git_e(&g, &["add", "-A"], &ge)?;
    git_e(&g, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "c"], &ge)?;
    mkd(&g.join("docs/benchmark"))?;
    let measured = "2026-01-02T03:00:00Z";
    wr0(&g.join("docs/benchmark/bench_report_fx.md"), &format!("# lite\n생성일 {measured}\n"))?;
    let lite_ev = rep(&new_cell_evidence("campaign", Some("fixture"), "cl", "main", "single"),
        &[("repo", pj(&g)), ("report_kind", J::from("lite")), ("bench_report_path", J::from("docs/benchmark/bench_report_fx.md"))]);
    let ta = lite_tool_args(&g, &lite_ev)?;
    let fl: Vec<(String, J)> = ta.as_ref().map(|t| t["flags"].items().iter().map(|p| (st(&p[0]), p[1].clone())).collect()).unwrap_or_default();
    let fget = |k: &str| fl.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.clone());
    let has_f = |k: &str| fl.iter().any(|(a, _)| a == k);
    let src = ta.as_ref().map(|t| st(&t["source"])).unwrap_or_default();
    c.ck("G3 lite 도구 원문 인자(리터럴만): --random-input-len 512 · --random-range-ratio 0 · --ignore-eos · 출처 도구@rev 줄 · MEASURED_UTC 줄",
        ta.is_some() && fget("--random-input-len") == Some(J::from("512")) && fget("--random-range-ratio") == Some(J::from("0")) && has_f("--ignore-eos")
        && fget("--max-concurrency") == Some(J::from("1")) && src.starts_with("lite_bench.sh@") && ta.as_ref().map_or(false, |t| eqs(&t["measured_line"], "L10"))
        && src.contains("L4-L8") && !fl.iter().any(|(_, v)| eqs(v, "9999") || eqs(v, "4096")));
    c.ck("★G3 음성대조: `$변수` · 따옴표 인자(--backend · --model · --num-prompts · --num-warmups)는 싣지 않는다 · git 아니면 None",
        !["--backend", "--model", "--num-prompts", "--num-warmups"].iter().any(|k| has_f(k)) && lite_tool_args(td, &lite_ev)?.is_none());
    let sweep_rel = "output/single/benchlog/sweep_cl";
    let sdir = g.join(sweep_rel);
    mkd(&sdir)?;
    for (leg, n) in [("cold", 1i64), ("warm", 3)] {
        wj(&sdir.join(format!("lite_{leg}_cl.json")), &jd! {"backend" => "openai-chat", "model_id" => "fixture", "num_prompts" => n, "completed" => n,
            "total_input_tokens" => 520 * n, "total_output_tokens" => 128 * n, "max_concurrency" => 1, "request_rate" => "inf", "date" => "20260102-025900"})?;
    }
    let full_ev = rep(&lite_ev, &[("report_kind", J::from("full")), ("certificate", jd! {"measured_utc" => measured}),
        ("sweep", jd! {"dir" => sweep_rel, "binding" => "output", "generated_utc" => measured, "index" => jd! {"lite" => J::dict()}, "levels" => jl![jd! {"concurrency" => 1}]})]);
    let commands = |e: &J| -> HR<(String, String)> {
        let ctx = crate::artifacts::_Ctx::new(&g, e, None, None, None, None, "single", "cl", "main");
        crate::artifacts::_bench_commands(&ctx)
    };
    let full_args = lite_tool_args(&g, &full_ev)?;
    let (command, source) = commands(&full_ev)?;
    c.ck("G3 full 내 lite: 실제 source resolver를 거쳐 512/128 복원 · 520은 측정 평균 주석으로만 보존",
        full_args == ta && command.contains("--random-input-len 512") && command.contains("--random-output-len 128")
        && !command.contains("--random-input-len 520") && command.contains("입력 1560/3 = 520")
        && command.contains("--random-range-ratio 0") && command.contains("--ignore-eos") && source == "reconstructed(bench json + 측정 도구 원문 lite 인자)");
    let lb_rel = format!("{REL_BENCH_TOOLS_DIR}/lite_bench.sh");
    core::git(&g, &["rm", "--", &lb_rel])?;
    git_e(&g, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "missing"], &genv(Some("2026-01-01T00:00:01Z"))?)?;
    wr0(&tdir.join("lite_bench.sh"), "vllm bench serve --random-input-len 2048\n")?;
    let (command_missing, source_missing) = commands(&full_ev)?;
    c.ck("★G3 full 원문 부재: 작업트리로 대체하지 않고 None · 평균 인자에는 원문 미관측 경고",
        lite_tool_args(&g, &full_ev)?.is_none() && lite_tool_args(&g, &lite_ev)?.is_none()
        && command_missing.contains("--random-input-len 520") && command_missing.contains("원문 인자 미관측")
        && !command_missing.contains("--random-input-len 2048") && source_missing == "reconstructed(bench json)");
    Ok(())
}

fn _selftest_custom_sessions(c: &mut Ck, repo: &Path) -> HR<()> {
    let (cell, config, node, sid) = ("fixture-cell", "fixture-config", "worker-a", "operator_attempt-01");
    let (start, stop, measured) = ("2026-01-02T02:00:01Z", "2026-01-02T03:10:00Z", "2026-01-02T03:00:00Z");
    let journal = repo.join(REL_EVENTS_RECOVERED_ROOT).join(node).join("sessions").join(format!("{sid}.json"));
    let local = repo.join(REL_EVENTS_ROOT).join(node).join("sessions").join(journal.file_name().unwrap());
    let doc = jd! {"session_id" => sid, "config_file" => config, "topology" => "single", "started_utc" => start, "stopped_utc" => stop};
    wj(&journal, &doc)?;
    wj(&local, &rep(&doc, &[("config_file", J::from("other-config"))]))?;
    let ledger = journal.parent().unwrap().parent().unwrap().join("events/2026-01.jsonl");
    std::fs::create_dir(ledger.parent().unwrap()).map_err(py)?;
    let start_event = jd! {"kind" => "serve_start", "session_id" => sid, "ts" => start, "source" => "blackbox_session", "topology" => "single"};
    let events = [jd! {"kind" => "budget_declare", "label" => "unrelated-label", "ts" => "2026-01-02T01:00:00Z"},
        jd! {"kind" => "budget_clear", "ts" => "2026-01-02T01:30:00Z"},
        jd! {"kind" => "budget_declare", "label" => "operator-label", "ts" => "2026-01-02T02:00:00Z"}, start_event.clone(),
        rep(&start_event, &[("ts", J::from("2026-01-02T02:01:00Z"))]),
        jd! {"kind" => "budget_renew", "label" => "operator-label", "ts" => "2026-01-02T02:30:00Z"},
        jd! {"kind" => "serve_stop", "session_id" => sid, "ts" => stop, "source" => "blackbox_session"},
        jd! {"kind" => "budget_clear", "ts" => "2026-01-02T03:10:01Z"}];
    wr0(&ledger, &jsonl(&events))?;
    let ev = rep(&new_cell_evidence("campaign", Some("fixture"), cell, "sub", "single"), &[("repo", pj(repo)),
        ("declaration", jd! {"nodes" => jl![jd! {"node_id" => node, "role" => "sub"}]}),
        ("sweep", jd! {"generated_utc" => measured, "index" => jd! {"meta" => jd! {"config_name" => config}}})]);
    let window = || crate::artifacts::current_serve_window(repo, &ev, Some("2026-01-02T02:30:00Z"), Some(measured));
    let lt = || -> HR<Vec<J>> { Ok(_ledger_timeline(repo, &ev, &J::Null)?.0) };
    let key = core::posix(&core::resolve(repo));
    let old_self = cache_put(&key, Some((Some("main".into()), "fixture".into())));
    let res = (|| -> HR<()> {
        let original: Vec<(PathBuf, Vec<u8>)> = vec![(journal.clone(), rdb(&journal)?), (local.clone(), rdb(&local)?), (ledger.clone(), rdb(&ledger)?)];
        let bound = window()?.unwrap_or_default();
        let jrel = _rel(repo, &journal)?;
        c.ck("custom ID: official mirrored journal binds config alias/node/topology and closed measured window",
            bound.truthy() && eqs(&bound["session_id"], sid) && eqs(&bound["node_id"], node) && eqs(&bound["source"], &jrel) && eqs(&bound["start_utc"], start));
        let (rows, _files, budget_window) = _ledger_timeline(repo, &ev, &J::Null)?;
        let starts = kinds(&rows, "serve_start");
        c.ck("custom ID: exact journal start/stop events are included; another timestamp is not this session event",
            starts.len() == 1 && eqs(&starts[0]["utc"], start) && eqs(&starts[0]["session_binding_source"], &jrel) && kinds(&rows, "serve_stop").len() == 1);
        let budgets = kinds(&rows, "budget_declare");
        c.ck("custom label: only overlapping budget window is shown, still explicitly other-label-same-node",
            budgets.len() == 1 && eqs(&budgets[0]["label"], "operator-label") && eqs(&budgets[0]["label_scope"], LABEL_SCOPE_OTHER) && budget_window.is_none());
        let bd = budgets.first().map(|b| st(&b["detail"])).unwrap_or_default();
        c.ck("custom label: literal mismatch does not deny the independently bound session's measurement",
            bound.truthy() && eqs(&bound["session_id"], sid) && bd.contains("라벨 불일치만으로 이 측정·기동의 귀속 여부를 판단하지 않는다")
            && bd.contains("셀 라벨 일치 집계에서 제외") && bd.contains("세션·호출 기록의 귀속 확인과 별도")
            && !bd.contains("이 셀의 선언으로 세지 않는다") && !bd.contains("이 측정의 기동으로 귀속하지 않는다"));
        c.ck("custom ID: binding journal is retained in timeline lineage", _timeline_parts(repo, &ev)?.1.contains(&(jrel.clone(), "raw_json".to_string())));
        let mut same = true;
        for (p, b) in &original {
            same &= rdb(p)? == *b;
        }
        c.ck("custom ID: observation never edits original journals/events", same);
        wj(&journal, &rep(&doc, &[("stopped_utc", J::Null)]))?;
        let open_rows = lt()?;
        c.ck("custom ID: open journal proves start only, never a closed measurement window or unmatched stop",
            window()?.is_none() && kinds(&open_rows, "serve_start").len() == 1 && kinds(&open_rows, "serve_stop").is_empty());
        for (label, k, v) in [("config", "config_file", "other"), ("topology", "topology", "multi"), ("filename/SID", "session_id", "different_id"),
            ("invalid start", "started_utc", "invalid"), ("stop before start", "stopped_utc", "2026-01-01T03:10:00Z")] {
            wj(&journal, &rep(&doc, &[(k, J::from(v))]))?;
            c.ck(&format!("custom ID: {label} mismatch cannot bind by free name/label or stale local copy"), window()?.is_none() && lt()?.is_empty());
        }
        wj(&journal, &doc)?;
        let sessions = _node_session_docs(repo, node)?;
        for (label, event) in [("timestamp", rep(&start_event, &[("ts", J::from("2026-01-02T02:01:00Z"))])),
            ("topology", rep(&start_event, &[("topology", J::from("multi"))])), ("source", rep(&start_event, &[("source", J::from("free-form-log"))]))] {
            c.ck(&format!("custom ID: conflicting event {label} is rejected"), !_session_event_is_cell(&event, &st(&event["ts"]), &ev, &sessions)?);
        }
        let legacy = format!("serve-{cell}-20260102T020001Z");
        let legacy_event = rep(&start_event, &[("session_id", J::from(legacy.as_str()))]);
        c.ck("legacy canonical event remains supported when no journal exists", _session_event_is_cell(&legacy_event, start, &ev, &J::dict())?);
        let mut contra = Map::new();
        contra.insert(legacy.as_str(), rep(&doc, &[("config_file", J::from("other"))]));
        c.ck("explicit contradictory journal overrides even a canonical-looking session name", !_session_event_is_cell(&legacy_event, start, &ev, &J::Dict(contra))?);
        let canonical_journal = journal.with_file_name(format!("{legacy}.json"));
        wj(&canonical_journal, &rep(&doc, &[("session_id", J::from(legacy.as_str())), ("config_file", J::from("other"))]))?;
        let canonical_label = format!("serve-{cell}");
        let labelled = [jd! {"kind" => "budget_declare", "label" => canonical_label.as_str(), "ts" => "2026-01-02T02:00:00Z"},
            rep(&legacy_event, &[("label", J::from(canonical_label.as_str()))]),
            jd! {"kind" => "budget_clear", "ts" => "2026-01-02T02:10:00Z"},
            rep(&legacy_event, &[("kind", J::from("serve_stop")), ("label", J::from(canonical_label.as_str())), ("ts", J::from(stop))])];
        wr0(&ledger, &jsonl(&labelled))?;
        let labelled_rows = lt()?;
        c.ck("canonical label cannot override contradictory journal inside or outside a same-label budget window",
            labelled_rows.iter().any(|r| eqs(&r["kind"], "budget_declare")) && !labelled_rows.iter().any(|r| eqs(&r["kind"], "serve_start") || eqs(&r["kind"], "serve_stop")));
        wrb(&ledger, &original[2].1)?;
        rm(&canonical_journal)?;
        cache_put(&key, Some((Some(node.into()), "fixture".into())));
        c.ck("own-node journal uses local bytes, never the recovered mirror", window()?.is_none());
        cache_put(&key, Some((Some("main".into()), "fixture".into())));
        rm(&journal)?;
        symlink(&local, &journal)?;
        c.ck("custom ID: symlink journal cannot supply session identity", window()?.is_none() && lt()?.is_empty());
        rm(&journal)?;
        wj(&journal, &doc)?;
        let jp = journal.parent().unwrap().to_path_buf();
        let saved_dir = jp.with_file_name("session-storage");
        mv(&jp, &saved_dir)?;
        symlink(&saved_dir, &jp)?;
        c.ck("custom ID: ancestor symlink cannot redirect node identity", window()?.is_none() && lt()?.is_empty());
        rm(&jp)?;
        mv(&saved_dir, &jp)?;
        let other = repo.join(REL_EVENTS_RECOVERED_ROOT).join("worker-b/sessions").join(journal.file_name().unwrap());
        mkd(other.parent().unwrap())?;
        mv(&journal, &other)?;
        c.ck("custom ID: same journal under another node does not bind this node's event/window", window()?.is_none() && lt()?.is_empty());
        Ok(())
    })();
    cache_put(&key, old_self);
    res
}

fn _selftest_runtime_epochs(c: &mut Ck, repo: &Path) -> HR<()> {
    let (cell, measured) = ("epoch-cell", "2026-01-02T03:00:00Z");
    let swrel = format!("output/single/benchlog/sweep_{cell}_fresh");
    let swdir = repo.join(&swrel);
    let idx = jd! {"generated_utc" => measured, "lite" => J::dict(), "levels" => J::list(), "meta" => jd! {"config_name" => cell, "vllm_build" => "0.8.0-old"}};
    wj(&swdir.join("sweep_index.json"), &idx)?;
    wj(&swdir.join(format!("lite_raw_{cell}.json")), &jd! {"config_name" => cell, "measured_utc" => "2026-01-02T02:30:00Z"})?;
    let (head, tail) = (swdir.join(format!("lite_engine_head_{cell}.log")), swdir.join(format!("lite_engine_{cell}.log")));
    wr0(&head, "INFO 01-01 01:00:00 Initializing a V1 LLM engine (v0.8.0-old)\n")?;
    wr0(&tail, "old capture\nINFO 01-02 02:00:05 Initializing a V1 LLM engine (v0.9.0-current)\nINFO 01-02 03:01:00 Initializing a V1 LLM engine (v0.9.1-after)\nInitializing a V1 LLM engine (v0.9.2-untimed)\n")?;
    let session = repo.join(format!("docs/logs/sub/sessions/serve-{cell}-20260102T020000Z.json"));
    wj(&session, &jd! {"session_id" => format!("serve-{cell}-20260102T020000Z"), "config_file" => cell, "topology" => "single",
        "started_utc" => "2026-01-02T02:00:00Z", "stopped_utc" => "2026-01-02T03:10:00Z"})?;
    let mut ev = rep(&new_cell_evidence("campaign", Some("epoch"), cell, "sub", "single"), &[("repo", pj(repo)),
        ("sweep", jd! {"dir" => swrel.as_str(), "binding" => "output", "generated_utc" => measured, "index" => idx.clone(), "levels" => J::list()}),
        ("build_identity", jd! {"vllm_sha" => J::Null, "source" => J::dict()})]);
    let captured: Vec<(PathBuf, Vec<u8>)> = vec![(head.clone(), rdb(&head)?), (tail.clone(), rdb(&tail)?), (session.clone(), rdb(&session)?)];
    let banner = _engine_banner(repo, &ev)?;
    c.ck("FC4 old head/meta, future and untimed banners do not replace the current bound epoch",
        eqs(&banner["value"], "0.9.0-current") && eqs(&banner["basis"], "this-measurement")
        && has(&banner["source"], &format!("lite_engine_{cell}.log:2")) && has(&banner["source"], &_rel(repo, &session)?));
    let mut same = true;
    for (p, b) in &captured {
        same &= rdb(p)? == *b;
    }
    c.ck("FC4 banner observation preserves captured bytes", same);
    wr0(&tail, "Initializing a V1 LLM engine (v0.9.2-untimed)\n")?;
    c.ck("FC4 a bound session with only historical/untimed banners stays unobserved", _engine_banner(repo, &ev)?["value"].is_null());
    wrb(&tail, &captured[1].1)?;
    rm(&session)?;
    c.ck("FC4 missing session boundary never claims this-measurement", !eqs(&_engine_banner(repo, &ev)?["basis"], "this-measurement"));
    wrb(&session, &captured[2].1)?;
    c.ck("FC4 yearless timestamps cross the year only inside the declared window",
        _banner_in_window(Some("12-31 23:59:55"), "2025-12-31T23:59:50Z", "2026-01-01T00:00:10Z")?.as_deref() == Some("2025-12-31T23:59:55Z")
        && _banner_in_window(Some("01-01 01:00:00"), "2025-12-31T23:59:50Z", "2026-01-01T00:00:10Z")?.is_none());
    let index_rel = "docs/simlog/epoch/attempt2/evidence_index.json";
    let runtime_rel = "docs/simlog/epoch/attempt2/runtime_identity.json";
    let observation = jd! {"source_head" => "e".repeat(40), "source_describe" => "v0.9.0", "vllm_version" => "0.9.1.dev0+gfixture.cu130"};
    let binding = jd! {"config" => cell, "measured_node" => "sub", "sweep_dir" => swrel.as_str(), "sweep_generated_utc" => measured, "installed_runtime_identity" => runtime_rel};
    wj(&repo.join(index_rel), &binding)?;
    wj(&repo.join(runtime_rel), &observation)?;
    ev.set("pointers", jl![jd! {"kind" => "simlog", "path" => index_rel, "cell_id" => cell, "node_id" => "sub"}]);
    let before = crate::value::dumps_tight_sorted(&ev["build_identity"]);
    let vo = |e: &J| vllm_observed(e, None);
    let observed = vo(&ev)?;
    c.ck("FC2 runtime checkout/describe are observed without minting a build SHA",
        eqs(&observed["runtime_source_checkout"], &"e".repeat(40)) && eqs(&observed["runtime_source_describe"], "v0.9.0")
        && has(&observed["runtime_source_checkout_source"], "컴파일 바이트")
        && crate::value::dumps_tight_sorted(&ev["build_identity"]) == before && ev["build_identity"]["vllm_sha"].is_null());
    c.ck("F3 generic runtime version is preserved separately, never promoted to distribution metadata",
        observed["runtime_version"] == observation["vllm_version"] && observed["wheel_meta"].is_null()
        && has(&observed["runtime_version_source"], runtime_rel) && has(&observed["runtime_version_source"], index_rel));
    let explicit = rep(&observation, &[("distribution_metadata_version", J::from("0.9.0+cu130")),
        ("distribution_metadata_version_source", J::from("importlib.metadata.distribution(\"vllm\").version"))]);
    wj(&repo.join(runtime_rel), &explicit)?;
    let got = vo(&ev)?;
    c.ck("explicit bound distribution observation remains separate from module version",
        eqs(&got["wheel_meta"], "0.9.0+cu130") && got["runtime_version"] == observation["vllm_version"] && has(&got["wheel_meta_source"], "수집 성공은 별도 검증"));
    wj(&repo.join(runtime_rel), &rep(&explicit, &[("distribution_metadata_version_source", J::from("another collector"))]))?;
    let declared = vo(&ev)?;
    c.ck("typed explicit distribution source is reported as an unverified declaration",
        declared["wheel_meta"] == explicit["distribution_metadata_version"] && has(&declared["wheel_meta_source"], "검증되지 않음"));
    wj(&repo.join(runtime_rel), &rep(&explicit, &[("distribution_metadata_version_source", J::from(""))]))?;
    let missing_source = vo(&ev)?;
    c.ck("distribution value without source is reported as unverified, not absent",
        missing_source["wheel_meta"].is_null() && has(&missing_source["wheel_meta_source"], "명시적 출처가 없어 미검증"));
    wj(&repo.join(runtime_rel), &explicit)?;
    let divergence = vo(&rep(&ev, &[("attestation", jd! {"nodes" => jd! {"main" => jd! {"vllm_dist_version" => "0.9.0"}, "sub" => jd! {"vllm_dist_version" => "0.9.1"}}})]))?;
    c.ck("divergent node attestation is not masked by bound runtime distribution", divergence["wheel_meta"].is_null() && has(&divergence["wheel_meta_source"], "상이"));
    wj(&repo.join(runtime_rel), &observation)?;
    c.ck("F3 missing collection field describes this record only, not other execution ledgers",
        has(&observed["runtime_version_source"], "이 runtime 기록에 collection_argv 필드 없음") && has(&observed["runtime_version_source"], "다른 실행 원장 미조회"));
    let ro = _runtime_observations(repo, &ev)?;
    c.ck("FC2/3 both declaration and observation files are retained for lineage",
        ro.first().map_or(false, |x| x["files"] == jl![jl![index_rel, "raw_json"], jl![runtime_rel, "raw_json"]]));
    for (name, argv) in [("module attribute", jl!["python3", "-c", "import vllm; print(vllm.__version__)"]),
        ("opaque distribution-looking command", jl!["python3", "-c", "import importlib.metadata as m; print(m.version('vllm'))"])] {
        wj(&repo.join(runtime_rel), &rep(&observation, &[("collection_argv", argv)]))?;
        let recorded = rdb(&repo.join(runtime_rel))?;
        let got = vo(&ev)?;
        let rs = &got["runtime_version_source"];
        c.ck(&format!("F3 {name}: preserve command pointer without inferring its output provenance"),
            got["runtime_version"] == observation["vllm_version"] && got["wheel_meta"].is_null()
            && has(rs, &format!("{runtime_rel}#/collection_argv")) && has(rs, "수집 명령 기록")
            && has(rs, "실행 성공·진위·값 출처는 이 필드만으로 미검증") && !has(rs, "collection_argv 필드 없음")
            && rdb(&repo.join(runtime_rel))? == recorded);
    }
    wj(&repo.join(runtime_rel), &rep(&observation, &[("collection_argv", J::from("python3 -c ..."))]))?;
    let malformed = vo(&ev)?;
    c.ck("F3 malformed argv still has a source pointer, without claiming a verified argv record",
        has(&malformed["runtime_version_source"], &format!("{runtime_rel}#/collection_argv"))
        && has(&malformed["runtime_version_source"], "argv 형식 미확인") && malformed["wheel_meta"].is_null());
    wj(&repo.join(runtime_rel), &observation)?;
    let distribution = "0.9.0+distribution";
    for (origin, with_d) in [("attestation", rep(&ev, &[("attestation", jd! {"nodes" => jd! {"sub" => jd! {"vllm_dist_version" => distribution}}})])),
        ("build ledger", rep(&ev, &[("image_ledger", jd! {"vllm" => jd! {"dist_version" => distribution}, "source" => "fixture build ledger"})]))] {
        let got = vo(&with_d)?;
        c.ck(&format!("F3 genuine {origin} metadata remains distinct from a different generic runtime value"),
            eqs(&got["wheel_meta"], distribution) && got["runtime_version"] == observation["vllm_version"] && !has(&got["wheel_meta_source"], runtime_rel));
    }
    let freeze_rel = "docs/simlog/epoch/pip-freeze-sub.txt";
    wr0(&repo.join(freeze_rel), &format!("vllm=={distribution}\n"))?;
    let native = vo(&rep(&ev, &[("plane", J::from("native")), ("pip_freeze_paths", jd! {"sub" => freeze_rel})]))?;
    c.ck("F3 native pip freeze retains distribution authority alongside a generic runtime record",
        eqs(&native["wheel_meta"], distribution) && has(&native["wheel_meta_source"], "pip freeze") && native["runtime_version"] == observation["vllm_version"]);
    for (key, bad) in [("config", "other"), ("measured_node", "main"), ("sweep_generated_utc", "2026-01-01T03:00:00Z"),
        ("sweep_dir", "output/single/benchlog/sweep_old"), ("installed_runtime_identity", "docs/simlog/../escape.json")] {
        wj(&repo.join(index_rel), &rep(&binding, &[(key, J::from(bad))]))?;
        c.ck(&format!("FC2/3 mismatched {key} is not runtime evidence"), _runtime_observations(repo, &ev)?.is_empty());
    }
    wj(&repo.join(index_rel), &binding)?;
    let outside = repo.join("docs/simlog/epoch/runtime-outside.json");
    wj(&outside, &observation)?;
    wj(&repo.join(index_rel), &rep(&binding, &[("installed_runtime_identity", J::from(_rel(repo, &outside)?))]))?;
    c.ck("FC2/3 runtime file outside the declared attempt is not adopted", _runtime_observations(repo, &ev)?.is_empty());
    wj(&repo.join(index_rel), &binding)?;
    let saved = rdb(&repo.join(runtime_rel))?;
    rm(&repo.join(runtime_rel))?;
    symlink(&outside, &repo.join(runtime_rel))?;
    c.ck("FC2/3 symlink observation is rejected", _runtime_observations(repo, &ev)?.is_empty());
    rm(&repo.join(runtime_rel))?;
    wrb(&repo.join(runtime_rel), &saved)?;
    let second_rel = "docs/simlog/epoch/attempt-other/evidence_index.json";
    let second_runtime = "docs/simlog/epoch/attempt-other/runtime_identity.json";
    wj(&repo.join(second_rel), &rep(&binding, &[("installed_runtime_identity", J::from(second_runtime))]))?;
    wj(&repo.join(second_runtime), &rep(&observation, &[("vllm_version", J::from("0.9.2-conflicting")), ("source_head", J::from("f".repeat(40)))]))?;
    pm(&mut ev, &["pointers"]).push(jd! {"kind" => "simlog", "path" => second_rel, "cell_id" => cell, "node_id" => "sub"});
    let conflict = vo(&ev)?;
    c.ck("FC2/3 contradictory current-runtime records are reported, never silently selected",
        conflict["wheel_meta"].is_null() && conflict["runtime_version"].is_null() && has(&conflict["runtime_version_source"], "상충")
        && conflict["runtime_source_checkout"].is_null() && has(&conflict["runtime_source_checkout_source"], "상충"));
    ev.set("pointers", jl![jd! {"kind" => "simlog", "path" => index_rel, "cell_id" => "other", "node_id" => "sub"}]);
    c.ck("FC2/3 another cell's pointer is not searched", _runtime_observations(repo, &ev)?.is_empty());
    Ok(())
}
