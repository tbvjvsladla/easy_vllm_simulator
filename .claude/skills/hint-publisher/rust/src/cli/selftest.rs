//! `hint.py --self-test` — port of hint.py lines 2048–3720: the module self-tests runner, the parser checks, the static
//! document cross-checks, the FACT producer checks (F2 · late naming) and the isolated E2E (temp repository · bare remote ·
//! fake docker · replay continue · lite cell) in [`super::selftest_e2e`].
//!
//! Translations (the original's Python-only facts → the closest Rust fact; each listed in the port report):
//! - `callable(a.fn)` → the namespace names a sub-command `main` dispatches (`catalog` also needs its sub-command).
//! - `inspect.getsource(_build_parser)` → this crate's `src/cli/mod.rs` source (`include_str!`) carries `.default("origin")`.
//! - `_docker_probe_only.__doc__` → [`super::_DOCKER_PROBE_ONLY_DOC`] (the docstring kept as a constant).
//! - `mock.patch.object(<module>, <fn>)` → the CLI injection points in [`super::hooks`].
//! - the fake docker callable (`Runtime(docker=_fx_docker(state))`) → a fake `docker` executable first on `PATH` (the
//!   evidence owner's J hook has no injected form; the CLI hands the artifacts owner the read-only runner marker, which
//!   answers non-read calls with docker's own rc 125 without executing them).
use std::path::{Path, PathBuf};

use super::*;
use crate::core::{self, HintError, HR};
use crate::value::J;
use crate::{artifacts, branch, evidence, naming, template};
use crate::{io, jd};

// ── 자체검사 상수 ──────────────────────────────────────────────────────────────────────────────
pub const _MODULES: [&str; 10] = ["core", "naming", "pii", "branch", "tag", "catalog", "lineage", "artifacts", "template", "evidence"];
pub const _FX_PUBLISH_UTC: &str = "2026-01-02T06:00:00Z";
pub const _FX_REPLAY_UTC: &str = "2026-01-02T06:30:00Z";
pub const _FX_CONTINUE_UTC: &str = "2026-01-02T07:00:00Z";
pub const _FX_ENV_MTIME_EPOCH: i64 = 1767225600;
pub const _FX_TESTLOG: &str = "docs/testlog/testlog_26010213_fixture.md";
pub const _FX_WALL_SIG: &str = "ValueError: fixture backend is disabled for this configuration.";
pub const _FX_MISDIAG_SIG: &str = "당시 판정: hang 은 모델 결함이다";
pub const _FX_LOG_REL: &str = "output/multi/benchlog/sweep_c1-a/master_c1-a.log";
pub const _FX_NIC_IFACE: &str = "enpfx7s0";
pub const _FX_NIC_HCA: &str = "rocefx7p1";
pub fn _FX_LOG() -> String {
    format!(
        "INFO start\n\x1b[36m(RayWorkerProc pid=7)\x1b[0m worker ready\r\nLoading 10%\rLoading 50%\rLoading 100%\n\
         NCCL INFO NET/IB : Using [0]{_FX_NIC_HCA}:1/RoCE ; OOB {_FX_NIC_IFACE}\n\
         AttributeError: 'tuple' object has no attribute 'start'\n"
    )
}
pub const _GIT_ENV_LEAKS: [&str; 8] = [
    "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR",
    "GIT_NAMESPACE", "GIT_PREFIX",
];

// ── 검사 기록기 ────────────────────────────────────────────────────────────────────────────────
/// `ck(name, cond)` with its `bad` list and `ran` counter.
pub(super) struct Ck {
    pub bad: Vec<String>,
    pub ran: usize,
}

impl Ck {
    pub fn ck(&mut self, name: &str, cond: bool) {
        self.ran += 1;
        io::outln(&format!("  {} {name}", if cond { "ok  " } else { "FAIL" }));
        if !cond {
            self.bad.push(name.to_string());
        }
    }
}

/// `_raises(fn, code)` → (code matched, `CODE: message[:300]` | "예외 없음"). stdout/stderr of `f` are discarded. A Python
/// built-in exception (`py:*`) is not a HintError: it escapes like in the original.
pub(super) fn _raises<T>(f: impl FnOnce() -> HR<T>, code: &str) -> HR<(bool, String)> {
    let (r, _o, _e) = io::capture(f);
    match r {
        Ok(_) => Ok((false, "예외 없음".to_string())),
        Err(e) if e.is_py() => Err(e),
        Err(e) => Ok((e.code == code, format!("{}: {}", e.code, core::head_chars(&e.message, 300)))),
    }
}

/// `quiet(fn, ...)`: stdout and stderr discarded.
pub(super) fn quiet<T>(f: impl FnOnce() -> T) -> T {
    io::capture(f).0
}

/// `s[:n]` (code points).
pub(super) fn h(s: &str, n: usize) -> String {
    core::head_chars(s, n)
}

/// `tempfile.TemporaryDirectory(prefix=...)` (debug: `HINT_SELFTEST_KEEP_TMP=1` keeps it).
pub(super) struct Tmp(Option<core::TempDir>, PathBuf);

impl Tmp {
    pub fn new(prefix: &str) -> Tmp {
        let t = core::TempDir::new(prefix);
        let p = t.path().to_path_buf();
        Tmp(Some(t), p)
    }
    pub fn path(&self) -> &Path {
        &self.1
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        if std::env::var_os("HINT_SELFTEST_KEEP_TMP").is_some() {
            if let Some(t) = self.0.take() {
                io::errln(&format!("[hint] self-test kept {}", core::posix(t.path())));
                std::mem::forget(t);
            }
        }
    }
}

/// A Python exception (`RuntimeError(...)` etc.).
pub(super) fn py(ty: &str, msg: impl Into<String>) -> HintError {
    HintError::py(ty, msg)
}

/// Remove env vars for a scope (`{k: os.environ.pop(k, None) for k in ...}`); [`EnvGuard::restore`] puts them back.
pub(super) struct EnvGuard(Vec<(String, Option<std::ffi::OsString>)>);

impl EnvGuard {
    pub fn pop(keys: &[&str]) -> EnvGuard {
        let saved = keys
            .iter()
            .map(|k| {
                let v = std::env::var_os(k);
                std::env::remove_var(k);
                (k.to_string(), v)
            })
            .collect();
        EnvGuard(saved)
    }
    /// `mock.patch.dict(os.environ, {k: v})`.
    pub fn set(pairs: &[(&str, &str)]) -> EnvGuard {
        let saved = pairs
            .iter()
            .map(|(k, v)| {
                let old = std::env::var_os(k);
                std::env::set_var(k, v);
                (k.to_string(), old)
            })
            .collect();
        EnvGuard(saved)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in self.0.drain(..).rev() {
            match v {
                Some(v) => std::env::set_var(&k, v),
                None => std::env::remove_var(&k),
            }
        }
    }
}

// ── 모듈 자체검사 ──────────────────────────────────────────────────────────────────────────────
fn module_selftest(name: &str) -> Vec<String> {
    match name {
        "core" => crate::core::selftest(),
        "naming" => crate::naming::selftest(),
        "pii" => crate::pii::selftest(),
        "branch" => crate::branch::selftest(),
        "tag" => crate::tag::selftest(),
        "catalog" => crate::catalog::selftest(),
        "lineage" => crate::lineage::selftest(),
        "artifacts" => crate::artifacts::selftest(),
        "template" => crate::template::selftest(),
        _ => crate::evidence::selftest(),
    }
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

/// `_selftest_modules()` — each module's own `selftest()` (stderr discarded · a crash counts as a failure).
fn _selftest_modules() -> Vec<String> {
    let mut bad = Vec::new();
    for name in _MODULES {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = io::quiet_stderr(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| module_selftest(name))));
        std::panic::set_hook(prev_hook);
        let res = match r {
            Ok(v) => v,
            Err(p) => vec![format!("{name}: selftest 가 예외로 죽었다 Panic: {}", panic_text(p.as_ref()))],
        };
        io::outln(&format!("  {} hintlib.{name}.selftest ({} 실패)", if res.is_empty() { "ok  " } else { "FAIL" }, res.len()));
        bad.extend(res);
    }
    bad
}

// ── 파서 ──────────────────────────────────────────────────────────────────────────────────────
fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn parser_cases() -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("publish", sv(&["publish", "--campaign", "c", "--cell", "x", "--generated-utc", _FX_PUBLISH_UTC])),
        ("continue", sv(&["continue", "--draft", "d", "--generated-utc", _FX_CONTINUE_UTC])),
        ("lint", sv(&["lint", "--draft", "d"])),
        ("refresh", sv(&["refresh", "--draft", "d"])),
        ("excerpt", sv(&["excerpt", "--draft", "d", "--source", "s", "--lines", "1-2"])),
        ("name", sv(&["name", "--publication", "t"])),
        ("verify", sv(&["verify", "--tag", "hint/a/b/c/d"])),
        ("push", sv(&["push", "--tag", "hint/a/b/c/d"])),
        ("catalog", sv(&["catalog", "derive", "--remote", "r", "--generated-kst", "2026-01-02T15:00:00", "--record-missing"])),
        ("match", sv(&["match", "--vllm", "0.9.0", "--model", "m"])),
        (
            "branch-transition",
            sv(&[
                "branch-transition", "--remote", "r", "--generated-utc", _FX_CONTINUE_UTC, "--approved-by", "사용자 발화 전사 — 픽스처",
                "--approved-utc", _FX_CONTINUE_UTC,
            ]),
        ),
        ("refacts", sv(&["refacts", "--draft", "d"])),
    ]
}

/// The sub-commands `main` dispatches (the Rust form of `set_defaults(fn=cmd_*)`).
const _HANDLED: [&str; 12] =
    ["publish", "continue", "lint", "refresh", "refacts", "excerpt", "name", "verify", "push", "catalog", "match", "branch-transition"];

fn has_handler(a: &Args) -> bool {
    match a.s("cmd") {
        Some(c) if c == "catalog" => a.s("catalog_cmd").is_some(),
        Some(c) => _HANDLED.contains(&c.as_str()),
        None => false,
    }
}

fn parse(p: &argparse::Parser, argv: &[String]) -> Option<Args> {
    p.parse_args(argv).ok().map(Args)
}

fn sub_help(p: &argparse::Parser, sub: &str, opt: &str) -> String {
    p.subs
        .iter()
        .find(|s| s.name == sub)
        .and_then(|s| s.parser.acts.iter().find(|a| a.opts.iter().any(|o| o == opt)))
        .and_then(|a| a.help.clone())
        .unwrap_or_default()
}

fn _probe_help_ok(text: &str) -> bool {
    text.contains("create+cp") && !["원장 cat 은 관측 실패", "원장 cat 의 `docker run` 도 거부"].iter().any(|x| text.contains(x))
}

fn run_rc(r: &std::rc::Rc<dyn artifacts::DockerRunner>, argv: &[&str]) -> (i32, String) {
    match r.run(&sv(argv), 60) {
        Ok(p) => (p.returncode, p.stderr),
        Err(_) => (-1, String::new()),
    }
}

fn _selftest_parser(c: &mut Ck) -> HR<()> {
    let p = build_parser();
    let cases = parser_cases();
    let case = |n: &str| cases.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone()).unwrap_or_default();
    for (name, argv) in &cases {
        let mut full = sv(&["--repo", "/nonexistent"]);
        full.extend(argv.iter().cloned());
        let ok = match parse(&p, &full) {
            Some(a) => a.s("cmd").as_deref() == Some(name) && has_handler(&a) && a.s("repo").as_deref() == Some("/nonexistent"),
            None => false,
        };
        c.ck(&format!("파서: `{name}` 최소 인자 · 전역 --repo 는 서브명령 앞"), ok);
    }
    let rem = |n: &str| parse(&p, &case(n)).and_then(|a| a.s("remote"));
    c.ck("파서: continue·push 의 --remote 기본값 origin", rem("continue").as_deref() == Some("origin") && rem("push").as_deref() == Some("origin"));
    c.ck(
        "파서: _build_parser 소스에 `default=\"origin\"` 리터럴(정책 LAST_GOOD C3)",
        include_str!("mod.rs").contains(".default(\"origin\")"),
    );
    let mut gate: Vec<(&str, &str)> = HINT_ACTION_FOR_CMD.to_vec();
    gate.sort();
    c.ck(
        "게이트 매핑 = continue·verify·push 3종(match·catalog·publish 무게이트 · C6)",
        gate == vec![("continue", "hint_finalize"), ("push", "hint_push"), ("verify", "hint_verify")],
    );
    let pr = artifacts::_runner_from_j(&docker_runner(false))?.ok_or_else(|| py("TypeError", "probe runner"))?;
    let ro = artifacts::_runner_from_j(&docker_runner(true))?.ok_or_else(|| py("TypeError", "read-only runner"))?;
    c.ck(
        "탐침 실행기만 image_probe 능력을 선언한다(artifacts.PROBE_ATTR · --docker-read-only 는 탐침 ✗)",
        pr.image_probe() && !ro.image_probe(),
    );
    let mut runners = Vec::new();
    for fl in [&[][..], &["--docker-probe"][..], &["--docker-read-only"][..]] {
        let mut argv = case("publish");
        argv.extend(sv(fl));
        let r = match parse(&p, &argv) {
            Some(a) => artifacts::_runner_from_j(&_publish_runner(&a))?,
            None => None,
        };
        runners.push(r);
    }
    c.ck(
        "★publish 실행기: 플래그 없음 = 탐침(image_probe) · --docker-probe = 같은 탐침(no-op 별칭) · --docker-read-only = 탐침 ✗ · None ✗",
        runners.iter().all(Option::is_some)
            && runners[0].as_ref().is_some_and(|r| r.image_probe())
            && runners[1].as_ref().is_some_and(|r| r.image_probe())
            && runners[2].as_ref().is_some_and(|r| !r.image_probe()),
    );
    c.ck(
        "★기본 실행기도 컨테이너를 시작하지 않는다(run · start 거부 rc 125)",
        runners[0].as_ref().is_some_and(|r| ["run", "start"].iter().all(|x| run_rc(r, &["docker", x, "x"]).0 == 125)),
    );
    c.ck(
        "★--docker-probe: run · start · exec · build 는 실행하지 않고 rc 125",
        ["run", "start", "exec", "build", "pull", "compose"].iter().all(|x| run_rc(&pr, &["docker", x, "x"]).0 == 125),
    );
    c.ck(
        "★--docker-probe: `--pull never` 없는 create 거부 · 만들지 않은 컨테이너의 cp · rm 거부(rc 125 · 실행 0)",
        run_rc(&pr, &["docker", "create", "img"]).0 == 125
            && run_rc(&pr, &["docker", "cp", "-L", "deadbeefdeadbeef:/x", "/tmp/y"]).0 == 125
            && run_rc(&pr, &["docker", "rm", "-f", "deadbeefdeadbeef"]).0 == 125,
    );
    c.ck(
        "★탐침 실행기 = artifacts 규칙 그대로: `--network none` 없는 create 도 거부(rc 125 · 실행 0)",
        run_rc(&pr, &["docker", "create", "--pull", "never", "--entrypoint", "/bin/true", "img"]).0 == 125
            && run_rc(&pr, &["docker", "create", "img"]).1.contains("hintlib.artifacts"),
    );
    let probe_help = sub_help(&p, "publish", "--docker-probe");
    c.ck(
        "--docker-probe 도움말 · 실행기 docstring = 원장도 create+cp 로 읽는다(옛 '원장 cat 관측 실패' 문구 ✗)",
        _probe_help_ok(&probe_help) && _probe_help_ok(_DOCKER_PROBE_ONLY_DOC),
    );
    c.ck(
        "★음성대조: 옛 도움말 문구('원장 cat 은 관측 실패로 남는다')는 같은 술어가 거부한다",
        !_probe_help_ok("docker 는 … create+cp · rm 만 — run/start ✗(원장 cat 은 관측 실패로 남는다)"),
    );
    let fails = |argv: Vec<String>| quiet(|| p.parse_args(&argv).is_err());
    c.ck(
        "★--docker-read-only 와 --docker-probe 는 함께 쓰지 않는다",
        fails(sv(&["publish", "--campaign", "c", "--cell", "x", "--generated-utc", _FX_PUBLISH_UTC, "--docker-read-only", "--docker-probe"])),
    );
    c.ck("★catalog 는 하위 명령 필수", fails(sv(&["catalog"])));
    c.ck(
        "★branch-transition 은 --approved-by · --approved-utc 필수",
        fails(sv(&["branch-transition", "--remote", "r", "--generated-utc", _FX_CONTINUE_UTC])),
    );
    c.ck("--node 도움말 = 생략 시 evidence 자동 파생(cluster · 모호 = 차단)", sub_help(&p, "publish", "--node").contains("자동 파생"));
    let reads: Vec<(&str, Vec<&str>)> = vec![
        (
            "publish",
            vec![
                "campaign", "cell", "publication", "replay", "node", "generated_utc", "lineage_add", "out", "remote", "no_remote_check",
                "docker_read_only", "docker_probe",
            ],
        ),
        (
            "continue",
            vec![
                "draft", "campaign", "cell", "node", "generated_utc", "approved_by", "approved_utc", "remote", "no_push", "factcheck_waiver",
                "guide_commit",
            ],
        ),
        ("refresh", vec!["draft"]),
        ("refacts", vec!["draft", "docker_read_only", "docker_probe"]),
        ("push", vec!["tag", "remote", "apply", "draft"]),
        ("verify", vec!["tag", "draft"]),
        ("lint", vec!["draft", "json"]),
        ("excerpt", vec!["draft", "source", "lines", "numbered"]),
        ("name", vec!["campaign", "cell", "publication", "node", "json"]),
        ("catalog", vec!["remote", "generated_kst", "dry_run", "record_missing", "allow_empty"]),
        ("match", vec!["vllm", "model", "arch", "include_other", "json"]),
        ("branch-transition", vec!["remote", "generated_utc", "approved_by", "approved_utc"]),
    ];
    for (name, attrs) in reads {
        let a = parse(&p, &case(name));
        let miss: Vec<&str> = attrs.iter().copied().filter(|x| a.as_ref().map_or(true, |a| !a.0.contains_key(*x))).collect();
        let tail = if miss.is_empty() { String::new() } else { util::list_repr(&miss) };
        c.ck(&format!("파서 배선: `{name}` 핸들러가 읽는 속성 전부 존재{tail}"), miss.is_empty());
    }
    Ok(())
}

// ── 정적 문서 교차검증 ─────────────────────────────────────────────────────────────────────────
fn generations() -> Vec<String> {
    template::NAME_GENERATIONS().items().iter().map(|g| g.at(0).display()).collect()
}

/// `_branch_readme_drift(text)` → the missing fragments.
pub fn _branch_readme_drift(text: &str) -> HR<Vec<String>> {
    let mut need: Vec<String> = template::BANNER_LINES.iter().map(|s| s.to_string()).collect();
    need.push(branch::PAYLOAD_FORMAT.to_string());
    need.extend(generations());
    let mut miss: Vec<String> = need.into_iter().filter(|x| !text.contains(x.as_str())).collect();
    let marker = branch::guide_marker(branch::BRANCH_FORMAT)?;
    if text.split('\n').next().unwrap_or("") != marker {
        miss.insert(0, marker);
    }
    Ok(miss)
}

fn _selftest_static_docs(c: &mut Ck) -> HR<()> {
    let rd = core::templates_dir().join("hint-branch-README.md");
    let text = if rd.is_file() { std::fs::read_to_string(&rd).unwrap_or_default() } else { String::new() };
    let miss = _branch_readme_drift(&text)?;
    let head2: Vec<&String> = miss.iter().take(2).collect();
    c.ck(
        &format!(
            "hint 브랜치 README = 배너 {}줄 + 형식 `{}` 글자 그대로(빠짐 {})",
            template::BANNER_LINES.len(),
            branch::PAYLOAD_FORMAT,
            util::list_repr(&head2.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        ),
        !text.is_empty() && miss.is_empty(),
    );
    c.ck(
        "★README 에서 배너 한 줄을 지우면 교차검증이 잡는다",
        !text.is_empty() && _branch_readme_drift(&text.replace(template::BANNER_LINES[0], ""))? == vec![template::BANNER_LINES[0].to_string()],
    );
    let marker = branch::guide_marker(branch::BRANCH_FORMAT)?;
    c.ck(
        &format!("안내 README 첫 줄 = 형식 마커 `{marker}`(branch.readme_format = {})", branch::BRANCH_FORMAT),
        branch::readme_format(&J::Str(text.clone()))?.as_deref() == Some(branch::BRANCH_FORMAT),
    );
    let rest = text.split_once('\n').map(|x| x.1).unwrap_or("");
    c.ck(
        "★마커 줄을 지우면 교차검증이 잡는다",
        !text.is_empty() && _branch_readme_drift(rest)?.first() == Some(&marker),
    );
    c.ck("★세대 라벨(v7)을 지우면 교차검증이 잡는다", _branch_readme_drift(&text.replace("v7", "vX"))?.iter().any(|x| x == "v7"));
    Ok(())
}

// ── FACT 생산자 교정 ───────────────────────────────────────────────────────────────────────────
fn _selftest_build_doc_local_digest(c: &mut Ck) {
    let a64 = format!("sha256:{}", "a".repeat(64));
    let loc_ok = jd! {"id" => a64.clone(), "same_build" => true, "source" => "docker image inspect fx .Id · 측정 전 빌드"};
    let b1 = _build_doc(&jd! {"build_identity" => jd! {"image_digest" => J::Null, "image_digest_local" => loc_ok.clone(), "source" => J::dict()}}, &J::dict());
    c.ck(
        "F2 측정 digest 없음 + 로컬 이미지 측정 전 빌드 → 값 = 로컬 Id · 출처 = inspect 관측",
        b1["image_digest"] == loc_ok["id"] && b1["source"]["image_digest"] == loc_ok["source"],
    );
    let mut loc2 = loc_ok.clone();
    loc2.set("same_build", false);
    loc2.set("source", "측정 뒤 빌드");
    let b2 = _build_doc(&jd! {"build_identity" => jd! {"image_digest" => J::Null, "image_digest_local" => loc2, "source" => J::dict()}}, &J::dict());
    c.ck(
        "★F2 음성대조: 측정 뒤 빌드면 값 ✗ · 관측은 image_digest_local_note 로만",
        b2["image_digest"].is_null() && b2["source"].get("image_digest_local_note") == Some(&J::from("측정 뒤 빌드")),
    );
    let b64 = format!("sha256:{}", "b".repeat(64));
    let b3 = _build_doc(
        &jd! {"build_identity" => jd! {"image_digest" => b64.clone(), "image_digest_local" => loc_ok,
                                      "source" => jd! {"image_digest" => "sweep meta(measured)"}}},
        &J::dict(),
    );
    c.ck(
        "★F2 음성대조: 측정 digest 가 있으면 로컬 관측이 덮지 않는다",
        b3["image_digest"] == J::from(b64) && b3["source"]["image_digest"] == J::from("sweep meta(measured)"),
    );
}

fn set_path(v: &mut J, keys: &[&str], val: J) {
    let mut cur = v;
    for k in &keys[..keys.len() - 1] {
        if cur.get(k).is_none() {
            cur.set(*k, J::dict());
        }
        cur = cur.get_mut(k).unwrap();
    }
    cur.set(keys[keys.len() - 1], val);
}

fn strs(v: &J) -> Vec<String> {
    v.items().iter().map(J::display).collect()
}

/// `unresolved(...)` of the late-naming check: the real evidence with resolve · attestation · build SHA unobserved.
fn unresolved(repo: &Path, campaign: &J, cell: &J, node: &J, docker: &J) -> HR<J> {
    let mut ev = evidence::from_campaign(repo, campaign, cell, node, docker)?;
    ev.set("attestation", J::Null);
    ev.set("resolve", J::Null);
    set_path(&mut ev, &["build_identity", "vllm_sha"], J::Null);
    set_path(&mut ev, &["build_identity", "source", "vllm_sha"], J::from("미관측(fixture unresolved build SHA)"));
    let mut m: std::collections::BTreeSet<String> = strs(&ev["missing"]).into_iter().collect();
    m.insert("HINT_MISSING_RESOLVE".into());
    ev.set("missing", J::from(m.into_iter().collect::<Vec<String>>()));
    Ok(ev)
}

fn _selftest_collect_late_naming(c: &mut Ck) -> HR<()> {
    use super::selftest_e2e::{_fx_docker, _fx_repo, _fx_tree, _refs};
    use std::cell::RefCell;
    use std::rc::Rc;
    let late_sha = "d".repeat(40);
    let view = jd! {"vllm" => jd! {"git_sha" => late_sha.clone(), "describe" => "v0.9.0"},
                    "source" => "fixture image:/opt/easy-vllm/build_ledger.json", "provenance" => "fixture-observed"};
    let live_view: Rc<RefCell<J>> = Rc::new(RefCell::new(J::Null));
    let live_drift = Rc::new(RefCell::new(false));
    let td = Tmp::new("hint-late-naming-");
    let _env = EnvGuard::set(&[("GIT_ALLOW_PROTOCOL", "file")]);
    let tdp = td.path().to_path_buf();
    let (repo, fx) = _fx_repo(&tdp)?;
    let _docker = _fx_docker(&tdp, &fx["docker"])?;
    let docker = docker_runner(true);
    let (lv, ld) = (live_view.clone(), live_drift.clone());
    let observed: super::hooks::CollectFn = Rc::new(move |repo: &Path, ev: &mut J, payload: &Path, runner: &J| {
        let mut art = artifacts::collect(repo, ev, payload, &J::Null, &J::Null, runner)?;
        art.set("build_ledger_vllm", lv.borrow().clone());
        if *ld.borrow() {
            // Input changes while collecting must not pair the new name with the caller's old tag, nor bind a
            // publication record first.
            set_path(ev, &["build_identity", "vllm_ref"], J::from("v0.9.1"));
        }
        Ok(art)
    });
    let collected = |ev: &mut J, dn: &naming::DerivedName, label: &str| -> HR<J> {
        let draft = tdp.join(label);
        let topic = evidence::topic_for(ev)?;
        _collect(&repo, ev, dn, _FX_PUBLISH_UTC, &topic, &draft, &draft.join("payload"), &[], &docker)
    };
    let nfj = |v: &J| -> J { v.clone() };
    let _ = nfj;
    super::hooks::with(
        |h| h.collect = Some(observed.clone()),
        || -> HR<()> {
            let vocab = naming::load_vocab(&repo)?;
            let mut ev = unresolved(&repo, &J::from("c1"), &J::from("c1-a"), &J::Null, &docker)?;
            let dn = naming::derive_name(&evidence::naming_facts(&repo, &ev)?, &vocab)?;
            let empty = collected(&mut ev, &dn, "no-ledger")?;
            c.ck(
                "late naming: no ledger keeps SHA unobserved and missing resolve",
                empty["naming_base"]["vllm_build_input"]["sha"].is_null() && strs(&empty["missing"]).iter().any(|m| m == "HINT_MISSING_RESOLVE"),
            );
            *live_view.borrow_mut() = view.clone();
            let late = collected(&mut ev, &dn, "late-ledger")?;
            c.ck(
                "late naming: observed SHA reaches naming with source and unchanged release name",
                late["naming_base"]["vllm_build_input"]["sha"] == J::from(late_sha.clone())
                    && late["naming_base"]["segments"]["vllm"]["source"].display().contains("fixture image:")
                    && naming::compose_from_payload(&late["naming_base"])? == dn.tag
                    && strs(&late["missing"]).iter().any(|m| m == "HINT_MISSING_RESOLVE"),
            );
            for (label, attested) in [("resolved", false), ("attestation", true)] {
                let mut prior = evidence::from_campaign(&repo, &J::from("c1"), &J::from("c1-a"), &J::Null, &docker)?;
                if attested {
                    prior.set("resolve", J::Null);
                    prior.set(
                        "attestation",
                        jd! {"_path" => "fixture attestation", "build_ledger" => jd! {
                            "vllm" => jd! {"git_sha" => evidence::_SHA_A, "describe" => "v0.9.0"}}},
                    );
                    set_path(&mut prior, &["build_identity", "vllm_sha"], J::Null);
                    evidence::bind_image_ledger_mut(&mut prior, &J::Null)?;
                } else {
                    prior.set("attestation", J::Null); // Exercise resolved priority without an attestation masking the image.
                }
                let prior_source = prior["build_identity"]["source"]["vllm_sha"].clone();
                let original = naming::derive_name(&evidence::naming_facts(&repo, &prior)?, &vocab)?;
                let retained = collected(&mut prior, &original, label)?;
                let Some(ps) = prior_source.as_str() else {
                    return Err(py("TypeError", "'in <string>' requires string as left operand, not NoneType"));
                };
                c.ck(
                    &format!("late naming: existing {label} SHA has priority over image SHA"),
                    retained["naming_base"]["vllm_build_input"]["sha"] == J::from(evidence::_SHA_A)
                        && retained["naming_base"]["segments"]["vllm"]["source"].display().contains(ps)
                        && naming::compose_from_payload(&retained["naming_base"])? == original.tag,
                );
            }
            let mut commit = evidence::from_campaign(&repo, &J::from("c1"), &J::from("c1-a"), &J::Null, &docker)?;
            set_path(&mut commit, &["build_identity", "vllm_ref"], J::from(evidence::_SHA_A));
            commit.set("attestation", jd! {"build_ledger" => jd! {"vllm" => jd! {"git_sha" => evidence::_SHA_A, "describe" => "v0.9.0"}}});
            let commit_name = naming::derive_name(&evidence::naming_facts(&repo, &commit)?, &vocab)?;
            let stable = collected(&mut commit, &commit_name, "commit")?;
            c.ck(
                "late naming: commit identity remains consistent with SHA-derived name",
                stable["naming_base"]["vllm_build_input"]["kind"] == J::from("commit")
                    && stable["naming_base"]["vllm_build_input"]["sha"] == J::from(evidence::_SHA_A)
                    && naming::compose_from_payload(&stable["naming_base"])? == commit_name.tag,
            );
            let unres: super::hooks::FromCampaignFn = Rc::new(unresolved);
            super::hooks::with(
                |h| h.from_campaign = Some(unres.clone()),
                || -> HR<()> {
                    *live_view.borrow_mut() = J::Null;
                    let before_refs = _refs(&repo, false)?;
                    let out = core::posix(&tdp.join("refacts-flow"));
                    let pa = |out: &str| -> HR<J> {
                        let o = out.to_string();
                        quiet(|| {
                            publish(
                                &repo,
                                &PublishArgs {
                                    campaign: Some("c1"),
                                    cell: Some("c1-a"),
                                    publication: None,
                                    replay: false,
                                    node: None,
                                    generated_utc: Some(_FX_PUBLISH_UTC),
                                    lineage_add: &[],
                                    out: Some(&o),
                                    remote: "origin",
                                    remote_check: false,
                                    docker: docker.clone(),
                                },
                            )
                        })
                    };
                    let result = pa(&out)?;
                    let draft = PathBuf::from(result["draft"].display());
                    let prior_base = _load_state(&draft)?["base_tag"].clone();
                    *live_view.borrow_mut() = view.clone();
                    quiet(|| refacts(&repo, &draft, &docker))?;
                    let nb = core::read_json(&draft.join("inputs").join(NAMING_BASE_NAME))?;
                    let payload = core::read_json(&draft.join("payload").join(branch::PAYLOAD_JSON))?;
                    let facts = core::read_json(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
                    let st = _load_state(&draft)?;
                    c.ck(
                        "late naming: refacts updates naming_base/PAYLOAD/FACT together without renaming or resolving missing input",
                        nb["vllm_build_input"]["sha"] == J::from(late_sha.clone())
                            && util::py_eq(&payload["naming"], &facts["naming"])
                            && util::py_eq(&facts["naming"], &nb)
                            && payload["tag"] == st["base_tag"]
                            && st["base_tag"] == prior_base
                            && prior_base == J::from(naming::compose_from_payload(&nb)?)
                            && strs(&payload["missing"]).iter().any(|m| m == "HINT_MISSING_RESOLVE")
                            && _refs(&repo, false)? == before_refs,
                    );
                    let published = pa(&core::posix(&tdp.join("publish-flow")))?;
                    let named = core::read_json(&PathBuf::from(published["draft"].display()).join("inputs").join(NAMING_BASE_NAME))?;
                    c.ck(
                        "late naming: publish uses post-probe SHA and source with its base tag",
                        named["vllm_build_input"]["sha"] == J::from(late_sha.clone())
                            && named["segments"]["vllm"]["source"].display().contains("fixture image:")
                            && published["tag"] == J::from(naming::compose_from_payload(&named)?),
                    );
                    let old_doc = fx["testlog"].display();
                    let new_doc = "docs/testlog/testlog_26010213_05_00_fixture.md".to_string();
                    std::fs::rename(repo.join(&old_doc), repo.join(&new_doc)).map_err(|e| core::io_err(&e, &repo.join(&old_doc)))?;
                    core::run_owner(
                        &repo,
                        core::REL_CAMPAIGN_INIT,
                        &sv(&[
                            "--campaign-id", "c1", "--evidence-relocate", "--kind", "testlog", "--path", &old_doc, "--to", &new_doc,
                            "--relocate-reason", "fixture documented rename", "--utc", _FX_PUBLISH_UTC,
                        ]),
                        None,
                        &[],
                        true,
                    )?;
                    let renamed = quiet(|| refacts(&repo, &draft, &docker))?;
                    let state = _load_state(&draft)?;
                    let manifest = core::read_json(&repo.join(state["manifest"].display()))?;
                    let pii_doc = core::read_json(&draft.join("inputs/pii.json"))?;
                    let full_lineage = core::read_json(&draft.join("inputs").join(crate::lineage::LINEAGE_FULL_NAME))?;
                    let paths: Vec<J> = full_lineage["documents"].items().iter().map(|d| d.get("path").cloned().unwrap_or(J::Null)).collect();
                    let ev_dump = crate::value::dumps_default(&manifest["evidence"]);
                    let (new_name, old_name) = (util::py_basename(&new_doc), util::py_basename(&old_doc));
                    let scanned = strs(&pii_doc["scanned_paths"]);
                    c.ck(
                        "late naming: refacts after official narrative relocation updates LINEAGE/manifest/PII to the new document",
                        util::py_eq(&renamed["rebind"]["rebound"], &crate::jl![jd! {"kind" => "testlog", "from" => old_doc.clone(), "to" => new_doc.clone()}])
                            && paths.contains(&J::from(new_doc.clone()))
                            && !paths.contains(&J::from(old_doc.clone()))
                            && ev_dump.contains(&new_name)
                            && !ev_dump.contains(&old_name)
                            && scanned.iter().any(|p| p.ends_with(&new_name))
                            && !scanned.iter().any(|p| p.ends_with(&old_name)),
                    );
                    *live_drift.borrow_mut() = true;
                    let guard_pub: super::hooks::DrivePublisherFn =
                        Rc::new(|_, _, _, _, _| Err(py("AssertionError", "publication bound before drift rejection")));
                    let (ok, why) = super::hooks::with(
                        |h| h.drive_publisher = Some(guard_pub.clone()),
                        || _raises(|| pa(&core::posix(&tdp.join("drift-flow"))), "HINT_DERIVED_NAME_MISMATCH"),
                    )?;
                    c.ck(&format!("late naming: publish drift fails before publication binding (draft-local writes allowed): {why}"), ok);
                    let snapshot = _fx_tree(&draft);
                    let guard_rb: super::hooks::RebindFn =
                        Rc::new(|_, _, _, _, _| Err(py("AssertionError", "publication rebound before drift rejection")));
                    let (ok, why) = super::hooks::with(
                        |h| h.rebind_campaign_narratives = Some(guard_rb.clone()),
                        || _raises(|| quiet(|| refacts(&repo, &draft, &docker)), "HINT_DERIVED_NAME_MISMATCH"),
                    )?;
                    c.ck(
                        &format!("late naming: refacts drift fails before publication rebind and keeps existing payload: {why}"),
                        ok && _fx_tree(&draft) == snapshot && _refs(&repo, false)? == before_refs,
                    );
                    Ok(())
                },
            )
        },
    )
}

// ── 본체 ──────────────────────────────────────────────────────────────────────────────────────
/// A Python exception escaping the self-test (the original prints a traceback and exits 1).
fn crashed(e: &HintError) -> i32 {
    io::errln("Traceback (most recent call last):");
    if e.is_py() {
        io::errln(&format!("{}: {}", e.code.trim_start_matches("py:"), e.message));
    } else {
        io::errln(&format!("hintlib.core.HintError: {}: {}", e.code, e.message));
    }
    1
}

fn _self_test_body(c: &mut Ck) -> HR<()> {
    io::outln(&format!("{_OUT} self-test — hintlib 모듈 자체검사"));
    let mb = _selftest_modules();
    c.bad.extend(mb);
    io::outln(&format!("{_OUT} self-test — CLI 파서"));
    _selftest_parser(c)?;
    io::outln(&format!("{_OUT} self-test — 정적 문서 교차검증"));
    _selftest_static_docs(c)?;
    io::outln(&format!("{_OUT} self-test — FACT 생산자 교정(plan_26092908 §4.5 F2)"));
    _selftest_build_doc_local_digest(c);
    _selftest_collect_late_naming(c)?;
    io::outln(&format!("{_OUT} self-test — 격리 E2E(임시 저장소 · bare 원격 · 가짜 docker · 라이브 비의존)"));
    let r = super::selftest_e2e::_selftest_e2e(c)
        .and_then(|_| super::selftest_e2e::_selftest_replay_continue(c))
        .and_then(|_| super::selftest_e2e::_selftest_lite_e2e(c));
    if let Err(e) = r {
        // 픽스처 사고도 실패로 센다 (traceback.print_exc() → stderr)
        io::errln("Traceback (most recent call last):");
        let (ty, msg) = if e.is_py() { (e.code.trim_start_matches("py:").to_string(), e.message.clone()) } else { ("HintError".to_string(), e.to_string()) };
        io::errln(&format!("{ty}: {msg}"));
        c.ck(&format!("E2E 픽스처 사고: {ty}: {msg}"), false);
    }
    let ran = c.ran;
    c.ck(&format!("검사 전수 실행({ran}) — 중도 반환으로 시험이 조용히 줄지 않게"), ran >= 188);
    Ok(())
}

/// `self_test()` → exit code.
pub fn self_test() -> i32 {
    let mut c = Ck { bad: Vec::new(), ran: 0 };
    let r = {
        let _git_env = EnvGuard::pop(&_GIT_ENV_LEAKS);
        let _bins = super::selftest_e2e::owner_bin_env();
        _self_test_body(&mut c)
    };
    if let Err(e) = r {
        return crashed(&e);
    }
    let verdict = if c.bad.is_empty() { "PASS".to_string() } else { format!("FAIL({})", c.bad.len()) };
    io::outln(&format!("{_OUT} self-test {verdict}"));
    if c.bad.is_empty() {
        0
    } else {
        1
    }
}
