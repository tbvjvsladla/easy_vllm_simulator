//! relay.py command line (`argparse` as declared by the original, help text captured in `help/relay.*`) and `_main`.

use super::core::*;
use super::dispatch::*;
use super::pv::{self, g, has, is, items, py_str, s, set, truthy, V};
use crate::argp::{self, flag, store, Arg, Outcome, Spec, HELP};
use crate::sysexit::{Exit, Res};
use std::path::Path;

/// The `argparse.Namespace` of relay.py plus the attributes the original sets on it while running.
#[derive(Clone, Default)]
pub struct Args {
    pub topology: Option<String>,
    pub manifest: Option<String>,
    pub context_id: Option<String>,
    pub max_turns: Option<i64>,
    pub provider: Option<String>,
    pub max_dispatches: Option<i64>,
    pub permission_profile: Option<String>,
    pub sandbox_mode: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub budget_source: Option<String>,
    pub backend: Option<String>,
    pub capacity_retries: Option<i64>,
    pub capacity_backoff_seconds: Option<i64>,
    pub capacity_deadline_utc: Option<String>,
    pub capacity_approval_file: Option<String>,
    pub reasoning_effort: Option<String>,
    pub runner_effort: Vec<String>,
    pub model: Option<String>,
    pub runners: Option<String>,
    pub resume: Option<String>,
    pub task: Option<String>,
    pub task_file: Option<String>,
    pub handoff_issue: Option<String>,
    pub continue_: bool,
    pub apply: bool,
    pub reconcile_inflight: bool,
    pub reconcile_result: Option<String>,
    pub emit_only: bool,
    pub repo_root: String,
    pub campaign_id: Option<String>,
    pub no_campaign: bool,
    pub node: Option<String>,
    pub context_kind: Option<String>,
    pub cell: Option<String>,
    pub budget_from_phase: bool,
    pub supervise_step: Option<String>,
    pub cost_cap_attempts: Option<i64>,
    pub utc: Option<String>,
    pub campaign_control: Option<String>,
    pub orphan_sweep: bool,
    pub reason: Option<String>,
    pub self_test: bool,
    // ── attributes set while running
    pub ladder: Vec<V>,
    pub runner_efforts: Option<V>,
    pub manifest_path: String,
    pub control_variables: Option<V>,
    pub historical: Option<V>,
    pub continuation_source_attempt: Option<i64>,
    pub capacity_resume_policy: Option<V>,
    pub prepared_capacity_policy: Option<V>,
    pub sweep_ids: Vec<String>,
    pub control_slice: Option<V>,
}

const EFFORTS: &[&str] = &["default", "none", "minimal", "low", "medium", "high", "xhigh", "max"];
const CONTROLS: &[&str] = &["suspend", "resume", "close", "abandon", "reslice"];

fn spec(backends: &'static [&'static str]) -> &'static Spec {
    let args: Vec<Arg> = vec![
        HELP,
        store(&["--topology"]).choices(&["single", "multi"]),
        store(&["--manifest"]),
        store(&["--context-id"]),
        store(&["--max-turns"]).int(),
        store(&["--provider"]).choices(&["claude_code", "codex_cli"]),
        store(&["--max-dispatches"]).int(),
        store(&["--permission-profile"]).group(1),
        store(&["--sandbox-mode"]).choices(&["read-only", "workspace-write"]).group(1),
        store(&["--timeout-seconds"]).int(),
        store(&["--budget-source"]),
        store(&["--backend"]).choices(backends),
        store(&["--capacity-retries"]).int(),
        store(&["--capacity-backoff-seconds"]).int(),
        store(&["--capacity-deadline-utc"]),
        store(&["--capacity-approval-file"]),
        store(&["--reasoning-effort"]).choices(EFFORTS),
        store(&["--runner-effort"]).append(),
        store(&["--model"]),
        store(&["--runners"]),
        store(&["--resume"]),
        store(&["--task"]),
        store(&["--task-file"]),
        store(&["--handoff-issue"]),
        flag(&["--continue"]),
        flag(&["--apply"]),
        flag(&["--reconcile-inflight"]),
        store(&["--reconcile-result"]),
        flag(&["--emit-only"]),
        store(&["--repo-root"]),
        store(&["--campaign-id"]),
        flag(&["--no-campaign"]),
        store(&["--node"]),
        store(&["--context-kind"]).choices(&["build", "cell"]),
        store(&["--cell"]),
        flag(&["--budget-from-phase"]),
        store(&["--supervise-step"]),
        store(&["--cost-cap-attempts"]).int(),
        store(&["--utc"]),
        store(&["--campaign-control"]).choices(CONTROLS),
        flag(&["--orphan-sweep"]),
        store(&["--reason"]),
        flag(&["--self-test"]),
    ];
    Box::leak(Box::new(Spec {
        prog: "relay.sh",
        usage: include_str!("../../help/relay.usage"),
        help: include_str!("../../help/relay.help"),
        args: Box::leak(args.into_boxed_slice()),
        subs: None,
    }))
}

fn parse(argv: &[String]) -> Res<Result<(Args, &'static Spec), i32>> {
    let backends = crate::bootstrap_canary::backend_choices()?;
    let backends: &'static [&'static str] = Box::leak(backends.into_iter().map(|b| &*Box::leak(b.into_boxed_str())).collect::<Vec<&'static str>>().into_boxed_slice());
    let sp = spec(backends);
    let p = match argp::parse(sp, argv) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return Ok(Err(c)),
    };
    let t = |i: usize| p.text(i).map(String::from);
    let a = Args {
        topology: t(1),
        manifest: t(2),
        context_id: t(3),
        max_turns: p.int(4),
        provider: t(5),
        max_dispatches: p.int(6),
        permission_profile: t(7),
        sandbox_mode: t(8),
        timeout_seconds: p.int(9),
        budget_source: t(10),
        backend: t(11),
        capacity_retries: p.int(12),
        capacity_backoff_seconds: p.int(13),
        capacity_deadline_utc: t(14),
        capacity_approval_file: t(15),
        reasoning_effort: t(16),
        runner_effort: p.many(17).to_vec(),
        model: t(18),
        runners: t(19),
        resume: t(20),
        task: t(21),
        task_file: t(22),
        handoff_issue: t(23),
        continue_: p.flag(24),
        apply: p.flag(25),
        reconcile_inflight: p.flag(26),
        reconcile_result: t(27),
        emit_only: p.flag(28),
        repo_root: t(29).unwrap_or_else(repo),
        campaign_id: t(30),
        no_campaign: p.flag(31),
        node: t(32),
        context_kind: t(33),
        cell: t(34),
        budget_from_phase: p.flag(35),
        supervise_step: t(36),
        cost_cap_attempts: p.int(37),
        utc: t(38),
        campaign_control: t(39),
        orphan_sweep: p.flag(40),
        reason: t(41),
        self_test: p.flag(42),
        ..Default::default()
    };
    Ok(Ok((a, sp)))
}

fn nonempty(x: &Option<String>) -> bool {
    x.as_deref().is_some_and(|v| !v.is_empty())
}

fn read_task_file(path: &str) -> Res<String> {
    let b = std::fs::read(path).map_err(|e| tb(&crate::pathfmt::os_error(&e, path)))?;
    String::from_utf8(b).map_err(|e| tb(&format!("UnicodeDecodeError: {e}")))
}

/// `context_id` must be one safe filename component (`[A-Za-z0-9][A-Za-z0-9_.-]*`).
fn safe_component(c: &str) -> bool {
    let mut it = c.chars();
    matches!(it.next(), Some(x) if x.is_ascii_alphanumeric()) && it.all(|x| x.is_ascii_alphanumeric() || "_.-".contains(x))
}

pub fn real_main(argv: &[String]) -> Res<i32> {
    let (mut a, sp) = match parse(argv)? {
        Ok(x) => x,
        Err(c) => return Ok(c),
    };
    if a.self_test {
        return super::selftest::self_test();
    }
    if nonempty(&a.task) && nonempty(&a.task_file) {
        return Ok(argp::error(sp, "--task and --task-file are mutually exclusive"));
    }
    if a.continue_ && nonempty(&a.task) {
        return Ok(argp::error(sp, "--continue accepts --task-file, not --task"));
    }
    if let Some(h) = &a.handoff_issue {
        if h.trim().is_empty() || !(nonempty(&a.task) || nonempty(&a.task_file)) {
            return Ok(argp::error(sp, "--handoff-issue requires an observed issue and an explicit correction task"));
        }
    }
    if nonempty(&a.supervise_step) && (nonempty(&a.task) || nonempty(&a.task_file)) {
        return Ok(argp::error(sp, "--supervise-step does not accept task input"));
    }
    if nonempty(&a.reconcile_result) && !a.reconcile_inflight {
        return Err(Exit::msg("[relay] --reconcile-result requires --reconcile-inflight"));
    }
    if a.reconcile_inflight && (a.continue_ || nonempty(&a.task) || nonempty(&a.task_file) || nonempty(&a.supervise_step) || nonempty(&a.campaign_control)) {
        return Err(Exit::msg("[relay] reconciliation cannot be combined with a dispatch or supervision action"));
    }
    if nonempty(&a.runners) && (nonempty(&a.backend) || nonempty(&a.model) || nonempty(&a.provider)) {
        return Err(Exit::msg("[relay] STOP: --runners 와 --backend/--model 은 같은 것을 정하는 두 창구다 — 하나만 선언하라(사다리를 쓸 거면 --runners)."));
    }
    a.ladder = resolve_ladder(&a.repo_root, a.runners.as_deref(), a.backend.as_deref(), a.model.as_deref(), a.provider.as_deref())?;
    if nonempty(&a.supervise_step) {
        return supervise_step(&a);
    }
    let Some(ctx) = a.context_id.clone().filter(|c| !c.is_empty()) else {
        return Err(Exit::msg("[relay] --context-id 필수"));
    };
    if !safe_component(&ctx) {
        return Err(Exit::msg("[relay] STOP: context-id must be a single safe filename component"));
    }
    if a.reconcile_inflight && nonempty(&a.campaign_id) {
        set_relay_campaign(a.campaign_id.clone());
    }
    let mut lp = ledger_path(&a.repo_root, &ctx)?;
    let mut doc = load_ledger(&lp)?;
    if a.reconcile_inflight {
        return reconcile_inflight(&a, &mut doc, &lp);
    }
    bind_campaign_context(&mut doc, a.node.as_deref(), a.context_kind.as_deref(), a.cell.as_deref())?;
    if nonempty(&a.runners) {
        let names: Vec<String> = a.ladder.iter().map(|r| py_str(g(r, "name"))).collect();
        set(&mut doc, "runner_ladder", s(&names.join(",")));
    }
    if !a.continue_ {
        restore_runner_efforts(&mut a, &mut doc)?;
    }
    let (camp, cv) = resolve_campaign_axis(&a)?;
    a.campaign_id = camp;
    a.control_variables = Some(cv);
    let old_axis = g(&doc, "control_variables").clone();
    if a.continue_ && !attempts(&doc).is_empty() && truthy(&a.cv()) && pv::ne(&old_axis, &a.cv()) {
        if !nonempty(&a.task_file) {
            return Err(Exit::msg("[relay] STOP: revised campaign controls require a complete explicit --task-file"));
        }
        require_official_axis_revision(&a.repo_root, a.campaign_id.as_deref().unwrap_or(""), &old_axis, &a.cv())?;
        let Some(previous) = last_reached(&doc) else {
            return Err(Exit::msg("[relay] STOP: revised controls have no prior reached checkpoint"));
        };
        let issued = instruction_request(&a.repo_root, &doc, &previous)?;
        if pv::ne(g(&issued, "control_variables"), &old_axis) || pv::ne(g(&issued, "campaign_id"), &pv::opt_s(a.campaign_id.as_deref())) {
            return Err(Exit::msg("[relay] STOP: prior checkpoint request does not bind its historical campaign axis"));
        }
        a.historical = Some(if truthy(&old_axis) { old_axis.clone() } else { pv::empty() });
    }
    if let Some(c) = a.campaign_id.clone().filter(|c| !c.is_empty()) {
        set(&mut doc, "campaign_id", s(&c));
    }
    if truthy(&a.cv()) {
        set(&mut doc, "control_variables", a.cv());
    }
    if (a.continue_ || nonempty(&a.task) || nonempty(&a.task_file)) && reuse_completed_handoff(&a, &mut doc, &lp)? {
        return Ok(0);
    }

    if a.continue_ {
        if attempts(&doc).is_empty() {
            return Err(Exit::msg(format!("[relay] STOP: 이어갈 원장이 없다 — {lp}. 첫 위임은 `--task` 로 연다.")));
        }
        ensure_recovery_pending(&a.repo_root, &doc)?;
        if !nonempty(&a.topology) {
            a.topology = pv::st(g(&doc, "topology"));
        }
        if !(nonempty(&a.runners) || nonempty(&a.backend) || nonempty(&a.model) || nonempty(&a.provider)) {
            if let Some(spec) = ledger_runner_spec(&doc) {
                a.ladder = resolve_ladder(&a.repo_root, Some(&spec), None, None, None)?;
            }
        }
        if a.reasoning_effort.is_none() {
            a.reasoning_effort = pv::st(g(&doc, "reasoning_effort"));
        }
        restore_runner_efforts(&mut a, &mut doc)?;
        if !(nonempty(&a.permission_profile) || nonempty(&a.sandbox_mode)) {
            apply_permission_selection(&mut a, &doc);
        }
        if !nonempty(&a.topology) {
            return Err(Exit::msg("[relay] STOP: 원장에 topology 가 없다(구버전 원장) — 명시하라."));
        }
        a.manifest_path = match a.manifest.clone().filter(|m| !m.is_empty()).or_else(|| pv::st(g(&doc, "manifest")).filter(|m| !m.is_empty())) {
            Some(m) => m,
            None => join(&repo(), &format!("output/{}/manifest.yaml", a.topology.clone().unwrap_or_default())),
        };
        let stalled = stalled_attempts(&doc);
        let last = attempts(&doc).iter().rev().find(|x| reached_sub(x)).cloned();
        if let Some(l) = &last {
            if is(g(l, "status"), "completed") && codex_recovery_requirement(l).is_none() && a.handoff_issue.is_none() && !a.historical.as_ref().is_some_and(truthy) {
                return Err(Exit::msg("[relay] STOP: 직전 attempt 가 completed 다 — 이을 중단점이 없다.\n  → 새 과업이면 새 --context-id 로 `--task` 를 연다."));
            }
        }
        if let Some(l) = &last {
            let svcs = g(l, "running_services");
            if pv::is_list(svcs) && !items(svcs).is_empty() {
                let names: Vec<String> = items(svcs).iter().filter(|x| pv::is_dict(x)).map(|x| py_str(g(x, "name"))).collect();
                return Err(Exit::msg(format!(
                    "[relay] STOP: 직전 턴이 서비스를 남긴 채 끝났다 — {}\n  그 상태로 다음 턴을 열면 두 서빙이 같은 호스트 메모리를 두고 다툰다.\n  → 서브에 정리를 지시하거나(A2A 제어명령), 의도적 상주면 그 사실을\n    `--task` 본문에 적고 **새 context** 로 열어라(우회로 잇지 않는다).",
                    names.join(", ")
                )));
            }
            if is(g(l, "budget_outcome"), "exhausted") && pv::is_none(g(l, "running_services")) {
                warn("[relay] ⚠ 직전 턴이 예산을 소진했는데 running_services 를 말하지 않았다 — 고아 서비스 여부가 **모름**이다. 서브 규약(task-report.running_services)을 따르게 하라.");
            }
            let camp = match a.campaign_id.clone().filter(|c| !c.is_empty()) {
                Some(c) => s(&c),
                None => g(&doc, "campaign_id").clone(),
            };
            let cv = match &a.historical {
                Some(h) => h.clone(),
                None => g(&doc, "control_variables").clone(),
            };
            let stop = echo_stop_reasons(l, &s(&ctx), &camp, &cv);
            if !stop.is_empty() {
                let lines: String = stop.iter().map(|r| format!("  - {r}\n")).collect();
                return Err(Exit::msg(format!(
                    "[relay] STOP: 서브 정체성 echo 가 성립하지 않는다 — 부재도 불일치도 STOP 이다.\n{lines}  → 서브가 위임 헤더의 값을 그대로 회신하도록 한 뒤 재개하라(우회 ✗)."
                )));
            }
        }
        let pend = pending_for(&a.repo_root, &s(&ctx))?;
        let blocked = resume_blockers(&pend);
        if !blocked.is_empty() {
            let ids: Vec<String> = blocked.iter().map(|e| py_str(g(e, "request_id"))).collect();
            return Err(Exit::msg(format!(
                "[relay] STOP: 답이 필요한 **차단성** 요청이 있다 — {}\n  → {}/{PENDING_HITL} 의 해당 항목 `answer` 에 실제 사용자 답을 기록하라.\n  → 답의 존재는 승인이 아니다. 사용자의 재개 의사를 확인한 경우에만 --continue 하라.\n  → Codex 미해결 실패의 명시적 수동 복구 조건은 그대로 적용된다.",
                ids.join(", "),
                relay_root(&a.repo_root)?
            )));
        }
        let lks = last_known_session(&doc);
        let mut task;
        if let Some(tf) = a.task_file.clone().filter(|t| !t.is_empty()) {
            task = read_task_file(&tf)?;
            if task.trim().is_empty() {
                return Err(Exit::msg("[relay] STOP: --task-file is empty"));
            }
        } else {
            a.continuation_source_attempt = pv::int(g(&continuation_instruction(&a.repo_root, &doc)?, "attempt"));
            task = assemble_continuation(&a.repo_root, &doc, &pend)?;
        }
        if let Some(h) = &a.handoff_issue {
            task = format!("[실제 인계 문제의 정정·설명 요청] {h}\n{task}");
        }
        let last_out = last.as_ref().map(|l| g(l, "budget_outcome").clone()).unwrap_or(V::None);
        if !a.apply {
            say(&format!("[relay] --continue DRY-RUN · context={ctx}"));
            let slot = a.ladder[next_runner_index(&doc, &a.ladder)].clone();
            let (effort, source) = runner_effort(&a, &slot);
            say(&format!("[relay] 추론 강도 선언: runner={} effort={effort} source={source}", py_str(g(&slot, "name"))));
            let sid = g(&lks, "session_id");
            say(&format!(
                "[relay] 마지막 알려진 세션: {} (attempt {} · sub_status={} · budget={}) → `--resume <id|new>` 로 선언하라",
                if truthy(sid) { py_str(sid) } else { "(없음)".into() },
                py_str(g(&lks, "attempt")),
                py_str(g(&lks, "status")),
                py_str(g(&lks, "budget_outcome"))
            ));
            let (text, flags) = continuation_budget_preview(&doc, &a.ladder, last.as_ref(), a.timeout_seconds)?;
            say(&format!(
                "[relay] 예산 사실: {text} · 직전 결과 {} · 대기 요청 {}건 · 전진 없는 연속 attempt {stalled}회",
                py_str(&last_out),
                pend.len()
            ));
            if is(&last_out, "exhausted") {
                say("[relay]   직전 호출은 소진됐다 — 새 실행 예산을 선언하고 checkpoint부터 이어라. 관측과 남은 과업에 따라 필요하면 조정한다.");
            }
            let rule: String = "─".repeat(72);
            say(&rule);
            say_raw(&task);
            say(&rule);
            say(&format!("[relay] 위 본문으로 위임하려면 {flags} 을 선언해 --apply 하라."));
            return Ok(0);
        }
        let bud = declared_budget(&a, a.timeout_seconds, a.budget_source.as_deref().unwrap_or(""))?;
        let resume = require_resume(a.resume.as_deref(), &lks)?;
        return run_attempt(&mut a, &mut doc, &lp, &task, &bud, &resume);
    }

    if !nonempty(&a.topology) {
        return Err(Exit::msg("[relay] --topology 필수"));
    }
    let mut task = a.task.clone().unwrap_or_default();
    if let Some(control) = a.campaign_control.clone() {
        let (Some(camp), Some(node)) = (a.campaign_id.clone().filter(|c| !c.is_empty()), a.node.clone().filter(|n| !n.is_empty())) else {
            return Err(Exit::msg("[relay] STOP: --campaign-control 은 --campaign-id 와 --node 가 필요하다(이름 추론 ✗)"));
        };
        if !task.is_empty() || nonempty(&a.task_file) {
            return Err(Exit::msg("[relay] STOP: --campaign-control 의 본문은 기계가 조립한다 — --task 와 함께 쓰지 않는다"));
        }
        let mbase = format!("{}/campaigns/{camp}", a.repo_root);
        let mut reason: Option<String> = None;
        let life;
        if a.orphan_sweep {
            if control != "abandon" {
                return Err(Exit::msg("[relay] STOP: --orphan-sweep 은 --campaign-control abandon 하고만 쓴다"));
            }
            a.sweep_ids = camp.split(',').map(strip).filter(|c| !c.is_empty()).collect();
            let present: Vec<String> = a.sweep_ids.iter().filter(|c| Path::new(&a.repo_root).join("campaigns").join(c).exists()).cloned().collect();
            if !present.is_empty() {
                return Err(Exit::msg(format!(
                    "[relay] STOP: 메인에 campaigns/{} 이 있다 — orphan sweep 이 아니라 메인 `--abandon` 뒤 전파로 간다(정식 경로)",
                    pv::py_repr(&pv::strs(&present))
                )));
            }
            if !nonempty(&a.reason) {
                return Err(Exit::msg("[relay] STOP: --orphan-sweep 은 --reason 이 필요하다(사람이 고른 포기 사유 전사)"));
            }
            reason = a.reason.clone();
            life = pv::obj(vec![("interrupted_cells", pv::list(vec![]))]);
        } else {
            life = fold_lifecycle(&a.repo_root, &camp).map_err(|m| tb(&m))?;
            let state = py_str(g(&life, "state"));
            if control == "reslice" && !has(g(&life, "reslice_pending"), &node) {
                return Err(Exit::msg(format!(
                    "[relay] STOP: {node} 에 미수렴 재배정이 없다({}) — 메인 `campaign_init --assign-set` 이 먼저다",
                    pv::py_repr(g(&life, "reslice_pending"))
                )));
            }
            if control != "reslice" && !pv::contains_str(g(&life, "sub_nodes"), &node) {
                return Err(Exit::msg(format!(
                    "[relay] STOP: {node} 는 {camp} 의 마지막 전이가 지목한 서브가 아니다({}) — 메인 전이를 먼저 하라",
                    pv::py_repr(g(&life, "sub_nodes"))
                )));
            }
            if control == "reslice" {
                if state != "active" {
                    return Err(Exit::msg(format!(
                        "[relay] STOP: 메인 {camp} 은 {state} 다 — 재배정은 active 캠페인에만 전한다(보류 중이면 서브 writer 도 거부한다)"
                    )));
                }
            } else if let Some(cr) = close_reason_of(&control) {
                if state != "closed" || !is(g(&life, "close_reason"), cr) {
                    return Err(Exit::msg(format!(
                        "[relay] STOP: 메인 {camp} 은 {state}({}) 다 — {control} 를 서브에 전하려면 메인이 먼저 closed({cr}) 여야 한다",
                        py_str(g(&life, "close_reason"))
                    )));
                }
                let closed = read_journey(&mbase).into_iter().rev().find(|e| pv::is_dict(e) && is(g(e, "kind"), "campaign_closed")).unwrap_or_else(pv::empty);
                reason = a.reason.clone().filter(|r| !r.is_empty()).or_else(|| pv::st(g(&closed, "note")).filter(|n| !n.is_empty()));
            } else {
                let want = if control == "suspend" { "suspended" } else { "active" };
                if state != want && state != "resume_pending" {
                    return Err(Exit::msg(format!(
                        "[relay] STOP: 메인 {camp} 은 {state} 다 — {control} 를 서브에 전할 상태가 아니다(메인 전이가 먼저)"
                    )));
                }
            }
        }
        let slice = if control == "reslice" { Some(emit_slice(&a.repo_root, &camp, &node)?) } else { None };
        a.control_slice = slice.clone();
        task = if a.orphan_sweep {
            orphan_sweep_task(&a.sweep_ids, reason.as_deref().unwrap_or(""))
        } else {
            campaign_control_task(&camp, &control, truthy(g(&life, "interrupted_cells")), reason.as_deref(), slice.as_ref().unwrap_or(&V::None))?
        };
        set_relay_campaign(if a.orphan_sweep { None } else { Some(camp.clone()) });
        lp = ledger_path(&a.repo_root, &ctx)?;
        doc = load_ledger(&lp)?;
        bind_campaign_context(&mut doc, a.node.as_deref(), a.context_kind.as_deref(), a.cell.as_deref())?;
        set(&mut doc, "campaign_id", s(&camp));
        if truthy(&a.cv()) {
            set(&mut doc, "control_variables", a.cv());
        }
        if nonempty(&a.runners) {
            let names: Vec<String> = a.ladder.iter().map(|r| py_str(g(r, "name"))).collect();
            set(&mut doc, "runner_ladder", s(&names.join(",")));
        }
    }
    if let Some(tf) = a.task_file.clone().filter(|t| !t.is_empty()) {
        task = read_task_file(&tf)?;
    }
    if task.is_empty() {
        return Err(Exit::msg("[relay] --task 또는 --task-file 필수"));
    }
    if let Some(h) = &a.handoff_issue {
        task = format!("[실제 인계 문제의 정정·설명 요청] {h}\n{task}");
    }
    a.manifest_path = a.manifest.clone().filter(|m| !m.is_empty()).unwrap_or_else(|| join(&repo(), &format!("output/{}/manifest.yaml", a.topology.clone().unwrap_or_default())));
    let mut timeout = a.timeout_seconds;
    let mut src = a.budget_source.clone().unwrap_or_default();
    if a.budget_from_phase && timeout.is_none() {
        let (t, why) = derive_timeout(&a.repo_root, a.campaign_id.as_deref(), a.node.as_deref(), 1.5)?;
        timeout = Some(match t {
            Some(x) if x != 0 => x,
            _ => schema_cap("timeout_seconds")?,
        });
        src = if src.is_empty() { format!("시간 예산 파생: {why}") } else { format!("{src} · 시간 예산 파생: {why}") };
    }
    let bud = declared_budget(&a, timeout, &src)?;
    let resume = require_resume(a.resume.as_deref(), &last_known_session(&doc))?;
    run_attempt(&mut a, &mut doc, &lp, &task, &bud, &resume)
}

/// `campaign_init.read_journey(base)`: the JSON lines of `journey.jsonl` (an unparsable line becomes `{"_unparsed": line}`).
pub fn read_journey(base: &str) -> Vec<V> {
    let path = Path::new(base).join("journey.jsonl");
    let Ok(t) = std::fs::read_to_string(&path) else { return vec![] };
    crate::py::splitlines(&t)
        .into_iter()
        .map(|l| strip(&l))
        .filter(|l| !l.is_empty())
        .map(|l| pv::loads(&l).unwrap_or_else(|_| pv::obj(vec![("_unparsed", s(&l))])))
        .collect()
}

pub fn main(argv: &[String]) -> Res<i32> {
    if argv.iter().any(|x| x == "--help" || x == "-h" || x == "--self-test") {
        return real_main(argv);
    }
    let mut repo_root = repo();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "--repo-root" && i + 1 < argv.len() {
            repo_root = argv[i + 1].clone();
        } else if let Some(v) = arg.strip_prefix("--repo-root=") {
            repo_root = v.to_string();
        }
        i += 1;
    }
    let abs = if repo_root.starts_with('/') { norm_path(&repo_root) } else { norm_path(&join(&std::env::current_dir().unwrap_or_default().display().to_string(), &repo_root)) };
    let _lock = state_lock(&format!("{abs}/output/.relay-dispatch"), false)?;
    real_main(argv)
}

impl Args {
    /// `a.control_variables` (None when unset).
    pub fn cv(&self) -> V {
        self.control_variables.clone().unwrap_or(V::None)
    }
}
