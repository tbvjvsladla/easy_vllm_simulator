//! Rust port of `selftest_branch_sync.py` -- the isolated-fixture run of the branch-sync and sub-delivery mechanism (⑧-pre D2).
//!
//! The checks run the **byte copies of the deployed scripts** in throwaway git repositories (S6 sync_to_sub entry guard and the
//! render_sub_env guard · S1/S3/S4/S7 closing_sequence execute multi-node → single-node · S8 delete respects excludes · the
//! inline_json locator · S9 B0 build-kit checksum scope). Check names and conditions follow the original one for one; the
//! differences are the judge (`layer_ledger.sh` + `rust/runtime/src/layer_ledger.rs`, the node-built ELF of the real runtime
//! root) and the broken-judge injection (a fake runtime root whose `layer_ledger` is this program in fault mode).
//! Other bundles' programs the scripts call (completion gate, campaign_init, render_sub_env, hint.py) run as those scripts call them.
//! Remote, sub node and real ledger are never touched: the remote is a temporary bare repository, ssh is a shim that records calls.
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const UVW: &str = ".claude/skills/upstream-version-watch/scripts";
const TN: &str = ".claude/skills/terraforming_node/scripts";
const SYNC_TO_SUB: &str = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";
const SYNC_BRANCHES: &str = ".claude/skills/upstream-version-watch/scripts/sync_branches.sh";
/// The judge: its launcher and its rule source (the ELF is the real runtime root's, the env below points the launcher at it).
const LAYER_LEDGER_SH: &str = ".claude/skills/upstream-version-watch/scripts/layer_ledger.sh";
const LAYER_LEDGER_RS: &str = ".claude/skills/upstream-version-watch/rust/runtime/src/layer_ledger.rs";
const INLINE_JSON_SH: &str = ".claude/skills/upstream-version-watch/scripts/inline_json.sh";
const CLOSING: &str = ".claude/skills/upstream-version-watch/scripts/closing_sequence.sh";
const PARITY: &str = ".claude/policies/runtime/topology_parity.py";
const PARITY_SH: &str = ".claude/policies/runtime/topology_parity.sh";
const CAMPAIGN_INIT: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";
/// The renderer: its launcher (the ELF is the real tree's terraforming_node/rust, the env below points the launcher at it)
/// and its Rust source (the guard's anchor; a compiled binary cannot be mutated in place).
const RENDER: &str = ".claude/skills/terraforming_node/scripts/render_sub_env.sh";
const RENDER_RS: &str = ".claude/skills/terraforming_node/rust/src/render_sub_env.rs";
const TERRAFORMING_BIN_REL: &str = ".claude/skills/terraforming_node/rust/target/release";
const GATE: &str = ".claude/policies/runtime/completion_gate.py";
const CATALOG: &str = ".claude/skills/hint-publisher/scripts/hint.py";
const HINTLIB_DIR: &str = ".claude/skills/hint-publisher/scripts/hintlib";
const CATALOG_COMPANIONS: [&str; 2] = [".claude/skills/hint-publisher/scripts/render_bench_section.py", "hints/vocab.json"];
const SCHEMAS: [&str; 3] = ["work-manifest.schema.json", "completion-manifest.schema.json", "side-effect-authorization.schema.json"];
const LEDGER_REL: &str = ".claude/policies/branch_layer_ledger.json";
const TOPOLOGY_RULES: &str = ".claude/rules/strategy.topology.md";
const CERT_REL: &str = "docs/benchmark/benchmark_26091400_fixture_gb10_v1.yaml";
const GUARD_CALL: &str = "assert_delivery_topology || exit \"$TOPOLOGY_GUARD_EXIT\"\n";
const PREPULL_JUDGE: &str = "    .claude/skills/upstream-version-watch/scripts/layer_ledger.sh \\\n    .claude/skills/upstream-version-watch/rust/runtime/src/layer_ledger.rs\n";
const RUNTIME_REL: &str = ".claude/skills/upstream-version-watch/rust/runtime";
const ENTRY_UTC: &str = "2026-09-14T00:00:00Z";
const APPROVED_AT: &str = "2026-09-14T00:01:00Z";
const HINT_TAG: &str = "hint/9.9.9/fixture-model/gb10-main-x/qfp8";
/// Fault mode: set (to the real `layer_ledger` ELF) only in the broken-judge case, on the fake runtime root's copy of this program.
pub const FAULT_ENV: &str = "SELFTEST_BRANCH_SYNC_FAULT_JUDGE";
const FAULT_TEXT: &str = "fixture: 판정기 고장 주입";

thread_local! {
    static FAILURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}

fn ck(name: &str, cond: bool, got: &str) {
    eprintln!("  {} {name}", if cond { "ok  " } else { "FAIL" });
    if !cond {
        FAILURES.with(|f| f.borrow_mut().push(trunc(&format!("{name} (got={got:?})"), 1500)));
    }
}

type R<T> = Result<T, String>;

struct Out {
    rc: i32,
    stdout: String,
    stderr: String,
}
impl Out {
    fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Temporary root · isolated git config · ssh shim. The real repository's config (hooks, signing) does not leak in.
struct Sandbox {
    root: PathBuf,
    tmp: PathBuf,
    ssh_marker: PathBuf,
    env: Vec<(String, String)>,
    real: PathBuf,
}

impl Sandbox {
    fn new(real: &Path, prefix: &str) -> R<Sandbox> {
        let root = make_tempdir(prefix)?;
        let tmp = root.join("tmp");
        mkdirs(&tmp)?;
        let gcfg = root.join("gitconfig");
        write(&gcfg, "[user]\n\tname = selftest\n\temail = t@t\n[commit]\n\tgpgsign = false\n[tag]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n")?;
        let ssh_marker = root.join("ssh_called");
        let shim = root.join("ssh_shim.sh");
        write(&shim, &format!("#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 255\n", ssh_marker.display()))?;
        chmod(&shim, 0o755)?;
        const DROP: [&str; 10] = ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_PREFIX", "GITHUB_TOKEN", "SRC", "SUB_HOST"];
        let mut env: Vec<(String, String)> = std::env::vars().filter(|(k, _)| !DROP.contains(&k.as_str()) && k != "SUB_WORK_DIR" && k != FAULT_ENV).collect();
        let set = |env: &mut Vec<(String, String)>, k: &str, v: String| {
            env.retain(|(a, _)| a != k);
            env.push((k.to_string(), v));
        };
        set(&mut env, "GIT_CONFIG_GLOBAL", gcfg.display().to_string());
        set(&mut env, "GIT_CONFIG_NOSYSTEM", "1".into());
        set(&mut env, "GIT_TERMINAL_PROMPT", "0".into());
        set(&mut env, "TMPDIR", tmp.display().to_string());
        set(&mut env, "SYNC_SSH_COMMAND", shim.display().to_string());
        // other bundles' Python the scripts still call must not leave __pycache__ in the copies ("side effect 0")
        set(&mut env, "PYTHONDONTWRITEBYTECODE", "1".into());
        // the copy trees hold no ELF: the launchers use the real runtime root's binaries
        set(&mut env, "EASY_VLLM_UPSTREAM_RUNTIME_ROOT", real.join(RUNTIME_REL).display().to_string());
        set(&mut env, "TERRAFORMING_BIN_DIR", real.join(TERRAFORMING_BIN_REL).display().to_string());
        set(&mut env, "TOPOLOGY_PARITY_BIN", real.join(".claude/skills/adversarial-benchmark/rust/target/release/topology_parity").display().to_string());
        Ok(Sandbox { root, tmp, ssh_marker, env, real: real.to_path_buf() })
    }

    fn git(&self, cwd: &Path, args: &[&str]) -> R<String> {
        let o = Command::new("git").arg("-C").arg(cwd).args(args).env_clear().envs(self.env.iter().cloned()).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
        if !o.status.success() {
            let name = cwd.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(format!("fixture git {args:?} @ {name} failed: {}", trunc(String::from_utf8_lossy(&o.stderr).trim(), 400)));
        }
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    fn git_ok(&self, cwd: &Path, args: &[&str]) -> bool {
        Command::new("git").arg("-C").arg(cwd).args(args).env_clear().envs(self.env.iter().cloned()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    fn run(&self, argv: &[String], cwd: &Path, extra: &[(&str, String)]) -> Out {
        let mut c = Command::new(&argv[0]);
        c.args(&argv[1..]).current_dir(cwd).env_clear().envs(self.env.iter().cloned()).stdin(Stdio::null());
        for (k, v) in extra {
            c.env(k, v);
        }
        match c.output() {
            Ok(o) => Out { rc: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).into_owned(), stderr: String::from_utf8_lossy(&o.stderr).into_owned() },
            Err(e) => Out { rc: -1, stdout: String::new(), stderr: format!("spawn {}: {e}", argv[0]) },
        }
    }

    fn copy_real(&self, dst_root: &Path, rel: &str) -> R<()> {
        let dst = dst_root.join(rel);
        mkdirs(dst.parent().unwrap())?;
        copy_file(&self.real.join(rel), &dst)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// ── small filesystem helpers ─────────────────────────────────────────────────────────────────────────

fn make_tempdir(prefix: &str) -> R<PathBuf> {
    let base = std::env::var_os("TMPDIR").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    for i in 0..1000u32 {
        let p = base.join(format!("{prefix}{}_{n}_{i}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return std::fs::canonicalize(&p).map_err(|e| e.to_string());
        }
    }
    Err("tempdir: no free name".into())
}
fn mkdirs(p: &Path) -> R<()> {
    std::fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {e}", p.display()))
}
fn write(p: &Path, text: &str) -> R<()> {
    write_bytes(p, text.as_bytes())
}
fn write_bytes(p: &Path, data: &[u8]) -> R<()> {
    if let Some(d) = p.parent() {
        mkdirs(d)?;
    }
    std::fs::write(p, data).map_err(|e| format!("write {}: {e}", p.display()))
}
fn write_rel(root: &Path, rel: &str, text: &str) -> R<()> {
    write(&root.join(rel), text)
}
fn read(p: &Path) -> R<String> {
    std::fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))
}
fn chmod(p: &Path, mode: u32) -> R<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())
}
/// `shutil.copy2`: bytes + mode (+ mtime is irrelevant to every check here).
fn copy_file(src: &Path, dst: &Path) -> R<()> {
    std::fs::copy(src, dst).map(|_| ()).map_err(|e| format!("copy {} → {}: {e}", src.display(), dst.display()))
}
fn walk_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let ft = match e.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() {
                if p.strip_prefix(root).map(|r| r.components().any(|c| c.as_os_str() == ".git")).unwrap_or(false) {
                    continue;
                }
                walk_files(root, &p, out);
            } else if ft.is_file() || (ft.is_symlink() && p.is_file()) {
                out.push(p);
            }
        }
    }
}
/// Side-effect-0 fingerprint: every file outside `.git` (path · bytes).
fn tree_snapshot(root: &Path) -> String {
    let mut files = Vec::new();
    walk_files(root, root, &mut files);
    files.retain(|p| !p.strip_prefix(root).map(|r| r.components().any(|c| c.as_os_str() == ".git")).unwrap_or(true));
    files.sort();
    let mut buf: Vec<u8> = Vec::new();
    for p in files {
        buf.extend_from_slice(p.strip_prefix(root).unwrap().to_string_lossy().as_bytes());
        buf.extend_from_slice(&std::fs::read(&p).unwrap_or_default());
    }
    crate::wheelhouse_util::sha256_hex(&buf)
}
fn dir_names(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}
fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}
fn p2s(p: &Path) -> String {
    p.display().to_string()
}

// ── JSON helpers (fixture writers keep Python's json.dumps shape; readers use the shared Python-compatible loader) ──

fn jq(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            '\n' => o += "\\n",
            '\r' => o += "\\r",
            '\t' => o += "\\t",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn loads(text: &str) -> Option<PyValue> {
    pyjson::loads(text).ok()
}
fn get<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(key),
        _ => None,
    }
}
fn text(v: Option<&PyValue>) -> Option<String> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => s.to_utf8(),
        _ => None,
    }
}
fn list(v: Option<&PyValue>) -> Vec<PyValue> {
    match v.map(|x| x.peel()) {
        Some(PyValue::List(l)) => l.clone(),
        _ => Vec::new(),
    }
}

/// One classification row / ledger entry, as the original's dicts (key order = the original's).
fn ledger_entry(id: &str, utc: &str, commit: &str, approved_by: &str, approved_utc: &str, files: &[&str], reason: &str) -> String {
    let rows: Vec<String> = files
        .iter()
        .map(|f| format!("{{\"file\": {}, \"verdict\": \"common_promote\", \"reason\": {}, \"provenance\": \"agent-judged\"}}", jq(f), jq(reason)))
        .collect();
    format!(
        "{{\"entry_id\": {}, \"utc\": {}, \"departure\": \"multi-node\", \"departure_commit\": {}, \"approved_by\": {}, \"approved_utc\": {}, \"classification\": [{}]}}",
        jq(id),
        jq(utc),
        jq(commit),
        jq(approved_by),
        jq(approved_utc),
        rows.join(", ")
    )
}

fn approval_fixture(repo: &Path) -> R<PathBuf> {
    let atoms = ["approved_by: selftest".to_string(), format!("approved_at_utc: {APPROVED_AT}"), "allowed_action: sync_branches".to_string()];
    let plan = repo.join("docs/plan/p_selftest.md");
    let plan_text = format!("# selftest plan\n\n## Execution approval\n\n{}\n", atoms.join("\n"));
    write(&plan, &plan_text)?;
    let manifest = repo.join("docs/_evidence/wm_selftest.json");
    let atoms_json: Vec<String> = atoms.iter().map(|a| jq(a)).collect();
    let doc = format!(
        "{{\"schema_version\": 1, \"task_class\": \"harness_change\", \"identity\": {{\"model\": \"m\", \"gpu\": \"g\", \"vllm\": \"v\", \"quant\": null, \"topology\": \"multi\", \"tp\": 1}}, \"evidence\": {{\"plan\": {{\"path\": \"../plan/p_selftest.md\"}}}}, \"pii_scan\": {{\"passed\": true}}, \"execution_approval\": {{\"approved\": true, \"approved_by\": \"selftest\", \"approved_at_utc\": {}, \"plan_path\": \"../plan/p_selftest.md\", \"plan_sha256\": {}, \"approval_anchor\": \"## Execution approval\", \"approval_atoms\": [{}], \"allowed_actions\": [\"sync_branches\"]}}}}",
        jq(APPROVED_AT),
        jq(&crate::wheelhouse_util::sha256_hex(plan_text.as_bytes())),
        atoms_json.join(", ")
    );
    write(&manifest, &doc)?;
    Ok(manifest)
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// S6 -- sync_to_sub.sh entry guard
// ════════════════════════════════════════════════════════════════════════════════════════════════

fn checkout(sb: &Sandbox, name: &str, branch: &str, header: &str, topo: &str, manifest_topo: Option<&str>, campaign_topo: Option<&str>) -> R<PathBuf> {
    let repo = sb.root.join(name);
    mkdirs(&repo)?;
    sb.git(&repo, &["init", "-q", "-b", branch])?;
    let mut rels: Vec<String> = sv(&[SYNC_TO_SUB, INLINE_JSON_SH, PARITY, PARITY_SH, GATE, CAMPAIGN_INIT]);
    rels.extend(SCHEMAS.iter().map(|s| format!(".claude/schemas/{s}")));
    for rel in &rels {
        sb.copy_real(&repo, rel)?;
    }
    write_rel(&repo, TOPOLOGY_RULES, &format!("# 특화헌법 픽스처\n\n**topology: {header}** · layer: topology\n"))?;
    // a single-node checkout also tracks an old output/multi build kit (the downgrade-delivery shape) -- the guard must stop before it
    write_rel(&repo, "output/multi/Dockerfile", "# fixture: 낡은 버전 핀을 든 빌드킷\n")?;
    sb.git(&repo, &["add", "-A"])?;
    sb.git(&repo, &["commit", "-qm", "fixture checkout"])?;
    // the manifest is an untracked product (the manifest leg of the 4-way match) -- on the filesystem only, as in the real thing
    write_rel(&repo, &format!("output/{topo}/manifest.yaml"), &format!("topology: {}\n", manifest_topo.unwrap_or(topo)))?;
    if let Some(ct) = campaign_topo {
        write_rel(&repo, "campaigns/ACTIVE", "camp-fixture\n")?;
        write_rel(&repo, "campaigns/camp-fixture/campaign.yaml", &format!("{{\"control_variables\": {{\"topology\": {}}}, \"nodes\": []}}\n", jq(ct)))?;
    }
    Ok(repo)
}

fn sync_to_sub(sb: &Sandbox, repo: &Path, args: &[&str], script: Option<&Path>) -> Out {
    let script = script.map(Path::to_path_buf).unwrap_or_else(|| repo.join(SYNC_TO_SUB));
    let mut argv = vec!["bash".to_string(), p2s(&script), "--mode".into(), "experimental".into()];
    argv.extend(args.iter().map(|s| s.to_string()));
    sb.run(&argv, &sb.root, &[])
}

fn test_sync_to_sub_guard(real: &Path) -> R<()> {
    eprintln!("[S6] sync_to_sub.sh 배달 통로 ↔ 체크아웃 특화층 가드");
    let sb = Sandbox::new(real, "sbs_s6_")?;
    let multi = checkout(&sb, "multi", "multi-node", "multi", "multi", None, None)?;
    let single = checkout(&sb, "single", "single-node", "single", "single", None, None)?;
    let no_side_effect = |label: &str, repo: &Path, before: &str| {
        let leaked = dir_names(&sb.tmp);
        let prov = std::fs::read_dir(repo.join("output")).map(|rd| rd.flatten().any(|e| e.path().join("sub_provision").exists())).unwrap_or(false);
        ck(
            &format!("{label}: 부작용 0 (트리 불변 · 트랜잭션 소스 0 · ssh 0 · 스테이징 0)"),
            tree_snapshot(repo) == before && leaked.is_empty() && !sb.ssh_marker.exists() && !prov,
            &format!("({leaked:?}, {})", sb.ssh_marker.exists()),
        );
    };

    let before = tree_snapshot(&multi);
    let r = sync_to_sub(&sb, &multi, &["--branch", "single"], None);
    ck(
        "★S6 음성①: 멀티 체크아웃 + --branch single → exit 12 · TOPOLOGY_DELIVERY_MISMATCH · EXPECT_MISMATCH",
        r.rc == 12 && r.stderr.contains("TOPOLOGY_DELIVERY_MISMATCH") && r.stderr.contains("EXPECT_MISMATCH"),
        &format!("({}, {})", r.rc, tail(&r.stderr, 600)),
    );
    ck("★S6 음성①: 인가 게이트에 닿기 전이다(게이트 JSON 없음)", !r.stdout.contains("authorization_state"), &tail(&r.stdout, 300));
    no_side_effect("S6 음성①", &multi, &before);

    let before = tree_snapshot(&single);
    let r = sync_to_sub(&sb, &single, &[], None);
    ck("★S6 음성②: 싱글 체크아웃 + 기본값(--branch multi) → exit 12 (옛 빌드킷 다운그레이드 배달 차단)", r.rc == 12 && r.stderr.contains("TOPOLOGY_DELIVERY_MISMATCH"), &format!("({}, {})", r.rc, tail(&r.stderr, 600)));
    ck(
        "★S6 음성② 안내: 이 체크아웃의 통로(--branch single)와 반대 체크아웃 두 선택지를 함께 말한다",
        r.stderr.contains("같은 명령에 --branch single") && r.stderr.contains("'multi' 토폴로지 브랜치를 체크아웃한 저장소"),
        &tail(&r.stderr, 600),
    );
    no_side_effect("S6 음성②", &single, &before);

    let man_bad = checkout(&sb, "man-bad", "multi-node", "multi", "multi", Some("single"), None)?;
    let before = tree_snapshot(&man_bad);
    let r = sync_to_sub(&sb, &man_bad, &["--branch", "multi"], None);
    ck(
        "★S6 음성④: --branch 는 맞고 manifest 가 어긋남 → exit 12 · MANIFEST_MISMATCH · 체크아웃 자체 RED 로 안내(--branch 교체 제안 ✗)",
        r.rc == 12 && r.stderr.contains("MANIFEST_MISMATCH") && r.stderr.contains("자체가 4자일치를 통과하지 못한다") && !r.stderr.contains("같은 명령에 --branch"),
        &format!("({}, {})", r.rc, tail(&r.stderr, 700)),
    );
    no_side_effect("S6 음성④", &man_bad, &before);
    let camp_bad = checkout(&sb, "camp-bad", "single-node", "single", "single", None, Some("multi"))?;
    let before = tree_snapshot(&camp_bad);
    let r = sync_to_sub(&sb, &camp_bad, &["--branch", "single"], None);
    ck(
        "★S6 음성⑤: 싱글 체크아웃 + 활성 멀티 캠페인 + --branch single → exit 12 · CAMPAIGN_MISMATCH 를 드러낸다",
        r.rc == 12 && r.stderr.contains("CAMPAIGN_MISMATCH") && r.stderr.contains("활성 캠페인") && !r.stderr.contains("같은 명령에 --branch"),
        &format!("({}, {})", r.rc, tail(&r.stderr, 700)),
    );
    no_side_effect("S6 음성⑤", &camp_bad, &before);
    let camp_ok = checkout(&sb, "camp-ok", "multi-node", "multi", "multi", None, Some("multi"))?;
    let r = sync_to_sub(&sb, &camp_ok, &["--branch", "multi"], None);
    ck(
        "★S6 양성(네 다리): 활성 캠페인까지 일치 → 가드 통과 · 판정 다리 수를 밝힌다(4/4 · C5 공개)",
        r.rc != 0 && r.rc != 12 && r.stderr.contains("배달 통로 대조 PASS") && r.stderr.contains("(4/4)") && r.stderr.contains("campaign"),
        &format!("({}, {})", r.rc, tail(&r.stderr, 500)),
    );

    let r = sync_to_sub(&sb, &multi, &["--branch", "both"], None);
    ck("★S6 both 거부 비회귀: exit 12 · TOPOLOGY_DELIVERY_BOTH(한 체크아웃은 특화층 하나)", r.rc == 12 && r.stderr.contains("TOPOLOGY_DELIVERY_BOTH"), &format!("({}, {})", r.rc, tail(&r.stderr, 400)));
    let r = sync_to_sub(&sb, &multi, &["--branch", "bogus"], None);
    ck("--branch 값역 밖은 종전대로 exit 6(파싱 단계)", r.rc == 6, &r.rc.to_string());

    for (repo, branch) in [(&multi, "multi"), (&single, "single")] {
        let before = tree_snapshot(repo);
        let r = sync_to_sub(&sb, repo, &["--branch", branch], None);
        let gate = loads(&r.stdout);
        let allowed_false = gate.as_ref().is_some_and(|g| matches!(g.peel(), PyValue::Dict(_)) && matches!(get(g, "allowed").map(|v| v.peel()), Some(PyValue::Bool(false))));
        let name = repo.file_name().unwrap().to_string_lossy().into_owned();
        ck(
            &format!("★S6 양성: {name} 체크아웃 + --branch {branch} → 가드 통과 · 다음 문(인가 게이트)이 manifest 부재로 거부"),
            r.rc != 0 && r.rc != 12 && !r.stderr.contains("STOP(TOPOLOGY") && allowed_false,
            &format!("({}, {}, {})", r.rc, tail(&r.stdout, 300), tail(&r.stderr, 300)),
        );
        ck(&format!("S6 양성({branch}): 통과 줄이 판정 다리를 밝힌다(캠페인 없는 체크아웃 = 3/4)"), r.stderr.contains("배달 통로 대조 PASS") && r.stderr.contains("(3/4)"), &tail(&r.stderr, 300));
        no_side_effect(&format!("S6 양성({branch})"), repo, &before);
    }

    let drift = checkout(&sb, "drift", "multi-node", "single", "multi", None, None)?;
    let r = sync_to_sub(&sb, &drift, &["--branch", "multi"], None);
    ck("★S6 음성③: 헤더 드리프트(브랜치 multi-node · 자기선언 single) → exit 12 · LAYER_HEADER_MISMATCH", r.rc == 12 && r.stderr.contains("LAYER_HEADER_MISMATCH"), &format!("({}, {})", r.rc, tail(&r.stderr, 400)));

    // mutation control: a copy without the guard call does not stop negative ① -- this check catches that regression
    let src = read(&multi.join(SYNC_TO_SUB))?;
    let n = src.matches(GUARD_CALL).count();
    ck("변이 앵커(가드 호출 줄) 정확히 1건", n == 1, &n.to_string());
    let mutated = multi.join(UVW).join("sync_to_sub_mutated.sh");
    write(&mutated, &src.replace(GUARD_CALL, ""))?;
    let r = sync_to_sub(&sb, &multi, &["--branch", "single"], Some(&mutated));
    ck("★S6 변이 음성대조: 가드 호출이 없으면 멀티 체크아웃 + single 이 인가 단계까지 간다(결함 재현)", r.rc != 12 && !r.stderr.contains("TOPOLOGY_DELIVERY"), &format!("({}, {})", r.rc, tail(&r.stderr, 300)));
    let _ = std::fs::remove_file(&mutated);
    Ok(())
}

fn test_render_guard(real: &Path) -> R<()> {
    eprintln!("[S6] render_sub_env 렌더 전 특화헌법 대조");
    let sb = Sandbox::new(real, "sbs_render_")?;
    let repo = sb.root.join("render");
    mkdirs(&repo.join(TN))?;
    sb.copy_real(&repo, RENDER)?;
    // the 4-way predicate's canonical owner is the base-layer runtime (plan_26093022) -- the renderer imports it skill → base layer
    sb.copy_real(&repo, PARITY)?;
    sb.copy_real(&repo, PARITY_SH)?;
    write_rel(&repo, TOPOLOGY_RULES, "# 특화헌법 픽스처\n\n**topology: multi** · layer: topology\n")?;
    let absent_manifest = p2s(&sb.root.join("no-such-manifest.yaml"));
    // render_sub_env belongs to the terraforming_node bundle: run as sync_to_sub.sh runs it (its launcher, the real tree's ELF)
    let render = |topo: &str| sb.run(&sv(&["bash", &p2s(&repo.join(RENDER)), "--topology", topo, "--manifest", &absent_manifest]), &sb.root, &[]);
    let before = tree_snapshot(&repo);
    let r = render("single");
    ck(
        "★S6 렌더 음성: 자기선언 multi 트리에서 --topology single → exit 5 · TOPOLOGY_RULES_MISMATCH · 스테이징 0",
        r.rc == 5 && r.stderr.contains("TOPOLOGY_RULES_MISMATCH") && tree_snapshot(&repo) == before,
        &format!("({}, {})", r.rc, tail(&r.stderr, 400)),
    );
    let r = render("multi");
    ck("★S6 렌더 양성: 일치하면 가드를 지나 다음 검사(manifest 부재 exit 3)에 닿는다", r.rc == 3 && !r.stderr.contains("TOPOLOGY_RULES_MISMATCH"), &format!("({}, {})", r.rc, tail(&r.stderr, 300)));
    // The renderer is a compiled program now: the mutation control is the guard's single call site in its source, and the
    //   positive case above (same tree and input, matching --topology → exit 3) shows what the program does without the stop.
    let src = read(&real.join(RENDER_RS))?;
    let anchor = "    let mut mismatch = topology_rules_mismatch(ctx, &topology, None);\n";
    let n = src.matches(anchor).count();
    ck("렌더 대조 호출 앵커 1건(Rust 소스 · run 의 첫 판정)", n == 1, &n.to_string());
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// S1 · S3 · S4 · S7 -- closing_sequence execute (multi-node → single-node)
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// `ROOT_RELOCATION_REPLACEMENTS=( … )` of the real sync_branches.sh (comments and blanks dropped).
fn relocation_replacements(real: &Path) -> R<Vec<String>> {
    let src = read(&real.join(SYNC_BRANCHES))?;
    let mut out = Vec::new();
    let mut inside = false;
    for line in src.lines() {
        if !inside {
            if let Some(rest) = line.strip_prefix("ROOT_RELOCATION_REPLACEMENTS=(") {
                inside = true;
                let v = rest.split('#').next().unwrap_or("").trim();
                if !v.is_empty() {
                    out.push(v.to_string());
                }
            }
            continue;
        }
        if line.starts_with(')') {
            break;
        }
        let v = line.split('#').next().unwrap_or("").trim();
        if !v.is_empty() {
            out.push(v.to_string());
        }
    }
    Ok(out)
}

struct Fixture {
    repo: PathBuf,
    manifest: PathBuf,
    head: String,
    e1: String,
}

#[derive(Default, Clone, Copy)]
struct FxOpts {
    target_only_change: bool,
    carry_target_change: bool,
    forged_target_ledger: bool,
}

/// Departure multi-node · target single-node. After one past sync (ledger E1 + single landing) multi changed the common layer again.
fn build_sequence_fixture(sb: &Sandbox, o: FxOpts) -> R<Fixture> {
    let repo = sb.root.join("repo");
    mkdirs(&repo)?;
    sb.git(&repo, &["init", "-q", "-b", "multi-node"])?;
    let reps = relocation_replacements(&sb.real)?;
    if reps.len() < 10 {
        return Err(format!("ROOT_RELOCATION_REPLACEMENTS 를 읽지 못했다: {reps:?}"));
    }
    let mut hintlib: Vec<String> = std::fs::read_dir(sb.real.join(HINTLIB_DIR))
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.ends_with(".py").then(|| format!("{HINTLIB_DIR}/{n}"))
        })
        .collect();
    hintlib.sort();
    if !hintlib.iter().any(|r| r.ends_with("/catalog.py")) {
        return Err(format!("hintlib 사본 목록에 catalog.py 가 없다: {hintlib:?}"));
    }
    let mut rels: std::collections::BTreeSet<String> = sv(&[LAYER_LEDGER_SH, LAYER_LEDGER_RS, INLINE_JSON_SH, SYNC_BRANCHES, CLOSING, PARITY, PARITY_SH, GATE, CATALOG, ".gitignore"]).into_iter().collect();
    rels.extend(SCHEMAS.iter().map(|s| format!(".claude/schemas/{s}")));
    rels.extend(reps);
    rels.extend(hintlib);
    rels.extend(CATALOG_COMPANIONS.iter().map(|s| s.to_string()));
    for rel in &rels {
        sb.copy_real(&repo, rel)?;
    }
    write_rel(&repo, "CLAUDE.md", "# 픽스처 헌법 v1\n")?;
    write_rel(&repo, ".claude/rules/common.md", "공통층 v1\n")?;
    write_rel(&repo, "docs/plan/example.md", "# plan 스켈레톤 v1\n")?;
    write_rel(&repo, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: multi** · layer: topology\n")?;
    // both real branches track hints/ files (contract · vocab) -- without them ⑤ has no place for the central-authority marker
    write_rel(&repo, "hints/FIXTURE_CONTRACT.md", "# hint 계약서 픽스처\n")?;
    sb.git(&repo, &["add", "-A"])?;
    sb.git(&repo, &["commit", "-qm", "A"])?;
    sb.git(&repo, &["branch", "single-node"])?;

    write_rel(&repo, "CLAUDE.md", "# 픽스처 헌법 v2\n")?;
    sb.git(&repo, &["commit", "-qam", "B 공통층"])?;
    let b = sb.git(&repo, &["rev-parse", "HEAD"])?;
    let e1 = ledger_entry("sync-1", "2026-09-12T06:16:53Z", &b, "selftest", "2026-09-12T06:20:00Z", &["CLAUDE.md"], "픽스처");
    write_rel(&repo, LEDGER_REL, &format!("{{\"schema_version\": 1, \"entries\": [{e1}]}}\n"))?;
    sb.git(&repo, &["add", "-A"])?;
    sb.git(&repo, &["commit", "-qm", "ledger(sync-1)"])?;

    // past landing: in a single-node worktree the B tree lands on the common layer (topology layer stays single · ledger not moved)
    let wt = sb.root.join("prior-landing");
    sb.git(&repo, &["worktree", "add", "-q", &p2s(&wt), "single-node"])?;
    sb.git(&wt, &["checkout", &b, "--", "."])?;
    write_rel(&wt, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: single** · layer: topology\n")?;
    sb.git(&wt, &["add", "-A"])?;
    sb.git(&wt, &["commit", "-qm", "sync: 공통층 multi-node→single-node (과거 회차)"])?;
    if o.target_only_change {
        write_rel(&wt, ".claude/rules/common.md", "single 에서만 고친 공통층\n")?;
        sb.git(&wt, &["commit", "-qam", "single 전용 공통층 변경"])?;
    }
    if o.forged_target_ledger {
        let forged = ledger_entry("sync-1", "2026-09-12T06:16:53Z", &b, "someone-else", "2026-09-12T06:20:00Z", &["CLAUDE.md"], "픽스처");
        write_rel(&wt, LEDGER_REL, &format!("{{\"schema_version\": 1, \"entries\": [{forged}]}}\n"))?;
        sb.git(&wt, &["add", "-A"])?;
        sb.git(&wt, &["commit", "-qm", "대상 원장(같은 id · 다른 내용)"])?;
    }
    sb.git(&repo, &["worktree", "remove", "--force", &p2s(&wt)])?;

    // this round moves: common layer · docs skeleton · the judge itself (the target's judge becomes the old version)
    write_rel(&repo, "CLAUDE.md", "# 픽스처 헌법 v3\n")?;
    write_rel(&repo, "docs/plan/example.md", "# plan 스켈레톤 v2\n")?;
    // the certificate (copy-only arm · overwrites the target's same path without a conflict check) -- must show in the blank form
    write_rel(&repo, CERT_REL, "fixture: 재현성 인증서\n")?;
    sb.git(&repo, &["add", "-f", CERT_REL])?;
    let judge = read(&repo.join(LAYER_LEDGER_RS))?;
    write(&repo.join(LAYER_LEDGER_RS), &format!("{judge}\n// fixture: 판정기 개정 회차(대상 브랜치의 판정기는 옛 판본이다)\n"))?;
    sb.git(&repo, &["commit", "-qam", "C 공통층·스켈레톤·판정기"])?;
    if o.carry_target_change {
        // the prescription verbatim ("move that commit to the departure branch first"): the target's bytes after another departure commit
        write_rel(&repo, ".claude/rules/common.md", "single 에서만 고친 공통층\n")?;
        sb.git(&repo, &["commit", "-qam", "cherry-pick: single 전용 공통층 변경을 출발 브랜치로 옮김"])?;
    }
    let head = sb.git(&repo, &["rev-parse", "HEAD"])?;

    // remote: a bare repository holding one hint tag that is not local (the truth source of the catalog derivation)
    let remote = sb.root.join("remote.git");
    sb.git(&sb.root, &["init", "-q", "--bare", &p2s(&remote)])?;
    let other = sb.root.join("other-pc");
    sb.git(&sb.root, &["clone", "-q", &p2s(&remote), &p2s(&other)])?;
    write_rel(&other, "payload.txt", "원격 전용\n")?;
    sb.git(&other, &["add", "-A"])?;
    sb.git(&other, &["commit", "-qm", "remote-only payload"])?;
    sb.git(&other, &["tag", "-a", HINT_TAG, "-m", "원격 전용 brief"])?;
    sb.git(&other, &["push", "-q", "origin", "HEAD:refs/heads/hint", &format!("refs/tags/{HINT_TAG}")])?;
    sb.git(&repo, &["remote", "add", "fx", &p2s(&remote)])?;
    mkdirs(&repo.join("hints"))?;
    write(&repo.join("hints/.central_authority"), "")?;

    // approval evidence (untracked · the gate resolves only under repo-root)
    let manifest = approval_fixture(&repo)?;
    let has_target_ledger = sb.git_ok(&repo, &["cat-file", "-e", &format!("single-node:{LEDGER_REL}")]);
    ck("픽스처 전제: 대상 브랜치에 원장이 없다(S1 의 형태) · 위조 원장 픽스처만 예외", (!has_target_ledger) != o.forged_target_ledger, "");
    let t = sb.git(&repo, &["rev-parse", &format!("single-node:{LAYER_LEDGER_RS}")])?;
    let s = sb.git(&repo, &["rev-parse", &format!("multi-node:{LAYER_LEDGER_RS}")])?;
    ck("픽스처 전제: 대상 브랜치의 판정기가 출발 브랜치와 다르다(옛 판본)", t != s, "");
    Ok(Fixture { repo, manifest, head, e1 })
}

fn entry(fx: &Fixture, approved_by: &str) -> String {
    ledger_entry("sync-2", ENTRY_UTC, &fx.head, approved_by, APPROVED_AT, &["CLAUDE.md", "docs/plan/example.md", LAYER_LEDGER_RS], "픽스처 공통층")
}

fn execute(sb: &Sandbox, fx: &Fixture, entry_json: &str, script: Option<&Path>, extra: &[(&str, String)]) -> R<Out> {
    let entry_file = sb.root.join("entry.json");
    write(&entry_file, entry_json)?;
    let script = script.map(Path::to_path_buf).unwrap_or_else(|| fx.repo.join(CLOSING));
    let argv = sv(&["bash", &p2s(&script), "execute", "--repo", &p2s(&fx.repo), "--entry-file", &p2s(&entry_file), "--mode", "experimental", "--manifest", &p2s(&fx.manifest), "--remote", "fx", "--no-sub", "--no-push"]);
    Ok(sb.run(&argv, &sb.root, extra))
}

fn worktree_count(sb: &Sandbox, repo: &Path) -> usize {
    sb.git(repo, &["worktree", "list", "--porcelain"]).map(|s| s.split("worktree ").count()).unwrap_or(0)
}

/// A fake runtime root: every top-level file of the real `target/release` linked in, except `layer_ledger`, which is this
/// program in fault mode (fails `divergence` with the injected text; passes every other subcommand to the real ELF).
fn fault_runtime_root(sb: &Sandbox) -> R<(PathBuf, String)> {
    let real_rel = sb.real.join(RUNTIME_REL).join("target/release");
    let fake = sb.root.join("fault-runtime");
    let rel = fake.join("target/release");
    mkdirs(&rel)?;
    for e in std::fs::read_dir(&real_rel).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        let n = e.file_name();
        if p.is_file() && n != "layer_ledger" {
            std::os::unix::fs::symlink(&p, rel.join(&n)).map_err(|e| e.to_string())?;
        }
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    copy_file(&me, &rel.join("layer_ledger"))?;
    chmod(&rel.join("layer_ledger"), 0o755)?;
    Ok((fake, p2s(&real_rel.join("layer_ledger"))))
}

fn test_closing_sequence(real: &Path) -> R<()> {
    eprintln!("[S1·S3·S4·S7] closing_sequence execute multi-node → single-node");

    // ── S4 · S3: the blank form comes from the ledger owner's placeholders; submitted as is it is refused before any side effect ──
    {
        let sb = Sandbox::new(real, "sbs_s4_")?;
        let fx = build_sequence_fixture(&sb, FxOpts::default())?;
        let repo = &fx.repo;
        let r = sb.run(&sv(&["bash", &p2s(&repo.join(CLOSING)), "preflight", "--repo", &p2s(repo)]), &sb.root, &[]);
        let marker = "② 분류표 빈칸";
        let skeleton: Option<PyValue> = r.stdout.split_once(marker).and_then(|(_, rest)| rest.split_once('\n')).and_then(|(_, body)| loads(body)).filter(|v| matches!(v.peel(), PyValue::Dict(_)));
        let files: Vec<String> = skeleton.as_ref().map(|s| list(get(s, "classification")).iter().filter_map(|row| text(get(row, "file"))).collect()).unwrap_or_default();
        ck("preflight 가 분류표 빈칸 JSON 을 낸다", r.rc == 0 && skeleton.is_some(), &format!("({}, {}, {})", r.rc, tail(&r.stdout, 500), tail(&r.stderr, 500)));
        let has = |f: &str| files.iter().any(|x| x == f);
        ck("★S3 빈칸이 docs/*/example.md 변경을 보여 준다(종전 pathspec 은 docs/benchmark 만 붙였다)", has("docs/plan/example.md") && has("CLAUDE.md") && !has(TOPOLOGY_RULES), &format!("{files:?}"));
        ck("★S3 빈칸이 충돌 검사 없이 덮는 인증서(복사 전용 arm)를 보여 준다 · report 는 뺀다", has(CERT_REL) && !files.iter().any(|f| f.starts_with("docs/report/")), &format!("{files:?}"));
        // the skeleton JSON as the preflight printed it (the loader's view would re-serialize; the bytes are what execute gets)
        let skeleton_text: String = r.stdout.split_once(marker).and_then(|(_, rest)| rest.split_once('\n')).map(|(_, body)| body.to_string()).filter(|_| skeleton.is_some()).unwrap_or_else(|| "{}".into());
        let sk_file = sb.root.join("skeleton.json");
        write(&sk_file, &skeleton_text)?;
        let v = sb.run(&sv(&["bash", &p2s(&repo.join(LAYER_LEDGER_SH)), "validate", "--entry-file", &p2s(&sk_file)]), &sb.root, &[]);
        let vdoc = loads(&v.stdout);
        let placeholder_n = vdoc.as_ref().map(|d| list(get(d, "problems")).iter().filter_map(|p| text(Some(p))).filter(|p| p.contains("자리표시자")).count()).unwrap_or(0);
        ck("★S4 빈칸 생성기의 자리표시자 = 원장이 거부하는 자리표시자(같은 소유자)", v.rc == 5 && placeholder_n >= 6, &trunc(&v.stdout, 600));

        let head_before = sb.git(repo, &["rev-parse", "multi-node"])?;
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let ledger_before = std::fs::read(repo.join(LEDGER_REL)).map_err(|e| e.to_string())?;
        let r = execute(&sb, &fx, &skeleton_text, None, &[])?;
        ck(
            "★S4 execute: 빈칸 그대로의 분류표 → exit 2 · 원장·커밋·워크트리 부작용 0",
            r.rc == 2
                && r.stderr.contains("자리표시자가 남아 있다")
                && r.stderr.contains("approved_utc")
                && sb.git(repo, &["rev-parse", "multi-node"])? == head_before
                && sb.git(repo, &["rev-parse", "single-node"])? == single_before
                && std::fs::read(repo.join(LEDGER_REL)).map_err(|e| e.to_string())? == ledger_before
                && worktree_count(&sb, repo) == 2,
            &format!("({}, {})", r.rc, tail(&r.stderr, 500)),
        );
        let r = execute(&sb, &fx, &entry(&fx, "<FILL: 사람>"), None, &[])?;
        ck("★S4 execute: 한 칸만 남아도 거부(approved_by 자리표시자 · 검사기 항목 문장으로 확인)", r.rc == 2 && r.stderr.contains("자리표시자가 남아 있다") && r.stderr.contains(": approved_by --"), &format!("({}, {})", r.rc, tail(&r.stderr, 300)));
        let r = execute(&sb, &fx, &entry(&fx, ""), None, &[])?;
        ck(
            "S4 음성대조: 빈 필드 거부는 자리표시자 문장이 아니다(원인이 섞이지 않는다)",
            r.rc == 2 && r.stderr.contains("필수 필드 없음: approved_by") && !r.stderr.contains("자리표시자가 남아 있다"),
            &format!("({}, {})", r.rc, tail(&r.stderr, 300)),
        );
    }

    // ── S1 · S7 normal path: no ledger on the target · old judge · remote-only tag ──
    {
        let sb = Sandbox::new(real, "sbs_s1_")?;
        let fx = build_sequence_fixture(&sb, FxOpts::default())?;
        let repo = &fx.repo;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), None, &[])?;
        let out = r.all();
        ck("★S1 closing execute(multi→single) 완주 rc=0", r.rc == 0, &format!("({}, {})", r.rc, tail(&out, 1500)));
        ck(
            "★S1 RED ② 가 대상에 원장이 없어도 실제로 판정한다: clean · basis=ledger-sync-point · 동기 지점=converged",
            out.contains("공통층 갈라짐 검사: clean") && out.contains("basis=ledger-sync-point") && out.contains("동기 지점=converged") && !out.contains("판정 불가"),
            &tail(&out, 1500),
        );
        ck(
            "★S7 종료 시퀀스가 원인을 분류한다: 로컬 태그 오브젝트 부재 · 그 원격(fx)의 fetch 명령 · 기재하고 진행",
            out.contains("원인: 원격 fx 에만 있는 hint 태그의 **로컬 태그 오브젝트 부재**")
                && out.contains("[closing]     해소(사람 · 자동 fetch 하지 않는다): git -C")
                && out.contains("fetch fx 'refs/tags/hint/*:refs/tags/hint/*'")
                && out.contains("카탈로그는 그 태그를 object: absent-local 행(brief '—')으로 싣고 갱신됐다"),
            &tail(&out, 1500),
        );
        let idx = sb.git(repo, &["show", "single-node:hints/index.json"]).ok().and_then(|t| loads(&t));
        let row = idx.as_ref().and_then(|i| list(get(i, "hints")).into_iter().find(|h| text(get(h, "tag")).as_deref() == Some(HINT_TAG)));
        ck(
            "★S7 대상 브랜치 카탈로그가 원격 전용 태그를 `absent-local` 행으로 싣는다(brief 합성 ✗ · X15 --record-missing)",
            row.as_ref().is_some_and(|r| text(get(r, "object")).as_deref() == Some("absent-local") && text(get(r, "brief")).as_deref() == Some("—")),
            &format!("{row:?}"),
        );
        ck("★S7 파생기 안내도 곧 지워질 임시 워크트리가 아니라 주 워크트리를 가리킨다(두 안내가 같은 저장소)", !out.contains(".wt-single-node fetch") && out.contains(&format!("git -C {} fetch fx", p2s(repo))), &tail(&out, 1500));
        let tag_local = |sb: &Sandbox| sb.git_ok(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/tags/{HINT_TAG}")]);
        ck("★S7 자동 fetch 하지 않는다(원격 전용 태그는 여전히 로컬에 없다)", !tag_local(&sb), "");
        // default mode (no --record-missing) = the old contract: a missing local object stops with its own exit code, and HintError.render()
        //   names the fetch command of the remote it queried (main worktree) · 0 writes · no auto fetch. The exit code is read from the
        //   derivation's constant in its file (not written by hand again -- the closing sequence's rule).
        let cat_src = read(&real.join(HINTLIB_DIR).join("catalog.py"))?;
        let exit_missing: Option<i32> = cat_src.lines().find_map(|l| l.strip_prefix("EXIT_LOCAL_TAG_OBJECT_MISSING").and_then(|r| r.trim_start().strip_prefix('=')).and_then(|v| v.split('#').next()).and_then(|v| v.trim().parse().ok()));
        let idx_p = repo.join("hints/index.json");
        let idx_before = std::fs::read(&idx_p).ok();
        // hint.py belongs to the hint-publisher bundle: run as closing_sequence.sh runs it (python3 -B)
        let r = sb.run(&sv(&["python3", "-B", &p2s(&repo.join(CATALOG)), "--repo", &p2s(repo), "catalog", "derive", "--remote", "fx", "--generated-kst", "2026-09-14T09:00:00"]), repo, &[]);
        ck(
            "★S7 기본 모드: 로컬 오브젝트 부재 = 전용 종료코드 · render() 가 조회한 원격(fx)의 주 워크트리 fetch 명령을 말한다 · 쓰기 0",
            exit_missing.is_some_and(|x| r.rc == x) && r.stderr.contains(&format!("git -C {} fetch fx 'refs/tags/hint/*:refs/tags/hint/*'", p2s(repo))) && std::fs::read(&idx_p).ok() == idx_before && !tag_local(&sb),
            &format!("({}, {exit_missing:?}, {})", r.rc, tail(&r.stderr, 800)),
        );
        let t_rs = sb.git(repo, &["rev-parse", &format!("single-node:{LAYER_LEDGER_RS}")])?;
        let s_rs = sb.git(repo, &["rev-parse", &format!("multi-node:{LAYER_LEDGER_RS}")])?;
        ck(
            "동기화가 공통층·docs 스켈레톤·판정기를 옮겼다 · 특화층은 옮기지 않았다",
            sb.git(repo, &["show", "single-node:CLAUDE.md"])? == "# 픽스처 헌법 v3"
                && sb.git(repo, &["show", "single-node:docs/plan/example.md"])? == "# plan 스켈레톤 v2"
                && t_rs == s_rs
                && sb.git(repo, &["show", &format!("single-node:{TOPOLOGY_RULES}")])?.contains("**topology: single**"),
            "",
        );
        let led = loads(&sb.git(repo, &["show", &format!("multi-node:{LEDGER_REL}")])?);
        let entries = led.as_ref().map(|l| list(get(l, "entries"))).unwrap_or_default();
        let ids: Vec<String> = entries.iter().filter_map(|e| text(get(e, "entry_id"))).collect();
        let e1 = loads(&fx.e1);
        ck(
            "원장: 과거 항목 불변(append-only) · 이번 항목 추가 · 대상 브랜치로 복제 안 됨",
            entries.first().is_some_and(|x| Some(x) == e1.as_ref()) && ids == ["sync-1", "sync-2"] && !sb.git_ok(repo, &["cat-file", "-e", &format!("single-node:{LEDGER_REL}")]),
            &format!("{ids:?}"),
        );
        ck("잔존 워크트리 0", worktree_count(&sb, repo) == 2, "");
    }

    // ── S1 negative ①: a target-only common-layer change → stops at diverged and leaves the target branch alone ──
    {
        let sb = Sandbox::new(real, "sbs_div_")?;
        let fx = build_sequence_fixture(&sb, FxOpts { target_only_change: true, ..Default::default() })?;
        let repo = &fx.repo;
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), None, &[])?;
        let out = r.all();
        ck(
            "★S1 음성대조: 대상 전용 공통층 변경 → COMMON_LAYER_DIVERGED · closing rc=5 · 대상 브랜치 불변",
            r.rc == 5 && out.contains("COMMON_LAYER_DIVERGED") && out.contains(".claude/rules/common.md") && sb.git(repo, &["rev-parse", "single-node"])? == single_before && worktree_count(&sb, repo) == 2,
            &format!("({}, {})", r.rc, tail(&out, 1200)),
        );
    }

    // ── deadlock resolved: moving the target-only change to the departure branch as prescribed lets the same sequence finish ──
    {
        let sb = Sandbox::new(real, "sbs_carry_")?;
        let fx = build_sequence_fixture(&sb, FxOpts { target_only_change: true, carry_target_change: true, ..Default::default() })?;
        let repo = &fx.repo;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), None, &[])?;
        let out = r.all();
        ck(
            "★교착 해소: 대상 전용 변경을 출발 브랜치로 옮긴 뒤(다른 출발 커밋 C 뒤) execute → rc=0 · clean · 흡수 1건",
            r.rc == 0 && out.contains("공통층 갈라짐 검사: clean") && out.contains("흡수 1건") && !out.contains("COMMON_LAYER_DIVERGED"),
            &format!("({}, {})", r.rc, tail(&out, 1500)),
        );
        ck("교착 해소: 옮긴 바이트가 대상에 그대로 남는다(잃은 것 없음)", sb.git(repo, &["show", "single-node:.claude/rules/common.md"])? == "single 에서만 고친 공통층", "");
    }

    // ── ledger integrity: a target holding the same id with different content stops (COMMON_LAYER_UNJUDGED), no weak verdict ──
    {
        let sb = Sandbox::new(real, "sbs_forged_")?;
        let fx = build_sequence_fixture(&sb, FxOpts { forged_target_ledger: true, ..Default::default() })?;
        let repo = &fx.repo;
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), None, &[])?;
        let out = r.all();
        ck(
            "★원장 충돌 → COMMON_LAYER_UNJUDGED · basis=ledger-integrity · closing rc=5 · 대상 브랜치 불변",
            r.rc == 5 && out.contains("COMMON_LAYER_UNJUDGED") && out.contains("ledger-integrity") && out.contains("ledger-conflict") && sb.git(repo, &["rev-parse", "single-node"])? == single_before,
            &format!("({}, {})", r.rc, tail(&out, 1200)),
        );
    }

    // ── a departure branch without the judge (pre-introduction branch) stops instead of overwriting unjudged ──
    {
        let sb = Sandbox::new(real, "sbs_legacy_")?;
        let fx = build_sequence_fixture(&sb, FxOpts::default())?;
        let repo = &fx.repo;
        sb.git(repo, &["branch", "legacy-no-judge", "multi-node"])?;
        let lg = sb.root.join("legacy");
        sb.git(repo, &["worktree", "add", "-q", &p2s(&lg), "legacy-no-judge"])?;
        sb.git(&lg, &["rm", "-q", LAYER_LEDGER_SH, LAYER_LEDGER_RS])?;
        sb.git(&lg, &["commit", "-qm", "판정기 도입 이전 형태"])?;
        sb.git(repo, &["worktree", "remove", "--force", &p2s(&lg)])?;
        let wt = sb.root.join("dst");
        sb.git(repo, &["worktree", "add", "-q", &p2s(&wt), "single-node"])?;
        sb.git(&wt, &["checkout", "legacy-no-judge", "--", SYNC_BRANCHES])?;
        for rel in ["docs/_evidence/wm_selftest.json", "docs/plan/p_selftest.md"] {
            mkdirs(wt.join(rel).parent().unwrap())?;
            copy_file(&repo.join(rel), &wt.join(rel))?;
        }
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let r = sb.run(&sv(&["bash", &p2s(&wt.join(SYNC_BRANCHES)), "--from", "legacy-no-judge", "--mode", "experimental", "--manifest", "docs/_evidence/wm_selftest.json"]), &wt, &[]);
        ck(
            "★출발 브랜치에 판정기 없음 → COMMON_LAYER_UNJUDGED exit 10(판정 없는 덮어쓰기 ✗) · 대상 불변",
            r.rc == 10 && r.stderr.contains("COMMON_LAYER_UNJUDGED") && r.stderr.contains("판정기") && sb.git(repo, &["rev-parse", "single-node"])? == single_before,
            &format!("({}, {}, {})", r.rc, tail(&r.stdout, 400), tail(&r.stderr, 600)),
        );
    }

    // ── S1 negative ③: when the judge itself fails, unjudged is not passed as a pass (COMMON_LAYER_UNJUDGED) ──
    {
        let sb = Sandbox::new(real, "sbs_broken_")?;
        let fx = build_sequence_fixture(&sb, FxOpts::default())?;
        let repo = &fx.repo;
        let (fake, real_ledger) = fault_runtime_root(&sb)?;
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), None, &[("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", p2s(&fake)), (FAULT_ENV, real_ledger)])?;
        let out = r.all();
        ck(
            "★S1 음성대조: 판정기 실패 → COMMON_LAYER_UNJUDGED · closing rc=5 · 대상 브랜치 불변(판정 못 한 덮어쓰기 ✗)",
            r.rc == 5 && out.contains("COMMON_LAYER_UNJUDGED") && out.contains(FAULT_TEXT) && sb.git(repo, &["rev-parse", "single-node"])? == single_before,
            &format!("({}, {})", r.rc, tail(&out, 1000)),
        );
    }

    // ── S1 negative ②: a closing sequence without the judge pre-pull → stops instead of judging with the target's old judge ──
    {
        let sb = Sandbox::new(real, "sbs_prepull_")?;
        let fx = build_sequence_fixture(&sb, FxOpts::default())?;
        let repo = &fx.repo;
        let src = read(&repo.join(CLOSING))?;
        let anchor = format!("sync_branches.sh \\\n{PREPULL_JUDGE}");
        let n = src.matches(&anchor).count();
        ck("종료 시퀀스 변이 앵커(판정기 선행 당김) 1건", n == 1, &n.to_string());
        let mutated = sb.root.join("closing_mutated.sh");
        write(&mutated, &src.replace(&anchor, "sync_branches.sh\n"))?;
        let single_before = sb.git(repo, &["rev-parse", "single-node"])?;
        let r = execute(&sb, &fx, &entry(&fx, "selftest"), Some(&mutated), &[])?;
        let out = r.all();
        ck(
            "★S1 변이 음성대조: 판정기를 당기지 않으면 LEDGER_JUDGE_STALE 로 멈춘다(옛 규칙 판정 ✗) · 대상 불변",
            r.rc == 5 && out.contains("LEDGER_JUDGE_STALE") && sb.git(repo, &["rev-parse", "single-node"])? == single_before,
            &format!("({}, {})", r.rc, tail(&out, 1000)),
        );
    }
    Ok(())
}

/// S8 -- the sync_branches delete decision respects the `:(exclude)` of PATHS (2026-09-15 measured defect).
fn test_branch_sync_delete_respects_excludes(real: &Path) -> R<()> {
    for mutant in [false, true] {
        let sb = Sandbox::new(real, "sbs_s8_")?;
        let repo = sb.root.join("repo");
        mkdirs(&repo)?;
        sb.git(&repo, &["init", "-q", "-b", "single-node"])?;
        let mut rels: std::collections::BTreeSet<String> = sv(&[SYNC_BRANCHES, INLINE_JSON_SH, GATE, PARITY, PARITY_SH, LAYER_LEDGER_SH, LAYER_LEDGER_RS, ".gitignore"]).into_iter().collect();
        rels.extend(SCHEMAS.iter().map(|s| format!(".claude/schemas/{s}")));
        rels.extend(relocation_replacements(real)?);
        for rel in &rels {
            sb.copy_real(&repo, rel)?;
        }
        write_rel(&repo, "CLAUDE.md", "# 헌법 v1\n")?;
        write_rel(&repo, ".claude/rules/common.md", "공통층 v1\n")?;
        write_rel(&repo, ".claude/rules/dest_only.md", "이 커밋에만 있는 파일(다음 커밋에서 삭제)\n")?;
        write_rel(&repo, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: single** · layer: topology\n")?;
        sb.git(&repo, &["add", "-A"])?;
        sb.git(&repo, &["commit", "-qm", "A1 source"])?;
        sb.git(&repo, &["branch", "multi-node"])?;
        sb.git(&repo, &["rm", "-q", ".claude/rules/dest_only.md"])?;
        sb.git(&repo, &["commit", "-qm", "A2 source: dest_only.md 삭제(미러 삭제의 정당한 형태)"])?;

        // target (multi) checkout: tracks the ledger (excluded) · the orphan of the pre-delete commit · the topology layer (excluded)
        sb.git(&repo, &["checkout", "-q", "multi-node"])?;
        let ex = ledger_entry("sync-x", ENTRY_UTC, &"0".repeat(40), "selftest", APPROVED_AT, &["CLAUDE.md"], "픽스처");
        write_rel(&repo, LEDGER_REL, &format!("{{\"schema_version\": 1, \"entries\": [{ex}]}}\n"))?;
        write_rel(&repo, TOPOLOGY_RULES, "# 특화헌법\n\n**topology: multi** · layer: topology\n")?;
        sb.git(&repo, &["add", "-A"])?;
        sb.git(&repo, &["commit", "-qm", "B 대상 전용(원장·특화층)"])?;
        approval_fixture(&repo)?;

        let mut script = repo.join(SYNC_BRANCHES);
        if mutant {
            let src = read(&script)?;
            let anchor = "        _is_sync_excluded \"$dest_path\" && continue\n";
            let n = src.matches(anchor).count();
            ck("변이 앵커(exclude 존중 줄) 1건", n == 1, &n.to_string());
            // gitignored plane -- inside the repo without tripping the dirty guard (the script finds the repo from its own path)
            let mutant_script = repo.join("docs/_evidence/sync_mutated.sh");
            write(&mutant_script, &src.replace(anchor, ""))?;
            let ij = mutant_script.parent().unwrap().join("inline_json.sh");
            copy_file(&repo.join(INLINE_JSON_SH), &ij)?; // the script uses the launcher next to itself
            chmod(&ij, 0o755)?;
            script = mutant_script;
        }
        let r = sb.run(&sv(&["bash", &p2s(&script), "--from", "single-node", "--mode", "experimental", "--manifest", "docs/_evidence/wm_selftest.json", "--apply"]), &repo, &[]);
        let out = r.all();
        let ledger_alive = repo.join(LEDGER_REL).is_file();
        let topo_alive = repo.join(TOPOLOGY_RULES).is_file() && read(&repo.join(TOPOLOGY_RULES)).unwrap_or_default().contains("**topology: multi**");
        let orphan_gone = !repo.join(".claude/rules/dest_only.md").is_file();
        if !mutant {
            ck("S8: exclude 경로(원장·특화층)는 삭제되지 않고, 미제외 고아는 삭제된다", r.rc == 0 && ledger_alive && topo_alive && orphan_gone, &format!("({}, {ledger_alive}, {topo_alive}, {orphan_gone}, {})", r.rc, tail(&out, 600)));
        } else {
            ck("★S8 음성대조: exclude 존중을 걷어낸 변이는 원장을 지운다(이 검사가 결함을 잡는다)", !ledger_alive && orphan_gone, &format!("({}, {ledger_alive}, {})", r.rc, tail(&out, 400)));
        }
    }
    Ok(())
}

/// The helper launcher (inline_json): ① next to the script → ② the main worktree → ③ fail-closed. The target root does not change.
fn test_inline_json_locator(real: &Path) -> R<()> {
    let sb = Sandbox::new(real, "ij_locator_")?;
    let root = &sb.root;
    let main = root.join("main");
    mkdirs(&main.join(UVW))?;
    sb.git(&main, &["init", "-q", "-b", "single-node"])?;
    copy_file(&real.join(SYNC_BRANCHES), &main.join(SYNC_BRANCHES))?;
    sb.git(&main, &["add", "-A"])?;
    sb.git(&main, &["commit", "-qm", "base"])?;
    let wt = root.join("wt");
    sb.git(&main, &["worktree", "add", "-q", "-b", "other", &p2s(&wt)])?;
    let marker = root.join("ij_called");
    let go = |repo: &Path| {
        let mut c = Command::new("bash");
        c.args([p2s(&repo.join(SYNC_BRANCHES)), "--from".into(), "single-node".into(), "--mode".into(), "experimental".into()])
            .current_dir(repo)
            .env_clear()
            .envs(sb.env.iter().filter(|(k, _)| k != "EASY_VLLM_UPSTREAM_RUNTIME_ROOT").cloned())
            .stdin(Stdio::null());
        match c.output() {
            Ok(o) => Out { rc: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).into_owned(), stderr: String::from_utf8_lossy(&o.stderr).into_owned() },
            Err(e) => Out { rc: -1, stdout: String::new(), stderr: e.to_string() },
        }
    };
    let r = go(&wt); // no launcher anywhere
    ck("IJ③ 둘 다 없으면 fail-closed(종료 3 · 설명 메시지 · Python 폴백 없음)", r.rc == 3 && r.stderr.contains("inline_json"), &format!("({}, {})", r.rc, tail(&r.stderr, 300)));

    // launcher + ELF only in the main worktree (untracked products)
    write(&main.join(UVW).join("inline_json.sh"), &format!("#!/bin/bash\necho \"$@\" >> \"{}\"\nexit 0\n", marker.display()))?;
    chmod(&main.join(UVW).join("inline_json.sh"), 0o755)?;
    let elf = main.join(RUNTIME_REL).join("target/release/inline_json");
    write_bytes(&elf, b"\x7fELF")?;
    chmod(&elf, 0o755)?;
    let r = go(&wt);
    let used = marker.is_file();
    ck("IJ② 런처 없는 작업 트리 사본은 주 작업 트리의 런처를 쓴다", used && !r.stderr.contains("FAIL: inline_json"), &format!("({}, {}, {used})", r.rc, tail(&r.stderr, 300)));
    ck("IJ② 대상 루트는 스크립트 위치(작업 트리) 그대로 — 주 작업 트리로 바뀌지 않는다", !r.stderr.contains("FAIL: --from 과 대상이 같은 브랜치"), &tail(&r.stderr, 300));
    Ok(())
}

/// The function and array definitions of the deployed sync_to_sub.sh, **byte for byte** (no stand-in rewrite).
fn extract_shell(text: &str, names: &[&str], arrays: &[&str]) -> R<String> {
    let mut parts: Vec<String> = Vec::new();
    for line in text.lines() {
        if arrays.iter().any(|p| line.starts_with(p)) {
            parts.push(line.to_string());
        }
    }
    for name in names {
        let head = format!("{name}() {{");
        let start = text.match_indices(&head).map(|(i, _)| i).find(|&i| i == 0 || text.as_bytes()[i - 1] == b'\n').ok_or_else(|| format!("sync_to_sub.sh 에 함수 {name} 가 없다(검사 앵커 상실)"))?;
        // the first line that is exactly "}" after the head closes it (the original's `^\}\n`, non-greedy)
        let mut end = None;
        let mut pos = start;
        for line in text[start..].split_inclusive('\n') {
            pos += line.len();
            if line == "}\n" {
                end = Some(pos);
                break;
            }
        }
        let end = end.ok_or_else(|| format!("sync_to_sub.sh 에 함수 {name} 가 없다(검사 앵커 상실)"))?;
        parts.push(text[start..end].to_string());
    }
    Ok(parts.join("\n") + "\n")
}

fn scaffold_case(sb: &Sandbox, text: &str, label: &str, src_files: &BTreeMap<&str, &[u8]>, sub_after_rsync: Option<&dyn Fn(&Path) -> String>) -> R<(i32, String, PathBuf)> {
    let root = sb.root.join(format!("s9_{label}"));
    let (src, sub) = (root.join("src"), root.join("sub"));
    for (rel, data) in src_files {
        write_bytes(&src.join("output/multi").join(rel), data)?;
    }
    mkdirs(&sub.join("output/multi"))?;
    let shim = root.join("ssh");
    write(&shim, "#!/bin/bash\nwhile [ $# -gt 0 ]; do case \"$1\" in -o|-p|-i|-l) shift 2 ;; -*) shift ;; *) break ;; esac; done\nshift\nexec /bin/sh -c \"$*\"\n")?;
    chmod(&shim, 0o755)?;
    let start = text.find("    local -a build_files=(\"${BAND2_TOP[@]}\")").ok_or("S9 앵커(build_files) 상실")?;
    let end = start + text[start..].find("    fi\n    # (2) 오버레이").ok_or("S9 앵커((2) 오버레이) 상실")?;
    let lp = &text[start..end];
    let defs = extract_shell(
        text,
        &["effective_band2_exclusions", "script_modules_active", "script_module_files", "band2_post_files", "_band2_filters"],
        &["BAND2_TOP=(", "BAND2_PATCH_DIRS=(", "declare -A BAND2_PATCH_DIR_PAYLOAD=(", "BAND2_EXCLUDED_TOP=(", "BAND2_CONFIGS=(", "BAND2_ENVS=(", "BAND2_RUNTIME_PATCH_STEMS=(", "SOURCE_PORT_DIR=", "SOURCE_PORT_PAYLOAD="],
    )?;
    let hook = sub_after_rsync.map(|f| f(&sub)).unwrap_or_default();
    let runner = format!(
        "set -euo pipefail\n{defs}SRC='{}/'; SUB_HOST=u@mock; SUB_WORK_DIR='{}'; TARGETS=(multi)\nSSH_ARGV=('{}'); printf -v SSH_RSH '%q ' \"${{SSH_ARGV[@]}}\"; SSH_RSH=\"${{SSH_RSH% }}\"\nssh_run() {{ \"${{SSH_ARGV[@]}}\" \"$@\"; }}\n_band2_filters multi\nrsync -az --delete \"${{FILT[@]}}\" -e \"$SSH_RSH\" \"${{SRC%/}}/output/multi/\" \"$SUB_HOST:$SUB_WORK_DIR/output/multi/\"\n{hook}verify_buildkit() {{\n    local fail=0 L R f\n{lp}    return $fail\n}}\n# the real caller runs verify_checksums inside an `||` list (set -e is off there): same context here\nverify_buildkit multi || exit 1\n",
        src.display(),
        sub.display(),
        shim.display()
    );
    write(&root.join("runner.sh"), &runner)?;
    let r = sb.run(&sv(&["bash", &p2s(&root.join("runner.sh"))]), &root, &[]);
    Ok((r.rc, r.all(), sub))
}

fn test_b0_build_slot_scaffold_checksum_scope(real: &Path) -> R<()> {
    eprintln!("[S9] B0 빌드킷 체크섬 범위 ↔ rsync 필터(빈 빌드패치 슬롯의 스캐폴드)");
    let text = read(&real.join(SYNC_TO_SUB))?;
    let sb = Sandbox::new(real, "sbs_s9_")?;
    let kit: [(&str, &[u8]); 3] = [("Dockerfile", b"FROM base\n"), ("requirements.txt", b"x==1\n"), (".gitkeep", b"")];
    let with = |extra: &[(&'static str, &'static [u8])]| -> BTreeMap<&'static str, &'static [u8]> {
        let mut m: BTreeMap<&str, &[u8]> = kit.iter().cloned().collect();
        m.extend(extra.iter().cloned());
        m
    };
    let empty = with(&[("build_patches/.gitkeep", b"")]);
    // ⓐ empty slot: only the scaffold (.gitkeep) -- the filter excludes the whole slot, so verification does not ask for it either
    let (rc, out, sub) = scaffold_case(&sb, &text, "empty_slot", &empty, None)?;
    ck("S9 ⓐ 빈 빌드패치 슬롯 = 체크섬 통과(실제 B0 의 exit 2 재현 금지)", rc == 0 && !out.contains('❌'), &tail(&out, 300));
    ck("S9 ⓐ 배달된 최상위 빌드킷은 계속 대조된다", out.contains("✅ output/multi/Dockerfile") && out.contains("✅ output/multi/requirements.txt"), &tail(&out, 300));
    ck("S9 ⓐ 스캐폴드를 패키지로 지어내지 않는다(서브에 build_patches 없음 · 검증 목록에도 없음)", !sub.join("output/multi/build_patches").exists() && !out.contains("build_patches"), &tail(&out, 300));
    // ⓑ payload slot: every delivered file (scaffold included) is compared strictly
    let slot = with(&[("build_patches/.gitkeep", b""), ("build_patches/10-real.sh", b"#!/bin/bash\necho real\n")]);
    let (rc, out, sub) = scaffold_case(&sb, &text, "payload_ok", &slot, None)?;
    ck(
        "S9 ⓑ payload 슬롯 = 통과 · 실제 파일과 스캐폴드가 모두 대조·배달됨",
        rc == 0 && out.contains("✅ output/multi/build_patches/10-real.sh") && out.contains("✅ output/multi/build_patches/.gitkeep") && sub.join("output/multi/build_patches/10-real.sh").is_file(),
        &tail(&out, 300),
    );
    let corrupt_payload = |sub: &Path| format!("echo corrupted >> '{}/output/multi/build_patches/10-real.sh'\n", sub.display());
    let (rc, out, _) = scaffold_case(&sb, &text, "payload_corrupt", &slot, Some(&corrupt_payload))?;
    ck("S9 ⓑ 배달된 실제 payload 가 변조되면 적색", rc != 0 && out.contains("❌ output/multi/build_patches/10-real.sh"), &tail(&out, 300));
    // ⓒ a top-level build-kit file
    let corrupt_top = |sub: &Path| format!("echo x >> '{}/output/multi/Dockerfile'\n", sub.display());
    let (rc, out, _) = scaffold_case(&sb, &text, "top_corrupt", &empty, Some(&corrupt_top))?;
    ck("S9 ⓒ 최상위 빌드킷 파일 변조 = 적색", rc != 0 && out.contains("❌ output/multi/Dockerfile"), &tail(&out, 300));
    // ⓓ negative control: a mutant without the fix (the slot scope that follows the filter's owner) must be red at ⓐ
    let re = fancy_regex::Regex::new(r#"(?s)        excluded=0; for name in .*?\n        \[ "\$excluded" = "0" \] \|\| continue\n"#).map_err(|e| e.to_string())?;
    let mutated = re.replacen(&text, 1, "").into_owned();
    ck("S9 ⓓ 변이 사본이 실제로 수정을 걷어냈다(앵커 확인)", mutated != text, "");
    let (rc, out, _) = scaffold_case(&sb, &mutated, "mutated_empty_slot", &empty, None)?;
    ck("S9 ⓓ 수정을 걷어낸 변이 = 빈 슬롯에서 적색(옛 결함 재현 · 이 검사의 검출력)", rc != 0 && out.contains("❌ output/multi/build_patches/.gitkeep"), &tail(&out, 300));
    Ok(())
}

/// The repository root: from the launcher's `argv[0]` (`<repo>/.claude/skills/upstream-version-watch/scripts/selftest_branch_sync.py`),
/// otherwise from the executable's own place (`<repo>/.claude/skills/upstream-version-watch/rust/runtime/target/release/…`).
fn real_root(argv0: Option<&str>) -> Option<PathBuf> {
    if let Some(a) = argv0 {
        let p = PathBuf::from(a);
        if p.file_name().is_some_and(|n| n == "selftest_branch_sync.py" || n == "selftest_branch_sync.sh") {
            if let Ok(c) = std::fs::canonicalize(p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."))) {
                return c.ancestors().nth(4).map(Path::to_path_buf);
            }
        }
    }
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().nth(8).map(Path::to_path_buf)
}

/// Fault mode of the broken-judge case: `divergence` fails with the injected text; anything else runs the real judge unchanged.
fn fault_judge(real_ledger: &str, args: &[String]) -> i32 {
    if args.first().map(String::as_str) == Some("divergence") {
        eprintln!("RuntimeError: {FAULT_TEXT}");
        return 1;
    }
    use std::os::unix::process::CommandExt;
    let arg0 = std::env::args_os().next().unwrap_or_default();
    let e = Command::new(real_ledger).arg0(arg0).args(args).env_remove(FAULT_ENV).exec();
    eprintln!("fault judge: exec {real_ledger}: {e}");
    1
}

pub fn run(argv0: Option<String>, args: &[String]) -> i32 {
    if let Some(real_ledger) = std::env::var(FAULT_ENV).ok().filter(|v| !v.is_empty()) {
        return fault_judge(&real_ledger, args);
    }
    if !args.is_empty() {
        eprintln!("usage: selftest_branch_sync.py (no arguments)");
        return 2;
    }
    let Some(real) = real_root(argv0.as_deref()) else {
        eprintln!("[selftest_branch_sync] FAIL: 저장소 루트를 찾지 못했다");
        return 1;
    };
    let need = [SYNC_TO_SUB, SYNC_BRANCHES, LAYER_LEDGER_SH, LAYER_LEDGER_RS, CLOSING, PARITY, RENDER, GATE, CATALOG, HINTLIB_DIR, CATALOG_COMPANIONS[0], CATALOG_COMPANIONS[1]];
    for rel in need {
        let ok = real.join(rel).is_file() || (rel == HINTLIB_DIR && real.join(rel).join("catalog.py").is_file());
        if !ok {
            eprintln!("[selftest_branch_sync] FAIL: 검사 대상이 없다: {rel}");
            return 1;
        }
    }
    let ledger_elf = real.join(RUNTIME_REL).join("target/release/layer_ledger");
    if !ledger_elf.is_file() {
        eprintln!("[selftest_branch_sync] FAIL: 판정기 ELF 부재: {} — upstream runtime 을 release 빌드한다(Python 폴백 ✗)", ledger_elf.display());
        return 1;
    }
    type Test = fn(&Path) -> R<()>;
    let tests: [(&str, Test); 6] = [
        ("test_sync_to_sub_guard", test_sync_to_sub_guard),
        ("test_render_guard", test_render_guard),
        ("test_closing_sequence", test_closing_sequence),
        ("test_branch_sync_delete_respects_excludes", test_branch_sync_delete_respects_excludes),
        ("test_inline_json_locator", test_inline_json_locator),
        ("test_b0_build_slot_scaffold_checksum_scope", test_b0_build_slot_scaffold_checksum_scope),
    ];
    for (name, t) in tests {
        if let Err(e) = t(&real) {
            // fixture-construction failures count as FAIL too (no silent pass)
            FAILURES.with(|f| f.borrow_mut().push(trunc(&format!("{name}: {e}"), 1500)));
            eprintln!("  FAIL {name} 예외: {e}");
        }
    }
    let failures = FAILURES.with(|f| f.borrow().clone());
    if !failures.is_empty() {
        for f in &failures {
            eprintln!("[selftest_branch_sync] FAIL: {f}");
        }
        return 1;
    }
    eprintln!("[selftest_branch_sync] PASS");
    0
}
