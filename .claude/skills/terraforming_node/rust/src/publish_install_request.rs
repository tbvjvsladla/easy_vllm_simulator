//! `publish_install_request.py` — publishes the node blackbox install procedure as a `docs/request/` instruction document
//! (plan_26081716). Same flags, exit codes, texts and document as the original. File naming stays with the naming SSOT
//! (`wiki-desk/src/doc_naming.ts`, called through `node` exactly as the original calls it); the topology fallback is the
//! single owner `role_contract::manifest_gate::resolve_topology` (the Rust owner `manifest_contract.resolve_topology`
//! delegates to), with the same branch probe (`git -C <repo> rev-parse --abbrev-ref HEAD`).
//!
//! Exit codes: 0 = published · 1 = argument/precondition error · 2 = naming collisions exhausted · 3 = installer/verifier
//! interface mismatch (argparse usage errors are 2 as well).

use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::bb::{self, repr, Fail, R};
use crate::py;
use crate::raise::Raised;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

pub const LEVELS: [&str; 3] = ["L1", "L2", "L3"];
const INSTALLER: &str = ".claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh";
const VERIFIER: &str = ".claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh";
const SUB_INSTALLER: &str = ".claude/runtime/node_blackbox/install_node_blackbox.sh";
const CONTRACT: [(&str, &[&str]); 2] = [
    ("install_node_blackbox.sh", &["--apply", "--level", "L1", "L2", "L3"]),
    ("verify_node_blackbox.sh", &["--check", "--post-crash"]),
];

/// `_die(code, *lines)`: lines on standard error, then the documented status.
fn die(code: i32, lines: &[String]) -> Fail {
    for l in lines {
        eprintln!("{l}");
    }
    Fail::Code(code)
}

fn find_repo(start: &str) -> Option<String> {
    let mut d = start.to_string();
    while !d.is_empty() && d != "/" {
        if bb::is_dir(&bb::join(&d, ".claude")) && bb::is_dir(&bb::join(&d, "docs")) {
            return Some(d);
        }
        d = bb::dirname(&d);
    }
    None
}

/// `_TsDocNaming._call(fn, *args)`: `node doc_naming.ts <fn> '<json args>'` → the JSON answer.
enum Naming {
    Value(PyValue),
    Collision(String),
}

fn naming_call(ts: &str, func: &str, args: Vec<PyValue>) -> R<Naming> {
    let payload = py::dumps_unicode(&PyValue::List(args));
    let argv = vec!["node".to_string(), ts.to_string(), func.to_string(), payload];
    let cp = crate::proc::run(&argv, None, None)?;
    let Ok(out) = agent_runtime_support::pyjson::loads(&cp.stdout) else {
        return Err(Fail::Exit(format!(
            "[bb-request] FAIL: doc_naming.ts 실패(rc={}): {}",
            cp.returncode,
            py::strip(&cp.stderr)
        )));
    };
    let get = |k: &str| -> Result<Option<PyValue>, Raised> {
        match out.peel() {
            PyValue::Dict(d) => Ok(d.get(k).cloned()),
            other => Err(Raised::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", other.type_name()),
            )),
        }
    };
    if get("ok")?.is_some_and(|v| v.is_truthy()) {
        return Ok(Naming::Value(get("value")?.unwrap_or(PyValue::None)));
    }
    let message = get("message")?.map_or("None".to_string(), |v| bb::s(&v));
    if get("error")?.is_some_and(|v| v == PyValue::str("NamingCollisionExhausted")) {
        return Ok(Naming::Collision(message));
    }
    Err(Fail::Raise(Raised::new("ValueError", &message)))
}

fn assert_contract(sdir: &str) -> R<()> {
    let mut missing: Vec<String> = vec![];
    for (fname, needles) in CONTRACT {
        let fpath = bb::join(sdir, fname);
        if !bb::is_file(&fpath) {
            missing.push(format!("{fname} 부재"));
            continue;
        }
        let bytes = std::fs::read(&fpath).map_err(|e| Fail::Raise(Raised::os(&e, &fpath)))?;
        let body = bb::universal_newlines(&String::from_utf8_lossy(&bytes));
        for n in needles {
            if !body.contains(n) {
                missing.push(format!("{fname} 에 {} 없음", repr(n)));
            }
        }
    }
    if !missing.is_empty() {
        return Err(die(
            3,
            &[
                format!(
                    "[bb-request] FAIL(3): 설치자/검증기 인터페이스가 이 발행기의 전제와 다르다 — {}",
                    missing.join(" · ")
                ),
                "[bb-request]        지시서의 명령 정본은 그 스크립트다. 발행기를 먼저 맞춰라.".into(),
            ],
        ));
    }
    Ok(())
}

fn level_desc(lv: &str) -> (&'static str, &'static str) {
    match lv {
        "L1" => (
            "재부팅 불필요",
            "수집기(1초 샘플) · ETA 워치독 · 이벤트 통합 · 로그 수명 집행 · drop-caches · earlyoom",
        ),
        "L2" => (
            "재부팅 불필요 (peer 필요)",
            "netconsole 교차 스트리밍 — **멀티노드 전용**(단일은 `N/A` 로 정직 기록)",
        ),
        _ => ("**재부팅 1회**", "사후 포착 = efi_pstore 확보 (crashkernel·ramoops **제거**)"),
    }
}

fn levels_upto(level: &str) -> &'static [&'static str] {
    let i = LEVELS.iter().position(|l| *l == level).unwrap_or(2);
    &LEVELS[..=i]
}

/// `_render(level, topology, forced, generated_utc, basename)`.
pub fn render(level: &str, topology: &str, forced: bool, generated_utc: &str, basename: &str) -> String {
    let levels = levels_upto(level);
    let multi = topology == "multi";
    let has_l3 = levels.contains(&"L3");
    let nodes: &[&str] = if multi { &["메인", "서브"] } else { &["이 노드"] };
    let mut l: Vec<String> = vec![];
    let mut add = |s: &str| l.push(s.to_string());
    let stem: String = {
        let cs: Vec<char> = basename.chars().collect();
        cs[..cs.len().saturating_sub(3)].iter().collect()
    };
    add(&format!("# {}", stem.replacen('_', " ", 1)));
    add("");
    add("> **수행 주체: 사람**. 에이전트는 무인 `sudo` 를 실행하지 않는다(`terraforming_node` SKILL.md §2.6 · 헌법 §안전 경계).");
    add(&format!(
        "> **발행 시각(UTC)**: `{}` · **대상 레벨**: `{}` · **토폴로지**: `{}`",
        generated_utc,
        levels.join(" → "),
        topology
    ));
    add("> **발행 근거**: `plan_26081716` · **명명**: `doc_naming.dated_doc_basename`(결정론)");
    if forced {
        add(">");
        add("> ⚠ **`--force` 발행분** — 이 레벨 조합에는 재부팅이 없어 원래는 채팅 안내로 족하다(`plan_26081716` D1-b). 요청에 따라 문서로 낸다.");
    }
    add("");
    add("---");
    add("");
    add("## 1. 전제 · 준비물");
    add("");
    add("| 항목 | 요구 |");
    add("|---|---|");
    add("| 권한 | 대상 노드의 `sudo` (암호 입력 가능한 세션) |");
    add("| 위치 | 리포 루트에서 실행 — `cd <레포 절대경로>` |");
    if has_l3 {
        add("| 재부팅 | **1회 필요**(L3). 서빙 중이면 먼저 내려라 — 「부록 B」 참고 |");
    }
    if multi {
        add("| 노드 | **메인·서브 각각** 수행. 메인이 서브의 `sudo` 를 대행하지 않는다(A2A 경계) |");
        add(&format!("| 서브 경로 | 서브에서는 `{SUB_INSTALLER}` (런타임 배달분) |"));
    }
    add("| 사전 확인 | 이 문서가 가리키는 스크립트가 실재하는지 — 발행 시점에 대조됨 |");
    add("");
    add("**이번에 활성화되는 것**");
    add("");
    add("| 레벨 | 재부팅 | 무엇을 |");
    add("|---|---|---|");
    for lv in levels {
        let (reboot, what) = level_desc(lv);
        add(&format!("| **{lv}** | {reboot} | {what} |"));
    }
    add("");
    if levels.contains(&"L2") && !multi {
        add("> ℹ **L2 는 단일노드에서 `N/A` 로 기록된다** — peer 가 없으면 netconsole 이 성립하지 않는다. 이는 실패가 아니라 정직한 부재 기록이다.");
        add("");
    }
    add("---");
    add("");
    add("## 2. 수행 절차");
    add("");
    add("> 각 단계는 **명령 → 성공 판정 → 실패 시** 순서다. 판정을 통과하지 못하면 다음 단계로 넘어가지 말고 §4 로 가라.");
    add("");
    let mut step = 0;
    for node in nodes {
        if multi {
            add(&format!("### {node} 노드"));
            add("");
        }
        let sub = multi && *node == "서브";
        let installer = if sub { SUB_INSTALLER } else { INSTALLER };
        for lv in levels {
            step += 1;
            add(&format!("#### 단계 {step} — {lv} 적용 ({node})"));
            add("");
            add("```bash");
            add("# 먼저 dry-run 으로 무엇을·왜 바꾸는지 확인한다 (기본이 dry-run)");
            add(&format!("sudo bash {installer} --level {lv}"));
            add("");
            add("# 확인했으면 적용");
            add(&format!("sudo bash {installer} --apply --level {lv}"));
            add("```");
            add("");
            add("- **성공 판정**: 종료코드 `0`. dry-run 출력의 변경 목록과 적용 결과가 일치.");
            if *lv == "L3" {
                add("- ⚠ **여기서 재부팅이 필요하다.** 적용 직후 아래를 실행한다:");
                add("");
                add("  ```bash");
                add("  sudo reboot");
                add("  ```");
                add("");
                add("  **재부팅하면 이 작업을 지시한 에이전트 세션은 사라진다.** 돌아온 뒤에는 이 문서의 §3 체크박스를 보고 이어서 진행하면 된다.");
            }
            add("- **실패 시**: 종료코드 `1`=전제 실패 · `2`=설치/검증 실패 → §4");
            add("");
        }
        step += 1;
        let verifier = if sub {
            SUB_INSTALLER.replace("install_node_blackbox.sh", "verify_node_blackbox.sh")
        } else {
            VERIFIER.to_string()
        };
        add(&format!("#### 단계 {step} — 검증 ({node})"));
        add("");
        add("```bash");
        add(&format!("bash {verifier} --check"));
        add("```");
        add("");
        add("- **성공 판정**: 종료코드 `0`(전항 통과). `pending` 항목은 정상 — 크래시가 있어야 증명되는 항목이다.");
        add("- **실패 시**: 종료코드 `2`=검사 실패 → §4");
        if has_l3 {
            add("");
            add("> 🔬 **선택 — 사후 포착 증명**: `--check` 는 *준비 상태*만 본다. 실제 포착을 증명하려면 `--crash-test`(★파괴적★ 강제 커널 패닉 → 재부팅) 후 `--post-crash` 로 확정한다. **일상 설치에는 불필요하다** — 증명이 필요할 때만.");
        }
        add("");
    }
    add("---");
    add("");
    add("## 3. 진행 체크박스 — 재부팅을 건너 살아남는 상태");
    add("");
    add("> 단순 명령 나열은 재부팅을 **텍스트로만** 생존하지 **상태로는** 생존하지 못한다. 한 단계 끝낼 때마다 `[ ]` 를 `[x]` 로 바꿔라. 재부팅 후 이 절만 보면 어디부터인지 자명해진다(새 에이전트 세션에 이 문서를 물려줘도 같다).");
    add("");
    for node in nodes {
        if multi {
            add(&format!("**{node} 노드**"));
            add("");
        }
        for lv in levels {
            let mark = if *lv == "L3" { " → **재부팅 필요** ← 여기서 세션이 끊긴다" } else { "" };
            add(&format!("- [ ] {lv} 적용{mark}"));
        }
        if has_l3 {
            add("- [ ] 재부팅 완료");
        }
        add("- [ ] `verify_node_blackbox.sh --check` 통과");
        add("");
    }
    add("- [ ] 회수물 2종 제출 (§5)");
    add("- [ ] 에이전트에게 완료 통지");
    add("");
    add("---");
    add("");
    add("## 4. 실패 시 분기");
    add("");
    add("| 증상 | 종료코드 | 처방 |");
    add("|---|---|---|");
    add("| 설치자가 전제 실패로 멈춤 | `1` | 출력이 지목한 부재물을 먼저 해소. `node_identity.sh` 부재면 배달 결손이다 |");
    add("| 설치/검증 실패 | `2` | dry-run 을 다시 돌려 무엇이 어긋났는지 대조 |");
    add("| 검증에서 `pending` 만 남음 | `0` | **정상**. 크래시 없이는 증명 불가한 항목이다 |");
    add("| 재부팅 후 상태를 모르겠다 | — | §3 체크박스 → 마지막 `[x]` 다음 단계부터 |");
    add("");
    add("> 🚫 **우회하지 마라.** 가드가 막는 것은 대개 *정상 차단*이다. 규칙대로 해소하고, 정말 배선이 없다고 판단되면 그 사실을 회수물에 적어 에이전트에게 돌려보내라 (`.claude/rules/workflow.md` §막힘 3분류).");
    add("");
    add("---");
    add("");
    add("## 5. 회수물 — 이것이 돌아와야 완결이다");
    add("");
    add("수행이 끝나면 아래를 **그 자리에 둔 채** 에이전트에게 \"완료\"만 알리면 된다. 에이전트가 직접 읽는다(채팅에 붙여넣지 않아도 된다).");
    add("");
    add("| # | 회수물 | 경로 | 무엇을 증명하나 |");
    add("|---|---|---|---|");
    add("| 1 | 검증 출력 | 터미널 출력 또는 리다이렉트 파일 | 각 항목의 pass/fail/pending |");
    add("| 2 | proof-of-capture 판정 | `docs/logs/<node_id>/capture_verified.json` | 상태 권위 — `installed:true` 가 아니라 **실제 포착 증명** |");
    add("");
    add("> `<node_id>` 는 `node_identity.sh` 가 해소한다(각자 파싱 금지). 멀티노드면 **노드마다 각각** 생성된다.");
    add("");
    add("> ⚠ **이 문서는 evidence chain 밖이다** — request 는 판정도 계측도 아니라 `completion_gate` 를 타지 않는다(`.claude/rules/docs.md` §request). 회수물이 돌아오면 그때 testlog/devlog 로 chain 에 편입한다.");
    add("");
    add("---");
    add("");
    add("## 6. 예상 소요 · 비용");
    add("");
    add("| 항목 | 값 |");
    add("|---|---|");
    add(&format!(
        "| 명령 실행 | 노드당 수 분 (레벨 {}개 × {}노드) |",
        levels.len(),
        nodes.len()
    ));
    if has_l3 {
        add("| 재부팅 | 노드당 1회 — 머신 부팅 시간에 종속 |");
        add("| 서빙 중단 | **필요** — 재부팅 전 서빙을 내려야 한다 |");
    } else {
        add("| 재부팅 | **없음** |");
        add("| 서빙 중단 | 불요 |");
    }
    add("| 디스크 | 로그 평면 노드당 상한 기본 512 MiB (`logs_lifecycle.py` 가 집행) |");
    if has_l3 {
        add("| RAM 회수 | crashkernel 예약 **해제** — 약 2.25 GiB 반환 |");
    }
    add("");
    add("---");
    add("");
    add("## 7. 참조");
    add("");
    add("- `plan_26081716` — 이 발행 배선의 근거");
    add("- `.claude/rules/docs.md` §request(7번째 산문형) · §명명 SSOT");
    add("- `.claude/skills/terraforming_node/SKILL.md` §2.6 — 호스트 안전체계 Y/N 선택조항");
    add(&format!("- `{INSTALLER}` — 설치 명령의 **정본**(이 문서는 여기서 조립됐다)"));
    add(&format!("- `{VERIFIER}` — 검증 명령의 정본"));
    add("");
    l.join("\n") + "\n"
}

struct GitBranch;
impl role_contract::manifest_gate::BranchProbe for GitBranch {
    fn branch(&mut self, repo: &[u8]) -> Option<PyStr> {
        use std::os::unix::ffi::OsStrExt;
        let o = std::process::Command::new("git")
            .arg("-C")
            .arg(std::ffi::OsStr::from_bytes(repo))
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stdin(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::piped())
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        Some(PyStr::new(&String::from_utf8(o.stdout).ok()?).strip())
    }
}

fn read_topology(repo: &str) -> R<String> {
    // the original imports `manifest_contract` from `<repo>/.claude/skills/terraforming_node/scripts` first; its absence
    // is the same explicit refusal here (the decision itself is the linked owner's)
    let path = bb::join(repo, ".claude/skills/terraforming_node/scripts");
    if !bb::is_file(&bb::join(&path, "manifest_contract.py")) {
        return Err(die(
            1,
            &[format!(
                "[bb-request] FAIL(1): manifest_contract 를 불러오지 못했다 (No module named 'manifest_contract'). 기대 경로: {path}"
            )],
        ));
    }
    let topo = role_contract::manifest_gate::resolve_topology(&mut GitBranch, repo.as_bytes(), None);
    match topo.as_deref() {
        Some(t @ ("single" | "multi")) => Ok(t.to_string()),
        _ => Err(die(
            1,
            &[
                "[bb-request] FAIL(1): topology 를 해소하지 못했다(브랜치 불명).".into(),
                "[bb-request]        --topology single|multi 로 명시하라 — 추측하지 않는다.".into(),
            ],
        )),
    }
}

/// `os.path.relpath(path, start)` for absolute paths.
fn relpath(path: &str, start: &str) -> String {
    let a = bb::abspath(path);
    let b = bb::abspath(start);
    let pa: Vec<&str> = a.split('/').filter(|x| !x.is_empty()).collect();
    let pb: Vec<&str> = b.split('/').filter(|x| !x.is_empty()).collect();
    let n = pa.iter().zip(pb.iter()).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; pb.len() - n];
    parts.extend(&pa[n..]);
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

static ARGS: [Arg; 8] = [
    HELP,
    store(&["--generated-utc"]).required(),
    store(&["--level"]).choices(&["L1", "L2", "L3"]),
    store(&["--topology"]).choices(&["single", "multi"]),
    flag(&["--force"]),
    store(&["--repo"]),
    store(&["--out-dir"]),
    flag(&["--print-path-only"]),
];

pub static SPEC: Spec = Spec {
    prog: "publish_install_request.py",
    usage: include_str!("../help/publish_install_request.usage"),
    help: include_str!("../help/publish_install_request.help"),
    args: &ARGS,
    subs: None,
};

fn run(a: &argp::Parsed) -> R<i32> {
    let gen = a.text(1).unwrap_or("").to_string();
    let level = a.text(2).unwrap_or("L3").to_string();
    let force = a.flag(4);
    let sdir = bb::dirname(
        &crate::owner::owner_path("node_blackbox/publish_install_request.py")
            .display()
            .to_string(),
    );
    let repo = match a.text(5).filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None => find_repo(&sdir).ok_or_else(|| {
            Fail::Exit("[bb-request] FAIL(1): 리포 루트를 찾지 못했다(.claude + docs 마커).".into())
        })?,
    };
    let ts = bb::join(&bb::join(&bb::join(&bb::join(&repo, ".claude"), "skills"), "wiki-desk"), "src/doc_naming.ts");
    if !bb::is_file(&ts) {
        return Err(Fail::Exit(format!(
            "[bb-request] FAIL: doc_naming 을 불러오지 못했다. 기대 경로: {ts}"
        )));
    }
    let tokens = match naming_call(&ts, "kst_tokens", vec![PyValue::str(&gen)])? {
        Naming::Value(v) => v,
        Naming::Collision(m) => return Err(Fail::Raise(Raised::new("NamingCollisionExhausted", &m))),
    };
    let first = match tokens.peel() {
        PyValue::List(l) | PyValue::Tuple(l) if l.len() == 3 => bb::s(&l[0]),
        other => {
            return Err(Fail::Raise(Raised::new(
                "ValueError",
                &format!("not enough values to unpack (expected 3, got {})", other.repr()),
            )))
        }
    };
    if first == "NA" {
        return Err(Fail::Exit(format!(
            "[bb-request] FAIL(1): --generated-utc 가 'YYYY-MM-DDTHH:MM:SSZ' 형식이 아니다: {}\n[bb-request]        예: 2026-08-18T05:00:00Z (마이크로초·오프셋 표기 불가)",
            repr(&gen)
        )));
    }
    assert_contract(&sdir)?;
    let topology = match a.text(3) {
        Some(t) => t.to_string(),
        None => read_topology(&repo)?,
    };
    let levels = levels_upto(&level);
    if !levels.contains(&"L3") && !force {
        return Err(Fail::Exit(format!(
            "[bb-request] 중단(1): 레벨 {level} 에는 재부팅이 없어 지시서를 발행하지 않는다 (plan_26081716 D1-b — 세션이 죽지 않으면 문서의 존재 이유가 약하고 §2.6 의 '잔소리 금지' 톤과도 맞다).\n[bb-request]        채팅 안내로 진행하거나, 정말 문서가 필요하면 --force."
        )));
    }
    let out_dir = match a.text(6).filter(|o| !o.is_empty()) {
        Some(o) => o.to_string(),
        None => bb::join(&bb::join(&repo, "docs"), "request"),
    };
    bb::makedirs(&out_dir, 0o777)?;
    let mut existing: Vec<String> = std::fs::read_dir(&out_dir)
        .map_err(|e| Fail::Raise(Raised::os(&e, &out_dir)))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    existing.sort();
    let topic = format!("노드블랙박스_{level}_설치_수행절차");
    let basename = match naming_call(
        &ts,
        "dated_doc_basename",
        vec![
            PyValue::str("request"),
            PyValue::str(&gen),
            PyValue::str(&topic),
            py::list_str(&existing),
            PyValue::str("md"),
        ],
    )? {
        Naming::Value(v) => bb::s(&v),
        Naming::Collision(m) => return Err(die(2, &[format!("[bb-request] FAIL(2): {m}")])),
    };
    let path = bb::join(&out_dir, &basename);
    let body = render(&level, &topology, force, &gen, &basename);
    bb::write(&path, &body)?;
    let rel = relpath(&path, &repo);
    if a.flag(7) {
        println!("{rel}");
    } else {
        println!("[bb-request] 발행: {rel}");
        println!(
            "[bb-request]   레벨 {} · 토폴로지 {}{}",
            levels.join(" → "),
            topology,
            if force { " · --force" } else { "" }
        );
        println!("[bb-request]   회수물: verify 출력 · docs/logs/<node_id>/capture_verified.json");
    }
    Ok(0)
}

pub fn main(argv: &[String]) -> i32 {
    match argp::parse(&SPEC, argv) {
        Outcome::Exit(c) => c,
        Outcome::Ok(a) => bb::finish(run(&a)),
    }
}
