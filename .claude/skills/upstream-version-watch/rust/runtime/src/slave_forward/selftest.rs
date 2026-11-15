//! `slave_forward.py --self-test` ported to Rust (same checks and the same `[slave-forward] self-test PASS|FAIL (n failure(s))` line).
//! The legacy-shell oracle really runs under bash (a test-plane control group); fixtures are the Python originals byte for byte.
use super::*;
use std::collections::BTreeMap;
use std::process::Command;

const LEGACY_SHELL: &str = r##"
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
IMG=$(val IMAGE_TAG); VREPO=$(val VLLM_REPO); VREF=$(val VLLM_REF); BDF=$(val BUILD_DOCKERFILE)
VPV=$(val VLLM_PRETEND_VERSION); SMPORT=$(val SM12X_PORT); SDA=$(val SRC_DEPS_AUTHORITY)
VVER=$(val VLLM_VERSION)
SLAVE_IMGVARS="${IMG:+IMAGE_TAG=$IMG }${BDF:+BUILD_DOCKERFILE=$BDF }${VVER:+VLLM_VERSION=$VVER }${VREPO:+VLLM_REPO=$VREPO }${VPV:+VLLM_PRETEND_VERSION=$VPV }${SMPORT:+SM12X_PORT=$SMPORT }${SDA:+SRC_DEPS_AUTHORITY=$SDA }${VREF:+VLLM_REF=$VREF}"
RAYP=$(val RAY_PORT); SLVC=$(val SLAVE_CONTAINER_NAME)
SLAVE_CLUSTERVARS="${RAYP:+RAY_PORT=$RAYP }${SLVC:+SLAVE_CONTAINER_NAME=$SLVC}"
SLAVE_IMGVARS="$SLAVE_IMGVARS $SLAVE_CLUSTERVARS"
MOUNTVARS=""
[ -f "$PENV_FILE" ] && MOUNTVARS="$(grep -E '^(NAS_MODEL_PATH|QUANT_MODEL_PATH|TIKTOKEN_HOST_PATH|PLE_MMAP_HOST_PATH)=' "$PENV_FILE" | tr '\n' ' ')"
PLEVARS="$(grep -E '^VLLM_PLE_MMAP(_DIR)?=' "$EF" 2>/dev/null | tr -d ' ' | tr '\n' ' ')"
BJOBS=$(val BUILD_JOBS)
SLAVE_BUILDVARS="${BJOBS:+BUILD_JOBS=$BJOBS}"
printf 'IMG=%s\nBUILD=%s\nMOUNT=%s\nPLE=%s\n' "$SLAVE_IMGVARS" "$SLAVE_BUILDVARS" "$MOUNTVARS" "$PLEVARS"
"##;
const TAG2_CELL_ENV: &str = r##"# fixture
# fixture
COMPOSE_PROJECT_NAME=vllm_nv4-bf-262k-mmp_project
CONTAINER_NAME=nv4-bf-262k-mmp-serving-container
MASTER_CONTAINER_NAME=nv4-bf-262k-mmp-master-container
SLAVE_CONTAINER_NAME=nv4-bf-262k-mmp-slave-container
SERVING_PORT=8080
SERVING_MODEL_NAME=qwen3.8-flash-next
CONFIG_FILE=nv4-bf-262k-mmp
IMAGE_TAG=easy-vllm:0.29.0rc6-cu133-aarch64-source
BUILD_DOCKERFILE=Dockerfile.source-build
VLLM_REF=v0.29.0rc6
BUILD_JOBS=8
VLLM_PLE_MMAP=1
VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4
"##;
const PROJECT_ENV: &str = "NAS_MODEL_PATH=/data/models\nQUANT_MODEL_PATH=/data/quant\nTIKTOKEN_HOST_PATH=/data/tiktoken\nPLE_MMAP_HOST_PATH=/data/ple\n";
const FIXTURE_COMPOSE: &str = r##"x-gpu-common: &gpu-common
  image: ${IMAGE_TAG:-easy-vllm:{{ IMAGE_TAG }}}
  build:
    context: .
    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}   # 주석 ${NOT_A_REF}
    args:
      VLLM_VERSION: ${VLLM_VERSION:-0.18.0}
      VLLM_REPO: ${VLLM_REPO:-https://example.invalid/vllm.git}
      VLLM_REF: ${VLLM_REF:-v0.29.0rc6}
      VLLM_PRETEND_VERSION: ${VLLM_PRETEND_VERSION:-}
      SM12X_PORT: ${SM12X_PORT:-0}
      SRC_DEPS_AUTHORITY: ${SRC_DEPS_AUTHORITY:-0}
      LITERAL_ARG: fixed
      BUILD_JOBS: ${BUILD_JOBS:-16}
  volumes:
    - ${NAS_MODEL_PATH:-/data/x}:/app/models:ro
    - ${QUANT_MODEL_PATH:-/data/x}:/app/quant_models:ro
    - ${TIKTOKEN_HOST_PATH:-/data/x}:/encodings:ro
    - ${PLE_MMAP_HOST_PATH:-/data/x}:/app/ple_mmap:ro
    - ./configs:/app/configs:ro
services:
  vllm-master-serve:
    <<: *gpu-common
    container_name: ${MASTER_CONTAINER_NAME:-m}
    environment:
      CONFIG_FILE: ${CONFIG_FILE:-default}
      RUNTIME_PATCH_STEM: ${RUNTIME_PATCH_STEM:-}
      MASTER_ONLY: ${MASTER_ONLY:-x}
  vllm-slave-serve:
    <<: *gpu-common
    profiles: [slave]
    container_name: ${SLAVE_CONTAINER_NAME:-vllm-slave-serve-container}
    environment:
      NODE_ROLE: slave
      VLLM_HOST_IP: ${SLAVE_HOST_IP}
      HEAD_NODE_IP: ${MASTER_HOST_IP}
      RAY_PORT: ${RAY_PORT:-6379}
      CONFIG_FILE: ${CONFIG_FILE:-default}
      RUNTIME_PATCH_STEM: ${RUNTIME_PATCH_STEM:-}
      ESCAPED: $${NOT_A_REF_EITHER}
      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}
      VLLM_PLE_MMAP_DIR: ${VLLM_PLE_MMAP_DIR:-/app/ple_mmap/q38fn-nvfp4}
    env_file:
      - envs/.env.interconnect
      - envs/.env.cluster
"##;

fn tripwire() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("image_identity", vec!["IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_VERSION", "VLLM_REPO", "VLLM_REF", "VLLM_PRETEND_VERSION", "SM12X_PORT", "SRC_DEPS_AUTHORITY"]),
        ("build_tuning", vec!["BUILD_JOBS"]),
        ("cluster", vec!["SLAVE_CONTAINER_NAME", "SLAVE_HOST_IP", "MASTER_HOST_IP", "RAY_PORT"]),
        ("mount", vec!["NAS_MODEL_PATH", "QUANT_MODEL_PATH", "TIKTOKEN_HOST_PATH", "PLE_MMAP_HOST_PATH"]),
        ("serve_env", vec!["RUNTIME_PATCH_STEM", "VLLM_PLE_MMAP", "VLLM_PLE_MMAP_DIR"]),
    ])
}

fn groups_map(c: &str) -> Result<BTreeMap<&'static str, Vec<String>>, ForwardError> {
    Ok(candidates(c, None)?.into_iter().collect())
}

fn matches_tripwire(g: &BTreeMap<&'static str, Vec<String>>) -> bool {
    let t = tripwire();
    g.len() == t.len() && t.iter().all(|(k, v)| g.get(k).is_some_and(|x| x.iter().map(String::as_str).eq(v.iter().copied())))
}

fn set_of(s: &str) -> HashSet<String> {
    s.split_whitespace().map(String::from).collect()
}

fn legacy(cell_text: &str, project_text: Option<&str>, dir: &Path) -> Result<HashMap<String, HashSet<String>>, ForwardError> {
    let ef = dir.join("legacy-cell.env");
    let pe = dir.join("legacy-project.env");
    let _ = std::fs::remove_file(&pe);
    std::fs::write(&ef, cell_text).map_err(|e| ForwardError { code: "SF_LEGACY_ORACLE", message: e.to_string() })?;
    if let Some(p) = project_text {
        std::fs::write(&pe, p).map_err(|e| ForwardError { code: "SF_LEGACY_ORACLE", message: e.to_string() })?;
    }
    let script = format!("EF=\"{}\"; PENV_FILE=\"{}\"\n{LEGACY_SHELL}", ef.display(), pe.display());
    let o = Command::new("bash").arg("-c").arg(script).output().map_err(|e| ForwardError { code: "SF_LEGACY_ORACLE", message: e.to_string() })?;
    if !o.status.success() {
        let e = String::from_utf8_lossy(&o.stderr).into_owned();
        let tail: String = e.chars().rev().take(400).collect::<Vec<_>>().into_iter().rev().collect();
        return ferr("SF_LEGACY_ORACLE", tail);
    }
    let mut out = HashMap::new();
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        let (k, _, v) = line.partition_once();
        out.insert(k.to_string(), set_of(v));
    }
    Ok(out)
}

trait Partition {
    fn partition_once(&self) -> (&str, &str, &str);
}
impl Partition for str {
    fn partition_once(&self) -> (&str, &str, &str) {
        match self.find('=') {
            Some(i) => (&self[..i], "=", &self[i + 1..]),
            None => (self, "", ""),
        }
    }
}

fn pfx(fw: &[Forward], groups: &[&str]) -> String {
    prefix(fw, groups).unwrap_or_default()
}

fn derive_s(cell: &Path, project: &Path, compose: &Path) -> Result<Vec<Forward>, ForwardError> {
    derive(&cell.to_string_lossy(), &project.to_string_lossy(), &compose.to_string_lossy(), None)
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn run_cli(args: &[String]) -> (i32, String, String) {
    let exe = std::env::current_exe().unwrap_or_default();
    match Command::new(exe).args(args).output() {
        Ok(o) => (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()),
        Err(e) => (-1, String::new(), e.to_string()),
    }
}

fn selftest() -> Vec<String> {
    let mut failures: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool| {
        if !cond {
            failures.push(format!("slave_forward: {name}"));
        }
    };
    // `expect_error` for results already evaluated.
    fn code_of<T>(r: &Result<T, ForwardError>) -> Option<&'static str> {
        r.as_ref().err().map(|e| e.code)
    }

    let root = std::env::temp_dir().join(format!("slave-forward-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    if std::fs::create_dir_all(&root).is_err() {
        return vec!["slave_forward: 임시 디렉터리를 만들지 못했다".into()];
    }
    let w = |p: &Path, t: &str| {
        let _ = std::fs::write(p, t);
    };
    let compose = root.join("docker-compose.yaml");
    w(&compose, FIXTURE_COMPOSE);
    let cell = root.join(".env.cell");
    let project = root.join(".env");
    w(&project, PROJECT_ENV);

    // 1) structure derivation = tripwire table
    let groups = groups_map(&s(&compose));
    ck("구조 파생 = tripwire 표", groups.as_ref().is_ok_and(matches_tripwire));
    ck("★CONFIG_FILE 은 어느 그룹에도 없다", groups.as_ref().is_ok_and(|g| g.values().all(|v| !v.iter().any(|k| k == "CONFIG_FILE"))));
    let banned = ["NOT_A_REF", "NOT_A_REF_EITHER", "MASTER_ONLY", "MASTER_CONTAINER_NAME"];
    ck("주석·$$ 이스케이프·마스터 전용 참조는 참조가 아니다", groups.as_ref().is_ok_and(|g| !g.values().flatten().any(|k| banned.contains(&k.as_str()))));

    // 2) tag-2 cell vs the legacy shell
    w(&cell, TAG2_CELL_ENV);
    let fw = derive_s(&cell, &project, &compose).unwrap_or_default();
    match legacy(TAG2_CELL_ENV, Some(PROJECT_ENV), &root) {
        Ok(lg) => {
            ck("태그2 image_identity+cluster = 옛 SLAVE_IMGVARS", set_of(&pfx(&fw, &["image_identity", "cluster"])) == lg["IMG"]);
            ck("태그2 build_tuning = 옛 SLAVE_BUILDVARS", set_of(&pfx(&fw, &["build_tuning"])) == lg["BUILD"]);
            ck("태그2 mount = 옛 MOUNTVARS", set_of(&pfx(&fw, &["mount"])) == lg["MOUNT"]);
            ck("태그2 serve_env = 옛 PLEVARS", set_of(&pfx(&fw, &["serve_env"])) == lg["PLE"]);
        }
        Err(e) => ck(&format!("옛 셸 대조군 실행({})", e.message), false),
    }
    ck(
        "태그2 image_identity 정확값",
        pfx(&fw, &["image_identity"]) == "IMAGE_TAG=easy-vllm:0.29.0rc6-cu133-aarch64-source BUILD_DOCKERFILE=Dockerfile.source-build VLLM_REF=v0.29.0rc6",
    );

    // 3) legacy predicate vectors
    let base = "IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x\nVLLM_REPO=https://github.com/jasl/vllm.git\nVLLM_REF=b5c0d43b967c\nBUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_PRETEND_VERSION=0.26.1\n";
    let vectors: Vec<(&str, String)> = vec![
        ("(a) 변종 SM12X_PORT=1", format!("{base}SM12X_PORT=1\n")),
        ("(b) stock 부재", base.to_string()),
        ("(b') 빈 SM12X_PORT=", format!("{base}SM12X_PORT=\n")),
        ("(c) wheel VLLM_VERSION", "IMAGE_TAG=easy-vllm:0.19.0-cu130-aarch64-wheel\nBUILD_DOCKERFILE=Dockerfile\nVLLM_VERSION=0.19.0\n".to_string()),
        ("(d) SRC_DEPS_AUTHORITY=1", format!("{base}SRC_DEPS_AUTHORITY=1\n")),
        ("첫 일치(중복 키)", format!("{base}SM12X_PORT=1\nSM12X_PORT=0\n")),
        ("RAY_PORT 셀 덮어쓰기", format!("{base}RAY_PORT=6383\n")),
    ];
    for (name, text) in &vectors {
        w(&cell, text);
        let got = set_of(&pfx(&derive_s(&cell, &project, &compose).unwrap_or_default(), &["image_identity", "cluster"]));
        let want = legacy(text, Some(PROJECT_ENV), &root).map(|m| m["IMG"].clone()).ok();
        ck(&format!("{name} = 옛 셸"), want.is_some_and(|x| x == got));
    }
    w(&cell, base);
    ck("★(b) 부재는 키째 생략", !pfx(&derive_s(&cell, &project, &compose).unwrap_or_default(), &["image_identity"]).contains("SM12X_PORT"));
    w(&cell, &format!("{base}SM12X_PORT=\n"));
    ck("★(b') 빈 값은 빈 대입으로 새지 않는다", !pfx(&derive_s(&cell, &project, &compose).unwrap_or_default(), &["image_identity"]).contains("SM12X_PORT"));

    // 4) (e) new build-arg is forwarded without code changes
    w(&compose, &FIXTURE_COMPOSE.replace(
        "      LITERAL_ARG: fixed\n",
        "      LITERAL_ARG: fixed\n      NEW_IDENTITY_ARG: ${NEW_IDENTITY_ARG:-0}\n      RENAMED_ARG: ${OTHER_NAME_VAR:-x}\n",
    ));
    w(&cell, &format!("{base}NEW_IDENTITY_ARG=1\nOTHER_NAME_VAR=y\nCONFIG_FILE=cell\n"));
    let got: HashMap<String, Forward> = derive_s(&cell, &project, &compose).unwrap_or_default().into_iter().map(|f| (f.key.clone(), f)).collect();
    ck("★(e) 새 build-arg 자동 전달", got.get("NEW_IDENTITY_ARG").is_some_and(|f| f.group == "image_identity"));
    ck("build-arg 가 참조하는 변수 이름을 넘긴다", got.contains_key("OTHER_NAME_VAR") && !got.contains_key("RENAMED_ARG"));
    ck("★CONFIG_FILE 은 셀이 정해도 넘기지 않는다", !got.contains_key("CONFIG_FILE"));
    w(&compose, FIXTURE_COMPOSE);

    // 5) mounts
    w(&cell, &format!("{base}NAS_MODEL_PATH=/data/cell-models\n"));
    let fwm: HashMap<String, Forward> = derive_s(&cell, &project, &compose).unwrap_or_default().into_iter().map(|f| (f.key.clone(), f)).collect();
    ck("마운트는 프로젝트 .env 가 이긴다", fwm.get("NAS_MODEL_PATH").is_some_and(|f| f.value == "/data/models" && f.source == "project_env"));
    let fwn: HashMap<String, Forward> = derive_s(&cell, &root.join("absent.env"), &compose).unwrap_or_default().into_iter().map(|f| (f.key.clone(), f)).collect();
    ck("프로젝트 .env 가 없으면 셀 env 값", fwn.get("NAS_MODEL_PATH").is_some_and(|f| f.source == "cell_env"));
    let rows = derive_s(&cell, &project, &compose).unwrap_or_default();
    let by_key = |k: &str| rows.iter().find(|f| f.key == k);
    ck("sub_recipe 는 마운트 원값을 싣지 않는다", by_key("NAS_MODEL_PATH").and_then(|f| value_shape(f).ok()).as_deref() == Some("<manifest.nas_model_path>"));
    ck("sub_recipe why 는 사고 이력", why("BUILD_DOCKERFILE").starts_with("2026-07-24"));

    // 6) unsafe values · shape violations · unknown group
    w(&cell, &format!("{base}VLLM_PLE_MMAP_DIR=/data/with space\n"));
    ck("★공백 값 차단(code=SF_UNSAFE_VALUE)", code_of(&derive_s(&cell, &project, &compose)) == Some("SF_UNSAFE_VALUE"));
    w(&cell, &base.replace("VLLM_REF=b5c0d43b967c", "VLLM_REF=v1$(id)"));
    ck("★셸 메타문자 차단(code=SF_UNSAFE_VALUE)", code_of(&derive_s(&cell, &project, &compose)) == Some("SF_UNSAFE_VALUE"));
    w(&cell, base);
    let bad = root.join("bad.yaml");
    w(&bad, &FIXTURE_COMPOSE.replace("  vllm-slave-serve:\n    <<: *gpu-common\n", "  vllm-slave-serve:\n"));
    ck("★slave 가 앵커를 병합하지 않음(code=SF_COMPOSE_SHAPE)", code_of(&derive_s(&cell, &project, &bad)) == Some("SF_COMPOSE_SHAPE"));
    w(&bad, &FIXTURE_COMPOSE.replace("vllm-slave-serve:", "vllm-other:"));
    ck("★slave 서비스 부재(code=SF_COMPOSE_SHAPE)", code_of(&derive_s(&cell, &project, &bad)) == Some("SF_COMPOSE_SHAPE"));
    ck("★compose 부재(code=SF_COMPOSE_ABSENT)", code_of(&derive_s(&cell, &project, &root.join("none.yaml"))) == Some("SF_COMPOSE_ABSENT"));
    ck("★모르는 그룹(code=SF_GROUP_UNKNOWN)", code_of(&prefix(&[], &["imgvars"])) == Some("SF_GROUP_UNKNOWN"));
    // renderer fail-loud → reason code (the "wrong return shape" case cannot occur: the Rust API is typed)
    let broken = tier_with("RAY_PORT", |_| Err::<(&'static str, Option<String>), _>("no common preset".to_string()));
    ck("★env_tier 예외는 사유 코드로(code=SF_ENV_TIER_FAILED)", code_of(&broken) == Some("SF_ENV_TIER_FAILED"));

    // 7) compose_default
    let cd = |p: &Path, k: &str| compose_default(&s(p), k);
    ck("compose_default 단순", cd(&compose, "VLLM_REF").ok().flatten().as_deref() == Some("v0.29.0rc6"));
    ck("compose_default 중첩 괄호", cd(&compose, "IMAGE_TAG").ok().flatten().as_deref() == Some("easy-vllm:{{ IMAGE_TAG }}"));
    ck("compose_default 빈 기본값", cd(&compose, "VLLM_PRETEND_VERSION").ok().flatten().as_deref() == Some(""));
    ck("compose_default 부재", matches!(cd(&compose, "SLAVE_HOST_IP"), Ok(None)));
    let commented = root.join("commented.yaml");
    w(&commented, &format!("# 옛 기본값 ${{VLLM_REF:-v0.27.1}} 은 주석이다\n{FIXTURE_COMPOSE}"));
    ck("★compose_default 는 주석 속 옛 기본값을 읽지 않는다", cd(&commented, "VLLM_REF").ok().flatten().as_deref() == Some("v0.29.0rc6"));
    ck("★compose_default compose 부재(code=SF_COMPOSE_ABSENT)", code_of(&cd(&root.join("none.yaml"), "VLLM_REF")) == Some("SF_COMPOSE_ABSENT"));

    // 8) CLI (this very binary)
    w(&cell, TAG2_CELL_ENV);
    let common = vec!["--cell-env".to_string(), s(&cell), "--project-env".into(), s(&project), "--compose".into(), s(&compose)];
    let mut a = vec!["prefix".to_string(), "--group".into(), "image_identity,cluster".into()];
    a.extend(common.clone());
    let (rc, so, se) = run_cli(&a);
    let tail: String = se.trim().chars().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect();
    ck(&format!("CLI prefix rc=0({tail})"), rc == 0);
    ck("CLI prefix = API prefix", so.trim() == pfx(&derive_s(&cell, &project, &compose).unwrap_or_default(), &["image_identity", "cluster"]));
    let (rc, so, _) = run_cli(&["default".into(), "--compose".into(), s(&compose), "--key".into(), "VLLM_REF".into()]);
    ck("CLI default", rc == 0 && so.trim() == "v0.29.0rc6");
    let (rc, so, _) = run_cli(&["default".into(), "--compose".into(), s(&compose), "--key".into(), "NO_SUCH_KEY".into()]);
    ck("★CLI default 부재 = rc 1(빈 값으로 통과시키지 않는다)", rc == 1 && so.trim().is_empty());
    w(&cell, &format!("{TAG2_CELL_ENV}VLLM_REPO=https://x.invalid/a b\n"));
    let mut a = vec!["prefix".to_string(), "--group".into(), "image_identity".into()];
    a.extend(common);
    let (rc, so, se) = run_cli(&a);
    ck("★CLI 안전하지 않은 값 = rc 2 · stdout 비어 있음(스모크 `|| exit 3` 가 잡는다)", rc == 2 && so.trim().is_empty() && se.contains("SF_UNSAFE_VALUE"));
    let _ = std::fs::remove_dir_all(&root);

    // 9) tripwire against the tracked template / render
    let mut skill = std::env::current_exe().unwrap_or_default();
    for _ in 0..5 {
        skill.pop();
    }
    let template = skill.join("templates").join("docker-compose.multi.template.yaml");
    ck("멀티 compose 템플릿 실재", template.is_file());
    if template.is_file() {
        ck("★템플릿 구조 = tripwire 표(새 참조면 WHY·표를 사람이 갱신)", groups_map(&s(&template)).as_ref().is_ok_and(matches_tripwire));
    }
    let tracked = skill.join("..").join("..").join("..").join("output").join("multi").join("docker-compose.yaml");
    if tracked.is_file() {
        ck("추적 렌더 구조 = tripwire 표(템플릿↔렌더 드리프트 K6)", groups_map(&s(&tracked)).as_ref().is_ok_and(matches_tripwire));
    }
    failures
}

pub fn run_selftest() -> i32 {
    let bad = selftest();
    for line in &bad {
        err(&format!("FAIL {line}\n"));
    }
    out(&format!("[slave-forward] self-test {} ({} failure(s))\n", if bad.is_empty() { "PASS" } else { "FAIL" }, bad.len()));
    i32::from(!bad.is_empty())
}
