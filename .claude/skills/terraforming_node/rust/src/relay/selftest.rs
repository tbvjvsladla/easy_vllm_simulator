//! relay.py `--self-test`: the same checks, labels and verdict line. The original patched module functions with
//! `unittest.mock`; here the three patched seams (`resolve_ladder`, `resolve_campaign_axis`, `run_attempt`) read
//! thread-local hooks, and `contextlib.redirect_stdout` is the `core::CAPTURE` buffer. Temporary directories live under
//! `TMPDIR` and are removed afterwards. No network, no provider CLI (the runner table is read from agent_control).

use super::cli::{real_main, Args};
use super::core::*;
use super::dispatch::*;
use super::pv::{self, g, obj, py_repr, py_str, s, set, truthy, V};
use crate::sysexit::{Exit, Res};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

thread_local! {
    static LADDER: RefCell<Option<Vec<V>>> = const { RefCell::new(None) };
    static AXIS: RefCell<bool> = const { RefCell::new(false) };
    /// `run_attempt` patch: None = not patched; Some(None) = AssertionError("worker must not start"); Some(Some(rc)).
    static RUN: RefCell<Option<Option<i32>>> = const { RefCell::new(None) };
    static RUN_CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn hook_resolve_ladder() -> Option<Vec<V>> {
    LADDER.with(|l| l.borrow().clone())
}
pub fn hook_campaign_axis() -> Option<(Option<String>, V)> {
    if AXIS.with(|a| *a.borrow()) {
        Some((None, V::None))
    } else {
        None
    }
}
pub fn hook_run_attempt(task: &str) -> Option<Res<i32>> {
    let mode = RUN.with(|r| *r.borrow())?;
    RUN_CALLS.with(|c| c.borrow_mut().push(task.to_string()));
    Some(match mode {
        None => Err(tb("AssertionError: worker must not start")),
        Some(rc) => Ok(rc),
    })
}

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> TempDir {
        let base = std::env::temp_dir();
        let p = base.join(format!("tmp-relay-{}", pv::uuid4_hex()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(std::fs::canonicalize(&p).unwrap())
    }
    fn s(&self) -> String {
        self.0.display().to_string()
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Chk {
    ok: bool,
}
impl Chk {
    fn c(&mut self, cond: bool, label: &str) {
        println!("  [{}] {label}", if cond { "PASS" } else { "FAIL" });
        self.ok = self.ok && cond;
    }
}

fn capture<T>(f: impl FnOnce() -> T) -> (T, String) {
    CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
    let r = f();
    let out = CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default();
    (r, out)
}

fn msg(e: &Exit) -> String {
    match e {
        Exit::Msg(m) => m.clone(),
        Exit::Code(c) => c.to_string(),
    }
}

fn j(t: &str) -> V {
    pv::loads(t).expect("fixture json")
}

fn handoff_args(td: &str, ctx: &str) -> Args {
    Args {
        repo_root: td.into(),
        context_id: Some(ctx.into()),
        topology: Some("single".into()),
        continue_: true,
        ..Default::default()
    }
}

fn self_test_handoff_reuse(chk: &mut Chk) -> Res<()> {
    let previous = relay_campaign();
    set_relay_campaign(None);
    let td = TempDir::new();
    let tds = td.s();
    let ctx = "handoff-test";
    let lp = ledger_path(&tds, ctx)?;
    let rel = relpath(&lp.replace(".json", ".reports/attempt-01.json"), &tds);
    let report = j(r#"{"task_id": "task-1", "context_id": "handoff-test", "turn": 1, "phase": "publish", "status": "completed", "node_id": "sub", "running_services": [], "self_verification": {"checks_run": ["identity", "schema_valid"], "identity_ok": true}, "artifacts": [{"kind": "report", "path": "docs/testlog/result.md"}]}"#);
    atomic_json(&format!("{tds}/{rel}"), &obj(vec![("report", report.clone())]))?;
    let att = obj(vec![
        ("attempt", pv::i(1)),
        ("status", s("completed")),
        ("control_status", s("completed")),
        ("sub_reported", V::Bool(true)),
        ("context_id_reported", s(ctx)),
        ("report_path", s(&rel)),
        ("running_services", pv::list(vec![])),
        ("provider", s("claude_code")),
        ("session_id", s("original-native")),
        ("execution_started", V::Bool(true)),
        ("reason_codes", pv::list(vec![])),
        ("budget_outcome", s("within_budget")),
    ]);
    let base = obj(vec![
        ("context_id", s(ctx)),
        ("topology", s("single")),
        ("campaign_node", s("sub")),
        ("manifest", s(&format!("{tds}/manifest.yaml"))),
        ("attempts", pv::list(vec![att.clone()])),
    ]);
    let a = handoff_args(&tds, ctx);
    let (reused, out) = capture(|| reuse_completed_handoff(&a, &mut base.clone(), &lp));
    let got = pv::loads(out.trim_end()).unwrap_or(V::None);
    chk.c(
        matches!(reused, Ok(true)) && pv::eq(g(&got, "report"), &report) && pv::is_false(g(&got, "execution_started")) && truthy(g(&got, "continue_workflow")),
        "handoff: return the original report without native execution",
    );
    let rejected = |doc: V, args: &Args| -> bool { capture(|| reuse_completed_handoff(args, &mut doc.clone(), &lp)).0.is_err() };
    let mut inflight = base.clone();
    set(&mut inflight, "inflight", obj(vec![("dispatch_id", s("running"))]));
    chk.c(rejected(inflight, &a), "handoff: unresolved execution is not hidden by reuse");
    let mut wrong = a.clone();
    wrong.topology = Some("multi".into());
    chk.c(rejected(base.clone(), &wrong), "handoff: other topology cannot reuse this report");
    let mut other = report.clone();
    set(&mut other, "context_id", s("other"));
    atomic_json(&format!("{tds}/{rel}"), &obj(vec![("report", other)]))?;
    chk.c(rejected(base.clone(), &a), "handoff: wrong report context is not accepted");
    atomic_json(&format!("{tds}/{rel}"), &obj(vec![("report", report.clone())]))?;
    let mut changed = base.clone();
    if let Some(V::List(l)) = pv::get_mut(&mut changed, "attempts") {
        set(&mut l[0], "control_variables_echo", obj(vec![("model", s("old"))]));
    }
    set(&mut changed, "control_variables", obj(vec![("model", s("new"))]));
    chk.c(matches!(capture(|| reuse_completed_handoff(&a, &mut changed, &lp)).0, Ok(false)), "handoff: changed declared inputs require actual work");
    let mut failed = base.clone();
    if let Some(V::List(l)) = pv::get_mut(&mut failed, "attempts") {
        set(&mut l[0], "status", s("failed"));
    }
    chk.c(matches!(capture(|| reuse_completed_handoff(&a, &mut failed, &lp)).0, Ok(false)), "handoff: failed work is not reused as success");

    let runner = vec![obj(vec![("name", s("fixture")), ("provider", s("claude_code")), ("backend", s("anthropic")), ("model", s("sonnet"))])];
    LADDER.with(|l| *l.borrow_mut() = Some(runner));
    AXIS.with(|x| *x.borrow_mut() = true);
    let cli = |extra: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = ["--repo-root", &tds, "--context-id", ctx].iter().map(|x| x.to_string()).collect();
        v.extend(extra.iter().map(|x| x.to_string()));
        v
    };
    for flags in [vec!["--continue", "--apply"], vec!["--topology", "single", "--task", "다시 전부 검증"]] {
        let _ = std::fs::remove_file(&lp);
        save_ledger(&lp, &mut base.clone())?;
        RUN.with(|r| *r.borrow_mut() = Some(None));
        let (rc, _) = capture(|| real_main(&cli(&flags)));
        RUN.with(|r| *r.borrow_mut() = None);
        let saved = load_ledger(&lp)?;
        chk.c(
            matches!(rc, Ok(0)) && pv::eq(g(&saved, "attempts"), g(&base, "attempts")) && pv::is_false(g(g(&saved, "handoff_reuse"), "execution_started")),
            "handoff CLI: suppress completed-context reissue without attempt or runner failure",
        );
    }
    let _ = std::fs::remove_file(&lp);
    save_ledger(&lp, &mut base.clone())?;
    RUN.with(|r| *r.borrow_mut() = Some(Some(73)));
    RUN_CALLS.with(|c| c.borrow_mut().clear());
    let rc = real_main(&cli(&[
        "--topology", "single", "--task", "해당 항목을 정정하라", "--handoff-issue", "docs/testlog/result.md#F03 실제 모순", "--max-turns", "2",
        "--timeout-seconds", "30", "--budget-source", "fixture", "--resume", "new",
    ]));
    RUN.with(|r| *r.borrow_mut() = None);
    let calls = RUN_CALLS.with(|c| c.borrow().clone());
    chk.c(
        matches!(rc, Ok(73)) && calls.len() == 1 && calls[0].contains("F03"),
        "handoff CLI: observed issue follows the existing explicit feedback path",
    );
    LADDER.with(|l| *l.borrow_mut() = None);
    AXIS.with(|x| *x.borrow_mut() = false);
    set_relay_campaign(previous);
    Ok(())
}

fn meta() -> AttemptMeta {
    AttemptMeta {
        resume_declared: V::None,
        request_path: V::None,
        runner: V::None,
        started_utc: V::None,
        ended_utc: V::None,
        session_binding: V::None,
        dispatch_id: V::None,
    }
}
fn rec(doc: &mut V, ctx: &str, bud: &V, result: &str, report: Option<&str>) -> V {
    record_attempt(doc, ctx, bud, &j(result), &report.map(j).unwrap_or(V::None), &meta())
}
fn rec_m(doc: &mut V, ctx: &str, bud: &V, result: &str, report: Option<&str>, m: AttemptMeta) -> V {
    record_attempt(doc, ctx, bud, &j(result), &report.map(j).unwrap_or(V::None), &m)
}
fn ex(r: Res<impl Sized>) -> Option<String> {
    r.err().map(|e| msg(&e))
}

extern "C" {
    fn utimes(path: *const std::os::raw::c_char, times: *const [i64; 4]) -> i32;
}
fn set_mtime(p: &Path, t: f64) {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).unwrap();
    let sec = t.floor() as i64;
    let usec = ((t - t.floor()) * 1e6) as i64;
    let tv = [sec, usec, sec, usec];
    unsafe {
        utimes(c.as_ptr(), &tv);
    }
}

fn write(p: &str, t: &str) {
    if let Some(d) = Path::new(p).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(p, t).unwrap();
}

pub fn self_test() -> Res<i32> {
    let mut chk = Chk { ok: true };
    self_test_handoff_reuse(&mut chk)?;
    let bud25 = declare(pv::i(25), pv::i(1800), "선언: 자체검사 픽스처", "native_turns")?;
    {
        let dd = TempDir::new();
        let d = dd.s();
        let lp = ledger_path(&d, "ctx-1")?;
        let mut doc = load_ledger(&lp)?;
        chk.c(pv::items(g(&doc, "attempts")).is_empty(), "빈 원장 초기화");
        rec(&mut doc, "ctx-1", &bud25, r#"{"num_turns": 12, "budget_outcome": "within_budget", "session_id": "s1", "status": "completed", "reason_codes": []}"#, Some(r#"{"status": "completed", "phase": "config"}"#));
        save_ledger(&lp, &mut doc)?;
        let a0 = pv::items(g(&load_json_or_raise(&lp)?, "attempts"))[0].clone();
        chk.c(
            pv::eq(g(&a0, "max_turns_used"), &pv::i(12))
                && pv::eq(g(&a0, "max_turns_allocated"), &pv::i(25))
                && pv::eq(g(&a0, "timeout_seconds_allocated"), &pv::i(1800))
                && py_str(g(&a0, "budget_source")).contains("선언")
                && !pv::has(&a0, "task_grade"),
            "원장이 예산 서사(선언값·timeout·출처)를 적고 등급 라벨은 남기지 않는다",
        );
        let lks = last_known_session(&doc);
        chk.c(pv::is(g(&lks, "session_id"), "s1") && pv::is(g(&lks, "status"), "completed"), "마지막 알려진 세션을 **사실로** 돌려준다(재개 여부는 판정하지 않는다)");
        rec(&mut doc, "ctx-1", &bud25, r#"{"num_turns": 5, "budget_outcome": "within_budget", "session_id": "s2", "status": "completed", "reason_codes": []}"#, Some(r#"{"status": "input-required", "phase": "config"}"#));
        chk.c(pv::is(g(&last_known_session(&doc), "session_id"), "s2"), "직전 input-required 세션이 사실로 보인다");
        rec(&mut doc, "ctx-1", &bud25, r#"{"num_turns": 25, "budget_outcome": "exhausted", "session_id": "s3", "status": "execution_failed", "reason_codes": ["NONZERO_EXIT"]}"#, None);
        let lk = last_known_session(&doc);
        chk.c(pv::is(g(&lk, "session_id"), "s3") && pv::is(g(&lk, "budget_outcome"), "exhausted"), "소진된 세션도 **사실로는** 보인다 — 이을지는 --resume 선언이 정한다");
        chk.c(budget_floor(&doc) == 25, &format!("집행된 예산 바닥이 사실로 남는다 → {}", budget_floor(&doc)));
        chk.c(pv::is_false(g(attempts(&doc).last().unwrap(), "sub_reported")), "서브 리포트 없는 attempt 는 sub_reported=false(메인 대행을 성공으로 집계 ✗)");
        let p = surface_requests(&d, "ctx-1", &j(r#"{"hitl": {"needed": true, "request_id": "h1", "prompt": "어느 핀?"}}"#), 1, &V::None)?;
        let ok = p.as_ref().is_some_and(|p| pv::is(g(&pv::items(g(&load_json_or_raise(p).unwrap(), "pending"))[0], "request_id"), "h1"));
        chk.c(ok, "HITL 표면화");
        let p2 = surface_requests(&d, "ctx-2", &j(r#"{"hitl": {"needed": false}}"#), 1, &V::None)?;
        chk.c(p2.is_none(), "요청이 없으면 새 항목을 만들지 않는다(메인이 대신 만들지 않는다)");
        chk.c(pv::is(g(&parse_report(&s("어쩌고 {\"status\": \"completed\"} 끝")), "status"), "completed"), "산문 속 JSON 리포트 추출");
        chk.c(pv::is_none(&parse_report(&s("리포트 없음"))), "JSON 이 없으면 None(추측 파싱 ✗)");
        let trunc = "[permission_denials] 1건 — 거부된 도구 호출:\n  - Bash {\"command\": \"cat > /tmp/slice.json <<'EOF'\\n{\\\"schema_version\\\":1,\\\"id\\\":\\\"camp\n\n[서브가 남긴 말]\n```json\n{\"task_id\": \"t\", \"status\": \"completed\", \"image\": {\"tag\": \"x\"}, \"artifacts\": [{\"path\": \"a\"}]}\n```\n끝";
        let pr = parse_report(&s(trunc));
        chk.c(
            pv::is_dict(&pr) && pv::is(g(&pr, "status"), "completed") && pv::is(g(&pr, "task_id"), "t"),
            &format!("★잘린 JSON 뒤의 완전한 리포트를 집는다(중첩 객체를 따로 집지 않는다) → {}", py_repr(&pr)),
        );
        chk.c(pv::is(g(&parse_report(&s("{\"a\": 1} 산문 {\"status\": \"completed\", \"n\": {\"x\": 1}}")), "status"), "completed"), "마지막 최상위 객체를 집는다(안쪽 중첩 객체 ✗)");
        chk.c(pv::is_none(&parse_report(&s("{\"status\": \"completed\""))), "★음성대조: 닫히지 않은 JSON 은 리포트가 아니다(추측 ✗)");
        let req = j(r#"{"target": {"host": "h", "ssh_user": "u", "work_dir": "/w"}}"#);
        let r1 = assert_sub_branch(&req, "single", Some(&|_t: &V, _c: &str| (0, "single".to_string(), String::new())));
        chk.c(matches!(r1.as_deref(), Ok("single")), "서브 브랜치가 토폴로지와 같으면 통과");
        match assert_sub_branch(&req, "single", Some(&|_t: &V, _c: &str| (0, "multi".to_string(), String::new()))) {
            Ok(_) => chk.c(false, "불일치는 STOP"),
            Err(e) => chk.c(msg(&e).contains("브랜치 불일치"), "불일치는 STOP(브랜치 불일치)"),
        }
        match assert_sub_branch(&req, "single", Some(&|_t: &V, _c: &str| (255, String::new(), "Connection refused".to_string()))) {
            Ok(_) => chk.c(false, "판독 불가는 STOP"),
            Err(e) => chk.c(msg(&e).contains("판독하지 못했다"), "판독 불가는 일치로 치지 않는다(fail-closed)"),
        }
        let d2 = j(r#"{"attempts": [{"attempt": 1, "status": "input-required", "session_id": "sA", "budget_outcome": "within_budget", "max_turns_allocated": 25}, {"attempt": 2, "status": "completed", "session_id": "sB", "budget_outcome": "within_budget", "max_turns_allocated": 25}]}"#);
        chk.c(pv::is(g(&last_known_session(&d2), "session_id"), "sB"), "★음성대조: 앞에 input-required 가 있어도 **마지막 것**을 돌려준다(뒤로 스캔 ✗)");
        let d3 = obj(vec![("attempts", pv::list(vec![pv::items(g(&d2, "attempts"))[0].clone(), j(r#"{"attempt": 2}"#)]))]);
        chk.c(pv::is(g(&last_known_session(&d3), "session_id"), "sA"), "닿지 못한 attempt 는 서브 상태를 바꾸지 못한다(건너뛴다)");
        let d4 = j(r#"{"attempts": [{"attempt": 1, "budget_outcome": "exhausted", "max_turns_allocated": 25, "status": null, "session_id": "s1"}, {"attempt": 2, "max_turns_allocated": 104}]}"#);
        chk.c(budget_floor(&d4) == 25, &format!("★음성대조: 거절된 요청(104)은 바닥이 되지 못한다 → {}", budget_floor(&d4)));
        chk.c(true, "예산을 자동으로 정하는 경로가 되살아나지 않았다(선언만 남는다)");
        let d6 = j(r#"{"attempts": [{"attempt": 1, "status": "input-required", "phase": "build", "session_id": "s1"}, {"attempt": 2, "status": "input-required", "phase": "build", "session_id": "s2"}, {"attempt": 3, "status": "input-required", "phase": "build", "session_id": "s3"}]}"#);
        chk.c(stalled_attempts(&d6) == 3, &format!("전진 없는 3연속을 센다 → {}", stalled_attempts(&d6)));
        let d7 = j(r#"{"attempts": [{"attempt": 1, "status": "input-required", "phase": "config", "session_id": "s1"}, {"attempt": 2, "status": "input-required", "phase": "build", "session_id": "s2"}]}"#);
        chk.c(stalled_attempts(&d7) == 1, "phase 가 바뀌면 전진으로 센다(예산만 태운 것과 구분)");
        let rep_lib = j(r#"{"hitl": {"needed": true, "prompt": "q1", "blocking": true, "request_id": "r1"}}"#);
        let pth = surface_requests(&d, "ctx-lib", &rep_lib, 1, &V::None)?;
        chk.c(pth.is_some(), "hitl.needed 는 표면화된다");
        chk.c(entry_blocking(&pending_for(&d, &s("ctx-lib"))?[0]), "blocking 을 메인이 **읽는다**(hitl 절의 차단성 선언)");
        let old = surface_requests(&d, "ctx-old", &j(r#"{"library_request": [{"question": "q0", "blocking": true, "request_id": "r0"}]}"#), 1, &V::None)?;
        chk.c(old.is_none() && !entry_blocking(&j(r#"{"library_request": [{"blocking": true}]}"#)), "★서고 교환 은퇴: 옛 요청 배열은 읽혀도 표면화·차단성을 만들지 않는다(plan_26101912)");
        surface_requests(&d, "ctx-nb", &j(r#"{"hitl": {"needed": true, "prompt": "q2", "blocking": false, "request_id": "r2"}}"#), 1, &V::None)?;
        let mut both = pending_for(&d, &s("ctx-nb"))?;
        both.extend(pending_for(&d, &s("ctx-lib"))?);
        let order: Vec<String> = sort_pending(both).iter().map(|e| py_str(g(e, "request_id"))).collect();
        chk.c(order.first().is_some_and(|x| x == "r1"), &format!("★우선순위: blocking 이 먼저다(사전순이 아니다) → {}", py_repr(&pv::strs(&order))));
        let pp = join(&relay_root(&d)?, PENDING_HITL);
        let mut pdoc = load_json_or_raise(&pp)?;
        if let Some(V::List(l)) = pv::get_mut(&mut pdoc, "pending") {
            for e in l.iter_mut() {
                if pv::is(g(e, "request_id"), "r1") {
                    set(e, "answer", s("NAS 경로 X 를 쓴다"));
                }
            }
        }
        std::fs::write(&pp, pv::dumps_u(&pdoc)).unwrap();
        surface_requests(&d, "ctx-lib", &rep_lib, 2, &V::None)?;
        chk.c(pv::is(g(&pending_for(&d, &s("ctx-lib"))?[0], "answer"), "NAS 경로 X 를 쓴다"), "재표면화가 사람의 답을 지우지 않는다(두 번 답하게 하지 않는다)");
        surface_requests(&d, "ctx-lib", &j(r#"{"hitl": {"needed": false}}"#), 3, &V::None)?;
        chk.c(pending_for(&d, &s("ctx-lib"))?.is_empty(), "★음성대조: 요청이 사라진 턴에는 낡은 항목이 제거된다(라이브 잔존 실측 교정)");

        let rd = join(&relay_root(&d)?, "ctx-c.reports");
        write(&format!("{rd}/attempt-01.json"), &pv::dumps_u(&j(r#"{"report": {"artifacts": [{"kind": "log", "path": "output/x.json"}], "next_steps": ["serve 재기동"], "notes": "KV 미정"}}"#)));
        let mut d8 = j(r#"{"context_id": "ctx-c", "topology": "multi", "task": "원 지시 본문", "attempts": [{"attempt": 1, "status": "input-required", "phase": "build", "session_id": "s1", "budget_outcome": "exhausted", "max_turns_allocated": 25, "report_path": "campaigns/_bootstrap/relay/ctx-c.reports/attempt-01.json"}]}"#);
        let im = format!("{d}/instruction-manifest.yaml");
        write(&im, "topology: multi\nnodes:\n  - role: sub\n    host: 192.0.2.11\n    ssh_user: fixture\n    work_dir: /srv/fixture\n");
        let iq = build_request(
            &d,
            "multi",
            &im,
            "원 지시 본문",
            &bud25,
            &BuildOpts {
                context_id: Some("ctx-c".into()),
                attempt: 1,
                model: Some("sonnet".into()),
                campaign_id: None,
                control_variables: V::None,
                backend: Some("anthropic".into()),
                delegated_node: None,
                provider: "claude_code".into(),
                sandbox_mode: None,
                permission_profile: None,
                reasoning_effort: None,
            },
        )?;
        let ip = format!("{d}/campaigns/_bootstrap/relay/ctx-c.requests/attempt-01.json");
        write(&ip, &pv::dumps_a(&iq));
        let ipb = std::fs::read(&ip).unwrap();
        let scope = instruction_scope(&d8);
        if let Some(V::List(l)) = pv::get_mut(&mut d8, "attempts") {
            let a = &mut l[0];
            set(a, "provider", s("claude_code"));
            set(a, "session_binding", permission_binding(&iq));
            set(a, "request_path", s(&relpath(&ip, &d)));
            set(a, "request_digest", s(&pv::sha256_hex(&ipb)));
            set(
                a,
                "task_instruction",
                obj(vec![
                    ("schema_version", pv::i(1)),
                    ("kind", s("explicit")),
                    ("source_attempt", pv::i(1)),
                    ("scope", scope),
                    ("body", s("원 지시 본문")),
                    ("body_digest", s(&pv::sha256_hex("원 지시 본문".as_bytes()))),
                ]),
            );
        }
        let body = assemble_continuation(&d, &d8, &[j(r#"{"request_id": "r9", "blocking": true, "answer": "답 A", "prompt": "질문 Q"}"#)]).unwrap_or_default();
        for token in ["output/x.json", "serve 재기동", "KV 미정", "답 A", "원 지시 본문", "예산 소진"] {
            chk.c(body.contains(token), &format!("조립 본문이 기록된 사실을 담는다: {}", pv::repr_s(token)));
        }
        chk.c(!body.replace("추측 없음", "").contains("추측"), "조립기는 추측을 적지 않는다");
        let hdr = relay_header("ctx-h", 3, &pv::i(40), "선언: 빌드 1회 — 원장 attempt 2 실측 turns=31", &V::None);
        chk.c(
            hdr.contains("context_id: ctx-h") && hdr.contains("max_turns_allocated: 40") && hdr.contains("실측 turns=31"),
            "위임 헤더가 context_id·예산·**그 근거**를 서브에게 알린다(comms.md 규약의 실배선)",
        );
        let mut d9 = j(r#"{"attempts": []}"#);
        let mut m = meta();
        m.resume_declared = s("wanted");
        rec_m(&mut d9, "ctx-h", &declare(pv::i(40), pv::i(3600), "선언: 픽스처", "native_turns")?, r#"{"session_id": "other", "status": "completed"}"#, Some(r#"{"status": "completed", "context_id": "ctx-h"}"#), m);
        let a9 = attempts(&d9)[0].clone();
        chk.c(pv::is_false(g(&a9, "resume_honored")), "★재개 불발이 원장에 남는다(침묵 새 세션 ✗)");
        chk.c(pv::items(g(&a9, "external_search")).is_empty() && pv::is_list(g(&a9, "external_search")), "외부검색 기록 자리가 있다(B안 자산화 입력)");
        let mut df = j(r#"{"attempts": []}"#);
        let mut m = meta();
        m.resume_declared = s("new");
        m.request_path = s("campaigns/_bootstrap/relay/ctx-f.requests/attempt-01.json");
        m.started_utc = s("2026-09-05T10:00:00Z");
        m.ended_utc = s("2026-09-05T10:01:30Z");
        let af = rec_m(&mut df, "ctx-f", &declare(pv::i(12), pv::i(900), "선언: F", "native_turns")?, r#"{"session_id": "sF", "status": "completed", "num_turns": 7, "budget_outcome": "within_budget", "reason_codes": [], "duration_ms": 90000, "duration_api_ms": 61000}"#, Some(r#"{"status": "completed", "phase": "bench"}"#), m);
        for k in ["request_path", "started_utc", "ended_utc", "duration_ms", "duration_api_ms", "end_reason", "resume_declared"] {
            chk.c(!pv::is_none(g(&af, k)), &format!("원장이 축 F 필드를 적는다: {k}={}", py_repr(g(&af, k))));
        }
        chk.c(pv::int(g(&af, "duration_ms")).unwrap_or(0) - pv::int(g(&af, "duration_api_ms")).unwrap_or(0) == 29_000, "정지 = wall − api 가 원장에서 파생된다(29,000ms)");
        chk.c(pv::is(g(&af, "resume_declared"), "new") && pv::is_none(g(&af, "resume_requested")), "`new` 선언은 재개 요청이 아니다(둘을 구분해 적는다)");
        chk.c(pv::is(g(&af, "end_reason"), "completed"), &format!("종료사유 파생 → {}", py_str(g(&af, "end_reason"))));
        let snap = pv::dumps_sorted_pretty(&attempts(&df)[0]);
        let mut m = meta();
        m.resume_declared = s("sF");
        rec_m(&mut df, "ctx-f", &declare(pv::i(20), pv::i(900), "선언: F2", "native_turns")?, r#"{"session_id": "sF2", "status": "execution_failed", "num_turns": 20, "budget_outcome": "exhausted", "reason_codes": ["NONZERO_EXIT"]}"#, None, m);
        chk.c(pv::dumps_sorted_pretty(&attempts(&df)[0]) == snap, "★append-only: 새 attempt 가 앞선 줄을 소급 수정하지 않는다");
        let a1 = attempts(&df)[1].clone();
        chk.c(pv::is(g(&a1, "end_reason"), "budget_exhausted") && pv::is(g(&a1, "resume_requested"), "sF"), "소진·재개요청이 각각의 자리에 남는다");
        chk.c(
            end_reason(&j(r#"{"budget_outcome": "external_interruption"}"#)) == "external_interruption"
                && end_reason(&j(r#"{"reason_codes": ["TIMEOUT"], "control_status": "timeout"}"#)) == "external_interruption",
            "외생 중단(timeout/SIGTERM)은 예산 소진과 **다른 사유**로 적힌다",
        );
        chk.c(end_reason(&j(r#"{"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed", "status": "completed"}"#)) == "completed_with_denials", "★거부 사건 + completed 리포트 → completed_with_denials");
        chk.c(
            end_reason(&j(r#"{"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed", "status": null}"#)) == "permission_denied"
                && end_reason(&j(r#"{"reason_codes": ["PERMISSION_DENIED"], "control_status": "execution_failed", "status": "failed"}"#)) == "permission_denied",
            "★음성대조: 리포트 부재·completed 아님은 종전대로 permission_denied",
        );
        let mut dg = pv::empty();
        let mut m = meta();
        m.resume_declared = s("new");
        let ag = rec_m(&mut dg, "ctx-g", &declare(pv::i(40), pv::i(3600), "선언: G", "native_turns")?, r#"{"session_id": "sG", "status": "execution_failed", "num_turns": 31, "reason_codes": ["PERMISSION_DENIED"], "output": "[permission_denials] 1건 — 거부된 도구 호출:\n  - Bash {}\n\n[서브가 남긴 말]\n{\"status\": \"completed\"}"}"#, Some(r#"{"status": "completed"}"#), m);
        let pdn = pv::st(g(&ag, "permission_denials")).unwrap_or_default();
        chk.c(
            pv::is(g(&ag, "end_reason"), "completed_with_denials") && pdn.starts_with("[permission_denials] 1건") && !pdn.contains("[서브가 남긴 말]"),
            &format!("★원장이 거부 블록을 따로 남긴다(숨기지 않는다) → {}", py_repr(g(&ag, "permission_denials"))),
        );
        chk.c(
            end_reason(&j(r#"{"control_status": "execution_failed", "session_id": null}"#)) == "transport_or_launch_failure" && end_reason(&pv::empty()) == "unclassified",
            "전송 실패와 분류 불가를 구분한다(모르는 것을 아는 척하지 않는다)",
        );
        match require_resume(None, &j(r#"{"session_id": "sZ", "attempt": 3, "status": "input-required", "budget_outcome": "within_budget"}"#)) {
            Ok(_) => chk.c(false, "재개 미선언 → fail-loud"),
            Err(e) => chk.c(msg(&e).contains("sZ") && msg(&e).contains("--resume new"), "★재개 미선언은 죽되 **마지막 알려진 세션**을 함께 제시한다"),
        }
        chk.c(
            require_resume(Some("new"), &pv::empty()).ok().as_deref() == Some("new") && require_resume(Some(" sX "), &pv::empty()).ok().as_deref() == Some("sX"),
            "선언 어휘 둘(session_id · new)을 그대로 받는다",
        );
        let pc = surface_requests(&d, "ctx-crash", &pv::empty(), 2, &j(r#"{"attempt": 2, "end_reason": "malformed_output", "control_status": "malformed_output", "reason_codes": ["PROVIDER_RESULT_INVALID"], "last_known_session": "sC"}"#))?;
        let e0 = pending_for(&d, &s("ctx-crash"))?;
        chk.c(pc.is_some() && !e0.is_empty() && py_str(g(&e0[0], "prompt")).contains("sC"), "★리포트 없이 끝난 턴은 대기 목록에 마지막 알려진 세션을 남긴다(침묵 종결 ✗)");
    }
    let cv = j(r#"{"model": "gpt-oss-20b", "vllm_version": "0.18.0", "topology": "single", "target_gpu": "NVIDIA GB10"}"#);
    let blk = campaign_control_block(Some("camp-x"), &cv);
    chk.c(blk.contains("campaign_id: camp-x"), "위임 헤더가 campaign_id 를 싣는다");
    chk.c(blk.contains("campaigns/camp-x/"), "서브 인스턴스 경로를 디렉터리명 = campaign_id 로 지시한다");
    chk.c(pv::entries(&cv).iter().all(|(k, v)| blk.contains(&format!("- {k}: {}", py_str(v)))), "★layer-1 통제변인 4키가 전부 실린다(2026-09-05 에 모델 행이 빠졌다)");
    chk.c(blk.contains("control_variables_echo") && blk.contains("부재도 불일치도"), "echo 규약과 fail-closed 를 서브에게 명시한다");
    chk.c(campaign_control_block(None, &V::None).is_empty(), "캠페인 밖(_bootstrap)에서는 캠페인 축을 싣지 않는다");
    chk.c(campaign_slice_block(&repo(), "multi", Some("camp-x"), Some("sub")).map(|x| x.is_empty()).unwrap_or(false), "★multi Ray-worker request에는 autonomous slice를 싣지 않는다");
    let att_of = |kw: &[(&str, V)]| -> V {
        let mut b = obj(vec![
            ("sub_reported", V::Bool(true)),
            ("context_id_reported", s("ctx-1")),
            ("campaign_id_reported", s("camp-x")),
            ("control_variables_echo", cv.clone()),
        ]);
        for (k, v) in kw {
            set(&mut b, k, v.clone());
        }
        b
    };
    let sr = |att: &V| echo_stop_reasons(att, &s("ctx-1"), &s("camp-x"), &cv);
    chk.c(sr(&att_of(&[])).is_empty(), "전부 일치하면 통과");
    chk.c(sr(&att_of(&[("context_id_reported", V::None)])).iter().any(|r| r.contains("context_id echo 부재")), "★음성대조 context_id echo **부재**도 STOP(종전 `if 값 and` 는 침묵 통과했다)");
    chk.c(sr(&att_of(&[("context_id_reported", s("other"))])).iter().any(|r| r.contains("context_id 불일치")), "★음성대조 context_id 불일치 STOP");
    chk.c(sr(&att_of(&[("campaign_id_reported", V::None)])).iter().any(|r| r.contains("campaign_id echo 부재")), "★음성대조 campaign_id echo 부재 STOP(서브가 이름을 스스로 짓는 것을 막는다)");
    chk.c(sr(&att_of(&[("campaign_id_reported", s("camp7-sub-native"))])).iter().any(|r| r.contains("campaign_id 불일치")), "★음성대조 서브 자작 인스턴스명 STOP(2026-09-07 실측 형태)");
    chk.c(sr(&att_of(&[("control_variables_echo", V::None)])).iter().any(|r| r.contains("control_variables echo 부재")), "★음성대조 통제변인 echo 부재 STOP");
    let mut drift = cv.clone();
    set(&mut drift, "model", s("gpt-oss-120b"));
    chk.c(
        sr(&att_of(&[("control_variables_echo", drift)])).iter().any(|r| r.contains("control_variables 불일치") && r.contains("gpt-oss-120b")),
        "★음성대조 서브가 모델을 바꾸면 드리프트로 STOP(2026-09-05 서브 120b 사건 형태)",
    );
    chk.c(sr(&att_of(&[("sub_reported", V::Bool(false)), ("context_id_reported", V::None)])).is_empty(), "리포트 없는 크래시는 echo 위반이 아니다(같은 사실을 두 사유로 세지 않는다)");
    chk.c(
        echo_stop_reasons(&att_of(&[("campaign_id_reported", V::None), ("control_variables_echo", V::None)]), &s("ctx-1"), &V::None, &V::None).is_empty(),
        "캠페인 축을 안 실었으면 그 echo 도 묻지 않는다",
    );
    {
        let dd = TempDir::new();
        let lp2 = ledger_path(&dd.s(), "ctx-camp")?;
        let mut doc2 = load_ledger(&lp2)?;
        let rep = obj(vec![
            ("status", s("completed")),
            ("phase", s("publish")),
            ("context_id", s("ctx-camp")),
            ("campaign_id", s("camp-x")),
            ("control_variables_echo", cv.clone()),
        ]);
        record_attempt(&mut doc2, "ctx-camp", &bud25, &j(r#"{"num_turns": 5, "session_id": "s", "status": "completed", "reason_codes": []}"#), &rep, &meta());
        let a = attempts(&doc2).last().unwrap().clone();
        chk.c(pv::is(g(&a, "campaign_id_reported"), "camp-x") && pv::eq(g(&a, "control_variables_echo"), &cv), "원장이 campaign echo 두 필드를 남긴다(소비자가 읽을 자리)");
    }
    {
        let dd = TempDir::new();
        let lp3 = ledger_path(&dd.s(), "ctx-svc")?;
        let mut doc3 = load_ledger(&lp3)?;
        rec(&mut doc3, "ctx-svc", &bud25, r#"{"num_turns": 25, "budget_outcome": "exhausted", "session_id": "s", "status": "completed", "reason_codes": []}"#, Some(r#"{"status": "input-required", "context_id": "ctx-svc", "running_services": [{"kind": "container", "name": "cell-x-serving", "port": 8000, "note": "예산 소진 시점 상주"}]}"#));
        chk.c(pv::is(g(&pv::items(g(attempts(&doc3).last().unwrap(), "running_services"))[0], "name"), "cell-x-serving"), "원장이 고아 서비스를 남긴다(소비자가 읽을 자리)");
        let mut doc4 = load_ledger(&ledger_path(&dd.s(), "ctx-svc2")?)?;
        rec(&mut doc4, "ctx-svc2", &bud25, r#"{"num_turns": 5, "session_id": "s", "status": "completed", "reason_codes": []}"#, Some(r#"{"status": "completed", "context_id": "ctx-svc2"}"#));
        chk.c(pv::is_none(g(attempts(&doc4).last().unwrap(), "running_services")), "★말하지 않은 것은 None(모름)이지 빈 목록(없다는 선언)이 아니다");
    }
    chk.c(relay_header("c", 1, &pv::i(40), "근거", &pv::i(3600)).contains("timeout_seconds_allocated"), "★위임 헤더가 시간 상한도 말한다(서브가 몇 초 뒤 잘리는지 알아야 한다)");
    chk.c(!relay_header("c", 1, &pv::i(40), "근거", &V::None).contains("timeout_seconds_allocated"), "시간 상한이 없으면 없다고 적지 않는다(모르는 값을 지어내지 않는다)");
    let mut dd = pv::empty();
    bind_campaign_context(&mut dd, Some("sub"), Some("cell"), Some("c1"))?;
    chk.c(pv::is(g(&dd, "campaign_node"), "sub") && pv::is(g(&dd, "campaign_context_kind"), "cell") && pv::is(g(&dd, "campaign_cell"), "c1"), "원장이 캠페인 축을 못박는다(P4 가 읽을 자리)");
    let raised = ex(bind_campaign_context(&mut dd, None, None, Some("c2")));
    chk.c(raised.is_some_and(|r| r.contains("셀 하나 = context 하나")), "★음성대조 한 context 에 셀 둘을 묶으면 STOP(D20 · 2026-09-07 캡 절단 2/2)");
    chk.c(ex(bind_campaign_context(&mut obj(vec![("campaign_node", s("main"))]), Some("sub"), None, None)).is_some(), "★음성대조 한 원장이 두 노드를 주장하면 STOP");
    chk.c(ex(bind_campaign_context(&mut pv::empty(), None, Some("nope"), None)).is_some(), "★음성대조 알 수 없는 context 종류 STOP");
    {
        let t = TempDir::new();
        write(&format!("{}/campaigns/camp-x/phases/sub/serve.status.json", t.s()), r#"{"first_started_utc": "2026-09-08T10:00:00Z", "ended_utc": "2026-09-08T10:20:00Z"}"#);
        let (secs, why) = derive_timeout(&t.s(), Some("camp-x"), Some("sub"), 1.5)?;
        chk.c(secs == Some(1800) && why.contains("실측 1200s"), "★시간 예산을 지난 phase 실측에서 파생한다(1200s x 1.5 = 1800s)");
        let (n, why2) = derive_timeout(&t.s(), Some("camp-x"), Some("ghost"), 1.5)?;
        chk.c(n.is_none() && why2.contains("스키마 상한") && why2.contains(&schema_cap("timeout_seconds")?.to_string()), "★실측이 없으면 파생하지 않고 **스키마 상한을 읽어** 그 사실을 적는다(3600 재기재 ✗)");
    }
    let mk = |kw: &[(&str, V)]| -> V {
        let mut b = obj(vec![("attempt", pv::i(1)), ("session_id", s("s1")), ("sub_reported", V::Bool(true)), ("control_status", s("completed"))]);
        for (k, v) in kw {
            set(&mut b, k, v.clone());
        }
        b
    };
    let atts = |v: Vec<V>| obj(vec![("attempts", pv::list(v))]);
    let sd = |doc: &V| py_str(g(&supervise_decide(doc, &V::None, &[], None), "action"));
    let sdb = |doc: &V, brief: &V| py_str(g(&supervise_decide(doc, brief, &[], None), "action"));
    chk.c(sd(&atts(vec![])) == "await_dispatch", "감독: 아무것도 안 나갔으면 지시서를 열라고 말한다");
    chk.c(sd(&atts(vec![mk(&[("end_reason", s("completed")), ("status", s("completed"))])])) == "next_cell", "감독: completed → 다음 셀");
    let cwd = supervise_decide(&atts(vec![mk(&[("end_reason", s("completed_with_denials")), ("status", s("completed")), ("control_status", s("execution_failed"))])]), &V::None, &[], None);
    chk.c(pv::is(g(&cwd, "action"), "next_cell") && py_str(g(&cwd, "reason")).contains("권한 거부"), "★감독: completed_with_denials → 다음 셀(차단 ✗) · 사유에 거부 사건을 적는다");
    let prog = atts(vec![mk(&[("end_reason", s("budget_exhausted")), ("phase", s("build"))]), mk(&[("attempt", pv::i(2)), ("end_reason", s("budget_exhausted")), ("phase", s("serve"))])]);
    chk.c(sd(&prog) == "resume", "★감독: 소진이지만 phase 가 전진했으면 **자동 재발급**(D9 항상 자동)");
    let stall = atts(vec![mk(&[("end_reason", s("unclassified")), ("phase", s("serve"))]), mk(&[("attempt", pv::i(2)), ("end_reason", s("unclassified")), ("phase", s("serve"))])]);
    chk.c(sd(&stall) == "popup", "★감독: 전진이 없으면 예산을 키우기 전에 사람에게 묻는다");
    let brief = obj(vec![("last_utc", s("2026-09-08T20:00:00Z"))]);
    chk.c(sdb(&stall, &brief) == "resume", "★감독: phase 가 그대로여도 브리핑이 전진했으면 재발급(R2 — 긴 벤치를 정체로 오판 ✗)");
    let mut seen = stall.clone();
    set(&mut seen, "supervisor_last_seen_utc", s("2026-09-08T21:00:00Z"));
    chk.c(sdb(&seen, &brief) == "popup", "★감독: 브리핑이 직전 스텝이 본 값보다 앞서지 않으면 전진이 아니다");
    chk.c(sd(&prog) == "resume", "★감독: 과거 누적 비용 상한은 재개를 막지 않는다");
    chk.c(sd(&atts(vec![mk(&[("end_reason", s("transport_or_launch_failure"))])])) == "popup", "★감독: 통신·모델 평면이 깨지면 재개로 낫지 않는다 → 팝업");
    let pd = supervise_decide(&atts(vec![mk(&[("end_reason", s("permission_denied"))])]), &V::None, &[], None);
    chk.c(
        pv::is(g(&pd, "action"), "popup") && py_str(g(&pd, "reason")).contains("권한 평면") && !py_str(g(&pd, "reason")).contains("모델·통신"),
        "★감독: 권한 거부는 **권한 거부라고** 말한다(사유가 과장되면 사람이 엉뚱한 곳을 본다)",
    );
    chk.c(sd(&atts(vec![mk(&[("end_reason", s("sub_input_required"))])])) == "popup", "감독: input-required 는 답이 승인이다");
    let y: Vec<(&str, V)> = vec![
        ("end_reason", s("sub_input_required")),
        ("hitl_needed", V::Bool(false)),
        ("max_turns_used", pv::i(8)),
        ("next_steps", pv::strs(&["T1 나머지".to_string()])),
        ("artifact_paths", pv::strs(&["docs/testlog/a.md".to_string()])),
    ];
    let with = |base: &[(&str, V)], over: &[(&str, V)]| -> V {
        let mut v = mk(base);
        for (k, x) in over {
            set(&mut v, k, x.clone());
        }
        v
    };
    chk.c(sd(&atts(vec![mk(&y)])) == "resume", "★감독: 예산 양보 input-required + 일한 흔적 → 자동 재발급(브리핑 없는 멀티 워커)");
    chk.c(sd(&atts(vec![with(&y, &[("hitl_needed", V::Bool(true))])])) == "popup", "★감독 음성대조: hitl.needed 가 있으면 질문이다 → 팝업");
    chk.c(sd(&atts(vec![with(&y, &[("hitl_needed", V::None)])])) == "popup", "★감독 음성대조: hitl 표지를 모르는 옛 원장(None)은 양보로 읽지 않는다 — 모르면 묻는다");
    chk.c(sd(&atts(vec![mk(&y)])) == "resume", "★감독 음성대조: 예산 양보 재개도 비용 상한을 넘지 않는다");
    chk.c(
        ledger_runner_spec(&j(r#"{"runner_ladder": "kimi-claude,sonnet-5.5", "attempts": [{"runner": {"name": "sonnet"}}]}"#)).as_deref() == Some("kimi-claude,sonnet-5.5")
            && ledger_runner_spec(&j(r#"{"attempts": [{"runner": {"name": "sonnet-5.5"}}, {"runner": {"name": "sonnet"}}]}"#)).as_deref() == Some("sonnet-5.5")
            && ledger_runner_spec(&j(r#"{"attempts": [{}]}"#)).is_none(),
        "★재발급 러너: 명시가 없으면 원장이 쓴 사다리를 잇는다(조용한 별칭 대체 ✗)",
    );
    chk.c(
        recommended_timeout(&atts(vec![mk(&[("budget_recommendation", obj(vec![("timeout_seconds", pv::i(900))]))])])) == Some(900)
            && recommended_timeout(&atts(vec![mk(&[("budget_recommendation", obj(vec![("timeout_seconds", s("많이"))]))])])).is_none()
            && recommended_timeout(&atts(vec![mk(&[])])).is_none(),
        "★재발급 timeout: 서브 제안 수치만 읽는다(산문·비수치는 None — 값을 만들지 않는다)",
    );
    chk.c(
        resume_blockers(&[j(r#"{"blocking": true}"#)]).len() == 1
            && resume_blockers(&[j(r#"{"blocking": true, "answer": "a"}"#)]).is_empty()
            && resume_blockers(&[j(r#"{"prompt": "비차단"}"#)]).is_empty(),
        "★재발급 차단: 답 없는 차단성 요청만 멈춘다 · 답했거나 비차단이면 통과",
    );
    let cut = obj(vec![
        ("context_id", s("c-t1")),
        ("attempts", pv::list(vec![mk(&[("end_reason", s("budget_exhausted")), ("sub_reported", V::Bool(false)), ("started_utc", s("2026-09-29T10:06:05Z")), ("ended_utc", s("2026-09-29T10:07:05Z"))])])),
    ]);
    {
        let dd = TempDir::new();
        let d = dd.s();
        let tl = format!("{d}/sync_staging/sub_docs/testlog");
        std::fs::create_dir_all(&tl).unwrap();
        let t = utc_epoch(&s("2026-09-29T10:06:50Z")).unwrap();
        let doc_at = |name: &str, body: &str, when: f64| {
            let fp = format!("{tl}/{name}");
            std::fs::write(&fp, body).unwrap();
            set_mtime(Path::new(&fp), when);
        };
        chk.c(doc_plane_progress(&d, &cut).is_none(), "★문서 평면 음성대조: 미러가 비면 전진 없음");
        doc_at("b.md", "context_id c-t10 · 다른 과업\n", t);
        chk.c(doc_plane_progress(&d, &cut).is_none(), "★문서 평면 음성대조: 다른 context(c-t10)의 문서는 세지 않는다(접두 일치 ✗)");
        doc_at("c.md", "context_id c-t1 · 지난 attempt\n", t - 3600.0);
        chk.c(doc_plane_progress(&d, &cut).is_none(), "★문서 평면 음성대조: attempt 창 밖 mtime 은 세지 않는다");
        doc_at("a.md", "# T1\ncontext_id c-t1 · campaign x\n", t);
        let dp = doc_plane_progress(&d, &cut);
        chk.c(dp.as_deref().is_some_and(|x| x.contains("a.md")), &format!("★문서 평면: 창 안 · 같은 context 문서 → 전진 ({})", pv::fmt_opt(&dp)));
        chk.c(
            sd(&cut) == "resume" && py_str(g(&supervise_decide(&cut, &V::None, &[], dp.as_deref()), "action")) == "resume",
            "★감독: 호출 예산 소진은 증거 검증 후 이어진다",
        );
    }
    chk.c(sd(&atts(vec![mk(&[("end_reason", s("sub_input_required")), ("next_steps", pv::strs(&["x".to_string()]))])])) == "popup", "★감독 음성대조: 표지 필드가 없는 옛 원장은 양보로 읽지 않는다(모르면 묻는다)");
    let stall2 = atts(vec![mk(&y), with(&y, &[("attempt", pv::i(2))])]);
    chk.c(sd(&stall2) == "popup", "★감독 음성대조: 같은 남은 일·같은 산출물 반복 = 정체 → 팝업(무한 재발급 ✗)");
    let mv = atts(vec![
        mk(&y),
        with(&y, &[("attempt", pv::i(2)), ("next_steps", pv::strs(&["T2".to_string()])), ("artifact_paths", pv::strs(&["docs/testlog/a.md".to_string()]))]),
    ]);
    chk.c(sd(&mv) == "resume", "★감독: 남은 일이 바뀌면 전진이다");
    let un = atts(vec![mk(&[("end_reason", s("unclassified")), ("phase", s("serve"))]), mk(&[("attempt", pv::i(2)), ("end_reason", s("unclassified")), ("phase", s("serve"))])]);
    chk.c(sdb(&un, &obj(vec![("last_utc", s("2026-09-08T07:44:28Z"))])) == "resume", "★감독: 판정 불가라도 **전진이 관측되면** 잇는다(리포트 형식 미준수가 D9 를 무력화 ✗)");
    chk.c(sd(&un) == "popup", "★감독 음성대조: 판정 불가 + 전진 없음이면 묻는다(모르면서 잇지 않는다)");
    let mut led = atts(vec![mk(&[("end_reason", s("budget_exhausted")), ("phase", s("serve"))])]);
    record_supervisor_step(&mut led, &obj(vec![("action", s("popup")), ("reason", s("r"))]), "t0", &obj(vec![("last_utc", s("u1"))]));
    chk.c(pv::items(g(&led, "supervisor_steps")).len() == 1 && pv::is(g(&led, "supervisor_last_seen_utc"), "u1"), "★감독 판정이 원장에 남는다(다음 스텝이 같은 것을 다시 판정하지 않는다)");

    // Tripwire: the time cap literal must not be written into the production modules (it is read from the schema).
    let cap = schema_cap("timeout_seconds")?.to_string();
    let prod = [
        include_str!("core.rs"),
        include_str!("dispatch.rs"),
        include_str!("cli.rs"),
        include_str!("pv.rs"),
        include_str!("../relay.rs"),
    ]
    .concat();
    let all = [prod.as_str(), include_str!("selftest.rs")].concat();
    chk.c(!prod.contains(&cap), &format!("★시간 상한({cap}) 리터럴이 프로덕션 코드에 없다 — 상한은 스키마에서 읽는다"));
    chk.c(all.contains(&cap), "음성대조: 슬라이스가 파일 전체를 지우지 않았다(도달 불가 시험 금지)");

    let tbl = runner_table(&repo())?;
    let mut names: Vec<String> = tbl.iter().map(|(n, _)| n.clone()).collect();
    names.sort_by(|a, b| a.chars().cmp(b.chars()));
    let need = ["sonnet", "haiku", "opus", "kimi-claude", "minimax-claude", "meta-claude"];
    chk.c(need.iter().all(|n| names.iter().any(|x| x == n)), &format!("별칭 표를 **어댑터에서** 읽는다(사본 ✗) → {}", py_repr(&pv::strs(&names))));
    let lad = resolve_ladder(&repo(), Some("kimi-claude,minimax-claude,sonnet,haiku"), None, None, None)?;
    let lnames: Vec<String> = lad.iter().map(|r| py_str(g(r, "name"))).collect();
    chk.c(
        lnames == ["kimi-claude", "minimax-claude", "sonnet", "haiku"] && pv::is(g(&lad[0], "backend"), "kimi") && pv::is(g(&lad[2], "model"), "sonnet"),
        "사다리는 **선언 순서 그대로**이고 각 칸이 (backend, model) 쌍으로 펴진다",
    );
    let def = resolve_ladder(&repo(), None, None, None, None)?;
    chk.c(
        def.len() == 1 && pv::is(g(&def[0], "name"), "sonnet") && pv::is(g(&def[0], "backend"), "anthropic"),
        "기본 사다리는 1칸(claude -p · sonnet) — 그 밖의 슬롯·호출 방식은 가변(--runners · 2026-10-26 사용자 정의)",
    );
    match resolve_ladder(&repo(), Some("sonnet,gpt-5"), None, None, None) {
        Ok(_) => chk.c(false, "미등재 별칭은 STOP"),
        Err(e) => chk.c(msg(&e).contains("등재되지 않은 러너"), "미등재 별칭은 STOP(오타가 조용히 통과하지 않는다)"),
    }
    let ru = |n: i64, name: &str, rotate: bool, session: &str| -> V {
        obj(vec![
            ("attempt", pv::i(n)),
            ("reason_codes", pv::strs(&[RUNNER_UNAVAILABLE.to_string()])),
            ("session_id", s(session)),
            ("runner", obj(vec![("name", s(name))])),
            ("end_reason", s("runner_unavailable")),
            ("runner_evidence", obj(vec![("rotate", V::Bool(rotate))])),
        ])
    };
    let dr = atts(vec![ru(1, "kimi-claude", true, "stub"), ru(2, "minimax-claude", true, "stub")]);
    chk.c(next_runner_index(&dr, &lad) == 2, "다음 칸은 원장에서 **파생**한다(커서 저장 ✗)");
    let d4r = atts(["kimi-claude", "minimax-claude", "sonnet", "haiku"].iter().enumerate().map(|(i, n)| ru(i as i64 + 1, n, true, "stub")).collect());
    chk.c(next_runner_index(&d4r, &lad) == 0, "★사다리는 **순환**한다(4 → 1)");
    let dok = atts(vec![
        ru(1, "kimi-claude", true, "stub"),
        obj(vec![("attempt", pv::i(2)), ("runner", obj(vec![("name", s("minimax-claude"))])), ("status", s("completed")), ("session_id", s("s9")), ("end_reason", s("completed"))]),
    ]);
    chk.c(next_runner_index(&dok, &lad) == 1, "성공한 칸은 **그 자리를 지킨다**(성공 뒤 회전은 이유 없는 강등이다)");
    chk.c(consecutive_runner_unavailable(&d4r) == 4 && consecutive_runner_unavailable(&dok) == 0, "소진 술어는 **연속** 러너 실패만 센다(서브에 닿으면 0으로 리셋)");
    chk.c(!reached_sub(&ru(1, "kimi-claude", true, "stub")), "★러너 실패는 세션 스텁을 싣고 와도 **닿은 것이 아니다**(예산 바닥·정체·마지막세션 오염 ✗)");
    let mut ra = ru(1, "kimi-claude", true, "stub");
    set(&mut ra, "max_turns_allocated", pv::i(40));
    chk.c(budget_floor(&atts(vec![ra])) == 0, "★음성대조: 회전 attempt 는 **예산 바닥이 되지 못한다**(집행된 적이 없다)");
    chk.c(
        pv::is(g(&last_known_session(&atts(vec![j(r#"{"attempt": 1, "session_id": "real", "status": "input-required"}"#), ru(2, "kimi-claude", true, "stub")])), "session_id"), "real"),
        "★음성대조: 회전이 만든 빈 세션이 **마지막 알려진 세션을 덮지 않는다**",
    );
    chk.c(end_reason(&j(r#"{"reason_codes": ["RUNNER_UNAVAILABLE"], "control_status": "execution_failed"}"#)) == "runner_unavailable", "러너 실패는 전송·예산과 **다른 라벨**을 받는다");
    chk.c(end_reason(&j(r#"{"reason_codes": ["NONZERO_EXIT"], "control_status": "execution_failed", "session_id": "s1", "status": "failed"}"#)) == "sub_failed", "★음성대조: 서브가 일하다 실패한 것은 러너 실패로 접히지 않는다");
    let dec = supervise_decide(&atts(vec![ru(1, "kimi-claude", true, "stub")]), &V::None, &lad, None);
    chk.c(pv::is(g(&dec, "action"), "rotate_runner"), "감독: 칸이 남으면 **회전**(예산·범위 불변)");
    let dec4 = supervise_decide(&d4r, &V::None, &lad, None);
    chk.c(pv::is(g(&dec4, "action"), "popup") && py_str(g(&dec4, "reason")).contains("소진"), "감독: 한 바퀴를 다 돌면 **팝업**(사용자 규격 — 다 안 되면 HITL)");
    let one = resolve_ladder(&repo(), Some("sonnet"), None, None, None)?;
    chk.c(pv::is(g(&supervise_decide(&atts(vec![ru(1, "sonnet", true, "stub")]), &V::None, &one, None), "action"), "popup"), "★사다리 1칸이면 첫 실패가 곧 소진 → 즉시 HITL(사용자 규격)");
    let capd = supervise_decide(&atts(vec![ru(1, "kimi-claude", true, "stub"), ru(2, "minimax-claude", true, "stub")]), &V::None, &lad, None);
    chk.c(pv::is(g(&capd, "action"), "rotate_runner"), "★음성대조: 회전은 **비용이 아니다** — 상한 2에 회전 2회가 닿아도 팝업이 아니다");
    let ru_out = "[runner_unavailable] {\"rotate\": true, \"signal\": \"api_error\", \"detail\": \"x\"}";
    chk.c(!pv::is_none(&parse_report(&s(ru_out))), "음성대조: `parse_report` 는 이 문자열에서 JSON 을 **집는다**(그래서 위험했다)");
    let ru_res = obj(vec![("reason_codes", pv::strs(&[RUNNER_UNAVAILABLE.to_string()])), ("output", s(ru_out)), ("status", s("execution_failed"))]);
    let guarded = if pv::contains_str(g(&ru_res, "reason_codes"), RUNNER_UNAVAILABLE) { V::None } else { parse_report(g(&ru_res, "output")) };
    chk.c(pv::is_none(&guarded), "★라이브 교정: 러너-불가 진단이 **서브 리포트로 집계되지 않는다** (종전엔 sub_reported=true 가 적혔다 — 메인 산출을 서브 산출로 세는 형태)");
    chk.c(echo_stop_reasons(&ru(1, "kimi-claude", true, "stub"), &s("ctx-1"), &V::None, &V::None).is_empty(), "닿지 않은 attempt 에는 echo 판정을 걸지 않는다(sub_reported 가드)");
    let pe = parse_runner_evidence(&j(r#"{"reason_codes": ["RUNNER_UNAVAILABLE"], "output": "[runner_unavailable] {\"rotate\": true, \"signal\": \"api_error\"}"}"#));
    chk.c(pv::is_true(g(&pe, "rotate")), "증거는 output 에서 **되읽힌다**(실어만 보내면 또 못 닿는다)");
    chk.c(pv::is_false(g(&parse_runner_evidence(&j(r#"{"reason_codes": ["RUNNER_UNAVAILABLE"], "output": "깨진 텍스트"}"#)), "rotate")), "★판독 불가 증거는 회전하지 않는다(모르면 태우지 않는다)");
    println!("self-test: {}", if chk.ok { "PASS" } else { "FAIL" });
    Ok(if chk.ok { 0 } else { 2 })
}
