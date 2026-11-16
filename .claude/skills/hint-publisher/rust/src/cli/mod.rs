//! hint — hint-publisher 단일 CLI. Port of `scripts/hint.py` lines 1–2047 (contract constants, gate wrapper, draft state
//! helpers, publish / continue / refacts / lint / refresh / excerpt / name / verify / push / catalog derive|classify /
//! match / branch-transition, the argparse parser and `main`). The self-test (`--self-test`) lives in [`self_test`].
//!
//! Invariants are the original's (see its module docstring): side-effect-free gate before any ref moves, approval first,
//! ACTIVE never read, gate scope = this tag, push = one exact refspec, injected time only, judgement by code only,
//! replay reads only, `match` is gitless.
//!
//! Intended differences (port boundary): the CLI path printed in instructions is `bash <REL_HINT_CLI>` (the Rust launcher
//! `scripts/hint.sh`) instead of `python3 scripts/hint.py`; owner CLIs run through their native launchers
//! (`core::run_owner`). The docker runners are passed to the evidence/artifacts owners as markers (see
//! [`docker_runner`]); the narrative linter is handed to `tag.verify_local` / `tag.push_tag` as a typed closure.
mod argparse;
pub mod hooks;
mod ops;
mod selftest;
mod selftest_e2e;
mod util;

pub use ops::*;

/// The self-test's E2E fixture assemblers, for the policies runtime self-check (`runtime_selftest` · the original imported
/// `hint._fx_repo` / `_fx_legacy_remote` / `_fx_author` / `_fx_tree` in-process — one assembler, never a second copy).
/// Read-only surface: thin wrappers, no behaviour of their own.
pub mod fixtures {
    use crate::core::HR;
    use crate::value::J;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// `hint._fx_repo(td)` -> (repo, fx).
    pub fn fx_repo(td: &Path) -> HR<(PathBuf, J)> {
        super::selftest_e2e::_fx_repo(td)
    }
    /// `hint._fx_legacy_remote(td, repo, bare)` -> {"old", "remote_only", "lightweight"}.
    pub fn fx_legacy_remote(td: &Path, repo: &Path, bare: &Path) -> HR<J> {
        super::selftest_e2e::_fx_legacy_remote(td, repo, bare)
    }
    /// `hint._fx_author(repo, draft, st)` (factcheck = True, the original's default).
    pub fn fx_author(repo: &Path, draft: &Path, st: &J) -> HR<()> {
        super::selftest_e2e::_fx_author(repo, draft, st, true)
    }
    /// `hint._fx_tree(root)`.
    pub fn fx_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
        super::selftest_e2e::_fx_tree(root)
    }
    /// The owner-launcher overrides the self-test sets (only keys the caller left unset); restored on drop.
    pub struct OwnerEnv(#[allow(dead_code)] super::selftest::EnvGuard);
    pub fn owner_bin_env() -> OwnerEnv {
        OwnerEnv(super::selftest_e2e::owner_bin_env())
    }
}

use crate::core::{self, HintError};
use crate::io;
use argparse::{Act, Ns, Parser, K, V};

// ── 계약 상수 ────────────────────────────────────────────────────────────────────────────────
/// CLI → 승격 게이트 액션(completion_gate ALLOWED_ACTIONS).
pub const HINT_ACTION_FOR_CMD: [(&str, &str); 3] = [("continue", "hint_finalize"), ("verify", "hint_verify"), ("push", "hint_push")];
pub const STATE_NAME: &str = "state.json";
pub const STATE_SCHEMA: i64 = 1;
pub const PAYLOAD_SCHEMA_VERSION: i64 = 3;
pub const PROVENANCE_SCHEMA_VERSION: i64 = 2;
pub const APPROVAL_KEYS: [&str; 3] = ["approved_by", "approved_utc", "source"];
pub const _BUILD_KEYS: [&str; 18] = [
    "track", "dockerfile", "image_tag", "image_digest", "vllm_repo", "vllm_ref", "vllm_sha", "torch", "cuda", "ngc", "cpu_arch",
    "driver", "driver_by_node", "driver_conflict", "driver_check", "os", "vllm_describe", "vllm_prev_pin",
];
pub const _RATIONALE_MAX: usize = 400;
pub const FACTCHECK_NAME: &str = "factcheck.json";
pub const FACTCHECK_SCHEMA: i64 = 1;
pub const FACTCHECK_VERDICTS: [&str; 3] = ["wrong", "misleading", "unsupported"];
pub const FACTCHECK_STATUSES: [&str; 3] = ["open", "fixed", "disputed"];
pub const FACTCHECK_TARGETS: [&str; 2] = ["prose", "fact"];
pub const TAIL_NAME: &str = "tail.json";
pub const NAMING_BASE_NAME: &str = "naming_base.json";
pub const TAIL_SOURCES_NAME: &str = "tail_sources.json";
pub const _OUT: &str = "[hint]";
pub const _UTC_EXAMPLE: &str = "<UTC YYYY-MM-DDTHH:MM:SSZ>";
/// 게이트를 실행조차 못 한 두 경우(결정론 포장 JSON 을 stdout 에 낸다).
pub const _GATE_BARE_CODES: [&str; 2] = ["HINT_GATE_SUBPROCESS_UNAVAILABLE", "HINT_GATE_OUTPUT_UNREADABLE"];
pub const _REFACTS_RESET_STEPS: [&str; 5] = ["name", "approval", "lint", "factcheck", "lineage"];
/// 카탈로그 파생물 두 경로(이 두 경로만 커밋한다).
pub const _CATALOG_PATHS: [&str; 2] = [core::REL_HINTS_MD, core::REL_INDEX];

/// `HINT_ACTION_FOR_CMD[cmd]`.
pub fn action_for_cmd(cmd: &str) -> Option<&'static str> {
    HINT_ACTION_FOR_CMD.iter().find(|(c, _)| *c == cmd).map(|(_, a)| *a)
}

/// `_log(msg)`: `[hint] <msg>` on stderr.
pub fn _log(msg: &str) {
    io::errln(&format!("{_OUT} {msg}"));
}

// ── 파서 ──────────────────────────────────────────────────────────────────────────────────────
fn build_parser() -> Parser {
    let node_axis: Vec<&str> = crate::evidence::NODE_AXIS.to_vec();
    let mut p = Parser::new("hint.py", Some("hint-publisher 단일 CLI — 셀 1개 = 태그 1개(plan_26092119 · v7 plan_26092908)."));
    p.add(Act::opt("--repo", K::Store).help("저장소 루트(기본: 현재 디렉터리의 git 최상위) — 서브명령 앞에 둔다"));
    p.add(Act::opt("--self-test", K::True).help("전 모듈 자체검사 + 파서 검사 + 격리 E2E(라이브 비의존)"));
    p.add_subparsers("cmd", "<command>", false);

    let s = p.add_parser("publish", Some("셀 1개 → 스캐폴드 → 정지(태그·브랜치 부수효과 0)"));
    s.add(Act::opt("--campaign", K::Store).help("캠페인 id(명시 · ACTIVE 를 읽지 않는다)"));
    s.add(Act::opt("--cell", K::Store).help("셀 id"));
    s.add(Act::opt("--publication", K::Store).help("발행 기록 토픽(docs/_evidence/<topic>.json) — --replay 와 함께"));
    s.add(Act::opt("--replay", K::True).help("발행 기록 재생(읽기 전용 · draft 밖 쓰기 0)"));
    s.add(Act::opt("--node", K::Store).choices(&node_axis).help(
        "측정 노드 축(생략 = evidence 자동 파생: 셀 측정 TP > 노드당 GPU 이거나 멀티 serve proof · attestation 이면 cluster · \
         그 밖은 인증서 measured_node — 모호하면 차단 · plan_26092908 §4.8)",
    ));
    s.add(Act::opt("--generated-utc", K::Store).req().help("주입 UTC YYYY-MM-DDTHH:MM:SSZ(벽시계 ✗)"));
    s.add(
        Act::opt("--lineage-add", K::Append)
            .metavar("PATH=REASON")
            .help("간선이 끊긴 계보 문서를 사람이 보충한다(source: declared · 사유 필수)"),
    );
    s.add(Act::opt("--out", K::Store).help("draft 디렉터리(기본 hints/.drafts/<draft_id>/)"));
    s.add(Act::opt("--remote", K::Store).default("origin").help("이름 충돌을 확인할 원격(ls-remote 읽기)"));
    s.add(Act::opt("--no-remote-check", K::True).help("원격 이름 충돌 조회를 건너뛴다(로컬만 · 오프라인)"));
    s.add_mutex(vec![
        Act::opt("--docker-read-only", K::True).help(
            "이미지 탐침을 끈다 — docker 는 image inspect·history 만(시작하지 않는 컨테이너의 create·cp·rm 도 ✗ · 이미지 안 \
             원장 · 패치 사본 관측 실패로 남긴다)",
        ),
        Act::opt("--docker-probe", K::True).help(
            "옛 이름(no-op) — 이미지 탐침은 기본이다: docker 는 image inspect·history + 시작하지 않는 컨테이너의 \
             create(--pull never)·cp·rm 만 — 이미지 파일 탐침 ✓ · 이미지 원장도 같은 create+cp 로 읽는다 · run/start ✗",
        ),
    ]);

    let c = p.add_parser(
        "continue",
        Some(
            "승인 → 린트 · 사실 검증 → 이름 확정(꼬리 · 중복 시 timestamp) → 커밋(부모 = 안내 커밋) → 봉인 → \
             검증 → push → 카탈로그(파생 · 두 경로 커밋)",
        ),
    );
    c.add(Act::opt("--campaign", K::Store));
    c.add(Act::opt("--cell", K::Store));
    c.add(Act::opt("--draft", K::Store).help("publish 가 만든 draft 디렉터리"));
    c.add(Act::opt("--node", K::Store).choices(&node_axis).help("--campaign/--cell 로 draft 를 찾을 때 노드 축으로 좁힌다"));
    c.add(Act::opt("--generated-utc", K::Store).req().help("주입 UTC(첫 커밋 뒤 재실행은 state.json 의 커밋 시각을 쓴다)"));
    c.add(Act::opt("--approved-by", K::Store).help("캠페인 밖 발행: 사람 발화 전사(한 줄)"));
    c.add(Act::opt("--approved-utc", K::Store).help("캠페인 밖 발행: 승인 시각 UTC"));
    c.add(Act::opt("--remote", K::Store).default("origin").help("push·원격 SHA 대조·카탈로그 파생 원격"));
    c.add(Act::opt("--no-push", K::True).help("봉인·로컬 검증까지만"));
    c.add(Act::opt("--factcheck-waiver", K::Store).help("발행 전 독립 사실 검증을 사람이 면제했다: 그 발화 전사(한 줄 · state·00 메타에 남는다)"));
    c.add(Act::opt("--hint-clone", K::Store).metavar("PATH").help(
        "별도 hint 클론(plan_26101104 · single 서브 자체 발행): 페이로드 커밋·봉인·push·원격 대조를 이 저장소에서 한다 \
         (워크스페이스 git 은 원격이 없다 · SUB_GIT_LOCAL_ONLY). 증거·게이트·PII 리터럴·토큰은 워크스페이스에서 읽는다",
    ));
    c.add(
        Act::opt("--guide-commit", K::Store)
            .metavar("SHA")
            .help("오프라인 재생 전용(--no-push 와만): 원격 조회 없이 이 로컬 안내 커밋(v7 형식 마커)을 페이로드 커밋의 부모로 쓴다"),
    );

    let li = p.add_parser("lint", Some("draft 린트(부수효과 0)"));
    li.add(Act::opt("--draft", K::Store).req());
    li.add(Act::opt("--json", K::True));

    let rf = p.add_parser("refresh", Some("draft 안 파생 블록(00 §0.3 · §0.5)을 다시 쓴다(미리 보기 · 태그·브랜치 부수효과 0)"));
    rf.add(Act::opt("--draft", K::Store).req());

    let rx = p.add_parser(
        "refacts",
        Some(
            "같은 generated_utc 로 사실(FACT · PAYLOAD · LINEAGE · PROVENANCE · 산출물)을 재파생 — 산문 · 꼬리 · \
             사실 검증 보존 · 바뀐 FACT diff 출력 · 부수효과 = draft 안뿐",
        ),
    );
    rx.add(Act::opt("--draft", K::Store).req());
    rx.add(Act::opt("--docker-read-only", K::True).help("이미지 탐침을 끈다(publish 와 같은 뜻)"));
    rx.set_default("docker_probe", V::Bool(false));

    let ex = p.add_parser("excerpt", Some("발췌 저작 도우미 — 치환 후 원문 출력(부수효과 0)"));
    ex.add(Act::opt("--draft", K::Store).req());
    ex.add(Act::opt("--source", K::Store).req().help("LINEAGE 문서 stem · 저장소 상대 경로 · artifacts 파일명"));
    ex.add(Act::opt("--lines", K::Store).help("a-b (1-기반 · 포함)"));
    ex.add(Act::opt("--numbered", K::True).help(
        "줄마다 정규화 뒤 번호를 `L<n>` 으로 붙인다 — 비-마크다운 발췌 머리 `§L<a>-<b>` 의 번호(발췌 본문에는 번호를 옮기지 않는다)",
    ));

    let n = p.add_parser("name", Some("파생 이름 · 축별 출처(읽기 전용)"));
    n.add(Act::opt("--campaign", K::Store));
    n.add(Act::opt("--cell", K::Store));
    n.add(Act::opt("--publication", K::Store));
    n.add(Act::opt("--node", K::Store).choices(&node_axis));
    n.add(Act::opt("--json", K::True));

    let v = p.add_parser("verify", Some("이 태그 1개 · push 전 로컬 봉인 검증(D10)"));
    v.add(Act::opt("--tag", K::Store).req());
    v.add(Act::opt("--draft", K::Store).help("그 태그를 봉인한 draft(기본: hints/.drafts 에서 태그로 찾는다)"));
    v.add(Act::opt("--hint-clone", K::Store).metavar("PATH").help("별도 hint 클론에서 태그 오브젝트를 읽고 게이트·출처는 워크스페이스에서 검증"));

    let pu = p.add_parser("push", Some("태그 1개 push(정확한 refspec · 기본 dry-run)"));
    pu.add(Act::opt("--tag", K::Store).req());
    pu.add(Act::opt("--remote", K::Store).default("origin"));
    pu.add(Act::opt("--apply", K::True).help("실제 push(없으면 dry-run — 실패를 삼키지 않는다)"));
    pu.add(Act::opt("--draft", K::Store));

    let ca = p.add_parser("catalog", Some("원격 발행 태그 → hints/index.json + HINTS.md"));
    ca.add_subparsers("catalog_cmd", "<catalog-command>", true);
    let d = ca.add_parser("derive", None);
    d.add(Act::opt("--remote", K::Store).req().help("진실원천 원격(ls-remote)"));
    d.add(Act::opt("--generated-kst", K::Store).req().help("주입 KST YYYY-MM-DDTHH:MM:SS"));
    d.add(Act::opt("--dry-run", K::True));
    d.add(Act::opt("--record-missing", K::True).help("로컬 오브젝트 없는 원격 태그를 `absent-local` 행으로 싣고 rc 0(X15 · continue·S5 재생성)"));
    d.add(Act::opt("--allow-empty", K::True).help("원격 hint 태그 0건을 정상으로 선언"));
    let cl = ca.add_parser("classify", Some("재파생 실패 원인 분류(읽기 전용 · closing_sequence 용)"));
    cl.add(Act::opt("--index", K::Store).req().help("방금 쓴 hints/index.json"));
    cl.add(Act::opt("--rc", K::Store).req().help("재파생 종료코드"));

    let m = p.add_parser("match", Some("수신자 근-미스 발견(읽기 전용 · gitless)"));
    m.add(Act::opt("--vllm", K::Store).req());
    m.add(Act::opt("--model", K::Store).req());
    m.add(Act::opt("--arch", K::Store));
    m.add(Act::opt("--include-other", K::True));
    m.add(Act::opt("--json", K::True));

    let bt = p.add_parser("branch-transition", Some("maintenance-only: README 하나의 hint 브랜치 형식 전환(CAS·정확한 branch push · 사람 승인)"));
    bt.add(Act::opt("--remote", K::Store).req().help("live hint tip을 대조하고 refs/heads/hint 하나만 밀 원격"));
    bt.add(Act::opt("--generated-utc", K::Store).req().help("주입 UTC YYYY-MM-DDTHH:MM:SSZ(벽시계 ✗)"));
    bt.add(Act::opt("--approved-by", K::Store).req().help("형식 전환 승인: 사람 발화 전사(한 줄 · 반환값에만 실린다 — 배포 오브젝트 ✗)"));
    bt.add(Act::opt("--approved-utc", K::Store).req().help("승인 시각 UTC"));
    p
}

/// The shape of the real parser (read-only introspection, the Rust form of the original's
/// `_argparse_subcommands(_build_parser())` + `_option_string_actions`): every top-level subcommand with its option
/// strings and their string defaults (`None` = no string default). Used by the policy predicates
/// (HINT_TAG_ACTIVATION_GATE.C6 · LAST_GOOD_ROLLBACK_ANCHOR.C3).
pub fn parser_shape() -> Vec<(String, Vec<(String, Option<String>)>)> {
    build_parser()
        .subs
        .iter()
        .map(|s| {
            let opts = s
                .parser
                .acts
                .iter()
                .flat_map(|a| {
                    let d = match &a.default {
                        V::Str(v) => Some(v.clone()),
                        _ => None,
                    };
                    a.opts.iter().map(move |o| (o.clone(), d.clone()))
                })
                .collect();
            (s.name.clone(), opts)
        })
        .collect()
}

/// The parsed namespace with typed accessors.
pub struct Args(Ns);

impl Args {
    fn s(&self, k: &str) -> Option<String> {
        match self.0.get(k) {
            Some(V::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }
    fn b(&self, k: &str) -> bool {
        matches!(self.0.get(k), Some(V::Bool(true)))
    }
    fn l(&self, k: &str) -> Vec<String> {
        match self.0.get(k) {
            Some(V::List(l)) => l.clone(),
            _ => Vec::new(),
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────────────────────
/// `main(argv)` → exit code.
pub fn main(args: &[String]) -> i32 {
    let p = build_parser();
    let ns = match p.parse_args(args) {
        Ok(ns) => Args(ns),
        Err(code) => return code,
    };
    if ns.b("self_test") {
        return self_test();
    }
    let Some(cmd) = ns.s("cmd") else {
        io::err(&p.format_help());
        return 2;
    };
    let r = match cmd.as_str() {
        "publish" => cmd_publish(&ns),
        "continue" => cmd_continue(&ns),
        "lint" => cmd_lint(&ns),
        "refresh" => cmd_refresh(&ns),
        "refacts" => cmd_refacts(&ns),
        "excerpt" => cmd_excerpt(&ns),
        "name" => cmd_name(&ns),
        "verify" => cmd_verify(&ns),
        "push" => cmd_push(&ns),
        "catalog" => match ns.s("catalog_cmd").as_deref() {
            Some("derive") => cmd_catalog_derive(&ns),
            _ => cmd_catalog_classify(&ns),
        },
        "match" => cmd_match(&ns),
        _ => cmd_branch_transition(&ns),
    };
    match r {
        Ok(c) => c,
        Err(e) => handle_error(&e),
    }
}

fn handle_error(e: &HintError) -> i32 {
    if e.is_py() {
        // An exception the original does not catch: Python prints a traceback and exits 1.
        io::errln("Traceback (most recent call last):");
        io::errln(&format!("{}: {}", e.code.trim_start_matches("py:"), e.message));
        return 1;
    }
    if e.code == "HINT_GATE_DENIED" {
        // 게이트 거부 = 게이트 JSON 원문을 stdout 에 그대로 + 그 종료코드. 부수효과 0.
        if e.message.ends_with('\n') {
            io::out(&e.message);
        } else {
            io::out(&format!("{}\n", e.message));
        }
        _log(&format!("승격 게이트 거부 — {}", e.remedy.clone().unwrap_or_default()));
        return if e.exit_code != 0 { e.exit_code } else { 1 };
    }
    if let Some(g) = e.gate_json.as_deref().filter(|g| !g.is_empty()) {
        io::out(g);
        io::errln(&e.render());
        return if e.exit_code != 0 { e.exit_code } else { 2 };
    }
    io::errln(&e.render());
    if let Some(p) = &e.problems {
        let codes = crate::tag::problem_codes(p).unwrap_or_default();
        _log(&format!("하위 code: {}", codes.join(", ")));
    }
    if e.exit_code != 0 {
        e.exit_code
    } else {
        1
    }
}

/// `--self-test` (hint.py lines 2048–3720): see [`selftest`].
pub fn self_test() -> i32 {
    selftest::self_test()
}
