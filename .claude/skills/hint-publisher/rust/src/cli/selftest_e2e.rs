//! The self-test's isolated E2E (hint.py `_fx_*` fixtures, `_selftest_e2e`, `_selftest_replay_continue`,
//! `_selftest_lite_e2e`): a temp repository (evidence's campaign fixture + templates + artifacts + lineage documents), a bare
//! remote, a fake `docker`, no live dependency.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::selftest::*;
use super::*;
use crate::core::{self, HR};
use crate::value::{Map, J};
use crate::{branch, catalog, evidence, lineage, naming, pii, tag, template};
use crate::{io, jd, jl};

// ── 소유 실행파일 ──────────────────────────────────────────────────────────────────────────────
fn code_root() -> PathBuf {
    core::skill_dir().ancestors().nth(3).map(Path::to_path_buf).unwrap_or_default()
}

/// The fixture repository links the owner launchers (`<stem>.sh`) the way the original links the owner `.py` files. A
/// launcher locates its native executable next to its own (symlinked) path, which inside the fixture has no build output —
/// the original's `python3 <owner>.py` needs none. The launchers' documented overrides point them at this checkout's
/// executables for the duration of the self-test (only when the caller did not set them).
pub(super) fn owner_bin_env() -> EnvGuard {
    let root = code_root();
    let own = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)).unwrap_or_default();
    let cands: Vec<(&str, PathBuf)> = vec![
        ("COMPLETION_GATE_BIN", root.join(".claude/rust/policy-gate/target/release/completion_gate")),
        ("DOC_NAMING_BIN", root.join(".claude/rust/policy-gate/target/release/doc_naming")),
        ("EVIDENCE_PUBLISHER_BIN", root.join(".claude/rust/policy-publisher/target/release/evidence_publisher")),
        ("CAMPAIGN_READ_BIN", root.join(".claude/skills/adversarial-benchmark/rust/target/release/campaign_init")),
        ("CAMPAIGN_VALIDATOR_BIN", root.join(".claude/skills/adversarial-benchmark/rust/target/release/campaign_template_validator")),
        ("MANIFEST_CONTRACT_BIN", root.join(".claude/skills/adversarial-benchmark/rust/target/release/manifest_contract")),
        ("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", root.join(".claude/skills/upstream-version-watch/rust/runtime")),
        ("RENDER_BENCH_SECTION_BIN", own.join("render_bench_section")),
    ];
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (k, p) in cands {
        if std::env::var_os(k).is_none() && p.exists() {
            pairs.push((k.to_string(), core::posix(&p)));
        }
    }
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    EnvGuard::set(&refs)
}

// ── E2E 픽스처 ─────────────────────────────────────────────────────────────────────────────────
fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn js(s: &str) -> J {
    J::Str(s.to_string())
}
fn ioe(e: std::io::Error, p: &Path) -> crate::core::HintError {
    core::io_err(&e, p)
}
fn read(p: &Path) -> HR<String> {
    std::fs::read_to_string(p).map_err(|e| ioe(e, p))
}
fn read_b(p: &Path) -> HR<Vec<u8>> {
    std::fs::read(p).map_err(|e| ioe(e, p))
}
fn write(p: &Path, s: &str) -> HR<()> {
    std::fs::write(p, s).map_err(|e| ioe(e, p))
}
fn write_b(p: &Path, s: &[u8]) -> HR<()> {
    std::fs::write(p, s).map_err(|e| ioe(e, p))
}
fn mkdirs(p: &Path) -> HR<()> {
    std::fs::create_dir_all(p).map_err(|e| ioe(e, p))
}
fn unlink(p: &Path) -> HR<()> {
    std::fs::remove_file(p).map_err(|e| ioe(e, p))
}
fn load(p: &Path) -> HR<J> {
    crate::value::loads(&read(p)?).map_err(|e| py("JSONDecodeError", e))
}
fn has(v: &J, s: &str) -> bool {
    v.items().iter().any(|x| x.as_str() == Some(s))
}
fn strs(v: &J) -> Vec<String> {
    v.items().iter().map(J::display).collect()
}
fn codes(issues: &[J]) -> std::collections::BTreeSet<String> {
    issues.iter().map(|i| i["code"].display()).collect()
}
fn pth(v: &J) -> PathBuf {
    PathBuf::from(v.display())
}
fn set_mtime(p: &Path, epoch: i64) -> HR<()> {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(epoch as u64);
    let f = std::fs::File::options().write(true).open(p).map_err(|e| ioe(e, p))?;
    f.set_times(std::fs::FileTimes::new().set_accessed(t).set_modified(t)).map_err(|e| ioe(e, p))
}
fn copy2(src: &Path, dst: &Path) -> HR<()> {
    std::fs::copy(src, dst).map_err(|e| ioe(e, src))?;
    if let Ok(m) = std::fs::metadata(src).and_then(|m| m.modified()) {
        if let Ok(f) = std::fs::File::options().write(true).open(dst) {
            let _ = f.set_times(std::fs::FileTimes::new().set_modified(m));
        }
    }
    Ok(())
}
fn copytree(src: &Path, dst: &Path, skip: &[&str]) -> HR<()> {
    mkdirs(dst)?;
    let mut ents: Vec<_> = std::fs::read_dir(src).map_err(|e| ioe(e, src))?.flatten().collect();
    ents.sort_by_key(|e| e.file_name());
    for e in ents {
        let name = e.file_name().to_string_lossy().into_owned();
        if skip.contains(&name.as_str()) {
            continue;
        }
        let (s, d) = (e.path(), dst.join(&name));
        let ft = e.file_type().map_err(|x| ioe(x, &s))?;
        if ft.is_symlink() {
            let t = std::fs::read_link(&s).map_err(|x| ioe(x, &s))?;
            std::os::unix::fs::symlink(&t, &d).map_err(|x| ioe(x, &d))?;
        } else if ft.is_dir() {
            copytree(&s, &d, skip)?;
        } else {
            copy2(&s, &d)?;
        }
    }
    Ok(())
}

pub(super) fn _fx_git_env(repo: &Path, args: &[&str], env: &[(String, String)], check: bool) -> HR<core::Proc> {
    let mail = ["fixture", "example.invalid"].join("@");
    let mut e: Vec<(String, String)> = vec![
        ("GIT_AUTHOR_NAME".into(), "fixture".into()),
        ("GIT_AUTHOR_EMAIL".into(), mail.clone()),
        ("GIT_COMMITTER_NAME".into(), "fixture".into()),
        ("GIT_COMMITTER_EMAIL".into(), mail),
        ("GIT_AUTHOR_DATE".into(), "@1767225600 +0000".into()),
        ("GIT_COMMITTER_DATE".into(), "@1767225600 +0000".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
    ];
    e.extend(env.iter().cloned());
    let mut argv = sv(&["git", "-c", "tag.gpgSign=false", "-c", "commit.gpgSign=false", "-c", "init.defaultBranch=multi-node"]);
    argv.extend(args.iter().map(|s| s.to_string()));
    let o = core::run_capture(&argv, Some(repo), &e, &[], None).map_err(|x| py("FileNotFoundError", x))?;
    let un = |b: &[u8]| String::from_utf8_lossy(b).replace("\r\n", "\n").replace('\r', "\n");
    let r = core::Proc { returncode: o.returncode, stdout: un(&o.stdout), stderr: un(&o.stderr) };
    if check && r.returncode != 0 {
        let head: Vec<&str> = args.iter().take(3).copied().collect();
        return Err(py(
            "RuntimeError",
            format!("fixture git {} rc={}: {}", util::tuple_repr(&head), r.returncode, core::tail_chars(&r.stderr, 400)),
        ));
    }
    Ok(r)
}

pub(super) fn _fx_git(repo: &Path, args: &[&str]) -> HR<core::Proc> {
    _fx_git_env(repo, args, &[], true)
}

/// The fake `docker` (the original's `_fx_docker(state)` callable): `image inspect [--format {{.Id}}|{{.Created}}]` and
/// `history` answer from the fixture state, `run` (ledger cat) = an old image without a ledger (rc 1), anything else rc 1.
/// It is an executable first on `PATH` for the scope of the guard; every call is logged.
pub(super) struct FakeDocker {
    dir: PathBuf,
    _path: EnvGuard,
}

impl FakeDocker {
    pub fn calls(&self) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(self.dir.join("calls.log")).unwrap_or_default();
        text.lines().map(|l| l.split('\u{1f}').filter(|x| !x.is_empty()).map(str::to_string).collect()).collect()
    }
}

fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub(super) fn _fx_docker(td: &Path, state: &J) -> HR<FakeDocker> {
    let dir = td.join("fx-docker-bin");
    mkdirs(&dir)?;
    let mut inspect_cases = String::new();
    let mut history_cases = String::new();
    if let Some(m) = state["images"].as_dict() {
        for (i, (k, img)) in m.iter().enumerate() {
            let base = dir.join(format!("img{i}"));
            write(&base.with_extension("json"), &crate::value::dumps_default(&jl![img.clone()]))?;
            for (ext, key) in [("id", "Id"), ("created", "Created")] {
                let v = &img[key];
                write(&base.with_extension(ext), &if v.truthy() { format!("{}\n", v.display()) } else { String::new() })?;
            }
            inspect_cases.push_str(&format!("    {}) d={} ;;\n", sq(k), sq(&core::posix(&base))));
        }
    }
    if let Some(m) = state["history"].as_dict() {
        for (i, (k, h)) in m.iter().enumerate() {
            let f = dir.join(format!("hist{i}.txt"));
            write(&f, &h.display())?;
            history_cases.push_str(&format!("    {}) cat {}; exit 0 ;;\n", sq(k), sq(&core::posix(&f))));
        }
    }
    let script = format!(
        "#!/bin/bash\n{{ for a in \"$@\"; do printf '%s\\037' \"$a\"; done; printf '\\n'; }} >> {log}\n\
         last=\"${{@: -1}}\"\n\
         if [ \"$1\" = image ] && [ \"$2\" = inspect ]; then\n  case \"$last\" in\n{inspect_cases}    *) printf 'No such image' >&2; exit 1 ;;\n  esac\n\
         \x20 fmt=''; prev=''; for a in \"$@\"; do [ \"$prev\" = --format ] && fmt=\"$a\"; prev=\"$a\"; done\n\
         \x20 if [ -n \"$fmt\" ]; then\n    case \"$fmt\" in '{{{{.Id}}}}') f=\"$d.id\" ;; '{{{{.Created}}}}') f=\"$d.created\" ;; *) f=/nonexistent ;; esac\n\
         \x20   if [ -s \"$f\" ]; then cat \"$f\"; exit 0; fi\n    exit 1\n  fi\n  cat \"$d.json\"; exit 0\nfi\n\
         if [ \"$1\" = history ]; then\n  case \"$last\" in\n{history_cases}    *) exit 1 ;;\n  esac\nfi\n\
         if [ \"$1\" = run ]; then printf 'cat: /opt/easy-vllm/build_ledger.json: No such file' >&2; exit 1; fi\n\
         printf 'unexpected docker call' >&2\nexit 1\n",
        log = sq(&core::posix(&dir.join("calls.log"))),
    );
    let exe = dir.join("docker");
    write(&exe, &script)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).map_err(|e| ioe(e, &exe))?;
    }
    let path = format!("{}:{}", core::posix(&dir), std::env::var("PATH").unwrap_or_default());
    Ok(FakeDocker { dir, _path: EnvGuard::set(&[("PATH", &path)]) })
}

/// `_fx_tree(root)`: every regular file (not a symlink · not under `.git` · not `ACTIVE`) → bytes.
pub(super) fn _fx_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if e.file_name() != ".git" {
                    walk(root, &p, out);
                }
                continue;
            }
            if e.file_name() == "ACTIVE" {
                continue;
            }
            if let (Ok(rel), Ok(b)) = (p.strip_prefix(root), std::fs::read(&p)) {
                out.insert(core::posix(rel), b);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn link_owner(repo: &Path, root: &Path, rel: &str) -> HR<()> {
    let dst = repo.join(rel);
    mkdirs(dst.parent().unwrap())?;
    // 의도된 차이: Rust 는 소유 CLI 를 `<stem>.sh` launcher 로 부른다 — launcher 를 링크하고, 원본 `.py` 는 있을 때만
    //   함께 링크한다(plan_26111615: 삭제 대기 참조본). 어느 쪽도 없으면 소유 모듈 부재(fail-closed).
    let launcher = rel.strip_suffix(".py").map(|s| format!("{s}.sh"));
    if root.join(rel).is_file() || launcher.is_none() {
        std::os::unix::fs::symlink(root.join(rel), &dst).map_err(|e| ioe(e, &dst))?;
    }
    if let Some(l) = launcher {
        let (ls, ld) = (root.join(&l), repo.join(&l));
        if ls.is_file() && !ld.exists() {
            std::os::unix::fs::symlink(&ls, &ld).map_err(|e| ioe(e, &ld))?;
        }
        if !ls.is_file() && !root.join(rel).is_file() {
            return core::fail("HINT_OWNER_MODULE_MISSING", format!("자체검사 소유 모듈 부재: {rel}"), None);
        }
    }
    Ok(())
}

/// `_fx_repo(td)`: evidence's isolated campaign fixture (c1 · cell c1-a · cluster · owner links) plus what the pre-publish
/// path needs: git repository · templates · naming SSOT · artifacts · lineage documents · bare remote · central authority.
pub(super) fn _fx_repo(td: &Path) -> HR<(PathBuf, J)> {
    let (repo, mut fx) = evidence::_fixture(td)?;
    let root = code_root();
    // The prose naming launcher and the bench naming Rust source (the lineage grammar reads it · plan_26111615 D4 · no `.py`).
    for rel in [core::REL_DOC_NAMING_WIKI, core::REL_DOC_NAMING_BENCH] {
        link_owner(&repo, &root, rel)?;
    }
    // 의도된 차이: 명명 SSOT 의 TS 정본은 사본으로 둔다(심링크 ✗) — 그 스크립트는 `argv[1]` 과 실경로(import.meta.url)가 같을
    //   때만 CLI 로 돈다. 원본 shim 은 실경로로 부르지만 native 소유 실행파일은 저장소 상대 경로로 부른다(의존 import 없음 · 바이트 동일).
    let ts = repo.join(core::REL_DOC_NAMING_TS);
    mkdirs(ts.parent().unwrap())?;
    copy2(&root.join(core::REL_DOC_NAMING_TS), &ts)?;
    // 의도된 차이: native 소유 실행파일은 심링크를 따라가지 않으므로 bench 보고 API launcher 도 같은 자리에 링크한다.
    {
        let rel = ".claude/skills/adversarial-benchmark/scripts/bench_reporting_api.sh";
        if root.join(rel).is_file() && !repo.join(rel).exists() {
            link_owner(&repo, &root, rel)?;
        }
    }
    // 의도된 차이: launcher 는 자기 (심링크) 자리 옆의 빌드 산출물(`target/`)에서 native 실행파일을 찾는다 — 원본 `python3 <소유>.py`
    //   는 빌드 산출물이 필요 없다. 이 체크아웃의 빌드 산출물 디렉터리를 같은 자리에 링크한다(parity 격리 사본과 같은 방식).
    for t in [
        ".claude/rust/policy-gate/target",
        ".claude/rust/policy-publisher/target",
        ".claude/skills/adversarial-benchmark/rust/target",
        ".claude/skills/upstream-version-watch/rust/runtime/target",
    ] {
        let (src, dst) = (root.join(t), repo.join(t));
        if src.is_dir() && !dst.exists() {
            mkdirs(dst.parent().unwrap())?;
            std::os::unix::fs::symlink(std::fs::canonicalize(&src).unwrap_or(src), &dst).map_err(|e| ioe(e, &dst))?;
        }
    }
    // 의도된 차이: evidence 픽스처는 campaign_init 을 사본(.py)으로 둔다 — Rust 는 그 launcher 를 부르므로 launcher 도 사본으로 둔다
    //   (사본 옆 .py 가 호환 경로의 소유자다 · CAMPAIGN_READ_COMPAT_OWNER 기본값).
    let ci_sh = core::REL_CAMPAIGN_INIT.to_string();
    if root.join(&ci_sh).is_file() && !repo.join(&ci_sh).exists() {
        std::fs::copy(root.join(&ci_sh), repo.join(&ci_sh)).map_err(|e| ioe(e, &repo.join(&ci_sh)))?;
    }
    // 의도된 차이: 원본 소유 모듈은 `Path(__file__).resolve()` 로 자기 체크아웃의 `.claude/schemas` 를 읽는다(심링크를 따라간다) —
    //   native 소유 실행파일은 저장소 루트 아래 `.claude/schemas` 를 읽으므로 같은 스키마를 링크로 둔다(코드 평면 · 데이터 아님).
    let schemas = repo.join(".claude/schemas");
    if root.join(".claude/schemas").is_dir() && !schemas.exists() {
        std::os::unix::fs::symlink(root.join(".claude/schemas"), &schemas).map_err(|e| ioe(e, &schemas))?;
    }
    copytree(&core::templates_dir(), &repo.join(core::REL_SKILL).join("templates"), &[])?;
    let out = repo.join("output/multi");
    let models = td.join("models");
    let man = read(&out.join("manifest.yaml"))?;
    write(
        &out.join("manifest.yaml"),
        &format!(
            "{man}quant_model_path: {}\ninterconnect:\n  socket_iface: {_FX_NIC_IFACE}\n  hca_devices: [{_FX_NIC_HCA}]\n",
            core::posix(&models)
        ),
    )?;
    write_b(&repo.join(_FX_LOG_REL), _FX_LOG().as_bytes())?;
    let lvl = repo.join("output/multi/benchlog/sweep_c1-a/level_01/bench_c1-a.json");
    let mut lv = load(&lvl)?;
    for (k, v) in [
        ("backend", js("openai")),
        ("endpoint", js("/v1/completions")),
        ("model_id", js("/app/quant_models/Org/Fixture-Model-NVFP4")),
        ("num_prompts", J::from(4)),
        ("max_concurrency", J::from(1)),
        ("total_input_tokens", J::from(4096)),
        ("total_output_tokens", J::from(1024)),
    ] {
        lv.set(k, v);
    }
    core::write_json(&lvl, &lv)?;
    let compose = "x-gpu-common: &gpu-common\n  image: ${IMAGE_TAG:-fixture:0.9.0-source}\n  build:\n    context: .\n\
        \x20   dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n    args:\n\
        \x20     VLLM_REPO: ${VLLM_REPO:-fixture-repo}\n      VLLM_REF: ${VLLM_REF:-v0.9.0}\n\
        \x20     SM12X_PORT: ${SM12X_PORT:-0}\n      BUILD_JOBS: ${BUILD_JOBS:-16}\n  volumes:\n\
        \x20   - ${QUANT_MODEL_PATH:-/srv/fixture-models}:/app/quant_models:ro\n    - ./configs:/app/configs:ro\n\
        services:\n  vllm-master-serve:\n    <<: *gpu-common\n    profiles: [master]\n    environment:\n\
        \x20     NODE_ROLE: master\n      CONFIG_FILE: ${CONFIG_FILE:-default}\n    env_file:\n\
        \x20     - envs/.env.interconnect\n      - envs/.env.cluster\n      - path: envs/.env.${CONFIG_FILE:-default}\n\
        \x20   command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n\
        \x20 vllm-slave-serve:\n    <<: *gpu-common\n    profiles: [slave]\n\
        \x20   container_name: ${SLAVE_CONTAINER_NAME:-slave}\n    environment:\n      NODE_ROLE: slave\n\
        \x20     CONFIG_FILE: ${CONFIG_FILE:-default}\n      RAY_PORT: ${RAY_PORT:-6379}\n\
        \x20     VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}\n    env_file:\n      - envs/.env.interconnect\n\
        \x20     - envs/.env.cluster\n    command: [\"bash\", \"/app/configs/serve_runner.sh\"]\n";
    write(&out.join("docker-compose.yaml"), compose)?;
    let ip = ["192", "168", "7", "11"].join(".");
    write(
        &out.join("envs/.env.cluster"),
        &format!("MASTER_HOST_IP={ip}\nSLAVE_HOST_IP=node-sub-fixture\nSSH_USER=opfixture\nMAX_JOBS=4\nRAY_PORT=6379\n"),
    )?;
    write(&out.join("envs/.env.interconnect"), "NCCL_DEBUG=INFO\nNCCL_SOCKET_IFNAME=enpfixture0\nNCCL_IB_DISABLE=1\n")?;
    let mut envs: Vec<PathBuf> = std::fs::read_dir(out.join("envs"))
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".env"))).collect())
        .unwrap_or_default();
    envs.push(out.join(".env"));
    for env_p in envs {
        if env_p.is_file() {
            set_mtime(&env_p, _FX_ENV_MTIME_EPOCH)?;
        }
    }
    let assets = root.join(".claude/skills/upstream-version-watch/assets/configs");
    for name in ["serve_runner.sh", "arm_patch.sh", "debug-init.sh"] {
        if assets.join(name).is_file() {
            copy2(&assets.join(name), &out.join("configs").join(name))?;
        }
    }
    write(
        &out.join("Dockerfile.source-build"),
        "FROM base\nARG VLLM_REPO=x\nARG VLLM_REF=y\nARG SM12X_PORT=0\nARG BUILD_JOBS=16\n\
         COPY requirements.txt /tmp/requirements.txt\nCOPY build_patches_src/ /tmp/build_patches_src/\n\
         COPY build_patches/ /tmp/build_patches/\n",
    )?;
    write(&out.join("Dockerfile"), "FROM wheel\nARG VLLM_VERSION=0.0.1\nCOPY requirements.txt /tmp/r.txt\n")?;
    write(&out.join("requirements.txt"), "transformers<5.15.0\n")?;
    for d in ["build_patches_src", "build_patches"] {
        mkdirs(&out.join(d))?;
    }
    write(
        &out.join("build_patches_src/50-port.sh"),
        "#!/bin/bash\n# model-trigger : OtherModel\n: \"${SM12X_PORT:=0}\"\nif [ \"${SM12X_PORT}\" != \"1\" ]; then\n    echo \"skip\"\n    exit 0\nfi\n",
    )?;
    write(&out.join("build_patches_src/60-fixture-patch.sh"), "#!/bin/bash\n# fixture patch: PLE 게이트 완화\n# gate : 없음\n")?;
    write(&out.join("build_patches/10-shared.sh"), "#!/bin/bash\n# model-trigger : 공유 이미지 arch-enablement\n")?;
    if let Some(img) = fx.get_mut("docker").and_then(|d| d.get_mut("images")).and_then(|i| i.get_mut(evidence::_DIGEST)) {
        img.set("Created", "2026-01-01T00:00:00Z");
    }
    let docs: Vec<(String, String)> = vec![
        (fx["plan"].display(), "# plan_26010200 — 픽스처 계획\n\n## 1. 목표\n픽스처 모델을 GB10 두 노드에서 4096 컨텍스트로 서빙한다.\n".into()),
        (fx["devlog"].display(), "# devlog_26010213 — 픽스처 서사\n\n## 1. 경과\n벽 하나를 넘고 측정까지 갔다.\n".into()),
        (
            fx["testlog"].display(),
            format!(
                "# testlog_26010213 — 픽스처 셀 판정\n\n## 1. 판정\n픽스처 셀 c1-a 는 health 200 과 추론 1회로 PASS 다.\n\n\
                 ## 2. 벽\n회차 3: {_FX_WALL_SIG}\n백엔드 명시를 걷자 통과했다.\n\n\
                 ## 3. 오진\n{_FX_MISDIAG_SIG}.\n정정: 호스트 워치독이 사살했다.\n\n\
                 ## 4. 측정\nvllm bench serve --max-concurrency 1 --num-prompts 4\n"
            ),
        ),
    ];
    for (rel, body) in &docs {
        write(&repo.join(rel), body)?;
    }
    write(&repo.join(core::REL_CENTRAL_FLAG), "픽스처 — 이 체크아웃이 자기 원격의 hint 색인·배포 권위다.\n")?;
    _fx_git(&repo, &["init", "-q"])?;
    write(&repo.join("README.fixture"), "fixture\n")?;
    _fx_git(&repo, &["add", "README.fixture"])?;
    _fx_git(&repo, &["commit", "-q", "-m", "fixture root"])?;
    let bare = td.join("remote.git");
    _fx_git(td, &["init", "-q", "--bare", &core::posix(&bare)])?;
    _fx_git(&repo, &["remote", "add", "origin", &core::posix(&bare)])?;
    let mail = ["fixture", "example.invalid"].join("@");
    for (k, v) in [("user.name", "fixture"), ("user.email", mail.as_str()), ("commit.gpgSign", "false")] {
        _fx_git(&repo, &["config", k, v])?;
    }
    let guide = branch::selftest_guide(&repo, Some(branch::BRANCH_FORMAT), None, branch::FX_GUIDE_BODY, None)?;
    _fx_git(&repo, &["push", "-q", "origin", &format!("{guide}:{}", core::HINT_BRANCH_REF)])?;
    _fx_git(&repo, &["remote", "add", "gh", "https://fixture.invalid/hint.git"])?;
    _fx_git(&repo, &["config", &format!("url.{}.insteadOf", core::posix(&bare)), "https://fixture.invalid/hint.git"])?;
    fx.set("bare", core::posix(&bare));
    fx.set("models", core::posix(&models));
    fx.set("guide", guide);
    Ok((repo, fx))
}

/// AC6 — old 5-segment annotated tag (local · remote), a remote-only tag without a local object, a remote lightweight tag.
pub(super) fn _fx_legacy_remote(td: &Path, repo: &Path, bare: &Path) -> HR<J> {
    let old = "hint/0.9.0/fixture-model-nvfp4/gb10x2-cluster-native/len4096-kvauto-plemmap";
    _fx_git(repo, &["tag", "-a", "-m", "옛 brief — 노드축 옛 문법", old, "HEAD"])?;
    _fx_git(repo, &["push", "-q", "origin", &format!("refs/tags/{old}:refs/tags/{old}")])?;
    let foreign = td.join("foreign");
    _fx_git(td, &["clone", "-q", &core::posix(bare), &core::posix(&foreign)])?;
    write(&foreign.join("x.txt"), "타 PC\n")?;
    _fx_git(&foreign, &["add", "x.txt"])?;
    _fx_git(&foreign, &["commit", "-q", "-m", "foreign"])?;
    let remote_only = "hint/0.9.0/other-model/gb10-1g1n-main-native/qbf16-len2048-kvauto-plenone-specoff-graph";
    let lightweight = "hint/0.8.0/lw-model/gb10-sim-h100/qfp8-len1024-kvfp8";
    _fx_git(&foreign, &["tag", "-a", "-m", "타 PC 발행", remote_only, "HEAD"])?;
    _fx_git(&foreign, &["tag", lightweight, "HEAD"])?;
    _fx_git(
        &foreign,
        &["push", "-q", "origin", &format!("refs/tags/{remote_only}:refs/tags/{remote_only}"), &format!("refs/tags/{lightweight}:refs/tags/{lightweight}")],
    )?;
    Ok(jd! {"old" => old, "remote_only" => remote_only, "lightweight" => lightweight})
}

fn _fx_quote(val: &str) -> String {
    if val.is_empty() {
        return "(빈 값)".into();
    }
    if val.chars().any(|ch| ",:#\"'".contains(ch)) || val.chars().next().is_some_and(|c| "[{|>&*!".contains(c)) {
        let q = if !val.contains('\'') { '\'' } else { '"' };
        return format!("{q}{val}{q}");
    }
    val.to_string()
}

fn testlog_stem() -> String {
    _FX_TESTLOG.rsplit('/').next().unwrap_or("").trim_end_matches(".md").to_string()
}
fn log_name() -> String {
    util::py_basename(_FX_LOG_REL)
}

fn py_int_or_zero(v: &J) -> i64 {
    match v {
        J::Null => 0,
        J::Bool(b) => *b as i64,
        J::Int(i) => *i,
        J::Float(f) => *f as i64,
        J::Str(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// `_fx_author(repo, draft, st, factcheck=True)` — programmatic authoring of the PROMPT sections from the template
/// instructions (block kinds · minimum counts · required excerpts), plus the independent fact-check stand-in's report.
pub(super) fn _fx_author(repo: &Path, draft: &Path, st: &J, factcheck: bool) -> HR<()> {
    let payload = draft.join("payload");
    let facts = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
    let stem = testlog_stem();
    let src = excerpt_text(repo, draft, &stem, None, false)?;
    let wall_line = src.split('\n').find(|l| l.contains(_FX_WALL_SIG)).ok_or_else(|| py("StopIteration", ""))?.to_string();
    let diag_line = src.split('\n').find(|l| l.contains(_FX_MISDIAG_SIG)).ok_or_else(|| py("StopIteration", ""))?.to_string();
    let mut counter: BTreeMap<&str, i64> = BTreeMap::new();
    let mut nid = |prefix: &'static str| -> String {
        let n = counter.entry(prefix).or_insert(0);
        *n += 1;
        format!("{prefix}{n}")
    };
    let mut event = |kind: &str| -> Vec<String> {
        if kind == "value-status" {
            let mut out = Vec::new();
            for c in facts["value_status_candidates"].items() {
                let status = if c["candidate_status"].truthy() { c["candidate_status"].display() } else { "inherited".into() };
                let why = if status == "declared-requirement" { "픽스처 — 빼면 W1 이 다시 난다" } else { "픽스처 — 이 셀에서 조정 기록 없음" };
                let val = if c["value"].is_null() { String::new() } else { c["value"].display() };
                out.push(format!(
                    "```hint-event\n{}\n```",
                    [
                        format!("id: {}", nid("V")),
                        "kind: value-status".into(),
                        format!("노브: {}", c["knob"].display()),
                        format!("값: {}", _fx_quote(&val)),
                        format!("지위: {status}"),
                        format!("근거: {why}"),
                        format!("출처: [{stem} §2]"),
                    ]
                    .join("\n")
                ));
            }
            return out;
        }
        let body: Vec<String> = match kind {
            "wall" => vec![
                "kind: wall".into(),
                "증상: 기동 중 백엔드 선택에서 실패".into(),
                format!("서명: \"{_FX_WALL_SIG}\""),
                "원인: 명시한 백엔드가 가드와 충돌".into(),
                "해소: 백엔드 명시 제거".into(),
                format!("검증: {stem} §2"),
                "전이등급: arch-scaled".into(),
                format!("출처: [{stem} §2]"),
            ],
            "misdiagnosis" => vec![
                "kind: misdiagnosis".into(),
                "증상: hang".into(),
                format!("서명: \"{_FX_MISDIAG_SIG}\""),
                "원인: 워치독을 못 봤다".into(),
                "해소: 호스트 워치독 사살로 정정".into(),
                format!("검증: {stem} §3"),
                "전이등급: judgment".into(),
                "현재지위: 반증".into(),
                format!("출처: [{stem} §3]"),
            ],
            "rejected" => vec!["kind: rejected".into(), "시도: 백엔드 명시".into(), "기각사유: 가드와 충돌".into(), format!("출처: [{stem} §2]")],
            "open-question" => {
                vec!["kind: open-question".into(), "물음: cold TTFT 이상치".into(), "현재상태: 미설명".into(), format!("출처: [{stem} §4]")]
            }
            other => return vec![format!("<<KeyError {other}>>")],
        };
        let prefix = match kind {
            "wall" => "W",
            "misdiagnosis" => "M",
            "rejected" => "R",
            _ => "Q",
        };
        let mut lines = vec![format!("id: {}", nid(prefix))];
        lines.extend(body);
        vec![format!("```hint-event\n{}\n```", lines.join("\n"))]
    };
    let excerpt = format!("> [원문] {stem} §2. 벽\n> {wall_line}");
    let ln = log_name();
    let log_rows: Vec<String> = match excerpt_text(repo, draft, &ln, None, false) {
        Ok(t) => t.split('\n').map(str::to_string).collect(),
        Err(e) if !e.is_py() => Vec::new(),
        Err(e) => return Err(e),
    };
    let log_excerpt = log_rows.iter().position(|r| r == "Loading 100%").map(|i| {
        let la = i + 1;
        format!("> [원문] {ln} §L{la}-{}\n> {}\n> {}", la + 1, log_rows[la - 1], log_rows[la])
    });
    let todo: Vec<J> = template::prompts(&payload)?;
    let chap_re = crate::pyre::Re::new(r"^## (\d+\.\d+) ", 0);
    for name in template::payload_docs(&facts)? {
        let path = payload.join(&name);
        let text = read(&path)?;
        let mut chapter: Option<String> = None;
        let mut out: Vec<String> = Vec::new();
        for line in text.split('\n') {
            if let Some(m) = chap_re.match_at_start(line) {
                chapter = crate::pyre::group(&m, 1).map(str::to_string);
            }
            if !line.starts_with(template::AGENT_MARK) {
                out.push(line.to_string());
                continue;
            }
            let ch = chapter.clone().unwrap_or_else(|| "None".into());
            let p = todo
                .iter()
                .find(|p| p["file"].as_str() == Some(name.as_str()) && chapter.as_deref().is_some_and(|c| p["section"].as_str() == Some(c)))
                .cloned()
                .unwrap_or_else(|| jd! {"blocks" => J::dict(), "excerpts_required" => 0});
            if ch == "0.1" {
                let m = if facts["measurement"].truthy() { facts["measurement"].clone() } else { J::dict() };
                let v = if !m["decode_tps_conc1"].is_null() { m["decode_tps_conc1"].clone() } else { m["lite"]["gen_tps"].clone() };
                let num = if !v.is_null() { format!("동시성 1 decode {} t/s", v.display()) } else { "판정점 수치는 03 표".into() };
                out.push(format!("픽스처 모델을 GB10 1g2n cluster(TP=2)·v0.9.0 소스빌드로 서빙했다 — {num}."));
                out.push("가장 비싼 벽은 백엔드 가드 충돌(W1)이었고 백엔드 명시를 걷어 넘었다.".into());
                continue;
            }
            let mut parts = vec![format!("픽스처 저작 — {name} §{ch}: 계보 testlog 를 읽고 이 절의 질문에 답했다.")];
            if ch == "1.2" {
                parts.push(
                    "### 60-fixture-patch.sh\n재구성 결과라 적용 미관측이다 — PLE 게이트를 완화하는 픽스처 패치(W1 과 무관 · 폐기 조건: 업스트림 정식 지원)."
                        .into(),
                );
            }
            if ch == "1.4" {
                if let Some(le) = &log_excerpt {
                    parts.push(le.clone());
                }
            }
            let blocks = p["blocks"].as_dict().cloned().unwrap_or_default();
            for (kind, need) in blocks.iter() {
                let n = if kind != "value-status" { py_int_or_zero(need).max(1) } else { 1 };
                for _ in 0..n {
                    parts.extend(event(kind));
                }
            }
            for _ in 0..py_int_or_zero(&p["excerpts_required"]) {
                parts.push(excerpt.clone());
            }
            if ch == "2.4" && blocks.contains_key("misdiagnosis") {
                parts.push(format!("> [원문] {stem} §3\n> {diag_line}"));
            }
            out.extend(parts.join("\n\n").split('\n').map(str::to_string));
        }
        write(&path, &out.join("\n"))?;
    }
    let tp = draft.join("inputs").join(TAIL_NAME);
    if !tp.is_file() {
        core::write_json(&tp, &J::list())?;
    }
    for req in facts["agent_requests"].items() {
        if req.is_dict() && req["path"].truthy() {
            let name = if req["name"].truthy() { req["name"].display() } else { util::py_basename(&req["path"].display()) };
            let why = req.get("why").cloned().unwrap_or(J::Null).display();
            write(
                &payload.join(req["path"].display()),
                &format!(
                    "{}\n{}\n",
                    template::agent_request_header_line(req)?,
                    template::_comment_line(&name, &format!("픽스처 저작 본문 — {why}"))?
                ),
            )?;
        }
    }
    if factcheck {
        _fx_factcheck(draft, st, &[])?;
    }
    Ok(())
}

/// `_fx_factcheck(draft, st, **over)` — a valid pre-publication fact-check report; `over` breaks one field at a time.
pub(super) fn _fx_factcheck(draft: &Path, st: &J, over: &[(&str, J)]) -> HR<J> {
    let mut classes: Vec<J> = template::FACTCHECK_CLASSES().items().to_vec();
    if classes.is_empty() {
        if let Some(m) = template::FACTCHECK_CLASSES().as_dict() {
            classes = m.keys().map(|k| k.parse::<i64>().map(J::from).unwrap_or_else(|_| J::from(k))).collect();
        }
    }
    classes.sort_by_key(|x| x.as_i64().unwrap_or(0));
    let mut doc = jd! {
        "schema_version" => FACTCHECK_SCHEMA, "tag" => st["tag"].clone(), "author" => "hint-author-agent", "checker" => "hint-factcheck-agent",
        "checked_utc" => _FX_CONTINUE_UTC, "claims_checked" => 12, "classes_checked" => J::List(classes),
        "items" => jl![jd! {"id" => "F1", "where" => "02-narrative.md §2.2", "claim" => "벽 W1 의 원인", "verdict" => "misleading", "class" => 5,
                           "truth" => "원인 문장에 출처가 없었다", "source" => testlog_stem(), "status" => "fixed",
                           "resolution" => "원인을 출처 문장으로 바꿨다", "target" => "prose"}],
    };
    for (k, v) in over {
        doc.set(*k, v.clone());
    }
    core::write_json(&draft.join("inputs").join(FACTCHECK_NAME), &doc)?;
    Ok(doc)
}

/// `_refs(repo, own=False)` — ref snapshot (own = local branches · tags only).
pub(super) fn _refs(repo: &Path, own: bool) -> HR<String> {
    let mut args = vec!["for-each-ref", "--format=%(refname) %(objectname)"];
    if own {
        args.extend(["refs/heads", "refs/tags"]);
    }
    Ok(_fx_git(repo, &args)?.stdout)
}

// ── CLI 호출 꼴 ────────────────────────────────────────────────────────────────────────────────
#[derive(Default, Clone)]
pub(super) struct Pub {
    pub campaign: Option<String>,
    pub cell: Option<String>,
    pub publication: Option<String>,
    pub replay: bool,
    pub generated_utc: String,
    pub out: Option<PathBuf>,
    pub remote_check: bool,
}

pub(super) fn run_publish(repo: &Path, a: &Pub, docker: &J) -> HR<J> {
    let out = a.out.as_ref().map(|p| core::posix(p));
    publish(
        repo,
        &PublishArgs {
            campaign: a.campaign.as_deref(),
            cell: a.cell.as_deref(),
            publication: a.publication.as_deref(),
            replay: a.replay,
            node: None,
            generated_utc: Some(&a.generated_utc),
            lineage_add: &[],
            out: out.as_deref(),
            remote: "origin",
            remote_check: a.remote_check,
            docker: docker.clone(),
        },
    )
}

fn campaign_pub(utc: &str, out: Option<PathBuf>) -> Pub {
    Pub { campaign: Some("c1".into()), cell: Some("c1-a".into()), generated_utc: utc.into(), out, remote_check: true, ..Default::default() }
}

#[derive(Default, Clone)]
pub(super) struct Cont {
    pub generated_utc: String,
    pub approved_by: Option<String>,
    pub approved_utc: Option<String>,
    pub remote: Option<String>,
    pub no_push: bool,
    pub factcheck_waiver: Option<String>,
    pub guide_commit: Option<String>,
    pub hint_clone: Option<PathBuf>,
}

fn cont(utc: &str) -> Cont {
    Cont { generated_utc: utc.into(), ..Default::default() }
}

pub(super) fn run_continue(repo: &Path, draft: &Path, a: &Cont, docker: &J) -> HR<J> {
    let remote = a.remote.clone().unwrap_or_else(|| "origin".into());
    continue_(
        repo,
        draft,
        &ContinueArgs {
            generated_utc: Some(&a.generated_utc),
            approved_by: a.approved_by.as_deref(),
            approved_utc: a.approved_utc.as_deref(),
            remote: &remote,
            no_push: a.no_push,
            factcheck_waiver: a.factcheck_waiver.as_deref(),
            guide_commit: a.guide_commit.as_deref(),
            docker: docker.clone(),
        },
        a.hint_clone.as_deref(),
    )
}

fn parse_args(argv: &[String]) -> HR<Args> {
    let p = build_parser();
    match quiet(|| p.parse_args(argv)) {
        Ok(ns) => Ok(Args(ns)),
        Err(code) => Err(py("SystemExit", code.to_string())),
    }
}

/// `main([...])` with stdout and stderr captured.
fn run_main(argv: &[String]) -> (i32, String, String) {
    io::capture(|| main(argv))
}

fn state_of(draft: &Path) -> HR<J> {
    _load_state(draft)
}

fn step(st: &J, name: &str) -> J {
    _step(st, name).unwrap_or(J::Null)
}

fn tuples_repr(v: &J) -> String {
    let parts: Vec<String> = v
        .items()
        .iter()
        .map(|t| {
            let items: Vec<String> = t.items().iter().map(J::repr).collect();
            if items.len() == 1 {
                format!("({},)", items[0])
            } else {
                format!("({})", items.join(", "))
            }
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

fn has_pair(v: &J, a: &str, b: &str) -> bool {
    v.items().iter().any(|t| t.at(0).as_str() == Some(a) && t.at(1).as_str() == Some(b))
}

fn blob_text(repo: &Path, anchor: &str, path: &str) -> HR<Option<String>> {
    Ok(branch::read_blob_bytes(repo, anchor, path)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
}

fn blob_json(repo: &Path, anchor: &str, path: &str) -> HR<J> {
    let t = blob_text(repo, anchor, path)?.ok_or_else(|| py("TypeError", "the JSON object must be str, bytes or bytearray, not NoneType"))?;
    crate::value::loads(&t).map_err(|e| py("JSONDecodeError", e))
}

fn fact_block(text: &str, key: &str) -> HR<String> {
    Ok(template::fact_blocks(text)?[key].display())
}

fn authored_lines(n: &str, text: &str) -> Vec<String> {
    let d = template::_Doc::parse(n, text);
    d.lines.iter().enumerate().filter(|(i, _)| !d.masked.get(*i).copied().unwrap_or(false)).map(|(_, l)| l.clone()).collect()
}

// ── 격리 E2E ───────────────────────────────────────────────────────────────────────────────────
const E2E_ENV: [&str; 5] = ["GITHUB_TOKEN", "QUANT_MODEL_PATH", "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"];

pub(super) fn _selftest_e2e(c: &mut Ck) -> HR<()> {
    let _saved = EnvGuard::pop(&E2E_ENV);
    let tdd = Tmp::new("hint-e2e-");
    let td = tdd.path().to_path_buf();
    let (repo, fx) = _fx_repo(&td)?;
    let fake = _fx_docker(&td, &fx["docker"])?;
    let rt = docker_runner(true);
    let bare = pth(&fx["bare"]);
    let guide_sha = fx["guide"].display();
    let legacy = _fx_legacy_remote(&td, &repo, &bare)?;

    // ── publish(campaign) ──
    let res = match quiet(|| run_publish(&repo, &campaign_pub(_FX_PUBLISH_UTC, None), &rt)) {
        Ok(r) => r,
        Err(e) if !e.is_py() => {
            c.ck(&format!("publish(campaign) 성공 — {}: {} → {}", e.code, h(&e.message, 600), e.remedy.clone().map_or("None".into(), |r| r)), false);
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let mut tagname = res["tag"].display();
    let draft = pth(&res["draft"]);
    let base_tag = tagname.clone();
    let payload = draft.join("payload");
    c.ck(
        &format!("publish 이름 = 도구 파생 v7 기본 이름(꼬리 전)({tagname})"),
        tagname == "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len4096-kvauto"
            && naming::parse_tag(&tagname).map(|t| t.grammar == naming::GRAMMAR_V7).unwrap_or(false),
    );
    let cands: Vec<J> = res["tail_candidates"].items().to_vec();
    let axes: std::collections::BTreeSet<String> = cands.iter().map(|x| x["axis"].display()).collect();
    let want: std::collections::BTreeSet<String> = naming::V6_TAIL_AXES.iter().map(|s| s.to_string()).collect();
    let graph_tok = cands.iter().rev().find(|x| x["axis"].as_str() == Some("graph")).map(|x| x["token"].clone()).unwrap_or(J::Null);
    let h00 = payload.join("00-hint.md");
    c.ck(
        "publish: 꼬리 후보 = 옛 뒤 3축(ple · spec · graph · 강제 ✗) · 00 §0.9 후보 표",
        axes == want && graph_tok.as_str() == Some("eager") && fact_block(&read(&h00)?, "name_tail")?.contains("`eager`"),
    );
    c.ck("publish 는 태그·hint 브랜치를 만들지 않는다", tag::tag_ref_kind(&repo, &tagname)?.is_none() && branch::hint_tip(&repo)?.is_none());
    let summ = _applied_summary(&jd! {"patches" => jl![jd! {"result" => "unobserved(적용 미관측)"}, jd! {"result" => "applied"}],
        "probes" => jl![jd! {"tier" => "image-probe", "result" => "skipped(x)"}]});
    c.ck(
        "publish 요약: 적용 판정 수 · 이미지 탐침 결과",
        util::py_eq(&summ, &jd! {"total" => 2, "unobserved" => 1, "image_probe" => "skipped(x)"})
            && res["applied"].is_dict()
            && res["applied"].get("total").map(|v| util::py_int(v)).unwrap_or(0) >= 1,
    );
    let mut outs = Vec::new();
    for ap in [
        jd! {"total" => 9, "unobserved" => 7, "image_probe" => "skipped(주입 실행기가 image_probe 를 선언하지 않았다)"},
        jd! {"total" => 9, "unobserved" => 1, "image_probe" => "done"},
    ] {
        let mut r2 = res.clone();
        r2.set("applied", ap);
        let (_, o, _) = io::capture(|| _print_publish(&repo, &r2));
        outs.push(o);
    }
    c.ck(
        "★탐침 꺼짐 + 미관측 = publish 출력이 처방(`--docker-read-only` 를 뺀다)을 말한다 · 탐침이 돌았으면 처방 줄 없음 · 옛 옵트인 처방 ✗",
        outs[0].contains("`--docker-read-only`") && outs[0].contains("미관측 7개") && !outs[1].contains("탐침이 돌지 않은") && !outs[0].contains("`--docker-probe`"),
    );
    let mut r3 = res.clone();
    r3.set("tool_snapshots", jl!["run_bench.sh@0123456789ab"]);
    let (_, bs, _) = io::capture(|| _print_publish(&repo, &r3));
    c.ck(
        "★publish 저작 안내: excerpt 출처에 `<도구>@<rev12>` · `--numbered`(§L 번호) · 스냅샷 토큰 목록(없으면 '없음')",
        bs.contains("<도구>@<rev12>")
            && bs.contains("[--numbered]")
            && bs.contains("스냅샷(발췌 출처 토큰 · 03 §3.4): run_bench.sh@0123456789ab")
            && res["tool_snapshots"].is_list()
            && res["tool_snapshots"].items().iter().all(|x| x.as_str().is_some_and(|s| s.contains('@'))),
    );
    c.ck(
        "draft = hints/.drafts/<topic>/ · state.json scaffolded",
        core::resolve(draft.parent().unwrap()) == core::resolve(&repo.join(core::REL_DRAFTS)) && state_of(&draft)?["stage"].as_str() == Some("scaffolded"),
    );
    let files: std::collections::BTreeSet<String> = strs(&res["files"]).into_iter().collect();
    let fhas = |s: &str| files.contains(s);
    c.ck("페이로드 최상위 8종", branch::PAYLOAD_ALLOWLIST_TOP.iter().all(|x| fhas(x)));
    c.ck(
        "적용된 것만: skip 패치(50)·쓰이지 않은 wheel Dockerfile 미포함 · 60·post 10 포함",
        !fhas("artifacts/build_patch_pre/50-port.sh")
            && !fhas("artifacts/build_recipe/Dockerfile")
            && fhas("artifacts/build_patch_pre/60-fixture-patch.sh")
            && fhas("artifacts/build_patch_post/10-shared.sh"),
    );
    c.ck("D-a source-build 가 COPY 하는 requirements.txt 는 싣는다", fhas("artifacts/build_recipe/requirements.txt"));
    c.ck(
        "멀티: sub_recipe.json · serve_runner.sh · cluster/interconnect 형상(AC8)",
        ["artifacts/compose/sub_recipe.json", "artifacts/compose/serve_runner.sh", "artifacts/compose/.env.cluster.template", "artifacts/compose/.env.interconnect.template"]
            .iter()
            .all(|x| fhas(x)),
    );
    let pdoc = load(&payload.join("PAYLOAD.json"))?;
    c.ck(
        "PAYLOAD schema 3 · format v7(= template.FORMAT) · naming v7 재조립 = 기본 이름",
        util::eq_int(pdoc.get("schema_version"), PAYLOAD_SCHEMA_VERSION)
            && PAYLOAD_SCHEMA_VERSION == 3
            && pdoc["format"].as_str() == Some(branch::PAYLOAD_FORMAT)
            && branch::PAYLOAD_FORMAT == template::FORMAT
            && template::FORMAT == "hint-payload/v7"
            && pdoc["naming"]["grammar"].as_str() == Some(naming::GRAMMAR_V7)
            && naming::compose_from_payload(&pdoc["naming"])? == tagname,
    );
    let mv: std::collections::BTreeSet<&str> = template::MEASUREMENT_VERDICTS.iter().copied().collect();
    let ev_mv: std::collections::BTreeSet<&str> = evidence::MEASUREMENT_VERDICTS.iter().copied().collect();
    c.ck(
        "판정 표면: PAYLOAD.measurement.verdict ∈ 3어휘 · sources.verdict · 00 FACT:grade 판정 행",
        pdoc["measurement"]["verdict"].as_str().is_some_and(|v| mv.contains(v))
            && pdoc["measurement"]["sources"].get("verdict").is_some()
            && fact_block(&read(&h00)?, "grade")?.contains("성능 판정(measurement.verdict)")
            && mv == ev_mv,
    );
    let lsum = load(&payload.join("LINEAGE.json"))?;
    let lfull = load(&draft.join("inputs").join(lineage::LINEAGE_FULL_NAME))?;
    c.ck(
        "LINEAGE: 페이로드 = 수신자 요약(경로 · 역할 · 날짜 · 발췌 수) · 전체 = draft inputs · 요약 ≤ 20KB",
        lsum["kind"].as_str() == Some(lineage::SUMMARY_KIND)
            && lsum["documents"].len() == lfull["documents"].len()
            && lfull["documents"].len() >= 1
            && !crate::value::dumps_default(&lsum).contains("via")
            && read_b(&payload.join("LINEAGE.json"))?.len() <= lineage::SUMMARY_MAX_BYTES
            && lfull.get("evidence_candidates").is_some()
            && draft.join("inputs").join(NAMING_BASE_NAME).is_file()
            && draft.join("inputs").join(TAIL_SOURCES_NAME).is_file(),
    );
    let readme0 = read(&payload.join("README.md"))?;
    c.ck(
        "README: 이 태그 이름 읽는 법(세그먼트 · native · -bare · 세대 표) · 판정 줄 · 슬롯 → 빌드 컨텍스트 요약",
        ["## 이 태그 이름 읽는 법", "`-bare` = **Docker 없이**", "| `v7` |", "| `v6` |", "**판정**", "## 슬롯 → 빌드 컨텍스트", "`artifacts/build_patch_pre/`", "`build_patches_src/`"]
            .iter()
            .all(|x| readme0.contains(x)),
    );
    let snap = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
    let new_keys = ["measurement_env_observed", "tool_snapshots", "attestation_scope"];
    let mut fact_ok = true;
    for (fid, doc) in [("measurement_env", "01-artifacts.md"), ("measurement_env_nodes", "01-artifacts.md"), ("tool_snapshots", "03-benchmark.md")] {
        fact_ok &= read(&payload.join(doc))?.contains(&format!("<!-- FACT:{fid} -->"));
    }
    c.ck(
        "R3 facts · PAYLOAD: measurement_env_observed · tool_snapshots(list) · attestation_scope 키 — 같은 값",
        new_keys.iter().all(|k| snap.get(k).is_some() && pdoc.get(k).is_some() && util::py_eq(&snap[*k], &pdoc[*k]))
            && snap["measurement_env_observed"].is_list()
            && snap["tool_snapshots"].is_list()
            && fact_ok,
    );
    let t03_pub = read(&payload.join("03-benchmark.md"))?;
    let mc_keys = |v: &J| -> std::collections::BTreeSet<String> { v.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default() };
    let mut want_mc: std::collections::BTreeSet<String> = evidence::DISTRIBUTED_MEASUREMENT_KEYS.iter().map(|s| s.to_string()).collect();
    want_mc.insert("source".into());
    c.ck(
        "full 셀: PAYLOAD/facts cell_grade = full · dataset_provenance(부재 추론 synthetic) · 03 §3.3 FACT:dataset_provenance(`###`)",
        pdoc["cell_grade"].as_str() == Some("full")
            && snap["cell_grade"].as_str() == Some("full")
            && pdoc["dataset_provenance"]["status"].as_str() == Some(evidence::DATASET_STATUS_INFERRED)
            && util::py_eq(&pdoc["dataset_provenance"], &snap["dataset_provenance"])
            && t03_pub.contains("<!-- FACT:dataset_provenance -->")
            && fact_block(&t03_pub, "dataset_provenance")?.starts_with("### 데이터셋 출처")
            && mc_keys(&pdoc["measurement_config"]) == want_mc,
    );
    let vobs = if pdoc["naming"]["vllm_observed"].truthy() { pdoc["naming"]["vllm_observed"].clone() } else { J::dict() };
    c.ck(
        "R3 vllm_observed = 키별 생산자(엔진 자기보고 · 인증서 · wheel 각각 출처 칸)",
        ["engine_self_report", "certificate_vllm_version", "wheel_meta"].iter().all(|k| vobs.get(&format!("{k}_source")).is_some()),
    );
    c.ck(
        "D-f measurement = 수치 · measurement_config = 배포 키 + source",
        pdoc["measurement"].get("decode_tps_conc1").is_some() && mc_keys(&pdoc["measurement_config"]) == want_mc,
    );
    c.ck(
        "D-b value-status 후보에서 model 제외",
        !snap["value_status_candidates"].items().iter().any(|x| x["knob"].as_str() == Some("model")),
    );
    c.ck("PROVENANCE 앵커는 커밋 전까지 비어 있다", load(&payload.join("PROVENANCE.json"))?["source_anchor"].is_null());
    c.ck(
        "docker 는 읽기(inspect·history)와 원장 cat(--network none)만",
        fake.calls().iter().all(|cl| {
            let a1 = cl.first().map(String::as_str).unwrap_or("");
            a1 == "image" || a1 == "history" || (a1 == "run" && cl.iter().any(|x| x == "--network") && cl.iter().any(|x| x == "none"))
        }),
    );

    // ── ★배포 PII 에 걸리는 파생 이름 = publish 에서 멈춘다 ──
    let ipish = ["0", "10", "1", "1"].join(".");
    let pii_tag = tagname.replace("/0.9.0/", &format!("/{ipish}/"));
    let (ok1, _) = _raises(|| _require_name_pii_clean(&repo, &pii_tag), "HINT_TAG_NAME_PII")?;
    let (_, w2) = _raises(|| _require_name_pii_clean(&repo, &tagname), "HINT_TAG_NAME_PII")?;
    c.ck("★4마디 릴리스가 사설 IPv4 모양이면 HINT_TAG_NAME_PII(저작 전에 멈춘다) · 정상 이름은 통과", ok1 && w2 == "예외 없음");
    let tree_p = _fx_tree(&repo);
    let pt = pii_tag.clone();
    let dn_hook: super::hooks::DeriveNameFn = Rc::new(move |f: &J, v: &J| {
        let mut d = naming::derive_name(f, v)?;
        d.tag = pt.clone();
        Ok(d)
    });
    let (ok, why) = super::hooks::with(
        |hk| hk.derive_name = Some(dn_hook.clone()),
        || _raises(|| run_publish(&repo, &campaign_pub(_FX_PUBLISH_UTC, Some(td.join("namepii"))), &rt), "HINT_TAG_NAME_PII"),
    )?;
    c.ck(
        &format!("★publish 가 이름 PII 를 발행기 구동 전에 막는다(쓰기 0)({})", h(&why, 60)),
        ok && _fx_tree(&repo) == tree_p && _fx_tree(&td.join("namepii")).is_empty(),
    );

    // ── ★끊긴 publish 를 다른 시각으로 다시 = 발행기 구동 전에 막힌다 ──
    let topic = res["topic"].display();
    let rec_p = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json"));
    let (rec0, tree0) = (read_b(&rec_p)?, _fx_tree(&repo));
    let (ok, why) = _raises(|| run_publish(&repo, &campaign_pub("2026-01-02T06:05:00Z", Some(td.join("timebound"))), &rt), "HINT_PUBLICATION_TIME_BOUND")?;
    c.ck(
        &format!("★발행 기록이 다른 시각으로 묶임 = HINT_PUBLICATION_TIME_BOUND · 쓰기 0({})", h(&why, 90)),
        ok && read_b(&rec_p)? == rec0 && _fx_tree(&repo) == tree0 && why.contains(_FX_PUBLISH_UTC) && _fx_tree(&td.join("timebound")).is_empty(),
    );

    // ── ★재생은 draft 밖에 쓰지 않는다 ──
    let (before, refs0) = (_fx_tree(&repo), _refs(&repo, false)?);
    let rout = td.join("replay");
    let replay_pub = |out: PathBuf, utc: &str| Pub {
        publication: Some(topic.clone()),
        replay: true,
        generated_utc: utc.into(),
        out: Some(out),
        remote_check: true,
        ..Default::default()
    };
    let rres = match quiet(|| run_publish(&repo, &replay_pub(rout.clone(), _FX_REPLAY_UTC), &rt)) {
        Ok(r) => Some(r),
        Err(e) if !e.is_py() => {
            c.ck(&format!("재생 publish 성공 — {}: {}", e.code, h(&e.message, 400)), false);
            None
        }
        Err(e) => return Err(e),
    };
    let rok = rres.is_some();
    if let Some(rres) = &rres {
        c.ck("★재생 = 저장소 트리·ref 불변(쓰기는 --out draft 안뿐)", _fx_tree(&repo) == before && _refs(&repo, false)? == refs0);
        c.ck(
            "재생 = 같은 셀 → 같은 이름 · 모드 publication-replay",
            rres["tag"].as_str() == Some(tagname.as_str()) && rres["mode"].as_str() == Some("publication-replay") && rout.join(STATE_NAME).is_file(),
        );
    }
    let mut no_flag = replay_pub(td.join("r2"), _FX_REPLAY_UTC);
    no_flag.replay = false;
    c.ck("★--replay 없는 --publication 거부", _raises(|| run_publish(&repo, &no_flag, &rt), "HINT_REPLAY_FLAG_REQUIRED")?.0);

    // ── ★승인 부재 = 어느 ref 도 움직이지 않는다 ──
    let decl_p = repo.join("campaigns/c1/campaign.yaml");
    let decl_text = read(&decl_p)?;
    let mut decl = fx["decl"].clone();
    decl.set("hint_targets", J::list());
    core::write_json(&decl_p, &decl)?;
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_APPROVAL_ABSENT")?;
    c.ck(&format!("★승인 부재 = HINT_APPROVAL_ABSENT({})", h(&why, 120)), ok);
    c.ck("★승인 부재 = ref 쓰기 0", _refs(&repo, false)? == refs0 && branch::hint_tip(&repo)?.is_none());
    write(&decl_p, &decl_text)?;
    let mut cli_appr = cont(_FX_CONTINUE_UTC);
    cli_appr.approved_by = Some("x".into());
    cli_appr.approved_utc = Some(_FX_CONTINUE_UTC.into());
    c.ck("★캠페인 셀에 CLI 승인 = 원천 충돌", _raises(|| run_continue(&repo, &draft, &cli_appr, &rt), "HINT_APPROVAL_SOURCE_CONFLICT")?.0);

    // ── ★린트 실패(미저작) = 커밋 전 차단 ──
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_LINT_FAILED")?;
    c.ck(&format!("★미저작 = HINT_LINT_FAILED(커밋 전) — {}", h(&why, 160)), ok);
    c.ck("★린트 실패 = hint 브랜치·태그 없음", branch::hint_tip(&repo)?.is_none() && tag::tag_ref_kind(&repo, &tagname)?.is_none());
    let lc = codes(&quiet(|| lint_draft(&repo, &draft))?);
    c.ck("lint 도 미저작을 잡는다(부수효과 0)", lc.contains("HINT_AGENT_PLACEHOLDER_RESIDUE") && lc.contains("HINT_SECTION_UNAUTHORED"));

    // ── 저작(프로그램) → lint 0 ──
    _fx_author(&repo, &draft, &state_of(&draft)?, true)?;
    let issues = quiet(|| lint_draft(&repo, &draft))?;
    let shown: Vec<String> = issues
        .iter()
        .take(4)
        .map(|i| format!("({}, {}, {})", i["code"].repr(), i["file"].repr(), crate::value::py_repr(&h(&i["message"].display(), 80))))
        .collect();
    c.ck(&format!("저작 뒤 lint 0건: [{}]", shown.join(", ")), issues.is_empty());

    // ── 이름 꼬리 저작 ──
    let yaml_rel = "output/multi/configs/c1-a.yaml";
    let tail_tok = "eager";
    let good = |token: &str, meaning: &str, file: &str, value: &str| -> J {
        jd! {"token" => token, "meaning" => meaning, "evidence" => jd! {"file" => file, "key" => "enforce-eager", "value" => value}}
    };
    let gm = "CUDA graph 대신 eager 실행(enforce-eager) — 같은 q·len·kv 의 graph 셀과 가른다";
    let good_tail = jl![good(tail_tok, gm, yaml_rel, "true")];
    let tp = draft.join("inputs").join(TAIL_NAME);
    let tail_codes = |doc: J| -> HR<std::collections::BTreeSet<String>> {
        core::write_json(&tp, &doc)?;
        Ok(codes(&quiet(|| lint_draft(&repo, &draft))?))
    };
    c.ck("★꼬리 근거 값 불일치(true → false) = lint HINT_TAIL_UNGROUNDED", tail_codes(jl![good(tail_tok, gm, yaml_rel, "false")])?.contains("HINT_TAIL_UNGROUNDED"));
    c.ck(
        "★꼬리 근거 파일이 이 셀 서빙 설정 밖 = HINT_TAIL_UNGROUNDED",
        tail_codes(jl![good(tail_tok, gm, "README.fixture", "true")])?.contains("HINT_TAIL_UNGROUNDED"),
    );
    c.ck("★꼬리 뜻 없음 = HINT_TAIL_MEANING_ABSENT", tail_codes(jl![good(tail_tok, " ", yaml_rel, "true")])?.contains("HINT_TAIL_MEANING_ABSENT"));
    c.ck(
        "★꼬리가 v6 뒤 3축 모양 그대로(ple…-spec…-eager) = HINT_TAIL_FORMAT",
        tail_codes(J::List(["plemmap", "spec2", "eager"].iter().map(|t| good(t, gm, yaml_rel, "true")).collect()))?.contains("HINT_TAIL_FORMAT"),
    );
    c.ck("★timestamp 모양 토큰(t + 숫자 10자리) = HINT_TAIL_FORMAT", tail_codes(jl![good("t2601021530", gm, yaml_rel, "true")])?.contains("HINT_TAIL_FORMAT"));
    unlink(&tp)?;
    c.ck("★꼬리 파일 없음 = lint HINT_TAIL_ABSENT(빈 꼬리도 [] 로 명시)", codes(&quiet(|| lint_draft(&repo, &draft))?).contains("HINT_TAIL_ABSENT"));
    let refs_t = _refs(&repo, false)?;
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_TAIL_ABSENT")?;
    c.ck(
        &format!("★꼬리 없음 = continue HINT_TAIL_ABSENT · ref 쓰기 0 · 이름 불변({})", h(&why, 50)),
        ok && _refs(&repo, false)? == refs_t && state_of(&draft)?["tag"].as_str() == Some(base_tag.as_str()),
    );
    core::write_json(&tp, &good_tail)?;
    c.ck("유효 꼬리(근거 = 서빙 yaml enforce-eager) = lint 0", quiet(|| lint_draft(&repo, &draft))?.is_empty());

    // ── refacts ──
    let keep: Vec<(&str, Vec<u8>)> = vec![("02-narrative.md", read_b(&payload.join("02-narrative.md"))?), ("01-artifacts.md", read_b(&payload.join("01-artifacts.md"))?)];
    let keep_in: Vec<(&str, Vec<u8>)> =
        vec![(TAIL_NAME, read_b(&draft.join("inputs").join(TAIL_NAME))?), (FACTCHECK_NAME, read_b(&draft.join("inputs").join(FACTCHECK_NAME))?)];
    let tl_p = repo.join(_FX_TESTLOG);
    write(&tl_p, &(read(&tl_p)? + "\n## 5. 추가\nrefacts 재파생 시험 줄.\n"))?;
    let rx = quiet(|| refacts(&repo, &draft, &rt))?;
    let rx_lint: Vec<String> = quiet(|| lint_draft(&repo, &draft))?.iter().map(|i| i["code"].display()).collect();
    let rx_full = _lineage_full(&draft, &payload)?;
    let rx_counts = template::cited_sources(&repo, &payload, &rx_full, Some(&draft))?["excerpt_counts"].clone();
    let rx_summary = load(&payload.join("LINEAGE.json"))?;
    c.ck(
        "refacts refreshes actual resolved excerpt counts without sealing sources",
        rx_counts.truthy()
            && util::py_eq(&rx_summary, &lineage::receiver_summary(&rx_full, &rx_counts, &J::Null)?)
            && rx_summary.get(lineage::SEALED_SOURCES_KEY).is_none(),
    );
    let st_now = state_of(&draft)?;
    c.ck(
        "refacts(campaign) = 서사 재바인딩 경로를 탄다 — 바인딩 = 캠페인 포인터면 no-op · 같은 manifest 재생성",
        util::py_eq(&rx["rebind"], &jd! {"rebound" => J::list(), "manifest" => st_now["manifest"].clone()})
            && util::py_eq(&st_now["steps"]["refacts"]["narratives_rebound"], &J::list())
            && st_now["steps"]["refacts"].get("narratives_rebound").is_some(),
    );
    c.ck(
        &format!("refacts: 바뀐 FACT(계보 크기) = diff 출력 · 같은 generated_utc({})", tuples_repr(&rx["changed"])),
        has_pair(&rx["changed"], "02-narrative.md", "lineage") && rx["diff"].display().contains("(재파생)") && st_now["generated_utc"].as_str() == Some(_FX_PUBLISH_UTC),
    );
    let mut auth_ok = true;
    for (n, b) in &keep {
        auth_ok &= authored_lines(n, &read(&payload.join(n))?) == authored_lines(n, &String::from_utf8_lossy(b));
    }
    c.ck("refacts: 저작 산문 · 발췌 · hint-event 보존(주석 · 사실 블록 밖 줄 불변)", auth_ok);
    let mut kin_ok = true;
    for (n, b) in &keep_in {
        kin_ok &= read_b(&draft.join("inputs").join(n))? == *b;
    }
    c.ck("refacts: inputs/tail.json · factcheck.json 보존", kin_ok);
    c.ck(&format!("refacts 뒤 lint 0({})", util::list_repr(&rx_lint.iter().take(4).collect::<Vec<_>>())), rx_lint.is_empty());
    let rx2 = quiet(|| refacts(&repo, &draft, &rt))?;
    c.ck(
        "refacts 멱등(두 번째 = 바뀐 FACT 없음 · 산출물 추가 · 제거 0)",
        rx2["changed"].items().is_empty() && !rx2["artifacts"]["added"].truthy() && !rx2["artifacts"]["removed"].truthy(),
    );
    let p01 = payload.join("01-artifacts.md");
    let original = read(&p01)?;
    let intro = template::slot_intro(&original)?.1;
    let legacy_i = template::_LEGACY_SLOT_INTRO;
    let authored = format!("\n저작 인용:\n{legacy_i}\n```\n{legacy_i}\n```\n");
    write(&p01, &(original.replacen(&intro, legacy_i, 1) + &authored))?;
    let rx_intro = quiet(|| refacts(&repo, &draft, &rt))?;
    let migrated = read(&p01)?;
    c.ck(
        "public refacts reports fixed introduction migration and preserves quoted/fenced legacy text",
        has_pair(&rx_intro["changed"], "01-artifacts.md", "fixed-introduction")
            && rx_intro["diff"].display().contains(&intro)
            && rx_intro["diff"].display().contains(legacy_i)
            && migrated.ends_with(&authored)
            && template::slot_intro(&migrated)?.1 == intro,
    );
    let rx_intro2 = quiet(|| refacts(&repo, &draft, &rt))?;
    c.ck("public fixed introduction migration is idempotent", !has_pair(&rx_intro2["changed"], "01-artifacts.md", "fixed-introduction"));
    write(&p01, &original)?;
    let stem = testlog_stem();
    let ex = excerpt_text(&repo, &draft, &stem, Some("7-8"), false)?;
    c.ck("excerpt 도우미 --lines(치환 후 원문 · 1-기반)", ex.contains(_FX_WALL_SIG) && ex.matches('\n').count() == 1);
    let (ok, why) = _raises(|| excerpt_text(&repo, &draft, &stem, Some("9999-9999"), false), "HINT_EXCERPT_LINES_OUT_OF_RANGE")?;
    c.ck(&format!("★excerpt --lines 범위 밖 = HINT_EXCERPT_LINES_OUT_OF_RANGE(빈 출력으로 조용히 돌려주지 않는다)({})", h(&why, 50)), ok);

    // ── F8: 출처 정규화 · NIC 치환 · 비-마크다운 §L 표지 ──
    let ln = log_name();
    let norm_log = excerpt_text(&repo, &draft, &ln, None, false)?;
    let raw = read_b(&repo.join(_FX_LOG_REL))?;
    let raw_rows: Vec<&[u8]> = raw.split(|b| *b == b'\n').collect();
    let att = raw_rows.iter().position(|r| r.starts_with(b"AttributeError")).ok_or_else(|| py("StopIteration", ""))?;
    let nrows: Vec<&str> = norm_log.split('\n').collect();
    c.ck(
        "excerpt 줄 번호 = 원본 `\\n` 줄 번호(grep -n) · ANSI ✗ · `\\r` 조각은 마지막만",
        nrows.len() == raw_rows.len()
            && nrows.get(att).is_some_and(|r| r.starts_with("AttributeError"))
            && !norm_log.contains('\x1b')
            && !norm_log.contains('\r')
            && !norm_log.contains("Loading 50%")
            && excerpt_text(&repo, &draft, &ln, Some(&format!("{}-{}", att + 1, att + 1)), false)?.starts_with("AttributeError"),
    );
    let num_all: Vec<String> = excerpt_text(&repo, &draft, &ln, None, true)?.split('\n').map(str::to_string).collect();
    let rng = format!("{}-{}", att + 1, att + 2);
    let num_rng: Vec<String> = excerpt_text(&repo, &draft, &ln, Some(&rng), true)?.split('\n').map(str::to_string).collect();
    let plain_rng: Vec<String> = excerpt_text(&repo, &draft, &ln, Some(&rng), false)?.split('\n').map(str::to_string).collect();
    let stripped: Vec<String> = num_rng.iter().map(|r| r.split_once('\t').map(|x| x.1.to_string()).unwrap_or_default()).collect();
    c.ck(
        "excerpt --numbered: `L<n>\\t` = grep -n 번호 · --lines 와 함께면 a 부터 · 번호 뗀 본문 = 번호 없는 출력",
        num_all.get(att).is_some_and(|r| *r == format!("L{}\t{}", att + 1, nrows[att]))
            && num_rng.first().is_some_and(|r| r.starts_with(&format!("L{}\tAttributeError", att + 1)))
            && num_rng.get(1).is_some_and(|r| r.starts_with(&format!("L{}\t", att + 2)))
            && stripped == plain_rng,
    );
    let one = format!("{}-{}", att + 1, att + 1);
    let (rc_n, out_n, _) = run_main(&sv(&["--repo", &core::posix(&repo), "excerpt", "--draft", &core::posix(&draft), "--source", &ln, "--numbered", "--lines", &one]));
    c.ck(
        "★excerpt --numbered CLI 배선(플래그가 핸들러까지 간다 · 번호 없는 출력과 다르다)",
        rc_n == 0 && out_n.starts_with(&format!("L{}\t", att + 1)) && !excerpt_text(&repo, &draft, &ln, None, false)?.starts_with("L1\t"),
    );
    c.ck(
        "excerpt NIC 치환: manifest 최상위 interconnect 장치 이름 → <nic:cluster>(두 노드 공통 · <nic:main> ✗)",
        norm_log.contains("Using [0]<nic:cluster>:1/RoCE ; OOB <nic:cluster>") && !norm_log.contains(_FX_NIC_HCA) && !norm_log.contains(_FX_NIC_IFACE),
    );
    c.ck("비-마크다운 §L 발췌가 실린 저작 = lint 0(위 판정에 포함)", read(&payload.join("01-artifacts.md"))?.contains(&format!("> [원문] {ln} §L")));

    let lint_variant = |tg: &str, old: &str, new: &str| -> HR<std::collections::BTreeSet<String>> {
        let vd = td.join("lintvar").join(tg);
        copytree(&draft, &vd, &[])?;
        let f = vd.join("payload").join("01-artifacts.md");
        let txt = read(&f)?;
        if !txt.contains(old) {
            return Err(py("RuntimeError", format!("변형 앵커 없음: {}", crate::value::py_repr(&h(old, 40)))));
        }
        write(&f, &txt.replacen(old, new, 1))?;
        Ok(codes(&quiet(|| lint_draft(&repo, &vd))?))
    };
    let head = read(&payload.join("01-artifacts.md"))?
        .split('\n')
        .find(|l| l.starts_with(&format!("> [원문] {ln} §L")))
        .map(str::to_string)
        .ok_or_else(|| py("StopIteration", ""))?;
    c.ck(
        "★E2E: 로그 발췌 머리를 제목식(§엔진 로그)으로 = HINT_EXCERPT_SECTION_SHAPE",
        lint_variant("shape", &head, &format!("> [원문] {ln} §엔진 로그"))?.contains("HINT_EXCERPT_SECTION_SHAPE"),
    );
    c.ck(
        "★E2E: 로그 발췌 줄 범위가 인용 밖(§L1-2) = HINT_EXCERPT_OUTSIDE_LINES",
        lint_variant("outside", &head, &format!("> [원문] {ln} §L1-2"))?.contains("HINT_EXCERPT_OUTSIDE_LINES"),
    );
    c.ck(
        "★E2E: ANSI 를 되살린 인용 = HINT_EXCERPT_MISMATCH",
        lint_variant("ansi", "> Loading 100%", "> \x1b[36mLoading 100%\x1b[0m")?.contains("HINT_EXCERPT_MISMATCH"),
    );

    // ── R3: 측정 도구 스냅샷 발췌 ──
    let snap_bytes = "#!/bin/bash\nvllm bench serve --ignore-eos --temperature 0\n";
    let blob_src = td.join("snap_blob.sh");
    write(&blob_src, snap_bytes)?;
    let blob = _fx_git(&repo, &["hash-object", "-w", &core::posix(&blob_src)])?.stdout.trim().to_string();
    let tree = core::git_full(&repo, &["mktree"], true, Some(&format!("100644 blob {blob}\trun_bench.sh\n")), &[])?.stdout.trim().to_string();
    let snap_commit = _fx_git(&repo, &["commit-tree", &tree, "-m", "fixture tool snapshot origin"])?.stdout.trim().to_string();
    let snap_name = format!("run_bench.sh@{}", h(&snap_commit, 12));
    let snapshot_draft = |tg: &str, listed: bool, body: &str| -> HR<PathBuf> {
        let vd = td.join("snapvar").join(tg);
        copytree(&draft, &vd, &[])?;
        mkdirs(&vd.join("inputs").join("sources"))?;
        write(&vd.join("inputs").join("sources").join(&snap_name), body)?;
        if listed {
            let lp = vd.join("inputs").join(lineage::LINEAGE_FULL_NAME);
            let mut lin_doc = load(&lp)?;
            let ent = jd! {"path" => format!("inputs/sources/{snap_name}"), "kind" => "tool-source@rev", "origin" => format!("git:{snap_commit}:run_bench.sh")};
            match lin_doc.get_mut("evidence_candidates") {
                Some(J::List(l)) => l.push(ent),
                _ => lin_doc.set("evidence_candidates", jl![ent]),
            }
            write(&lp, &crate::value::dumps_pretty(&lin_doc))?;
        }
        let quote = body.split('\n').nth(1).unwrap_or("").to_string();
        let f = vd.join("payload").join("01-artifacts.md");
        write(&f, &read(&f)?.replacen(&head, &format!("> [원문] {snap_name} §L2-2\n> {quote}\n\n{head}"), 1))?;
        Ok(vd)
    };
    let refs_snap = _refs(&repo, false)?;
    let sd = snapshot_draft("listed", true, snap_bytes)?;
    let sd_codes = codes(&quiet(|| lint_draft(&repo, &sd))?);
    c.ck(
        &format!(
            "R3 E2E: 스냅샷 발췌(`<도구>@<rev12>` §L · origin = 실재 커밋) = lint_draft(임시 사본) 통과({})",
            util::list_repr(&sd_codes.iter().collect::<Vec<_>>())
        ),
        sd_codes.is_empty() && snap_commit.len() == 40,
    );
    c.ck(
        "R3 E2E: excerpt 도우미도 스냅샷을 draft 에서 읽는다",
        excerpt_text(&repo, &sd, &snap_name, Some("2-2"), false)? == "vllm bench serve --ignore-eos --temperature 0",
    );
    let unl = snapshot_draft("unlisted", false, snap_bytes)?;
    c.ck(
        "★R3 E2E: LINEAGE 에 등재하지 않은 스냅샷 = HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE",
        codes(&quiet(|| lint_draft(&repo, &unl))?).contains("HINT_EXCERPT_SOURCE_OUTSIDE_LINEAGE"),
    );
    let tampered = snapshot_draft("tampered", true, &snap_bytes.replace("--temperature 0", "--temperature 1"))?;
    c.ck(
        "★R3 E2E: 스냅샷 파일을 고치고 발췌를 거기 맞추면 = HINT_EXCERPT_SNAPSHOT_DRIFT(실물 git show 대조) · excerpt 도 거부",
        codes(&quiet(|| lint_draft(&repo, &tampered))?).contains("HINT_EXCERPT_SNAPSHOT_DRIFT")
            && _raises(|| excerpt_text(&repo, &tampered, &snap_name, None, false), "HINT_EXCERPT_SNAPSHOT_DRIFT")?.0,
    );
    c.ck("R3 E2E: 픽스처 origin 커밋은 ref 를 움직이지 않았다(매달린 커밋)", _refs(&repo, false)? == refs_snap);

    // ── F10: refresh ──
    let before00 = read(&h00)?;
    let rc_r = quiet(|| main(&sv(&["--repo", &core::posix(&repo), "refresh", "--draft", &core::posix(&draft)])));
    let after00 = read(&h00)?;
    let after_wall = after00.split("<!-- FACT:wall_map -->").next().unwrap_or("");
    let before_wall = before00.split("<!-- FACT:wall_map -->").next().unwrap_or("");
    let knob_seg = before00.split("<!-- FACT:knob_status -->").nth(1).map(|s| h(s, 40)).unwrap_or_default();
    c.ck(
        "CLI refresh rc 0 · 00 §0.3 벽 지도 · §0.5 노브 요약이 저작 내용으로 채워진다(그 밖 바이트 불변)",
        rc_r == 0 && fact_block(&after00, "wall_map")?.contains("W1") && !knob_seg.contains("`V1`") && before_wall == after_wall,
    );
    c.ck("refresh 멱등(두 번째 = 바뀐 블록 없음) · refresh 뒤에도 lint 0", refresh_draft(&repo, &draft)?.is_empty() && quiet(|| lint_draft(&repo, &draft))?.is_empty());
    let refresh_copy = td.join("lineage-refresh-copy");
    copytree(&draft, &refresh_copy, &[])?;
    let refresh_payload = refresh_copy.join("payload");
    let narrative = refresh_payload.join("02-narrative.md");
    let before_counts =
        template::cited_sources(&repo, &refresh_payload, &_lineage_full(&refresh_copy, &refresh_payload)?, Some(&refresh_copy))?["excerpt_counts"].clone();
    let source_line = excerpt_text(&repo, &refresh_copy, &stem, Some("1-1"), false)?;
    let block = format!("\n\n> [원문] {stem} §L1-1\n> {source_line}\n");
    write(&narrative, &(read(&narrative)? + &block.repeat(4)))?;
    let changed_summary = refresh_draft(&repo, &refresh_copy)?;
    let updated = load(&refresh_payload.join("LINEAGE.json"))?;
    let updated_counts =
        template::cited_sources(&repo, &refresh_payload, &_lineage_full(&refresh_copy, &refresh_payload)?, Some(&refresh_copy))?["excerpt_counts"].clone();
    let testlog_row = updated["documents"].items().iter().find(|x| x["path"].as_str() == Some(_FX_TESTLOG)).cloned().unwrap_or(J::dict());
    let cnt = |v: &J| v.get(_FX_TESTLOG).map(util::py_int).unwrap_or(0);
    c.ck(
        "refresh counts four newly authored excerpts without sealing or weakening source checks",
        changed_summary.iter().any(|x| x == "LINEAGE.json")
            && cnt(&updated_counts) == cnt(&before_counts) + 4
            && util::py_eq(&testlog_row["excerpts"], &updated_counts[_FX_TESTLOG])
            && updated.get(lineage::SEALED_SOURCES_KEY).is_none()
            && refresh_draft(&repo, &refresh_copy)?.is_empty(),
    );
    let nd = core::posix(&td.join("no-draft"));
    let ns = parse_args(&sv(&["--repo", &core::posix(&repo), "refresh", "--draft", &nd]))?;
    let (ok, why) = _raises(|| cmd_refresh(&ns), "HINT_DRAFT_ABSENT")?;
    c.ck(&format!("★refresh: publish 된 draft 가 아니면 HINT_DRAFT_ABSENT({})", h(&why, 60)), ok);

    // ── F11: 발행 전 독립 사실 검증 게이트 ──
    let fcp = draft.join("inputs").join(FACTCHECK_NAME);
    c.ck("저작 대역은 독립 검증 대역의 보고까지 남긴다(author ≠ checker 역할 이름)", fcp.is_file());
    unlink(&fcp)?;
    let refs_fc = _refs(&repo, false)?;
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_FACTCHECK_ABSENT")?;
    let st_fc = state_of(&draft)?;
    c.ck(
        &format!("★사실 검증 보고 없음 = HINT_FACTCHECK_ABSENT · ref 쓰기 0 · 봉인 전({})", h(&why, 70)),
        ok && _refs(&repo, false)? == refs_fc
            && branch::hint_tip(&repo)?.is_none()
            && read(&h00)?.contains("<!-- PROMPT")
            && step(&st_fc, "factcheck")["ok"] == J::Bool(false)
            && step(&st_fc, "lint")["ok"] == J::Bool(true)
            && step(&st_fc, "lint")["sealed"] == J::Bool(false),
    );
    c.ck("사실 검증 미통과 상태가 00 메타에 기계 기재(미통과 · 사유코드)", fact_block(&read(&h00)?, "meta")?.contains("미통과(`HINT_FACTCHECK_ABSENT`)"));
    let st0 = state_of(&draft)?;
    let item = |verdict: &str, class: i64, status: &str, extra: &[(&str, &str)]| -> J {
        let mut d = jd! {"id" => "F1", "where" => "w", "claim" => "c", "verdict" => verdict, "class" => class, "status" => status};
        for (k, v) in extra {
            d.set(*k, *v);
        }
        d
    };
    let bad_cases: Vec<(&str, (&str, J), &str)> = vec![
        ("checker == author", ("checker", js("hint-author-agent")), "HINT_FACTCHECK_INVALID"),
        ("7부류 중 하나 빠짐", ("classes_checked", jl![1, 2, 3, 4, 5, 6]), "HINT_FACTCHECK_INVALID"),
        ("다른 draft 의 태그", ("tag", js("hint/x/y/z/w")), "HINT_FACTCHECK_INVALID"),
        ("fixed 인데 resolution 없음", ("items", jl![item("wrong", 1, "fixed", &[("target", "prose")])]), "HINT_FACTCHECK_INVALID"),
        ("열린 오답", ("items", jl![item("wrong", 7, "open", &[("target", "prose")])]), "HINT_FACTCHECK_OPEN"),
        ("이견(disputed)도 열림", ("items", jl![item("misleading", 6, "disputed", &[("target", "fact")])]), "HINT_FACTCHECK_OPEN"),
        ("근거 없음(unsupported)도 막는다", ("items", jl![item("unsupported", 5, "open", &[("target", "prose")])]), "HINT_FACTCHECK_OPEN"),
        ("지적 대상(target) 없음", ("items", jl![item("wrong", 7, "fixed", &[("resolution", "r")])]), "HINT_FACTCHECK_INVALID"),
    ];
    for (label, (k, v), code) in bad_cases {
        _fx_factcheck(&draft, &st0, &[(k, v)])?;
        let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), code)?;
        c.ck(&format!("★사실 검증 {label} = {code} · ref 쓰기 0({})", h(&why, 60)), ok && _refs(&repo, false)? == refs_fc);
    }
    let st_w = state_of(&draft)?;
    let mut wv = cont(_FX_CONTINUE_UTC);
    wv.factcheck_waiver = Some("<<FILL>>".into());
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &wv, &rt), "HINT_FACTCHECK_WAIVER_INVALID")?;
    c.ck(
        &format!("★사실 검증 면제 자리표시 = HINT_FACTCHECK_WAIVER_INVALID · 쓰기 전 거부(state 불변)({})", h(&why, 50)),
        ok && util::py_eq(&state_of(&draft)?, &st_w) && _refs(&repo, false)? == refs_fc,
    );
    _fx_factcheck(&draft, &st0, &[])?;

    // ── ★footer 에 묶을 계측 산출물이 없으면 커밋 전에 막힌다 ──
    let none_bind: super::hooks::BindingPathFn = Rc::new(|_, _| Ok(None));
    let (ok, why) = super::hooks::with(
        |hk| hk.binding_artifact_path = Some(none_bind.clone()),
        || _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_CERTIFICATE_BINDING_ABSENT"),
    )?;
    c.ck(
        &format!("★계측 산출물 바인딩 없음 = 커밋 전 차단 · hint 브랜치 없음({})", h(&why, 80)),
        ok && branch::hint_tip(&repo)?.is_none() && _step(&state_of(&draft)?, "commit").is_none(),
    );

    // ── ★커밋 직후 끊김 → 새 시각 재실행도 같은 커밋으로 재개 ──
    let crashed_sha: Rc<std::cell::RefCell<Vec<String>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let cs = crashed_sha.clone();
    let crash: super::hooks::CommitPayloadFn = Rc::new(move |r, p, m, u, g, rm| {
        cs.borrow_mut().push(branch::commit_payload(r, p, m, u, g, rm)?);
        Err(py("RuntimeError", "fixture: 커밋 직후 끊김"))
    });
    let mut nc = cont("2026-01-02T06:55:00Z");
    nc.no_push = true;
    let crashed = super::hooks::with(
        |hk| hk.commit_payload = Some(crash.clone()),
        || match quiet(|| run_continue(&repo, &draft, &nc, &rt)) {
            Ok(_) => Ok(false),
            Err(e) if e.code == "py:RuntimeError" => Ok(true),
            Err(e) => Err(e),
        },
    )?;
    let crash_tip = crashed_sha.borrow().first().cloned();
    let stc = state_of(&draft)?;
    c.ck(
        "커밋 직후 끊김 픽스처: 커밋은 됐고 상태엔 앵커가 없다(커밋 시각 · 안내 커밋만 먼저 적혔다)",
        crashed
            && crash_tip.is_some()
            && _step(&stc, "commit").is_none()
            && stc["commit_utc"].as_str() == Some("2026-01-02T06:55:00Z")
            && stc["guide"]["sha"].as_str() == Some(guide_sha.as_str()),
    );
    tagname = stc["tag"].display();
    c.ck(
        &format!("이름 확정: 기본 이름 + 저작 꼬리(`{tail_tok}` · 근거 대조 통과 · 중복 없음 → timestamp 없음)({tagname})"),
        tagname == format!("{base_tag}-{tail_tok}")
            && step(&stc, "name")["timestamp"].is_null()
            && load(&payload.join("PAYLOAD.json"))?["tag"].as_str() == Some(tagname.as_str())
            && load(&payload.join("PROVENANCE.json"))?["tag"].as_str() == Some(tagname.as_str())
            && read(&payload.join("README.md"))?.contains(&format!("`{tagname}`"))
            && fact_block(&read(&h00)?, "header")?.contains(&tagname),
    );

    // ── ★push 권위(D8) 없음 = 커밋·봉인 전에 차단 ──
    let central_p = repo.join(core::REL_CENTRAL_FLAG);
    let central_text = read(&central_p)?;
    unlink(&central_p)?;
    let refs_d8 = _refs(&repo, false)?;
    let remote_d8 = _fx_git(&repo, &["ls-remote", &core::posix(&bare)])?.stdout;
    let st_d8 = state_of(&draft)?;
    let r1 = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT");
    let r2 = parse_args(&sv(&["--repo", &core::posix(&repo), "push", "--tag", &tagname, "--remote", "origin", "--apply"]))
        .and_then(|ns| _raises(|| cmd_push(&ns), "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT"));
    write(&central_p, &central_text)?;
    let ((ok, why), (ok_cli, why_cli)) = (r1?, r2?);
    let remedy = match _require_central(&td, "x") {
        Ok(()) => String::new(),
        Err(e) => e.remedy.unwrap_or_default(),
    };
    c.ck(
        &format!("★권위 마커 없음 = continue(push) 가 커밋·봉인 전에 HINT_PUSH_CENTRAL_AUTHORITY_ABSENT({})", h(&why, 70)),
        ok && remedy.contains("--no-push") && remedy.contains(core::REL_CENTRAL_FLAG),
    );
    c.ck(
        "★권위 마커 없음 = ref·원격·draft 상태 불변(커밋·봉인·push 0)",
        _refs(&repo, false)? == refs_d8 && _fx_git(&repo, &["ls-remote", &core::posix(&bare)])?.stdout == remote_d8 && util::py_eq(&state_of(&draft)?, &st_d8),
    );
    c.ck(&format!("★권위 마커 없음 = 단독 push --apply 도 차단({})", h(&why_cli, 60)), ok_cli);

    // ── ★별도 hint 클론 ──
    let notrepo = td.join("not-a-repo");
    mkdirs(&notrepo)?;
    let mut hc1 = cont(_FX_CONTINUE_UTC);
    hc1.hint_clone = Some(notrepo.clone());
    let (ok1, w1) = _raises(|| run_continue(&repo, &draft, &hc1, &rt), "HINT_CLONE_NOT_A_REPO")?;
    let mut hc2 = cont(_FX_CONTINUE_UTC);
    hc2.hint_clone = Some(repo.clone());
    let (ok2, w2) = _raises(|| run_continue(&repo, &draft, &hc2, &rt), "HINT_CLONE_IS_WORKSPACE")?;
    c.ck(
        &format!("★--hint-clone 저장소 아님/워크스페이스 자신 = 거부({} | {})", h(&w1, 50), h(&w2, 50)),
        ok1 && ok2 && core::aux_root(&notrepo) == notrepo,
    );
    let prev_aux = core::set_aux_root(Some(&repo));
    let aux_terms = pii::load_pii_terms(&notrepo);
    core::set_aux_root(prev_aux.as_deref());
    let aux_terms = aux_terms?;
    c.ck(
        "보조 루트 설정 시 PII 리터럴은 워크스페이스에서 읽는다 · 해제 뒤엔 넘겨받은 repo",
        aux_terms == pii::load_pii_terms(&repo)? && aux_terms.is_some() && pii::load_pii_terms(&notrepo)?.is_none(),
    );

    // ── ★https 원격 + 토큰 없음 = push 에서 차단 ──
    let mut gh = cont(_FX_CONTINUE_UTC);
    gh.remote = Some("gh".into());
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &gh, &rt), "HINT_PUSH_CREDENTIAL_ABSENT")?;
    c.ck(&format!("★https 원격 토큰 없음 = HINT_PUSH_CREDENTIAL_ABSENT({})", h(&why, 160)), ok);
    let st = state_of(&draft)?;
    let anchor = step(&st, "commit")["anchor"].as_str().map(str::to_string);
    let parents: Vec<String> = match &anchor {
        Some(a) => _fx_git(&repo, &["rev-list", "--parents", "-n", "1", a])?.stdout.split_whitespace().skip(1).map(str::to_string).collect(),
        None => Vec::new(),
    };
    let anchor = anchor.unwrap_or_default();
    c.ck(
        "커밋·봉인·로컬 검증은 이미 끝났다(원격엔 없다) · 페이로드 커밋 부모 = 안내 커밋 · 로컬 hint 브랜치 불변(없음)",
        !anchor.is_empty()
            && parents == vec![guide_sha.clone()]
            && branch::hint_tip(&repo)?.is_none()
            && step(&st, "verify")["ok"] == J::Bool(true)
            && tag::remote_tag_object(&repo, "origin", &tagname)?.is_none(),
    );
    c.ck(
        "페이로드 커밋 = 합성 신원",
        branch::commit_identity(&repo, &anchor)?["author"]["ident"].as_str() == Some(branch::synthetic_identity()?.as_str()),
    );
    c.ck(
        "★끊김 뒤 재실행 = 같은 커밋(첫 커밋 시각 재사용 · 같은 부모 = 안내 커밋 · 중복 커밋 ✗)",
        Some(&anchor) == crash_tip.as_ref() && parents == vec![guide_sha.clone()],
    );

    // Separate object repository: gate, draft, PII and source checks remain workspace-owned.
    let verify_clone = td.join("verify-object-clone");
    let clone_ok = core::run_capture(&sv(&["git", "clone", "-q", "--no-checkout", &core::posix(&repo), &core::posix(&verify_clone)]), None, &[], &[], None)
        .map(|o| o.returncode == 0)
        .unwrap_or(false);
    let dstr = core::posix(&draft);
    let mut verify_infos: Vec<J> = Vec::new();
    let clone_probs = if clone_ok { verify_tag(&repo, &tagname, Some(&dstr), Some(&mut verify_infos), Some(&verify_clone))? } else { vec!["clone failed".into()] };
    c.ck(
        "standalone verify uses separate object clone and workspace gate/source, without false drift",
        clone_ok && clone_probs.is_empty() && verify_infos.is_empty() && !verify_clone.join(core::REL_COMPLETION_GATE).exists() && core::aux_root(&verify_clone) == verify_clone,
    );
    c.ck(
        "verify CLI --hint-clone uses workspace and verifies the clone tag",
        quiet(|| main(&sv(&["--repo", &core::posix(&repo), "verify", "--tag", &tagname, "--draft", &dstr, "--hint-clone", &core::posix(&verify_clone)]))) == 0,
    );
    c.ck("single-repository verify remains compatible", verify_tag(&repo, &tagname, Some(&dstr), None, None)?.is_empty());
    let injected = jl![jd! {"code" => "HINT_BENCH_SECTION_DRIFT", "message" => "fixture unchanged source"}];
    let inj = injected.clone();
    let inj_fn = move |_: &Path, _: &str, _: &str| -> HR<J> { Ok(inj.clone()) };
    let mut drift_infos: Vec<J> = Vec::new();
    let prev = core::set_aux_root(Some(&repo));
    let drift_problems = tag::verify_local(&verify_clone, &tagname, Some(&inj_fn), Some(&mut drift_infos), Some(&repo));
    core::set_aux_root(prev.as_deref());
    let drift_problems = drift_problems?;
    c.ck(
        "clone source absence cannot downgrade an unchanged-workspace lint failure",
        tag::problem_codes(&drift_problems)?.iter().any(|x| x == "HINT_BENCH_SECTION_DRIFT") && drift_infos.is_empty(),
    );
    let prev = core::set_aux_root(Some(&repo));
    let pb = _raises(|| tag::push_tag(&verify_clone, "origin", &tagname, true, Some(&inj_fn), None, Some(&repo)), "HINT_PUSH_UNVERIFIED");
    core::set_aux_root(prev.as_deref());
    c.ck("clone push verifies against workspace sources and blocks unchanged-source lint failure", pb?.0);
    let continue_copy = td.join("verify-continue-copy");
    copytree(&draft, &continue_copy, &[])?;
    let mut ccp = cont(_FX_CONTINUE_UTC);
    ccp.hint_clone = Some(verify_clone.clone());
    ccp.no_push = true;
    let (blocked_continue, _) = super::hooks::with(
        |hk| hk.lint_findings = Some(injected.clone()),
        || _raises(|| run_continue(&repo, &continue_copy, &ccp, &rt), "HINT_LOCAL_VERIFY_FAILED"),
    )?;
    c.ck("separate-clone continue cannot downgrade unchanged-workspace source failure", blocked_continue);
    // The workspace gate is its launcher (and the original `.py` while it exists): both go, then both come back.
    let mut saved: Vec<(PathBuf, Option<PathBuf>, Option<Vec<u8>>)> = Vec::new();
    for rel in [core::REL_COMPLETION_GATE.to_string(), crate::evidence::_cgate_launcher_rel()] {
        let p = repo.join(&rel);
        let link = std::fs::read_link(&p).ok();
        let bytes = if link.is_none() && p.is_file() { Some(read_b(&p)?) } else { None };
        if link.is_some() || bytes.is_some() {
            unlink(&p)?;
            saved.push((p, link, bytes));
        }
    }
    let missing_gate = _raises(|| verify_tag(&repo, &tagname, Some(&dstr), None, Some(&verify_clone)), "HINT_OWNER_MODULE_MISSING");
    for (p, link, bytes) in &saved {
        if let Some(l) = link {
            std::os::unix::fs::symlink(l, p).map_err(|e| ioe(e, p))?;
        } else if let Some(b) = bytes {
            write_b(p, b)?;
        }
    }
    c.ck("separate clone never substitutes missing workspace gate", missing_gate?.0 && core::aux_root(&verify_clone) == verify_clone);

    // ── ★커밋 뒤 draft 가 바뀌면 재실행이 막힌다 ──
    let nar = payload.join("02-narrative.md");
    let nar_text = read(&nar)?;
    write(&nar, &(nar_text.clone() + "\n커밋 뒤 덧붙인 줄\n"))?;
    let refs_c = _refs(&repo, false)?;
    let (ok, why) = _raises(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt), "HINT_DRAFT_CHANGED_AFTER_COMMIT")?;
    c.ck(&format!("★커밋 뒤 draft 변경 = HINT_DRAFT_CHANGED_AFTER_COMMIT · ref 불변({})", h(&why, 80)), ok && _refs(&repo, false)? == refs_c);
    write(&nar, &nar_text)?;

    // ── ★push 역할만 ──
    let push_p = repo.join(core::REL_PUSH_FLAG);
    unlink(&central_p)?;
    write(&push_p, "픽스처 — 이 체크아웃은 태그 push 역할만 갖는다.\n")?;
    let idx_p = repo.join(core::REL_INDEX);
    let idx_before = if idx_p.is_file() { Some(read_b(&idx_p)?) } else { None };
    let pr = quiet(|| run_continue(&repo, &draft, &cont("2026-01-02T08:30:00Z"), &rt));
    unlink(&push_p)?;
    write(&central_p, &central_text)?;
    let (pres_role, role_err) = match pr {
        Ok(r) => (Some(r), "None".to_string()),
        Err(e) if !e.is_py() => (None, format!("{}: {}", e.code, h(&e.message, 300))),
        Err(e) => return Err(e),
    };
    let idx_after = if idx_p.is_file() { Some(read_b(&idx_p)?) } else { None };
    c.ck(
        &format!("★push 역할만 = push 성공 · 카탈로그 건너뜀(skipped-push-role-only) · 색인 불변({role_err})"),
        pres_role.as_ref().is_some_and(|r| r["pushed"] == J::Bool(true) && r["catalog_commit"]["status"].as_str() == Some("skipped-push-role-only"))
            && idx_after == idx_before
            && step(&state_of(&draft)?, "catalog")["skipped"].as_str() == Some("push-role-only"),
    );
    let mut ok_role = _require_central(&td, "x").is_err();
    mkdirs(&td.join("hints"))?;
    write(&td.join(core::REL_PUSH_FLAG), "x\n")?;
    if _require_central(&td, "x").is_err() {
        ok_role = false;
    }
    unlink(&td.join(core::REL_PUSH_FLAG))?;
    c.ck("push 역할 마커 하나로 push 게이트가 열린다(마커 둘 다 없으면 닫힌다)", ok_role);

    // ── continue 재실행(origin) ──
    let cres = match quiet(|| run_continue(&repo, &draft, &cont("2026-01-02T09:00:00Z"), &rt)) {
        Ok(r) => r,
        Err(e) if !e.is_py() => {
            c.ck(&format!("continue(origin) 성공 — {}: {}", e.code, h(&e.message, 600)), false);
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    c.ck(
        "재실행: 같은 페이로드 커밋(중복 커밋 없음 · 첫 커밋 시각 재사용) · 로컬 hint 브랜치는 여전히 없다",
        cres["anchor"].as_str() == Some(anchor.as_str()) && branch::hint_tip(&repo)?.is_none(),
    );
    let local_obj = tag::read_tag(&repo, &tagname)?.unwrap_or(J::Null)["object_sha"].display();
    c.ck(
        "원격 태그 오브젝트 = 로컬(AC9)",
        tag::remote_tag_object(&repo, "origin", &tagname)?.as_deref() == Some(local_obj.as_str()) && cres["remote_object"].as_str() == Some(local_obj.as_str()),
    );
    let remote_heads = _fx_git(&repo, &["ls-remote", "--heads", &core::posix(&bare)])?.stdout;
    c.ck("발행 push 는 브랜치를 밀지 않는다(O2 · U8) — 원격 hint = 안내 커밋 그대로", remote_heads.contains(&format!("{guide_sha}\t{}", core::HINT_BRANCH_REF)));
    let body = tag::read_tag(&repo, &tagname)?.unwrap_or(J::Null)["body"].display();
    let body_f = tag::parse_annotation(&body)?["footer"].clone();
    let bb = tag::bench_binding(&body_f)?;
    c.ck(
        "footer v2: bench_ref · bench_kind(이름 파생과 같다) · certificate_ref ✗",
        body_f["version"].as_str() == Some("2")
            && body_f.get("certificate_ref").is_none()
            && bb["kind"].as_str().map(str::to_string) == tag::bench_kind_of(&bb["ref"])?
            && bb["kind"].as_str().is_some_and(|k| tag::BENCH_KINDS.contains(&k)),
    );
    let clog = _fx_git(&repo, &["log", "-1", "--format=%s"])?.stdout.trim().to_string();
    let mut cfiles: Vec<String> = _fx_git(&repo, &["show", "--name-only", "--format=", "HEAD"])?.stdout.split_whitespace().map(str::to_string).collect();
    cfiles.sort();
    let mut catp: Vec<String> = _CATALOG_PATHS.iter().map(|s| s.to_string()).collect();
    catp.sort();
    c.ck(
        &format!("카탈로그 자동 커밋: 두 경로만 · 메시지 · Co-Authored-By 없음({clog})"),
        clog == format!("chore(hint): 카탈로그 파생 — {tagname} 발행 반영")
            && cfiles == catp
            && !_fx_git(&repo, &["log", "-1", "--format=%B"])?.stdout.contains("Co-Authored-By")
            && step(&state_of(&draft)?, "catalog")["commit"]["status"].as_str() == Some("committed"),
    );
    let head_c = _fx_git(&repo, &["rev-parse", "HEAD"])?.stdout.trim().to_string();
    write(&repo.join("README.fixture"), "fixture — 발행과 무관한 수정\n")?;
    let hm = repo.join(core::REL_HINTS_MD);
    write(&hm, &(read(&hm)? + "\n"))?;
    let dres = quiet(|| _commit_catalog(&repo, &tagname));
    _fx_git(&repo, &["checkout", "-q", "--", "README.fixture", core::REL_HINTS_MD])?;
    let dres = dres?;
    c.ck(
        "★추적 파일에 다른 변경이 있으면 카탈로그를 커밋하지 않는다(경고 · HEAD 불변)",
        dres["status"].as_str() == Some("skipped-dirty") && has(&dres["others"], "README.fixture") && _fx_git(&repo, &["rev-parse", "HEAD"])?.stdout.trim() == head_c,
    );
    c.ck("카탈로그 변경 없음 = 커밋 없음(unchanged)", util::py_eq(&quiet(|| _commit_catalog(&repo, &tagname))?, &jd! {"status" => "unchanged"}));
    c.ck("로컬 봉인 검증 PASS(verify · 서사 린트 포함)", quiet(|| verify_tag(&repo, &tagname, None, None, None))?.is_empty());
    c.ck(
        "A7 모양: 봉인된 full 페이로드의 03 에 데이터셋 출처 절이 있다(태그 zip = 이 트리)",
        blob_text(&repo, &anchor, "03-benchmark.md")?.unwrap_or_default().contains("### 데이터셋 출처")
            && blob_json(&repo, &anchor, "PAYLOAD.json")?["cell_grade"].as_str() == Some("full"),
    );
    let ptree_l = blob_json(&repo, &anchor, "LINEAGE.json")?;
    let tl_now = read(&tl_p)?;
    write(&tl_p, &tl_now.replace(_FX_WALL_SIG, "ValueError: 봉인 뒤 정정된 문장이다(출처 변경)."))?;
    let mut v_infos: Vec<J> = Vec::new();
    let vp = quiet(|| verify_tag(&repo, &tagname, None, Some(&mut v_infos), None));
    write(&tl_p, &tl_now)?;
    let v_probs = vp?;
    c.ck(
        &format!("★봉인 뒤 출처 변경 = verify PASS + INFO(출처 변경됨 · FAIL ✗)({})", util::list_repr(&v_probs.iter().take(2).collect::<Vec<_>>())),
        v_probs.is_empty()
            && v_infos.iter().any(|x| x.display().contains(tag::INFO_SOURCE_CHANGED))
            && ptree_l[lineage::SEALED_SOURCES_KEY].items().iter().any(|x| x["path"].as_str() == Some(_FX_TESTLOG))
            && ptree_l["kind"].as_str() == Some(lineage::SUMMARY_KIND)
            && ptree_l["documents"].items().iter().any(|d| d.get("excerpts").map(util::py_int).unwrap_or(0) >= 1),
    );
    let idx = load(&idx_p)?;
    let row = idx["hints"].items().iter().find(|x| x["tag"].as_str() == Some(tagname.as_str())).cloned();
    c.ck(
        "카탈로그 행 · grammar v7 · base_model · 판정 열(PAYLOAD.measurement.verdict)",
        row.as_ref().is_some_and(|r| {
            r["grammar"].as_str() == Some(naming::GRAMMAR_V7) && r["base_model"].as_str() == Some("Org/Fixture-Model") && util::py_eq(&r["verdict"], &pdoc["measurement"]["verdict"])
        }),
    );
    let rows: Map = {
        let mut m = Map::new();
        for x in idx["hints"].items() {
            m.insert(x["tag"].display(), x.clone());
        }
        m
    };
    c.ck(
        "★AC6 옛 태그·원격 전용·lightweight 가 공존해도 신규 발행 PASS · 셋 다 행으로 실림",
        ["old", "remote_only", "lightweight"].iter().all(|k| rows.contains_key(&legacy[*k].display())),
    );
    c.ck(
        "AC6 원격 전용 = absent-local 행(합성 ✗)",
        rows.get(&legacy["remote_only"].display()).map(|r| r["object"].clone()).unwrap_or(J::Null).as_str() == Some(catalog::OBJECT_ABSENT_LOCAL),
    );
    let ph = repo.join("campaigns/c1/phases/main/publish.status.json");
    let phd = if ph.is_file() { load(&ph)? } else { J::dict() };
    c.ck(
        "캠페인 publish 위상 = 원격 SHA 출처",
        phd["state"].as_str() == Some("done") && phd["proof"]["source"].as_str() == Some(format!("refs/tags/{tagname}@{local_obj}").as_str()),
    );
    let tree_paths = branch::tree_paths(&repo, &anchor)?;
    let ptree = blob_json(&repo, &anchor, "PAYLOAD.json")?;
    let appr_keys: std::collections::BTreeSet<String> = ptree["approval"].as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
    let want_ak: std::collections::BTreeSet<String> = APPROVAL_KEYS.iter().map(|s| s.to_string()).collect();
    c.ck(
        "PAYLOAD.approval = 캠페인 사전승인 3키",
        ptree["approval"]["source"].as_str() == Some("campaign:hint_targets") && appr_keys == want_ak,
    );
    let mut no_selfcheck = true;
    for n in template::PAYLOAD_DOCS {
        no_selfcheck &= !branch::read_blob_bytes(&repo, &anchor, n)?.map(|b| String::from_utf8_lossy(&b).contains("SELFCHECK")).unwrap_or(true);
    }
    c.ck(
        "PAYLOAD.factcheck = 통과(검증자 ≠ 저작자 · 열림 0) · 00 메타 '통과' · 저작 자기점검은 배포 ✗",
        ptree["factcheck"]["status"].as_str() == Some("passed")
            && util::py_eq(&ptree["factcheck"]["open"], &J::list())
            && ptree["factcheck"].get("open").is_some()
            && blob_text(&repo, &anchor, "00-hint.md")?.unwrap_or_default().contains("통과 — 검증자 `hint-factcheck-agent`")
            && no_selfcheck,
    );
    c.ck("PAYLOAD.bench_definition = evidence 가 읽은 docs.md 현행 full 정의(키 있음)", ptree.get("bench_definition").is_some());
    let (ok, why) = _raises(|| refresh_draft(&repo, &draft), "HINT_DRAFT_ALREADY_COMMITTED")?;
    c.ck(&format!("★커밋 뒤 refresh = HINT_DRAFT_ALREADY_COMMITTED(커밋한 바이트와 갈라지지 않게)({})", h(&why, 50)), ok);
    c.ck(
        "슬롯 사유(01 §1.2 소제목) 반영 · 신호 재계산",
        ptree["slots"].as_dict().is_some_and(|m| m.values().any(|r| r.get("rationale").is_some_and(J::truthy))),
    );
    let prov = blob_json(&repo, &anchor, "PROVENANCE.json")?;
    let head1 = _fx_git(&repo, &["rev-parse", "HEAD~1"])?.stdout.trim().to_string();
    c.ck(
        "PROVENANCE 소스 앵커 = publish 때 HEAD(카탈로그 커밋의 부모)",
        prov["source_anchor"] == state_of(&draft)?["source_anchor"] && prov["source_anchor"].as_str() == Some(head1.as_str()) && tree_paths.iter().any(|p| p == "README.md"),
    );
    let body2 = tag::read_tag(&repo, &tagname)?.unwrap_or(J::Null)["body"].display();
    c.ck(
        "annotation = brief + 포인터 + footer(zip 단일화 · D4)",
        body2.contains(tag::ANNOTATION_POINTER) && tag::parse_annotation(&body2)?["footer"]["anchor"].as_str() == Some(anchor.as_str()),
    );
    c.ck(
        "continue 재실행 = 멱등(이미 원격 · 같은 오브젝트)",
        quiet(|| run_continue(&repo, &draft, &cont(_FX_CONTINUE_UTC), &rt))?["remote_object"].as_str() == Some(local_obj.as_str()),
    );

    // ── ★같은 셀 재발행 = 시각 재바인딩 차단 · 원격 이름 조회 ──
    let rec = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json"));
    let rec_bytes = read_b(&rec)?;
    let (ok, why) = _raises(|| run_publish(&repo, &campaign_pub("2026-01-02T10:00:00Z", Some(td.join("again"))), &rt), "HINT_PUBLICATION_TIME_BOUND")?;
    c.ck(&format!("★같은 셀 재발행(draft 있음) = HINT_PUBLICATION_TIME_BOUND · 발행 기록 불변({})", h(&why, 80)), ok && read_b(&rec)? == rec_bytes);
    _fx_git(&repo, &["tag", "-d", &tagname])?;
    c.ck(
        "이름 조회: 로컬에 없고 원격에만 있어도 hit = remote(timestamp 판정의 입력 · ls-remote)",
        _name_hit(&repo, &tagname, "origin", true, false)?.0.as_deref() == Some("remote"),
    );

    // ── CLI 표면 ──
    let rs = core::posix(&repo);
    let main_r = |argv: &[&str]| -> (i32, String, String) {
        let mut a = sv(&["--repo", &rs]);
        a.extend(sv(argv));
        run_main(&a)
    };
    _fx_git(&repo, &["fetch", "-q", "origin", &format!("refs/tags/{tagname}:refs/tags/{tagname}")])?;
    let (rc, o, _) = main_r(&["verify", "--tag", &tagname]);
    let tail = if rc == 0 { String::new() } else { format!(" · {}", core::tail_chars(&o, 300)) };
    c.ck(&format!("CLI verify rc 0(PASS){tail}"), rc == 0 && o.contains("PASS"));
    let (rc, o, _) = main_r(&["lint", "--draft", &dstr, "--json"]);
    c.ck("CLI lint --json rc 0 · [] (봉인된 draft 도 통과)", rc == 0 && crate::value::loads(&o).map(|v| util::py_eq(&v, &J::list())).unwrap_or(false));
    let (rc, _o, e) = main_r(&["catalog", "derive", "--remote", "origin", "--generated-kst", "2026-01-02T20:00:00"]);
    c.ck(
        "CLI catalog derive 기본 모드 = 로컬 오브젝트 부재 rc 4 + 조회한 원격 fetch 안내",
        rc as i64 == catalog::EXIT_LOCAL_TAG_OBJECT_MISSING && e.contains("fetch") && e.contains("origin"),
    );
    let mp = repo.join(res["manifest"].display());
    let mbytes = read_b(&mp)?;
    let mut mdoc = crate::value::loads(&String::from_utf8_lossy(&mbytes)).map_err(|x| py("JSONDecodeError", x))?;
    mdoc.set("task_class", "bogus_class");
    write(&mp, &crate::value::dumps_default(&mdoc))?;
    let (rc, o, _) = main_r(&["verify", "--tag", &tagname]);
    let gj = crate::value::loads(&o).ok().filter(J::is_dict);
    c.ck(
        &format!("CLI 게이트 거부 = 게이트 JSON 원문 stdout + 게이트 종료코드(rc={rc})"),
        rc != 0 && gj.as_ref().is_some_and(|g| g["allowed"] == J::Bool(false) && g["action"].as_str() == Some("hint_verify")),
    );
    write_b(&mp, &mbytes)?;
    // 번역: 원본은 게이트 .py 를 `print('not json')` 으로 바꾼다 — Rust 는 게이트를 launcher 로 부르므로 launcher 를 바꾼다.
    let gate_sh = repo.join(core::REL_COMPLETION_GATE.to_string());
    let gate_sh_link = std::fs::read_link(&gate_sh).ok();
    let gate_sh_bytes = if gate_sh_link.is_none() && gate_sh.is_file() { Some(read_b(&gate_sh)?) } else { None };
    if gate_sh.exists() || gate_sh_link.is_some() {
        unlink(&gate_sh)?;
    }
    write(&gate_sh, "#!/bin/bash\necho 'not json'\n")?;
    let (rc, o, _) = main_r(&["verify", "--tag", &tagname]);
    unlink(&gate_sh)?;
    if let Some(l) = &gate_sh_link {
        std::os::unix::fs::symlink(l, &gate_sh).map_err(|x| ioe(x, &gate_sh))?;
    } else if let Some(b) = &gate_sh_bytes {
        write_b(&gate_sh, b)?;
    }
    let bj = crate::value::loads(&o).ok().filter(J::is_dict);
    c.ck(
        &format!("CLI 게이트 판독 불가 = 결정론 포장 JSON stdout(allowed:false) · rc 2(옛 계약)(rc={rc})"),
        rc == 2
            && bj.as_ref().is_some_and(|b| {
                b["allowed"] == J::Bool(false) && b["action"].as_str() == Some("hint_verify") && util::py_eq(&b["reason_codes"], &jl!["HINT_GATE_OUTPUT_UNREADABLE"])
            }),
    );
    let nope = core::posix(&draft.join("nope"));
    let (rc, _o, e) = main_r(&["continue", "--draft", &nope, "--generated-utc", _FX_CONTINUE_UTC]);
    c.ck("CLI HintError = render(메시지+remedy) stderr · rc 1", rc == 1 && e.contains("[hint] FAIL HINT_DRAFT_ABSENT") && e.contains('→'));

    // ── gitless match(배포 아카이브 모양 · git·PATH 없음) ──
    let gl = td.join("gitless");
    let skill = gl.join(core::REL_SKILL);
    copytree(&core::scripts_dir(), &skill.join("scripts"), &["__pycache__"])?;
    copytree(&core::templates_dir(), &skill.join("templates"), &[])?;
    mkdirs(&gl.join("hints"))?;
    copy2(&idx_p, &gl.join(core::REL_INDEX))?;
    // 번역: 원본은 `python3 -B <사본>/hint.py match …` 를 PATH 없이 돌린다 — Rust 는 이 실행파일을 사본 스킬 디렉터리로 돌린다.
    let mut exe = std::env::current_exe().map_err(|x| py("OSError", x.to_string()))?;
    if exe.file_name().is_some_and(|n| n != "hint") {
        // run from the library bridge (`hint_api`): the CLI is the sibling `hint` executable (else `HINT_BIN`)
        let sib = exe.with_file_name("hint");
        exe = if sib.is_file() { sib } else { std::env::var_os("HINT_BIN").map(PathBuf::from).unwrap_or(sib) };
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(["match", "--vllm", "0.9.0", "--model", "fixture-model-nvfp4", "--json"])
        .current_dir(&gl)
        .env_clear()
        .env("PATH", "/nonexistent")
        .env("HINT_SKILL_DIR", &skill)
        .stdin(std::process::Stdio::null());
    let r = cmd.output().map_err(|x| py("OSError", x.to_string()))?;
    let got = if r.status.success() { crate::value::loads(&String::from_utf8_lossy(&r.stdout)).ok() } else { None };
    let tail = if got.is_some() { String::new() } else { format!(" · {}", core::tail_chars(&String::from_utf8_lossy(&r.stderr), 300)) };
    c.ck(
        &format!("gitless match(PATH 없음 · .git 없음) rc 0 · 새 태그 발견{tail}"),
        got.as_ref().is_some_and(|g| g.is_list() && g.items().iter().any(|x| x["tag"].as_str() == Some(tagname.as_str()))),
    );

    // ── ★timestamp: 같은 셀 · 같은 꼬리의 새 판(재생 draft) = 이름 중복 → `-t<YYMMDDHHMM>` ──
    if rok {
        _fx_author(&repo, &rout, &state_of(&rout)?, true)?;
        core::write_json(&rout.join("inputs").join(TAIL_NAME), &good_tail)?;
        let ts_tok = naming::timestamp_token(_FX_REPLAY_UTC)?;
        let ts_name = format!("{tagname}-{ts_tok}");
        let mut appr = cont("2026-01-02T11:00:00Z");
        appr.no_push = true;
        appr.approved_by = Some("사용자 발화 전사 — 픽스처".into());
        appr.approved_utc = Some("2026-01-02T11:00:00Z".into());
        _fx_git(&repo, &["tag", &ts_name, "HEAD"])?;
        let refs_ts = _refs(&repo, false)?;
        let (ok, why) = _raises(|| run_continue(&repo, &rout, &appr, &rt), "HINT_NAME_COLLISION")?;
        c.ck(&format!("★timestamp 를 붙인 이름도 이미 있다(같은 분) = HINT_NAME_COLLISION · ref 쓰기 0({})", h(&why, 60)), ok && _refs(&repo, false)? == refs_ts);
        _fx_git(&repo, &["tag", "-d", &ts_name])?;
        match quiet(|| run_continue(&repo, &rout, &appr, &rt)) {
            Err(e) if !e.is_py() => c.ck(&format!("timestamp 재생 continue 성공 — {}: {}", e.code, h(&e.message, 400)), false),
            Err(e) => return Err(e),
            Ok(tres) => {
                let ta = tres["anchor"].display();
                let rpd = blob_json(&repo, &ta, "PAYLOAD.json")?;
                let par: Vec<String> = _fx_git(&repo, &["rev-list", "--parents", "-n", "1", &ta])?.stdout.split_whitespace().skip(1).map(str::to_string).collect();
                let coll = step(&state_of(&rout)?, "name")["collision"].display();
                c.ck(
                    &format!(
                        "timestamp: 중복 = 이름 + `-{ts_tok}`(재생 publish 시각 KST · 결정론) · PAYLOAD.naming 재조립 · 부모 = 안내({})",
                        tres["tag"].display()
                    ),
                    tres["tag"].as_str() == Some(ts_name.as_str())
                        && naming::parse_tag(&ts_name)?.timestamp.as_deref() == Some(ts_tok.as_str())
                        && rpd["naming"]["timestamp"].as_str() == Some(ts_tok.as_str())
                        && naming::compose_from_payload(&rpd["naming"])? == ts_name
                        && par == vec![guide_sha.clone()]
                        && (coll == "local" || coll == "remote"),
                );
            }
        }
    }
    drop(fake);
    Ok(())
}

/// Out-of-campaign publication (replay draft → `continue --approved-by`) in a separate isolated repository.
pub(super) fn _selftest_replay_continue(c: &mut Ck) -> HR<()> {
    let _saved = EnvGuard::pop(&E2E_ENV);
    let tdd = Tmp::new("hint-e2e-replay-");
    let td = tdd.path().to_path_buf();
    let (repo, fx) = _fx_repo(&td)?;
    let _fake = _fx_docker(&td, &fx["docker"])?;
    let rt = docker_runner(true);
    let bound_utc = "2026-01-02T05:50:00Z";
    quiet(|| run_publish(&repo, &campaign_pub(bound_utc, Some(td.join("first"))), &rt))?;
    std::fs::remove_dir_all(td.join("first")).map_err(|e| ioe(e, &td.join("first")))?;
    let (cres, _o, err) = io::capture(|| run_publish(&repo, &campaign_pub(_FX_PUBLISH_UTC, None), &rt));
    let cres = cres?;
    c.ck(
        "★스캐폴드 전에 멈춘 publish 뒤 새 시각 재실행 = 묶인 시각 채택 · PASS(로그로 알림)",
        cres["generated_utc"].as_str() == Some(bound_utc) && state_of(&pth(&cres["draft"]))?["generated_utc"].as_str() == Some(bound_utc) && err.contains("채택"),
    );
    let out = td.join("replay");
    let topic = cres["topic"].display();
    let rp = Pub { publication: Some(topic.clone()), replay: true, generated_utc: _FX_REPLAY_UTC.into(), out: Some(out.clone()), remote_check: true, ..Default::default() };
    let rres = quiet(|| run_publish(&repo, &rp, &rt))?;
    let recp = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json"));
    let rec_b = read_b(&recp)?;
    let rrx = quiet(|| refacts(&repo, &out, &rt))?;
    c.ck("★재생 refacts = 서사 재바인딩을 부르지 않는다(발행 기록 바이트 불변 · rebind None)", rrx["rebind"].is_null() && read_b(&recp)? == rec_b);
    let (ok, why) = _raises(|| run_continue(&repo, &out, &cont(_FX_CONTINUE_UTC), &rt), "HINT_APPROVAL_ABSENT")?;
    c.ck(&format!("★재생 continue 승인 전사 없음 = HINT_APPROVAL_ABSENT({})", h(&why, 80)), ok && branch::hint_tip(&repo)?.is_none());
    let mut ph = cont(_FX_CONTINUE_UTC);
    ph.approved_by = Some("<<FILL>>".into());
    ph.approved_utc = Some(_FX_CONTINUE_UTC.into());
    c.ck("★자리표시 승인 = HINT_APPROVAL_INVALID", _raises(|| run_continue(&repo, &out, &ph, &rt), "HINT_APPROVAL_INVALID")?.0);
    _fx_author(&repo, &out, &state_of(&out)?, false)?;
    let wv = |utc: &str| -> Cont {
        let mut a = cont(utc);
        a.approved_by = Some("사용자 발화 전사 — 재생 픽스처".into());
        a.approved_utc = Some("2026-01-02T06:45:00Z".into());
        a.factcheck_waiver = Some("사용자 발화 전사 — 재생 픽스처는 사실 검증을 면제한다".into());
        a
    };
    let mut gp = wv(_FX_CONTINUE_UTC);
    gp.guide_commit = Some(fx["guide"].display());
    let (ok, why) = _raises(|| run_continue(&repo, &out, &gp, &rt), "HINT_GUIDE_PIN_REQUIRES_NO_PUSH")?;
    c.ck(&format!("★--guide-commit 은 --no-push 와만({})", h(&why, 50)), ok);
    let old_guide = branch::selftest_guide(&repo, None, None, branch::FX_GUIDE_BODY, None)?;
    _fx_git(&repo, &["push", "-q", "-f", "origin", &format!("{old_guide}:{}", core::HINT_BRANCH_REF)])?;
    let refs_g = _refs(&repo, true)?;
    let mut np = wv(_FX_CONTINUE_UTC);
    np.no_push = true;
    let rr = _raises(|| run_continue(&repo, &out, &np, &rt), "HINT_BRANCH_FORMAT_MISMATCH");
    _fx_git(&repo, &["push", "-q", "-f", "origin", &format!("{}:{}", fx["guide"].display(), core::HINT_BRANCH_REF)])?;
    let (ok, why) = rr?;
    c.ck(
        &format!("★원격 안내 커밋 형식 ≠ v7 = HINT_BRANCH_FORMAT_MISMATCH · 커밋 · ref 쓰기 0({})", h(&why, 60)),
        ok && _refs(&repo, true)? == refs_g && _step(&state_of(&out)?, "commit").is_none(),
    );
    let res = match quiet(|| run_continue(&repo, &out, &np, &rt)) {
        Ok(r) => r,
        Err(e) if !e.is_py() => {
            c.ck(&format!("재생 continue(--no-push) 성공 — {}: {}", e.code, h(&e.message, 500)), false);
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let rtag = rres["tag"].display();
    c.ck(
        "재생 continue: 봉인·로컬 검증까지 · push 없음",
        res["pushed"] == J::Bool(false) && tag::tag_ref_kind(&repo, &rtag)?.as_deref() == Some("tag") && tag::remote_tag_object(&repo, "origin", &rtag)?.is_none(),
    );
    c.ck(
        "재생 continue: 발행기 입력은 draft 안에 도구가 썼다(손 JSON 0)",
        ["pii.json", "runtime.json"].iter().all(|f| out.join("inputs").join(f).is_file()),
    );
    let man = load(&repo.join(rres["manifest"].display()))?;
    c.ck(
        "재생 continue: 발행 기록의 승격 목표 = 새 태그 · 페이로드 커밋",
        man["promotion_target"]["tag"].as_str() == Some(rtag.as_str()) && man["promotion_target"]["anchor"] == res["anchor"],
    );
    let anchor = res["anchor"].display();
    let ptree = blob_json(&repo, &anchor, "PAYLOAD.json")?;
    c.ck("재생 continue: PAYLOAD.approval = CLI 전사 3키", ptree["approval"]["source"].as_str() == Some("cli:--approved-by"));
    let fcw = &ptree["factcheck"];
    c.ck(
        "재생 continue: 사실 검증 사람 면제 = PAYLOAD · state · 00 메타에 '면제' 로 남는다(수신자가 안다)",
        fcw["status"].as_str() == Some("waived")
            && fcw["reason_code"].as_str() == Some("HINT_FACTCHECK_ABSENT")
            && step(&state_of(&out)?, "factcheck")["status"].as_str() == Some("waived")
            && blob_text(&repo, &anchor, "00-hint.md")?.unwrap_or_default().contains("**사람 면제**"),
    );
    Ok(())
}

/// A declared lite-only cell (lite-grade certificate + light report): publish → author → continue(--no-push) → verify.
pub(super) fn _selftest_lite_e2e(c: &mut Ck) -> HR<()> {
    let _saved = EnvGuard::pop(&E2E_ENV);
    let tdd = Tmp::new("hint-lite-e2e-");
    let td = tdd.path().to_path_buf();
    let (repo, fx) = _fx_repo(&td)?;
    let _fake = _fx_docker(&td, &fx["docker"])?;
    let rt = docker_runner(true);
    use crate::render_bench_section as rbs;
    let m_utc = evidence::_M_UTC;
    write(
        &repo.join(fx["cert"].display()),
        &format!(
            "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: not_applicable\nmodel: fixture-model-nvfp4\n\
             gpu_model: \"NVIDIA GB10\"\nvllm_version: 0.9.0\nquantization: N/A\ntopology: multi\ntensor_parallel_size: 2\n\
             benchmark_mode: lite\nlite_verdict: pass\nentry_path: beta_lite_only_cell\nlite_included: true\nlite_gen_tps_warm: 9.0\n\
             measured_utc: \"{m_utc}\"\nmeasured_node: cluster\n"
        ),
    )?;
    let gen = format!("> 생성일 {m_utc}.");
    let report = [
        "# 경량", "", rbs::REPORT_LITE_MODE_LINE, "", &gen, "", rbs::MEASUREMENT_CONFIG_TITLE, "", "| 키 | 값 |", "|---|---|", "| bench_mode | lite |",
        "| bench_mode_kind | declared-lite |", "| repeats | 1 |", "| dataset_kind | synthetic |", "", "## lite 지표 (서빙 성공 직후)", "",
        "| 메트릭 | Main | Sub |", "|---|---|---|", "| gen tokens/sec (warm) [master] | 9.00 t/s | — |", "", "## 측정 환경 스냅샷", "", "| 키 | 값 |",
        "|---|---|", "| model | fixture-model-nvfp4 |", "| gpu_model | NVIDIA GB10 |", "| vllm_version | 0.9.0 |", "| topology | multi |", "",
    ]
    .join("\n");
    write(&repo.join(fx["report"].display()), &report)?;
    let env_c = repo.join("output/multi/envs/.env.c1-a");
    write(&env_c, &(read(&env_c)? + &format!("VLLM_REPO=https://{}.git\n", naming::UPSTREAM_VLLM_REPOS[0])))?;
    set_mtime(&env_c, _FX_ENV_MTIME_EPOCH)?;
    let bl = repo.join("output/multi/benchlog");
    let warm = bl.join("lite_warm_c1-a.json");
    core::write_json(&warm, &jd! {"completed" => 3, "failed" => 0})?;
    core::write_json(&bl.join("lite_raw_c1-a.json"), &jd! {"config_name" => "c1-a", "measured_utc" => m_utc, "bench_warm_json" => core::posix(&warm)})?;
    let res = match quiet(|| run_publish(&repo, &campaign_pub(_FX_PUBLISH_UTC, None), &rt)) {
        Ok(r) => r,
        Err(e) if !e.is_py() => {
            c.ck(&format!("lite E2E publish 성공 — {}: {} → {}", e.code, h(&e.message, 600), e.remedy.clone().unwrap_or_else(|| "None".into())), false);
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let draft = pth(&res["draft"]);
    let payload = draft.join("payload");
    let snap = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
    let pdoc = load(&payload.join("PAYLOAD.json"))?;
    c.ck(
        "lite E2E publish: 03-benchmark.md 미스캐폴드 · payload_files 에도 없다 · PAYLOAD/facts cell_grade = lite",
        !payload.join(template::BENCH_DOC).exists()
            && !has(&res["files"], template::BENCH_DOC)
            && pdoc["cell_grade"].as_str() == Some("lite")
            && snap["cell_grade"].as_str() == Some("lite")
            && snap["bench_section_md"].is_null(),
    );
    let readme = read(&payload.join("README.md"))?;
    c.ck(
        "lite E2E: 기계 판정 칸 = OBSERVATION-ONLY 그대로(어휘 불변) · 표시 라벨은 00 등급 · README 판정 줄 · 03 행 없음",
        pdoc["measurement"]["verdict"].as_str() == Some(evidence::OBSERVATION_ONLY_VERDICT)
            && fact_block(&read(&payload.join("00-hint.md"))?, "grade")?.contains(&format!("`{}`", template::LITE_LABEL))
            && readme.contains(&format!("> **판정** `{}`", template::LITE_LABEL))
            && !readme.contains(&format!("| 4 | `{}` |", template::BENCH_DOC)),
    );
    _fx_author(&repo, &draft, &state_of(&draft)?, true)?;
    let l00 = fact_block(&read(&payload.join("00-hint.md"))?, "lite_summary")?;
    let issues = quiet(|| lint_draft(&repo, &draft))?;
    let shown: Vec<String> = issues.iter().take(4).map(|i| format!("({}, {})", i["code"].repr(), i["file"].repr())).collect();
    c.ck(
        &format!("lite E2E 저작 뒤 lint 0(brief 수치 = 00 lite 블록 근거 · 03 부재 정상): [{}]", shown.join(", ")),
        issues.is_empty() && l00.contains("9.0"),
    );
    let mut np = cont(_FX_CONTINUE_UTC);
    np.no_push = true;
    let cres = match quiet(|| run_continue(&repo, &draft, &np, &rt)) {
        Ok(r) => r,
        Err(e) if !e.is_py() => {
            c.ck(&format!("lite E2E continue(--no-push) 성공 — {}: {}", e.code, h(&e.message, 600)), false);
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let anchor = cres["anchor"].display();
    let fin = state_of(&draft)?["tag"].display();
    let tree = branch::tree_paths(&repo, &anchor)?;
    c.ck(
        "A8 모양: 봉인된 lite 페이로드 트리에 03-benchmark.md 없음 · 필수 위반 0 · PAYLOAD.cell_grade = lite",
        !tree.iter().any(|p| p == template::BENCH_DOC)
            && tree.iter().any(|p| p == "00-hint.md")
            && branch::tree_violations(&repo, &anchor)?.is_empty()
            && blob_json(&repo, &anchor, "PAYLOAD.json")?["cell_grade"].as_str() == Some("lite"),
    );
    c.ck(
        "A8 모양: 봉인된 00 · README 에 판정 표기 `lite(기동 가능)`(성능 판정 아님)",
        blob_text(&repo, &anchor, "00-hint.md")?.unwrap_or_default().contains(template::LITE_LABEL)
            && blob_text(&repo, &anchor, "README.md")?.unwrap_or_default().contains("성능 판정 아님"),
    );
    let vp = quiet(|| verify_tag(&repo, &fin, None, None, None))?;
    c.ck(
        &format!("lite E2E 로컬 봉인 검증 PASS(verify · 서사 린트 · 트리 allowlist)({})", util::list_repr(&vp.iter().take(2).collect::<Vec<_>>())),
        vp.is_empty() && cres["pushed"] == J::Bool(false),
    );
    Ok(())
}
