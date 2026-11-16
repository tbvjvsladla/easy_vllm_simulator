//! relay.py: dispatch (one attempt, the runner ladder, capacity bursts), recovery/reconciliation and the supervisor step.

use super::cli::Args;
use super::core::*;
use super::pv::{self, eq, g, has, is, is_any, items, ne, obj, py_str, s, set, truthy, V};
use crate::sysexit::{Exit, Res};
use std::path::Path;

pub fn runner_effort(a: &Args, runner: &V) -> (String, String) {
    let name = py_str(g(runner, "name"));
    if let Some(ov) = &a.runner_efforts {
        if has(ov, &name) {
            return (py_str(g(ov, &name)), format!("runner:{name}"));
        }
    }
    match &a.reasoning_effort {
        Some(e) => (e.clone(), "caller".into()),
        None => ("default".into(), "native-default".into()),
    }
}

pub fn effort_matches(a: &Args, runner: &V, request: &V) -> bool {
    let have = if has(request, "reasoning_effort") { py_str(g(request, "reasoning_effort")) } else { "default".into() };
    have == runner_effort(a, runner).0
}

pub fn restore_runner_efforts(a: &mut Args, doc: &mut V) -> Res<()> {
    let mut values = if truthy(g(doc, "runner_efforts")) { g(doc, "runner_efforts").clone() } else { pv::empty() };
    let names: Vec<String> = a.ladder.iter().map(|r| py_str(g(r, "name"))).collect();
    if a.runners.as_deref().is_some_and(|r| !r.is_empty()) {
        let mut kept = pv::empty();
        for (k, v) in pv::entries(&values) {
            if names.contains(&k) {
                set(&mut kept, &k, v.clone());
            }
        }
        values = kept;
    }
    let mut seen: Vec<String> = vec![];
    for entry in a.runner_effort.clone() {
        let (name, sep, level) = match entry.find('=') {
            Some(i) => (entry[..i].to_string(), true, entry[i + 1..].to_string()),
            None => (entry.clone(), false, String::new()),
        };
        let schema = crate::agent_control::load_schema(&crate::agent_control::request_schema_path()).map_err(|m| tb(&m))?;
        let allowed = g(g(g(&schema, "properties"), "reasoning_effort"), "enum");
        if !sep || !names.contains(&name) || !pv::contains_str(allowed, &level) || seen.contains(&name) {
            return Err(Exit::msg("[relay] STOP: --runner-effort requires a unique declared RUNNER=LEVEL"));
        }
        set(&mut values, &name, s(&level));
        seen.push(name);
    }
    if pv::keys(&values).iter().any(|k| !names.contains(k)) {
        return Err(Exit::msg("[relay] STOP: saved effort slots do not match the declared ladder"));
    }
    a.runner_efforts = Some(values.clone());
    if truthy(&values) || has(doc, "runner_efforts") {
        set(doc, "runner_efforts", values);
    }
    Ok(())
}

pub fn build_request(a_repo: &str, topology: &str, manifest: &str, task: &str, bud: &V, o: &BuildOpts) -> Res<V> {
    let opts = crate::bootstrap_canary::RequestOpts {
        max_turns: g(bud, "max_turns").clone(),
        timeout_seconds: g(bud, "timeout_seconds").clone(),
        budget_source: pv::st(g(bud, "source")),
        model: o.model.clone(),
        backend: o.backend.clone(),
        provider: o.provider.clone(),
        sandbox_mode: o.sandbox_mode.clone(),
        permission_profile: o.permission_profile.clone(),
        reasoning_effort: o.reasoning_effort.clone(),
        ..Default::default()
    };
    let mut base = crate::bootstrap_canary::build_request(topology, manifest, &opts)?;
    let allocated = g(bud, "max_turns").clone();
    set(&mut base, "intent", s("delegate"));
    let camp_block = campaign_control_block(o.campaign_id.as_deref(), &o.control_variables);
    let slice_block = campaign_slice_block(a_repo, topology, o.campaign_id.as_deref(), o.delegated_node.as_deref())?;
    let body = match o.context_id.as_deref().filter(|c| !c.is_empty()) {
        Some(ctx) => {
            relay_header(ctx, o.attempt, &allocated, &py_str(g(bud, "source")), g(bud, "timeout_seconds")) + &camp_block + &slice_block + task
        }
        None => camp_block + &slice_block + task,
    };
    set(&mut base, "task", s(&body));
    if let Some(c) = o.campaign_id.as_deref().filter(|c| !c.is_empty()) {
        set(&mut base, "campaign_id", s(c));
    }
    if truthy(&o.control_variables) {
        set(&mut base, "control_variables", o.control_variables.clone());
    }
    set(&mut base, "capabilities", pv::strs(&DEFAULT_CAPABILITIES.iter().map(|x| x.to_string()).collect::<Vec<_>>()));
    if o.provider == "codex_cli" {
        let t = format!(
            "예산 단위는 wall_time 이며 native turn 제한은 없다. 시간 상한 {}초. max_turns_allocated=null 을 0턴이나 무제한 실행 승인으로 해석하지 마라. AGENTS.md와 그 문서가 지정한 정본을 읽고, 제공된 아티팩트에서 이어가라.\n{}",
            py_str(g(bud, "timeout_seconds")),
            py_str(g(&base, "task"))
        );
        set(&mut base, "task", s(&t));
    }
    if let Some(b) = o.backend.as_deref().filter(|b| !b.is_empty()) {
        set(&mut base, "backend", s(b));
    }
    Ok(base)
}

#[derive(Clone)]
pub struct BuildOpts {
    pub context_id: Option<String>,
    pub attempt: i64,
    pub model: Option<String>,
    pub campaign_id: Option<String>,
    pub control_variables: V,
    pub backend: Option<String>,
    pub delegated_node: Option<String>,
    pub provider: String,
    pub sandbox_mode: Option<String>,
    pub permission_profile: Option<String>,
    pub reasoning_effort: Option<String>,
}

pub fn resolve_campaign_axis(a: &Args) -> Res<(Option<String>, V)> {
    if let Some(h) = super::selftest::hook_campaign_axis() {
        return Ok(h);
    }
    if a.no_campaign {
        return Ok((None, V::None));
    }
    let camp = match a.campaign_id.clone().filter(|c| !c.is_empty()) {
        Some(c) => c,
        None => match active_campaign_id(&a.repo_root) {
            Ok(c) => c,
            Err(_) => return Ok((None, V::None)),
        },
    };
    if camp.is_empty() || camp == "_bootstrap" {
        return Ok((None, V::None));
    }
    let decl_path = Path::new(&a.repo_root).join("campaigns").join(&camp).join("campaign.yaml");
    if !decl_path.is_file() {
        return Ok((Some(camp), V::None));
    }
    let Ok(decl) = pv::load_file(&decl_path) else { return Ok((Some(camp), V::None)) };
    let revs = g(&decl, "revisions");
    let mut cur = if pv::is_list(revs) && !items(revs).is_empty() { g(items(revs).last().unwrap(), "values").clone() } else { V::None };
    if !truthy(&cur) {
        cur = g(&decl, "control_variables").clone();
    }
    let mut control = pv::empty();
    for (k, v) in pv::entries(&cur) {
        if !k.starts_with('_') && !pv::is_dict(v) && !pv::is_list(v) {
            set(&mut control, &k, v.clone());
        }
    }
    Ok((Some(camp), if truthy(&control) { control } else { V::None }))
}

pub fn require_official_axis_revision(repo_root: &str, campaign_id: &str, old_axis: &V, new_axis: &V) -> Res<()> {
    let ok = (|| -> Option<()> {
        let path = Path::new(repo_root).join("campaigns").join(campaign_id).join("campaign.yaml");
        let declaration = pv::loads(&std::fs::read_to_string(path).ok()?).ok()?;
        let revisions = g(&declaration, "revisions");
        if !is(g(&declaration, "id"), campaign_id)
            || !pv::is_dict(old_axis)
            || !truthy(old_axis)
            || !pv::is_list(revisions)
            || items(revisions).len() < 2
            || !pv::is_dict(g(&declaration, "control_variables"))
            || ne(g(&items(revisions)[0], "values"), g(&declaration, "control_variables"))
        {
            return None;
        }
        let axis = |values: &V| -> Option<V> {
            if !pv::is_dict(values) {
                return None;
            }
            let mut d = pv::empty();
            for (k, v) in pv::entries(values) {
                if !k.starts_with('_') && !pv::is_dict(v) && !pv::is_list(v) {
                    set(&mut d, &k, v.clone());
                }
            }
            Some(d)
        };
        for (i, revision) in items(revisions).iter().enumerate() {
            if !pv::is_dict(revision) || !pv::is_int(g(revision, "rev")) || !eq(g(revision, "rev"), &pv::i(i as i64)) {
                return None;
            }
            if i > 0
                && ["declared_utc", "reason", "evidence", "approved_by"]
                    .iter()
                    .any(|k| pv::st(g(revision, k)).map_or(true, |t| t.trim().is_empty()))
            {
                return None;
            }
        }
        let revs = items(revisions);
        if ne(&axis(g(revs.last().unwrap(), "values"))?, new_axis) {
            return None;
        }
        let mut found = false;
        for r in &revs[..revs.len() - 1] {
            if eq(&axis(g(r, "values"))?, old_axis) {
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
        Some(())
    })();
    ok.ok_or_else(|| Exit::msg("[relay] STOP: control axis change lacks an approved campaign revision"))
}

pub fn require_dispatch_recovery(a: &Args, doc: &V, resume_declared: &str) -> Res<()> {
    let pending = ensure_recovery_pending(&a.repo_root, doc)?;
    if !resume_blockers(&pending).is_empty() {
        return Err(Exit::msg("[relay] STOP: unanswered blocking HITL request; record the human answer before dispatch"));
    }
    let last = last_attempt(doc).filter(|x| truthy(x));
    let recovery = last.and_then(codex_recovery_requirement);
    let manual = a.continue_ && a.apply && a.resume.as_deref().map(strip).is_some_and(|r| !r.is_empty() && r == resume_declared);
    if let Some(rec) = &recovery {
        let answered = pending.iter().any(|e| {
            eq(g(e, "recovery_required"), rec) && entry_blocking(e) && pv::st(g(e, "answer")).is_some_and(|t| !t.trim().is_empty())
        });
        if !answered || !manual {
            return Err(Exit::msg("[relay] STOP: unresolved Codex failure requires its human recovery answer and explicit --continue --apply --resume <session_id|new>; automatic replay is forbidden"));
        }
    }
    let reasons = continuation_stop_reasons(doc, recovery.is_some() && manual, a.historical.as_ref());
    if !reasons.is_empty() {
        return Err(Exit::msg(format!("[relay] STOP: {}", reasons.join("; "))));
    }
    Ok(())
}

pub fn reuse_completed_handoff(a: &Args, doc: &mut V, lp: &str) -> Res<bool> {
    if a.campaign_control.is_some() || attempts(doc).is_empty() {
        return Ok(false);
    }
    let last = attempts(doc).last().unwrap().clone();
    if !is(g(&last, "status"), "completed")
        || !is(g(&last, "control_status"), "completed")
        || !truthy(g(&last, "sub_reported"))
        || codex_recovery_requirement(&last).is_some()
    {
        return Ok(false);
    }
    if a.handoff_issue.is_some() {
        return Ok(false);
    }
    let prior_axis = g(&last, "control_variables_echo");
    if !pv::is_none(prior_axis) && ne(g(doc, "control_variables"), prior_axis) {
        return Ok(false);
    }
    if let Some(t) = a.topology.as_deref().filter(|t| !t.is_empty()) {
        if !is(g(doc, "topology"), t) {
            return Err(Exit::msg("[relay] HANDOFF_RESULT_SCOPE_MISMATCH: topology differs"));
        }
    }
    if let Some(m) = a.manifest.as_deref().filter(|m| !m.is_empty()) {
        let dm = pv::st(g(doc, "manifest")).unwrap_or_default();
        if realpath(m) != realpath(&dm) {
            return Err(Exit::msg("[relay] HANDOFF_RESULT_SCOPE_MISMATCH: manifest differs"));
        }
    }
    let mut blockers = continuation_stop_reasons(doc, false, None);
    if !resume_blockers(&pending_for(&a.repo_root, &pv::opt_s(a.context_id.as_deref()))?).is_empty() {
        blockers.push("existing blocking request remains".into());
    }
    if !blockers.is_empty() {
        return Err(Exit::msg(format!("[relay] HANDOFF_RESULT_UNAVAILABLE: {}", blockers.join("; "))));
    }
    let ctx = a.context_id.clone().unwrap_or_default();
    let checked = (|| -> Result<V, String> {
        let lpr = std::fs::canonicalize(Path::new(lp).parent().unwrap_or(Path::new("."))).unwrap_or_default().join(Path::new(lp).file_name().unwrap_or_default());
        let stem = lpr.with_extension("reports");
        let n = pv::int(g(&last, "attempt")).ok_or("KeyError")?;
        let expected_abs = stem.join(format!("attempt-{n:02}.json"));
        let root = std::fs::canonicalize(&a.repo_root).unwrap_or_else(|_| Path::new(&a.repo_root).to_path_buf());
        let expected = expected_abs
            .strip_prefix(&root)
            .map_err(|_| format!("{} is not in the subpath of {}", pv::repr_s(&expected_abs.display().to_string()), pv::repr_s(&root.display().to_string())))?
            .display()
            .to_string();
        if !is(g(&last, "report_path"), &expected) {
            return Err("report does not belong to this context/attempt".into());
        }
        let p = owned_artifact(&a.repo_root, &expected).map_err(|e| match e {
            Exit::Msg(m) => m,
            Exit::Code(c) => c.to_string(),
        })?;
        let stored = pv::loads(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?).map_err(|e| e)?;
        if !has(&stored, "report") {
            return Err("'report'".into());
        }
        let report = g(&stored, "report").clone();
        let schema = crate::agent_control::load_schema(&task_report_schema())?;
        if !crate::agent_control::schema_violations(&report, &schema, "$").is_empty()
            || !is(g(&report, "status"), "completed")
            || !is(g(&report, "context_id"), &ctx)
            || (truthy(g(doc, "campaign_node")) && ne(g(&report, "node_id"), g(doc, "campaign_node")))
        {
            return Err("completed report scope/schema differs".into());
        }
        Ok(report)
    })();
    let report = checked.map_err(|m| Exit::msg(format!("[relay] HANDOFF_RESULT_UNAVAILABLE: {m}")))?;
    let decision = obj(vec![
        ("action", s("reuse_completed_result")),
        ("reason", s("no_new_handoff_issue")),
        ("context_id", s(&ctx)),
        ("source_attempt", g(&last, "attempt").clone()),
        ("report_path", g(&last, "report_path").clone()),
        ("execution_started", V::Bool(false)),
        ("continue_workflow", V::Bool(true)),
    ]);
    let persist = !a.emit_only && (!a.continue_ || a.apply);
    if persist && ne(g(doc, "handoff_reuse"), &decision) {
        set(doc, "handoff_reuse", decision.clone());
        save_ledger(lp, doc)?;
    }
    let mut out = decision.clone();
    set(&mut out, "report", report);
    say(&pv::dumps_u(&out));
    Ok(true)
}

pub fn realpath(p: &str) -> String {
    if p.is_empty() {
        return std::env::current_dir().unwrap_or_default().display().to_string();
    }
    match std::fs::canonicalize(p) {
        Ok(c) => c.display().to_string(),
        Err(_) => {
            let abs = if p.starts_with('/') { p.to_string() } else { join(&std::env::current_dir().unwrap_or_default().display().to_string(), p) };
            norm_path(&abs)
        }
    }
}

// ── supervisor ─────────────────────────────────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// CHECK-ONLY seam (the lifecycle self-test's `relay.supervise_decide = lambda ...: {...}`): when set, the supervisor
    /// takes this fixed decision instead of computing one. In-process only; production never sets it.
    pub static SUPERVISE_DECIDE_HOOK: std::cell::RefCell<Option<V>> = const { std::cell::RefCell::new(None) };
}

fn check_hook_supervise_decide() -> Option<V> {
    SUPERVISE_DECIDE_HOOK.with(|h| h.borrow().clone())
}

pub fn supervise_step(a0: &Args) -> Res<i32> {
    let camp = a0.supervise_step.clone().unwrap_or_default();
    let ledgers = campaign_ledgers(&a0.repo_root, &camp);
    if ledgers.is_empty() {
        say(&format!(
            "[relay] 감독: campaigns/{camp}/relay 에 캠페인 축이 못박힌 원장이 없다 — `--node`/`--context-kind` 를 선언하고 지시서를 열어라(이름 추론 ✗)"
        ));
        return Ok(0);
    }
    let utc = a0.utc.clone().filter(|u| !u.is_empty()).unwrap_or_else(utcnow);
    let mut rc = 0;
    let explicit_runner = [&a0.runners, &a0.backend, &a0.model, &a0.provider].iter().any(|x| x.as_deref().is_some_and(|v| !v.is_empty()));
    for (path, mut doc) in ledgers {
        let mut a = a0.clone();
        if !explicit_runner {
            if let Some(spec) = ledger_runner_spec(&doc) {
                a.ladder = resolve_ladder(&a.repo_root, Some(&spec), None, None, None)?;
            }
        }
        let node = g(&doc, "campaign_node").clone();
        ensure_recovery_pending(&a.repo_root, &doc)?;
        let brief = read_brief(&a.repo_root, &node);
        let dp = doc_plane_progress(&a.repo_root, &doc);
        let mut decision = match check_hook_supervise_decide() {
            Some(fixed) => fixed,
            None => supervise_decide(&doc, &brief, &a.ladder, dp.as_deref()),
        };
        if is_any(g(&decision, "action"), &["resume", "rotate_runner"]) {
            let deferred = g(&doc, "capacity_deferred").clone();
            let last = last_attempt(&doc).cloned().unwrap_or(V::None);
            let allow = eq(g(&deferred, "dispatch_id"), g(&last, "dispatch_id"))
                && is(g(g(&last, "failure_evidence"), "signal"), "provider_capacity");
            let reasons = continuation_stop_reasons(&doc, allow, None);
            if !reasons.is_empty() {
                decision = decision_v("popup", &reasons.join("; "));
            }
        }
        let life = lifecycle_state(&a.repo_root, &camp);
        if life != "active" {
            decision = decision_v(
                "hold",
                &format!(
                    "캠페인 {camp} 은 {life} 다 — 감독 판정({}: {})을 보류한다. 묻지도 잇지도 않는다(계기 a·d 전까지 · D4). 재개는 사람 결정(`campaign_init.sh --resume-campaign {camp}`) · 정상 차단",
                    py_str(g(&decision, "action")),
                    head(&py_str(g(&decision, "reason")), 80)
                ),
            );
        }
        record_supervisor_step(&mut doc, &decision, &utc, &brief);
        save_ledger(&path, &mut doc)?;
        let action = py_str(g(&decision, "action"));
        say(&format!(
            "[relay] 감독 · {} (node={} kind={} cell={})\n  → {action}: {}",
            py_str(g(&doc, "context_id")),
            py_str(&node),
            py_str(g(&doc, "campaign_context_kind")),
            py_str(g(&doc, "campaign_cell")),
            py_str(g(&decision, "reason"))
        ));
        match action.as_str() {
            "popup" => {
                rc = rc.max(3);
                say(&format!(
                    "  → 이것은 **질문이지 차단이 아니다**. 사람이 답하면 그대로 잇는다(pending: {}).",
                    join(&relay_root(&a.repo_root)?, PENDING_HITL)
                ));
            }
            "next_cell" => say("  → 다음 셀의 지시서는 **새 context** 로 연다(셀 하나 = context 하나 · D20). 모드가 HITL 이면 열기 전에 사람에게 묻는다."),
            "rotate_runner" => {
                if !a.apply {
                    say("  → 회전은 `--supervise-step <camp> --apply` 로 실행한다(단일 스텝).");
                } else {
                    rc = rc.max(supervise_resume(&a, &path, &mut doc, None, "러너 회전; context의 호출 증거로 예산 선택")?);
                }
            }
            "resume" => {
                if !a.apply {
                    say("  → 재발급은 `--supervise-step <camp> --apply` 로 실행한다(단일 스텝).");
                } else {
                    rc = rc.max(supervise_resume(&a, &path, &mut doc, None, "context의 호출 증거로 예산 선택")?);
                }
            }
            _ => {}
        }
    }
    Ok(rc)
}

fn decision_v(action: &str, reason: &str) -> V {
    decision(action, reason)
}

fn positive(v: &V) -> Option<i64> {
    pv::int(v).filter(|n| *n > 0)
}

pub fn select_supervised_budget(a: &Args, doc: &V) -> Res<V> {
    let atts = attempts(doc).to_vec();
    let last = last_reached(doc).unwrap_or_else(pv::empty);
    let phase = g(&last, "phase").clone();
    let previous = if atts.is_empty() { None } else { atts[..atts.len() - 1].iter().rev().find(|x| reached_sub(x)).cloned() };
    let progress = truthy(g(&last, "sub_reported"))
        && ((previous.as_ref().is_some_and(|p| truthy(&phase) && truthy(g(p, "phase")) && ne(g(p, "phase"), &phase)))
            || ledger_progress(&last, previous.as_ref()).is_some());
    let rec = if truthy(g(&last, "sub_reported")) { g(&last, "budget_recommended").clone() } else { V::None };
    let rec = if pv::is_dict(&rec) { rec } else { pv::empty() };
    let providers: std::collections::BTreeSet<String> = a.ladder.iter().map(provider_of).collect();
    let selected_runner = a.ladder[next_runner_index(doc, &a.ladder)].clone();
    let selected_provider = provider_of(&selected_runner);
    let selected_model = g(&selected_runner, "model").clone();
    let mut reasons: Vec<String> = vec![];

    let observed_value = |item: &V, sample_key: &str| -> (Option<i64>, &'static str) {
        if let Some(v) = positive(g(item, sample_key)) {
            return (Some(v), if sample_key == "duration_ms" { "duration_ms" } else { "max_turns_used" });
        }
        if sample_key == "duration_ms" {
            if let (Some(st), Some(en)) = (utc_epoch(g(item, "started_utc")), utc_epoch(g(item, "ended_utc"))) {
                if en > st {
                    return (Some(((en - st) * 1000.0) as i64), "main_wall_ms");
                }
            }
        }
        (None, "")
    };
    let observed_model_matches = |item: &V| -> bool {
        let observed = g(item, "model_used");
        !truthy(observed) || (pv::is_list(observed) && items(observed).iter().all(|m| pv::is_str(m) && eq(m, &selected_model)))
    };
    let successful_checkpoint = |item: &V| -> bool {
        if !is(g(item, "control_status"), "completed")
            || !truthy(g(item, "sub_reported"))
            || is(g(item, "budget_outcome"), "exhausted")
            || truthy(g(item, "permission_denials"))
            || items(g(item, "reason_codes")).iter().any(|c| is_any(c, &["PERMISSION_DENIED", "TIMEOUT", "NONZERO_EXIT"]))
        {
            return false;
        }
        is(g(item, "status"), "completed") || (is(g(item, "status"), "input-required") && pv::is_false(g(item, "hitl_needed")))
    };
    let mut choose = |field: &str, explicit: Option<i64>, rec_key: &str, allocation_key: &str, sample_key: &str, exhausted: bool| -> Res<i64> {
        let cap = schema_cap(field)?;
        if let Some(e) = explicit {
            reasons.push(format!("{field}: caller explicit={e}"));
            return Ok(e);
        }
        let prior = positive(g(&last, allocation_key));
        let recommendation = positive(g(&rec, rec_key));
        if exhausted {
            if let Some(p) = prior {
                let raw = ((p as f64) * 1.5).ceil() as i64;
                let raw = raw.max(recommendation.unwrap_or(0));
                let chosen = raw.min(cap);
                reasons.push(format!(
                    "{field}: observed exhaustion allocation={p} × policy margin=1.5 recommendation={} selected={chosen}{}",
                    pv::fmt_opt(&recommendation.map(|x| x.to_string())),
                    if raw > cap { format!(" schema ceiling={cap}; checkpoint continues") } else { String::new() }
                ));
                return Ok(chosen);
            }
        }
        if let Some(r) = recommendation {
            if prior.is_none() || r >= prior.unwrap() || progress {
                let chosen = r.min(cap);
                reasons.push(format!(
                    "{field}: sub recommendation={r} progress={} selected={chosen}{}",
                    if progress { "True" } else { "False" },
                    if r > cap { format!(" schema ceiling={cap}") } else { String::new() }
                ));
                return Ok(chosen);
            }
        }
        let sample = atts.iter().rev().find(|x| {
            truthy(&phase)
                && eq(g(x, "phase"), &phase)
                && successful_checkpoint(x)
                && {
                    let p = if truthy(g(x, "provider")) { py_str(g(x, "provider")) } else { py_str(g(g(x, "runner"), "provider")) };
                    p == selected_provider
                }
                && {
                    let m = if truthy(g(x, "model_declared")) { g(x, "model_declared").clone() } else { g(g(x, "runner"), "model").clone() };
                    eq(&m, &selected_model)
                }
                && truthy(&selected_model)
                && observed_model_matches(x)
                && observed_value(x, sample_key).0.is_some()
        });
        if let Some(sample) = sample {
            let (observed, src) = observed_value(sample, sample_key);
            let observed = observed.unwrap();
            let raw = ((observed as f64) * 1.5 / if sample_key == "duration_ms" { 1000.0 } else { 1.0 }).ceil() as i64;
            let chosen = raw.max(1).min(cap);
            if prior.is_none() || chosen >= prior.unwrap() || progress {
                reasons.push(format!(
                    "{field}: same-context same-phase provider={selected_provider} declared_model={} attempt={} observed {src}={observed} × policy margin=1.5 selected={chosen}{}",
                    py_str(&selected_model),
                    py_str(g(sample, "attempt")),
                    if raw > cap { format!(" schema ceiling={cap}") } else { String::new() }
                ));
                return Ok(chosen);
            }
        }
        if let Some(p) = prior {
            reasons.push(format!("{field}: last declared per-call allocation={p}; no comparable observation"));
            return Ok(p);
        }
        Err(Exit::msg(format!("[relay] STOP: no comparable per-call {field} evidence; declare it explicitly")))
    };
    let timeout_signal = g(g(&last, "failure_evidence"), "signal").clone();
    let secs = choose(
        "timeout_seconds",
        a.timeout_seconds,
        "timeout_seconds",
        "timeout_seconds_allocated",
        "duration_ms",
        is_any(&timeout_signal, &["controlled_local_deadline", "controlled_remote_deadline"]),
    )?;
    let only_codex = providers.len() == 1 && providers.contains("codex_cli");
    let mut turns = None;
    if !only_codex {
        turns = Some(choose("max_turns", a.max_turns, "max_turns", "max_turns_allocated", "max_turns_used", is(g(&last, "budget_outcome"), "exhausted"))?);
    }
    let source = format!("supervisor per-invocation: {}", reasons.join("; "));
    if only_codex {
        return declare(pv::opt_i(a.max_turns), pv::i(secs), &source, "wall_time");
    }
    declare(pv::opt_i(turns), pv::i(secs), &source, "native_turns")
}

pub fn supervise_resume(a0: &Args, lp: &str, doc: &mut V, _secs: Option<i64>, _why: &str) -> Res<i32> {
    let mut a = a0.clone();
    let last = last_attempt(doc).cloned().filter(truthy).unwrap_or_else(pv::empty);
    let deferred = g(doc, "capacity_deferred").clone();
    let allow_capacity = eq(g(&deferred, "dispatch_id"), g(&last, "dispatch_id"))
        && is(g(g(&last, "failure_evidence"), "signal"), "provider_capacity");
    let reasons = continuation_stop_reasons(doc, allow_capacity, None);
    if !reasons.is_empty() {
        say(&format!("[relay] STOP: {}", reasons.join("; ")));
        return Ok(3);
    }
    let pend = pending_for(&a.repo_root, g(doc, "context_id"))?;
    if !resume_blockers(&pend).is_empty() {
        say("  → STOP: 답이 필요한 차단성 요청이 남아 있다 — 자동 재발급하지 않는다.");
        return Ok(3);
    }
    if a.topology.as_deref().map_or(true, str::is_empty) {
        a.topology = pv::st(g(doc, "topology"));
    }
    a.manifest_path = match a.manifest.clone().filter(|m| !m.is_empty()).or_else(|| pv::st(g(doc, "manifest")).filter(|m| !m.is_empty())) {
        Some(m) => m,
        None => join(&repo(), &format!("output/{}/manifest.yaml", a.topology.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "single".into()))),
    };
    a.context_id = pv::st(g(doc, "context_id"));
    a.campaign_id = pv::st(g(doc, "campaign_id"));
    a.control_variables = Some(g(doc, "control_variables").clone());
    a.node = pv::st(g(doc, "campaign_node"));
    a.context_kind = pv::st(g(doc, "campaign_context_kind"));
    a.cell = pv::st(g(doc, "campaign_cell"));
    if a.permission_profile.as_deref().map_or(true, str::is_empty) && a.sandbox_mode.as_deref().map_or(true, str::is_empty) {
        apply_permission_selection(&mut a, doc);
    }
    if a.reasoning_effort.is_none() {
        a.reasoning_effort = pv::st(g(doc, "reasoning_effort"));
    }
    restore_runner_efforts(&mut a, doc)?;
    let lks = last_known_session(doc);
    let deadline_resume = is_any(g(g(&last, "failure_evidence"), "signal"), &["controlled_local_deadline", "controlled_remote_deadline"]);
    let native_exhausted = is(g(&last, "end_reason"), "budget_exhausted");
    if native_exhausted && !native_budget_exhaustion_ready(&a.repo_root, doc) {
        say("[relay] STOP: native budget exhaustion lacks durable same-session evidence");
        return Ok(3);
    }
    if deadline_resume && !controlled_deadline_ready(&a.repo_root, doc) {
        say("[relay] STOP: controlled deadline lacks durable same-session/command evidence");
        return Ok(3);
    }
    if allow_capacity {
        let previous = items(g(doc, "capacity_bursts")).iter().rev().find(|b| eq(g(b, "burst_id"), g(&deferred, "burst_id"))).cloned();
        let prev = previous.clone().unwrap_or_else(pv::empty);
        let rec = codex_recovery_requirement(&last).unwrap_or(V::None);
        let answered = pend.iter().any(|e| {
            eq(g(e, "recovery_required"), &rec) && eq(g(e, "answer"), g(&prev, "authorization")) && eq(g(e, "answer_source"), g(&prev, "source"))
        });
        if !truthy(g(&last, "session_id")) || previous.is_none() || !answered {
            say("[relay] STOP: deferred capacity lacks its recorded burst authorization");
            return Ok(3);
        }
        a.capacity_resume_policy = previous;
        a.continue_ = true;
        a.apply = true;
        a.resume = pv::st(g(&last, "session_id"));
    }
    let bud = select_supervised_budget(&a, doc)?;
    a.continuation_source_attempt = pv::int(g(&continuation_instruction(&a.repo_root, doc)?, "attempt"));
    let mut task = assemble_continuation(&a.repo_root, doc, &pend)?;
    if deadline_resume || native_exhausted {
        task = format!("다른 행동 전에 기존 백그라운드 명령, 실행 중 서비스, sweep 상태와 산출물을 확인해 상태를 조정하고 부분 파일을 보존하라. 조정 전에는 벤치를 재시작하거나 기존 파일을 덮어쓰지 마라.\n{task}");
    }
    let resume = if truthy(g(&lks, "session_id")) { py_str(g(&lks, "session_id")) } else { "new".into() };
    run_attempt(&mut a, doc, lp, &task, &bud, &resume)
}

pub fn apply_permission_selection(a: &mut Args, doc: &V) {
    for (k, v) in pv::entries(g(doc, "permission_selection")) {
        match k.as_str() {
            "permission_profile" => a.permission_profile = pv::st(v),
            "sandbox_mode" => a.sandbox_mode = pv::st(v),
            _ => {}
        }
    }
}

// ── dispatch ───────────────────────────────────────────────────────────────────────────────────────────────────────────

pub const SSH_PROBE: [&str; 21] = [
    "ssh", "-F", "/dev/null", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=yes", "-o", "IdentitiesOnly=yes", "-o",
    "ForwardAgent=no", "-o", "ClearAllForwardings=yes", "-o", "RequestTTY=no", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2",
];

pub fn ssh_run(target: &V, cmd: &str, timeout_seconds: i64) -> (i32, String, String) {
    let mut argv: Vec<String> = SSH_PROBE.iter().map(|x| x.to_string()).collect();
    argv.push("--".into());
    argv.push(format!("{}@{}", py_str(g(target, "ssh_user")), py_str(g(target, "host"))));
    argv.push(cmd.into());
    use std::process::{Command, Stdio};
    let child = Command::new(&argv[0]).args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let Ok(mut child) = child else {
        return (127, String::new(), "ssh: not found".into());
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(_) => break,
        }
        if start.elapsed().as_secs_f64() >= timeout_seconds as f64 {
            let _ = child.kill();
            let _ = child.wait();
            return (124, String::new(), "branch preflight exceeded the declared wall deadline".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let o = child.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        strip(&String::from_utf8_lossy(&o.stdout)),
        strip(&String::from_utf8_lossy(&o.stderr)),
    )
}

pub type BranchRunner<'a> = &'a dyn Fn(&V, &str) -> (i32, String, String);

pub fn assert_sub_branch(req: &V, topology: &str, runner: Option<BranchRunner>) -> Res<String> {
    let t = g(req, "target");
    let cmd = format!("git -C {} rev-parse --abbrev-ref HEAD", shell_quote(&py_str(g(t, "work_dir"))));
    let (rc, out, err) = match runner {
        Some(r) => r(t, &cmd),
        None => {
            let seconds = pv::int(g(req, "timeout_seconds")).unwrap_or(0);
            let remote = format!("timeout --signal=TERM --kill-after=3s {seconds}s {cmd}");
            ssh_run(t, &remote, seconds + 3)
        }
    };
    if rc != 0 {
        let what = if !err.is_empty() { err } else if !out.is_empty() { out } else { "(출력 없음)".into() };
        return Err(Exit::msg(format!(
            "[relay] STOP: 서브 브랜치를 판독하지 못했다(rc={rc}) — {what}\n  → 판독 불가는 일치가 아니다(fail-closed). 서브 도달성·work_dir 를 먼저 확인하라."
        )));
    }
    if out != topology {
        return Err(Exit::msg(format!(
            "[relay] STOP(브랜치 불일치): 서브는 '{out}' 에 있고 요청 토폴로지는 '{topology}' 다.\n  → 브랜치별 CLAUDE.md 가 정체성을 정하므로 이 상태의 위임은 다른 정체성에게 말하는 것이다.\n  → `sync_to_sub.sh --branch {topology}` 가 서브를 그 브랜치에 안착시킨다(REST_BRANCH)."
        )));
    }
    Ok(out)
}

/// `shlex.quote(s)`.
pub fn shell_quote(t: &str) -> String {
    if t.is_empty() {
        return "''".into();
    }
    if t.chars().all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c)) {
        return t.into();
    }
    format!("'{}'", t.replace('\'', "'\"'\"'"))
}

pub fn utcnow() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    fmt_utc(now)
}
fn now_f() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn bound_resume(doc: &V, declared: &str, request: &V) -> Res<(String, V)> {
    if declared == "new" {
        return Ok(("new".into(), V::None));
    }
    let prior = attempts(doc).iter().rev().find(|x| is(g(x, "session_id"), declared)).cloned();
    let provider = py_str(g(request, "provider"));
    let Some(prior) = prior else {
        if provider == "claude_code" {
            return Ok((declared.into(), V::None));
        }
        return Err(Exit::msg("[relay] STOP: Codex resume has no recorded session binding"));
    };
    let previous_provider = provider_of(&if truthy(g(&prior, "runner")) { g(&prior, "runner").clone() } else { pv::empty() });
    if previous_provider != provider {
        return Ok(("new".into(), V::None));
    }
    let binding = g(&prior, "session_binding").clone();
    if pv::is_none(&binding) && provider == "claude_code" {
        return Ok((declared.into(), V::None));
    }
    if ne(&binding, &permission_binding(request)) {
        return Err(Exit::msg("[relay] STOP: resume target or permission binding changed"));
    }
    Ok((declared.into(), binding))
}

pub fn declared_budget(a: &Args, timeout_seconds: Option<i64>, source: &str) -> Res<V> {
    let providers: std::collections::BTreeSet<String> = a.ladder.iter().map(provider_of).collect();
    let unit = if providers.len() == 1 && providers.contains("codex_cli") { "wall_time" } else { "native_turns" };
    declare(pv::opt_i(a.max_turns), pv::opt_i(timeout_seconds), source, unit)
}

fn read_json_text(p: &Path) -> Result<V, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{e}"))?;
    pv::loads(&t)
}

pub fn load_dispatch_outcome(repo_root: &str, doc: &V, selected_path: Option<&str>) -> Res<(V, V)> {
    let reservation = g(doc, "inflight").clone();
    let relative = g(&reservation, "outcome_path");
    if !truthy(relative) {
        return Err(Exit::msg("[relay] STOP: this reservation has no durable provider outcome; it cannot be discarded or replayed"));
    }
    let path = owned_artifact(repo_root, &py_str(relative))?;
    if let Some(sp) = selected_path.filter(|s| !s.is_empty()) {
        let sel = if Path::new(sp).is_absolute() { Path::new(sp).to_path_buf() } else { Path::new(repo_root).join(sp) };
        if std::fs::canonicalize(&sel).ok() != std::fs::canonicalize(&path).ok() {
            return Err(Exit::msg("[relay] STOP: selected outcome is not the reserved dispatch artifact"));
        }
    }
    if !has(&reservation, "request_path") {
        return Err(tb("KeyError: 'request_path'"));
    }
    let request_path = owned_artifact(repo_root, &py_str(g(&reservation, "request_path")))?;
    let rb = std::fs::read(&request_path).map_err(|e| tb(&crate::pathfmt::os_error(&e, &request_path.display().to_string())))?;
    if !is(g(&reservation, "request_digest"), &pv::sha256_hex(&rb)) {
        return Err(Exit::msg("[relay] STOP: reserved request bytes changed; model/permissions/budget/resume cannot be rewritten"));
    }
    let (wrapper, request) = match (read_json_text(&path), read_json_text(&request_path)) {
        (Ok(w), Ok(r)) => (w, r),
        _ => return Err(Exit::msg("[relay] STOP: unreadable recovery evidence: JSONDecodeError")),
    };
    let mut wk = pv::keys(&wrapper);
    wk.sort();
    if !pv::is_dict(&wrapper)
        || wk != ["dispatch_id", "request", "result", "schema_version"]
        || !eq(g(&wrapper, "schema_version"), &pv::i(1))
        || ne(g(&wrapper, "dispatch_id"), g(&reservation, "dispatch_id"))
        || ne(g(&wrapper, "request"), &request)
    {
        return Err(Exit::msg("[relay] STOP: stale or mismatched dispatch outcome"));
    }
    let mut violations = crate::agent_control::request_schema_violations(&request);
    let result = g(&wrapper, "result").clone();
    let rschema = crate::agent_control::load_schema(&crate::agent_control::result_schema_path()).map_err(|m| tb(&m))?;
    violations.extend(crate::agent_control::schema_violations(&result, &rschema, "$"));
    if violations.is_empty() {
        violations.extend(crate::agent_control::result_contract_violations(&result, &request));
    }
    if !violations.is_empty() || ne(g(&request, "target"), g(&reservation, "target")) || ne(g(&request, "provider"), g(&reservation, "provider")) {
        return Err(Exit::msg("[relay] STOP: recovered request/result violates the reserved execution contract"));
    }
    Ok((request, result))
}

pub fn commit_dispatch_outcome(a: &Args, doc: &mut V, lp: &str, request: &V, result: &V, ended: &str, control_stderr: &str, reconciled: bool) -> Res<(V, V)> {
    let reservation = g(doc, "inflight").clone();
    let dispatch_id = g(&reservation, "dispatch_id").clone();
    let latest = load_ledger(lp)?;
    if ne(g(g(&latest, "inflight"), "dispatch_id"), &dispatch_id) {
        return Err(Exit::msg("[relay] STOP: stale dispatch result; owner generation changed"));
    }
    if !eq(g(&reservation, "attempt"), &pv::i(attempts(doc).len() as i64 + 1)) {
        return Err(Exit::msg("[relay] STOP: recovery attempt sequence changed"));
    }
    let mut report = if pv::contains_str(g(result, "reason_codes"), RUNNER_UNAVAILABLE) {
        V::None
    } else {
        parse_report(&if truthy(g(result, "output")) { g(result, "output").clone() } else { s("") })
    };
    if reconciled && !pv::is_none(&report) {
        let schema = crate::agent_control::load_schema(&task_report_schema()).map_err(|m| tb(&m))?;
        if !crate::agent_control::schema_violations(&report, &schema, "$").is_empty() {
            report = V::None;
        }
    }
    let meta = AttemptMeta {
        resume_declared: g(&reservation, "resume_declared").clone(),
        request_path: g(&reservation, "request_path").clone(),
        runner: g(&reservation, "runner").clone(),
        started_utc: g(&reservation, "started_utc").clone(),
        ended_utc: s(ended),
        session_binding: permission_binding(request),
        dispatch_id: dispatch_id.clone(),
    };
    let ctx = py_str(g(doc, "context_id"));
    let mut att = record_attempt(doc, &ctx, g(&reservation, "budget"), result, &report, &meta);
    let n = pv::int(g(&att, "attempt")).unwrap_or(0);
    let mut extra: Vec<(&str, V)> = vec![];
    if truthy(g(&reservation, "permission_source")) {
        extra.push(("permission_source", g(&reservation, "permission_source").clone()));
    }
    let directory = join(&relay_root(&a.repo_root)?, &format!("{ctx}.reports"));
    let _ = std::fs::create_dir_all(&directory);
    let path = join(&directory, &format!("attempt-{n:02}.json"));
    let mut control = pv::empty();
    for k in [
        "status", "exit_code", "reason_codes", "session_id", "num_turns", "budget_outcome", "duration_ms", "duration_api_ms", "raw_exit_code",
        "execution_started", "usage", "failure_evidence", "raw_events",
    ] {
        set(&mut control, k, g(result, k).clone());
    }
    let tail4k = tail(control_stderr, 4000);
    atomic_json(
        &path,
        &obj(vec![
            ("attempt", pv::i(n)),
            ("report", report.clone()),
            (
                "raw_output",
                if truthy(&report) { V::None } else { s(&pv::st(g(result, "output")).filter(|x| !x.is_empty()).unwrap_or_default()) },
            ),
            ("control_stderr_tail", if tail4k.is_empty() { V::None } else { s(&tail4k) }),
            ("control", control),
        ]),
    )?;
    extra.push(("report_path", s(&relpath(&path, &a.repo_root))));
    extra.push(("outcome_path", g(&reservation, "outcome_path").clone()));
    extra.push(("request_digest", g(&reservation, "request_digest").clone()));
    if has(&reservation, "task_instruction") {
        extra.push(("task_instruction", g(&reservation, "task_instruction").clone()));
    }
    if reconciled {
        extra.push((
            "reconciliation",
            obj(vec![
                ("utc", s(ended)),
                ("source", g(&reservation, "outcome_path").clone()),
                ("method", s("dispatch-bound-provider-outcome")),
                ("replayed", V::Bool(false)),
            ]),
        ));
    }
    for (k, v) in &extra {
        set(&mut att, k, v.clone());
    }
    // the attempt appended by record_attempt is the same object in Python: mirror the late fields into the ledger copy
    if let Some(V::List(l)) = pv::get_mut(doc, "attempts") {
        if let Some(lastv) = l.last_mut() {
            for (k, v) in &extra {
                set(lastv, k, v.clone());
            }
        }
    }
    pv::pop(doc, "inflight");
    save_ledger(lp, doc)?;
    Ok((att, report))
}

pub fn reconcile_inflight(a: &Args, doc: &mut V, lp: &str) -> Res<i32> {
    if !truthy(g(doc, "inflight")) {
        return Err(Exit::msg("[relay] STOP: no unresolved in-flight dispatch exists"));
    }
    let (request, result) = load_dispatch_outcome(&a.repo_root, doc, a.reconcile_result.as_deref())?;
    let reservation = g(doc, "inflight").clone();
    say(&format!(
        "[relay] reconciliation evidence: {} -> {}; no replacement dispatch",
        py_str(g(&reservation, "dispatch_id")),
        py_str(g(&result, "status"))
    ));
    if !a.apply {
        say("[relay] dry-run: pass --apply to append this observed outcome and consume its dispatch");
        return Ok(0);
    }
    let (att, report) = commit_dispatch_outcome(a, doc, lp, &request, &result, &utcnow(), "", true)?;
    let crash = dispatch_crash(&att, &report);
    let rep = if truthy(&report) { report.clone() } else { pv::empty() };
    surface_requests(&a.repo_root, &py_str(g(doc, "context_id")), &rep, pv::int(g(&att, "attempt")).unwrap_or(0), &crash)?;
    say(&format!(
        "[relay] reconciled attempt {}: {}; dispatch counted, native observations retained",
        py_str(g(&att, "attempt")),
        py_str(g(&att, "end_reason"))
    ));
    Ok(0)
}

pub fn run_attempt_once(a: &mut Args, doc: &mut V, lp: &str, task: &str, bud0: &V, resume_declared: &str, runner: &V) -> Res<i32> {
    let attempt_no = attempts(doc).len() as i64 + 1;
    if truthy(g(doc, "inflight")) {
        return Err(Exit::msg("[relay] STOP: unresolved dispatch remains; reconcile remote outcome before replay"));
    }
    let provider = provider_of(runner);
    let mut bud = bud0.clone();
    if provider == "codex_cli" {
        set(&mut bud, "max_turns", V::None);
        set(&mut bud, "budget_unit", s("wall_time"));
    } else {
        pv::pop(&mut bud, "budget_unit");
    }
    require_dispatch_recovery(a, doc, resume_declared)?;
    let mut profile = a.permission_profile.clone().filter(|x| !x.is_empty());
    let sandbox = a.sandbox_mode.clone().filter(|x| !x.is_empty());
    let mut permission_source = if profile.is_some() || sandbox.is_some() { s("caller") } else { V::None };
    if provider == "codex_cli" && profile.is_none() && sandbox.is_none() {
        profile = crate::agent_control::provisioned_permission_profile("sub");
        if profile.is_none() {
            return Err(Exit::msg(format!(
                "[relay] STOP: provisioned Codex default_permissions unreadable ({}) — 테라포밍 §2.5.2 설치를 확인하라",
                crate::agent_control::provisioned_codex_config("sub")
            )));
        }
        a.permission_profile = profile.clone();
        permission_source = s("provisioned");
    }
    let (effort, effort_source) = runner_effort(a, runner);
    let topology = a.topology.clone().unwrap_or_default();
    let mut req = build_request(
        &a.repo_root,
        &topology,
        &a.manifest_path,
        task,
        &bud,
        &BuildOpts {
            context_id: a.context_id.clone(),
            attempt: attempt_no,
            model: pv::st(g(runner, "model")),
            backend: pv::st(g(runner, "backend")),
            provider: provider.clone(),
            permission_profile: profile.clone(),
            sandbox_mode: sandbox.clone(),
            reasoning_effort: Some(effort.clone()),
            campaign_id: a.campaign_id.clone(),
            control_variables: a.cv(),
            delegated_node: if a.campaign_control.is_some() { None } else { a.node.clone() },
        },
    )?;
    set(&mut req, "reasoning_effort_source", s(&effort_source));
    let (effective_resume, binding) = bound_resume(doc, resume_declared, &req)?;
    let resume = if effective_resume == "new" { None } else { Some(effective_resume.clone()) };
    if effective_resume != resume_declared {
        let t = format!(
            "[하네스 인계] native session은 provider 경계에서 새로 시작한다. 이전 실행의 증거·통제변인·원장에 따라 이어가라.\n{}",
            py_str(g(&req, "task"))
        );
        set(&mut req, "task", s(&t));
        warn("[relay] provider changed: fresh native session with artifact handoff");
    }
    if let Some(r) = &resume {
        set(&mut req, "resume_session_id", s(r));
        if provider == "codex_cli" {
            set(&mut req, "resume_binding", binding.clone());
        }
    }
    let violations = crate::agent_control::request_schema_violations(&req);
    if !violations.is_empty() {
        return Err(Exit::msg(format!("[relay] STOP: request rejected before remote preflight: {}", violations.join("; "))));
    }
    let mut task_instruction = obj(vec![("schema_version", pv::i(1)), ("scope", instruction_scope(doc))]);
    {
        let sc = pv::get_mut(&mut task_instruction, "scope").unwrap();
        set(sc, "context_id", pv::opt_s(a.context_id.as_deref()));
        set(sc, "topology", pv::opt_s(a.topology.as_deref()));
    }
    match a.continuation_source_attempt {
        None => {
            set(&mut task_instruction, "kind", s("explicit"));
            set(&mut task_instruction, "source_attempt", pv::i(attempt_no));
            set(&mut task_instruction, "body", s(task));
            set(&mut task_instruction, "body_digest", s(&pv::sha256_hex(task.as_bytes())));
        }
        Some(src) => {
            let resolved = continuation_instruction(&a.repo_root, doc)?;
            if !eq(g(&resolved, "attempt"), &pv::i(src)) {
                return Err(instruction_refusal("continuation source changed before dispatch"));
            }
            set(&mut task_instruction, "kind", s("continuation"));
            set(&mut task_instruction, "source_attempt", pv::i(src));
        }
    }
    if a.emit_only {
        say(&pv::dumps_pretty(&req));
        return Ok(0);
    }
    let branch = assert_sub_branch(&req, &topology, None)?;
    let policy = a.prepared_capacity_policy.clone();
    if let Some(p) = &policy {
        if !capacity_deadline_fits(p, &bud) {
            return Err(Exit::msg("[relay] STOP: capacity deadline cannot fit the full attempt after preflight"));
        }
    }
    say(&format!(
        "[relay] attempt={attempt_no} runner={} (backend={} model={}) max_turns={} timeout={}s ({}) resume={resume_declared} sub_branch={branch}",
        py_str(g(runner, "name")),
        py_str(g(runner, "backend")),
        py_str(g(runner, "model")),
        py_str(g(&bud, "max_turns")),
        py_str(g(&bud, "timeout_seconds")),
        py_str(g(&bud, "source"))
    ));
    say(&format!(
        "[relay] reasoning_effort={} source={} (selection declaration; upstream observation is separate)",
        py_str(g(&req, "reasoning_effort")),
        py_str(g(&req, "reasoning_effort_source"))
    ));
    let ctx = a.context_id.clone().unwrap_or_default();
    let rroot = relay_root(&a.repo_root)?;
    let qdir = join(&rroot, &format!("{ctx}.requests"));
    let _ = std::fs::create_dir_all(&qdir);
    let qp = join(&qdir, &format!("attempt-{attempt_no:02}.json"));
    std::fs::write(&qp, pv::dumps_sorted_pretty(&req) + "\n").map_err(|e| tb(&crate::pathfmt::os_error(&e, &qp)))?;
    let started = utcnow();
    let dispatch_id = pv::uuid4_hex();
    let outcome_dir = join(&rroot, &format!("{ctx}.outcomes"));
    let _ = std::fs::create_dir_all(&outcome_dir);
    let outcome_path = join(&outcome_dir, &format!("{dispatch_id}.json"));
    set(doc, "context_id", s(&ctx));
    set(doc, "topology", pv::opt_s(a.topology.as_deref()));
    set(doc, "manifest", s(&a.manifest_path));
    if !truthy(g(doc, "task")) {
        set(doc, "task", s(task));
    }
    if let Some(p) = &policy {
        let bursts = pv::setdefault(doc, "capacity_bursts", pv::list(vec![]));
        if !items(bursts).iter().any(|b| eq(g(b, "burst_id"), g(p, "burst_id"))) {
            pv::append(bursts, p.clone());
        }
    }
    let qbytes = std::fs::read(&qp).unwrap_or_default();
    let mut inflight = obj(vec![
        ("dispatch_id", s(&dispatch_id)),
        ("attempt", pv::i(attempt_no)),
        ("target", g(&req, "target").clone()),
        ("provider", s(&provider)),
        ("request_path", s(&relpath(&qp, &a.repo_root))),
        ("request_digest", s(&pv::sha256_hex(&qbytes))),
        ("task_instruction", task_instruction),
        ("outcome_path", s(&relpath(&outcome_path, &a.repo_root))),
        ("budget", bud.clone()),
        ("runner", runner.clone()),
        ("resume_declared", s(&effective_resume)),
        ("started_utc", s(&started)),
        ("owner_pid", pv::i(std::process::id() as i64)),
    ]);
    match &a.reasoning_effort {
        Some(e) => set(doc, "reasoning_effort", s(e)),
        None => {
            pv::pop(doc, "reasoning_effort");
        }
    }
    set(doc, "runner_efforts", a.runner_efforts.clone().filter(truthy).unwrap_or_else(pv::empty));
    if provider == "codex_cli" {
        let mut sel = pv::empty();
        for k in ["permission_profile", "sandbox_mode"] {
            if has(&req, k) {
                set(&mut sel, k, g(&req, k).clone());
            }
        }
        set(doc, "permission_selection", sel);
        set(&mut inflight, "permission_source", permission_source);
    }
    set(doc, "inflight", inflight);
    save_ledger(lp, doc)?;
    if let Some(p) = &policy {
        if !capacity_deadline_fits(p, &bud) {
            pv::pop(doc, "inflight");
            save_ledger(lp, doc)?;
            return Err(Exit::msg("[relay] STOP: capacity deadline cannot fit the full attempt before launch"));
        }
    }
    let argv = vec![
        "bash".to_string(),
        agent_control_sh(),
        "invoke".into(),
        "--request".into(),
        qp.clone(),
        "--result-file".into(),
        outcome_path.clone(),
        "--dispatch-id".into(),
        dispatch_id.clone(),
    ];
    let out = run(&argv, None, None, false).map_err(|e| tb(&crate::pathfmt::os_error(&e, "bash")))?;
    let ended = utcnow();
    {
        use std::io::Write;
        let _ = std::io::stderr().write_all(out.stderr.as_bytes());
    }
    let control_stderr = strip(&out.stderr);
    let result = match pv::loads(&out.stdout) {
        Ok(r) => r,
        Err(_) => return Err(Exit::msg(format!("[relay] FAIL: agent_control 출력을 읽지 못했다 — {}", head(&out.stdout, 300)))),
    };
    let (_, durable) = load_dispatch_outcome(&a.repo_root, doc, None)?;
    if ne(&result, &durable) {
        return Err(Exit::msg("[relay] STOP: stdout result differs from durable dispatch evidence"));
    }
    let (att, report) = commit_dispatch_outcome(a, doc, lp, &req, &result, &ended, &control_stderr, false)?;
    let crash = dispatch_crash(&att, &report);
    let rep = if truthy(&report) { report.clone() } else { pv::empty() };
    let hp = surface_requests(&a.repo_root, &ctx, &rep, pv::int(g(&att, "attempt")).unwrap_or(0), &crash)?;
    say(&format!("[relay] 원장 → {lp}"));
    let stall = match (g(&att, "duration_ms").peel(), g(&att, "duration_api_ms").peel()) {
        (V::Int(_), V::Int(_)) => Some(pv::int(g(&att, "duration_ms")).unwrap_or(0) - pv::int(g(&att, "duration_api_ms")).unwrap_or(0)),
        _ => None,
    };
    say(&format!(
        "[relay] control={} sub_status={} turns={}/{} budget={} end_reason={}",
        py_str(g(&att, "control_status")),
        py_str(g(&att, "status")),
        py_str(g(&att, "max_turns_used")),
        py_str(g(&att, "max_turns_allocated")),
        py_str(g(&att, "budget_outcome")),
        py_str(g(&att, "end_reason"))
    ));
    say(&format!(
        "[relay] 시간: {started} → {ended} · provider wall={}ms api={}ms 정지={}ms",
        py_str(g(&att, "duration_ms")),
        py_str(g(&att, "duration_api_ms")),
        stall.map_or("(모름)".to_string(), |x| x.to_string())
    ));
    if truthy(g(&att, "permission_denials")) {
        say(&format!("[relay] ⚠ 권한 거부 사건 — 과업 판정은 리포트가 한다(end_reason={}):", py_str(g(&att, "end_reason"))));
        for ln in crate::py::splitlines(&py_str(g(&att, "permission_denials"))) {
            say(&format!("[relay]   {ln}"));
        }
    }
    if pv::is_false(g(&att, "resume_honored")) {
        say(&format!(
            "[relay] ⚠ 재개 불발: 요청한 세션 {} 과 다른 세션 {} 이 열렸다 — 서브가 컨텍스트를 처음부터 재구축했을 수 있다(소진의 주된 원인). 다음 턴의 예산을 그렇게 읽어라.",
            pv::fmt_opt(&resume),
            py_str(g(&att, "session_id"))
        ));
    }
    if reached_sub(&att) {
        let camp = match a.campaign_id.clone().filter(|c| !c.is_empty()) {
            Some(c) => s(&c),
            None => g(doc, "campaign_id").clone(),
        };
        for r in echo_stop_reasons(&att, &s(&ctx), &camp, g(doc, "control_variables")) {
            say(&format!("[relay] ⚠ 정체성 echo: {r} — `--continue` 는 이 상태에서 진행하지 않는다."));
        }
    }
    if let Some(control) = a.campaign_control.clone().filter(|_| reached_sub(&att)) {
        let camp = a.campaign_id.clone().filter(|c| !c.is_empty()).or_else(|| pv::st(g(doc, "campaign_id"))).unwrap_or_default();
        let (ok, why) = if a.orphan_sweep {
            orphan_sweep_echo_ok(&report, &a.sweep_ids)
        } else {
            campaign_control_echo_ok(&report, &camp, &control, a.control_slice.as_ref().unwrap_or(&V::None))
        };
        let node = a.node.clone().unwrap_or_default();
        if ok && a.orphan_sweep {
            say(&format!(
                "[relay] 수명 제어 · orphan sweep {camp}@{node} · {why} — 메인 인스턴스가 없어 journey 기록 ✗ (echo 원문은 원장 {lp} · docs 평면으로 옮겨라)"
            ));
        } else if ok {
            let echo = g(&rep, "campaign_control_echo").clone();
            let mut args: Vec<String> = vec!["--sub-echo".into(), camp.clone(), "--node".into(), node.clone(), "--control".into(), control.clone(), "--utc".into(), ended.clone(), "--apply".into()];
            if control == "reslice" {
                args.push("--rev".into());
                args.push(py_str(g(&echo, "rev")));
            }
            let refs: Vec<&str> = args.iter().map(|x| x.as_str()).collect();
            match campaign_init(&refs, &a.repo_root, None) {
                Ok(out) => {
                    for line in crate::py::splitlines(&out) {
                        let m = line.strip_prefix("[campaign_init] APPLIED ").unwrap_or(&line).to_string();
                        say(&format!("[relay] 수명 제어 · {m} · {why}"));
                    }
                }
                Err((_, msg)) => say(&format!("[relay] ⚠ 수명 제어 echo 기록 실패 — {msg} (서브 미수렴으로 남는다)")),
            }
        } else {
            say(&format!(
                "[relay] ⚠ 수명 제어 · 서브 미수렴 — {why}. 메인은 진행한다(차단 ✗ · D9) — `--resume-brief` 첫 줄에 '서브 미수렴' 으로 남는다"
            ));
        }
    }
    if truthy(g(&att, "external_search")) {
        say(&format!(
            "[relay] 서브가 외부검색 {}건을 기록했다 — 자산화 후보. 근거는 원장 attempt {}.external_search 에 있다.",
            items(g(&att, "external_search")).len(),
            py_str(g(&att, "attempt"))
        ));
    }
    if let Some(hp) = &hp {
        let who = if truthy(&crash) { "메인이 사실을" } else { "서브가 요청을" };
        say(&format!(
            "[relay] ⚠ {who} 표면화했다 → {hp} (`answer` 를 적고 `--continue` 로 재개. blocking 은 답 없이는 진행하지 않는다)"
        ));
    }
    if is(g(&att, "end_reason"), "runner_unavailable") {
        let ev = if truthy(g(&att, "runner_evidence")) { g(&att, "runner_evidence").clone() } else { pv::empty() };
        say(&format!(
            "[relay] ⚠ 러너 평면 실패 — {}: {} (signal={} status={} provenance={} 회전가능={})",
            py_str(g(runner, "name")),
            py_str(g(&ev, "detail")),
            py_str(g(&ev, "signal")),
            py_str(g(&ev, "api_error_status")),
            py_str(g(&ev, "provenance")),
            py_str(g(&ev, "rotate"))
        ));
        if truthy(g(&ev, "message")) {
            say(&format!("[relay]   백엔드가 남긴 말: {}", head(&py_str(g(&ev, "message")), 300)));
        }
        return Ok(5);
    }
    if is(g(&att, "budget_outcome"), "exhausted") {
        say(&format!(
            "[relay] 호출 예산 소진. 다음 실행 예산을 선언하고 checkpoint부터 이어라; 관측과 남은 과업에 따라 필요하면 조정한다. context={ctx}"
        ));
        return Ok(3);
    }
    if pv::is_none(&report) {
        let raw = g(&result, "output");
        let what = if pv::is_none(raw) || py_str(raw).trim().is_empty() {
            "출력이 비었다(서브가 아무것도 돌려주지 않았거나 전송이 실패했다)".to_string()
        } else {
            format!("출력 {}자가 왔지만 그 안에서 JSON 객체를 찾지 못했다", py_str(raw).chars().count())
        };
        let codes: Vec<String> = items(g(&att, "reason_codes")).iter().map(py_str).collect();
        let codes = if codes.is_empty() { "-".to_string() } else { codes.join(",") };
        say(&format!(
            "[relay] ⚠ 서브 리포트(JSON) 없음 — {what}. control={} codes={codes}. 성공으로 집계하지 않는다.",
            py_str(g(&att, "control_status"))
        ));
        if !control_stderr.is_empty() {
            say(&format!("[relay]   provider 진단(끝 400자): {}", tail(&control_stderr, 400)));
        }
        say(&format!("[relay]   원문 보존: {}", py_str(g(&att, "report_path"))));
        return Ok(4);
    }
    Ok(if is(g(&att, "status"), "completed") { 0 } else { 2 })
}

// ── capacity bursts ───────────────────────────────────────────────────────────────────────────────────────────────────

pub fn capacity_policy(a: &Args, doc: &V, bud: &V) -> Res<Option<V>> {
    let any_given = a.capacity_retries.is_some() || a.capacity_backoff_seconds.is_some() || a.capacity_deadline_utc.is_some() || a.capacity_approval_file.is_some();
    let inherited = a.capacity_resume_policy.clone().filter(truthy);
    if !any_given && inherited.is_none() {
        return Ok(None);
    }
    let timeout = pv::int(g(bud, "timeout_seconds")).unwrap_or(0);
    let (retries, delay, deadline, not_after, approval, expiry);
    if let (Some(inh), false) = (&inherited, any_given) {
        let r = g(inh, "retries");
        let d = g(inh, "backoff_seconds");
        let cap = schema_cap("timeout_seconds")?;
        let ok = pv::int(r).is_some_and(|x| (1..=10).contains(&x)) && pv::int(d).is_some_and(|x| 1 <= x && x <= cap);
        if !ok {
            return Err(Exit::msg("[relay] STOP: invalid inherited capacity burst bounds"));
        }
        retries = pv::int(r).unwrap();
        delay = pv::int(d).unwrap();
        approval = obj(vec![("authorization", g(inh, "authorization").clone()), ("source", g(inh, "source").clone())]);
        let mut exp = now_f() + ((retries + 1) * timeout + retries * delay) as f64;
        let na = if has(inh, "not_after_utc") { g(inh, "not_after_utc").clone() } else { g(inh, "deadline_utc").clone() };
        let independent = pv::st(&na).and_then(|t| parse_utc(&t)).ok_or_else(|| Exit::msg("[relay] STOP: invalid recorded capacity not-after deadline"))?;
        exp = exp.min(independent as f64);
        expiry = exp;
        deadline = s(&fmt_utc(exp.floor() as i64));
        not_after = na;
    } else {
        let (Some(r), Some(d), Some(dl), Some(file)) = (a.capacity_retries, a.capacity_backoff_seconds, a.capacity_deadline_utc.clone(), a.capacity_approval_file.clone()) else {
            return Err(Exit::msg("[relay] STOP: capacity policy requires all four options"));
        };
        retries = r;
        delay = d;
        not_after = s(&dl);
        if !(1..=10).contains(&r) || !(1 <= d && d <= schema_cap("timeout_seconds")?) {
            return Err(Exit::msg("[relay] STOP: invalid capacity retry bounds"));
        }
        let e = parse_utc(&dl).ok_or_else(|| Exit::msg("[relay] STOP: invalid capacity deadline or approval file"))?;
        let ap = std::fs::read_to_string(&file).ok().and_then(|t| pv::loads(&t).ok()).ok_or_else(|| Exit::msg("[relay] STOP: invalid capacity deadline or approval file"))?;
        expiry = e as f64;
        deadline = s(&dl);
        approval = ap;
    }
    let mut ak = pv::keys(&approval);
    ak.sort();
    if !pv::is_dict(&approval)
        || ak != ["authorization", "source"]
        || pv::entries(&approval).iter().any(|(_, v)| pv::st(v).map_or(true, |t| t.trim().is_empty()))
    {
        return Err(Exit::msg("[relay] STOP: capacity approval needs verbatim authorization and source"));
    }
    if truthy(g(doc, "inflight")) {
        return Err(Exit::msg("[relay] STOP: cannot start capacity burst with inflight dispatch"));
    }
    if expiry - now_f() < timeout as f64 {
        return Err(Exit::msg("[relay] STOP: insufficient capacity burst deadline"));
    }
    Ok(Some(obj(vec![
        ("burst_id", s(&pv::uuid4_hex())),
        ("retries", pv::i(retries)),
        ("backoff_seconds", pv::i(delay)),
        ("deadline_utc", deadline),
        ("not_after_utc", not_after),
        ("authorization", g(&approval, "authorization").clone()),
        ("source", g(&approval, "source").clone()),
        ("start_attempt", pv::i(attempts(doc).len() as i64)),
        ("allocated_slots", pv::i(1 + retries)),
    ])))
}

pub fn capacity_deadline_fits(policy: &V, bud: &V) -> bool {
    let dl = pv::st(g(policy, "deadline_utc")).and_then(|t| parse_utc(&t)).unwrap_or(0) as f64;
    dl - now_f() >= pv::num(g(bud, "timeout_seconds")).unwrap_or(0.0)
}

fn control_of(result: &V) -> V {
    let mut c = pv::empty();
    for k in [
        "status", "exit_code", "reason_codes", "session_id", "num_turns", "budget_outcome", "duration_ms", "duration_api_ms", "raw_exit_code",
        "execution_started", "usage", "failure_evidence", "raw_events",
    ] {
        set(&mut c, k, g(result, k).clone());
    }
    c
}

fn reservation_of(last: &V, req: &V) -> Option<V> {
    for k in ["outcome_path", "request_path", "request_digest", "dispatch_id"] {
        if !has(last, k) {
            return None;
        }
    }
    Some(obj(vec![(
        "inflight",
        obj(vec![
            ("outcome_path", g(last, "outcome_path").clone()),
            ("request_path", g(last, "request_path").clone()),
            ("request_digest", g(last, "request_digest").clone()),
            ("dispatch_id", g(last, "dispatch_id").clone()),
            ("target", g(req, "target").clone()),
            ("provider", g(req, "provider").clone()),
        ]),
    )]))
}

pub fn capacity_handoff_reentry(a: &Args, doc: &V, runner: &V, resume_declared: &str) -> bool {
    let atts = attempts(doc);
    if truthy(g(doc, "inflight")) || !a.continue_ || !a.apply || a.task_file.as_deref().map_or(true, str::is_empty) || atts.is_empty() {
        return false;
    }
    let last = atts.last().unwrap();
    let session = g(last, "session_id");
    let pending_block = match pending_for(&a.repo_root, g(doc, "context_id")) {
        Ok(p) => !resume_blockers(&p).is_empty(),
        Err(_) => return false,
    };
    if !is(g(last, "control_status"), "completed")
        || !is(g(last, "status"), "input-required")
        || !is(g(last, "end_reason"), "sub_input_required")
        || !truthy(g(last, "sub_reported"))
        || codex_recovery_requirement(last).is_some()
        || !truthy(session)
        || !is(session, resume_declared)
        || !a.resume.as_deref().is_some_and(|r| is(session, r))
        || pv::is_false(g(last, "resume_honored"))
        || ne(g(last, "runner"), runner)
        || !continuation_stop_reasons(doc, false, a.historical.as_ref()).is_empty()
        || pending_block
    {
        return false;
    }
    (|| -> Option<bool> {
        let mut hist = doc.clone();
        if let Some(h) = &a.historical {
            set(&mut hist, "control_variables", h.clone());
        }
        let req = instruction_request(&a.repo_root, &hist, last).ok()?;
        let binding = if truthy(g(last, "session_binding")) { g(last, "session_binding").clone() } else { pv::empty() };
        let (issued, result) = load_dispatch_outcome(&a.repo_root, &reservation_of(last, &req)?, None).ok()?;
        let camp = if truthy(g(doc, "campaign_id")) { py_str(g(doc, "campaign_id")) } else { "_bootstrap".into() };
        let expected = format!("campaigns/{camp}/relay/{}.reports/attempt-{:02}.json", py_str(g(doc, "context_id")), pv::int(g(last, "attempt"))?);
        if !is(g(last, "report_path"), &expected) || ne(&issued, &req) {
            return Some(false);
        }
        let wrapped = pv::loads(&std::fs::read_to_string(owned_artifact(&a.repo_root, &expected).ok()?).ok()?).ok()?;
        if !pv::is_dict(&wrapped) {
            return Some(false);
        }
        let report = g(&wrapped, "report").clone();
        let rs = crate::agent_control::load_schema(&task_report_schema()).ok()?;
        let out = if truthy(g(&result, "output")) { g(&result, "output").clone() } else { s("") };
        if ne(g(&wrapped, "attempt"), g(last, "attempt"))
            || !pv::is_dict(&report)
            || !crate::agent_control::schema_violations(&report, &rs, "$").is_empty()
            || ne(&parse_report(&out), &report)
            || !is(g(&result, "status"), "completed")
            || ne(g(&result, "session_id"), session)
            || ne(g(&result, "raw_exit_code"), g(last, "raw_exit_code"))
            || ne(g(&result, "reason_codes"), g(last, "reason_codes"))
            || ne(g(&wrapped, "control"), &control_of(&result))
            || [
                ("status", "status"),
                ("phase", "phase"),
                ("context_id_reported", "context_id"),
                ("campaign_id_reported", "campaign_id"),
                ("control_variables_echo", "control_variables_echo"),
                ("running_services", "running_services"),
            ]
            .iter()
            .any(|(lk, rk)| ne(g(last, lk), g(&report, rk)))
        {
            return Some(false);
        }
        Some(
            is(g(&req, "provider"), "codex_cli")
                && eq(g(runner, "provider"), &s("codex_cli"))
                && eq(g(&req, "model"), g(runner, "model"))
                && effort_matches(a, runner, &req)
                && eq(g(&binding, "permission_profile"), &pv::opt_s(a.permission_profile.as_deref()))
                && eq(g(&binding, "sandbox_mode"), &pv::opt_s(a.sandbox_mode.as_deref())),
        )
    })()
    .unwrap_or(false)
}

pub fn native_budget_exhaustion_ready(repo_root: &str, doc: &V) -> bool {
    let last = last_attempt(doc).cloned().filter(truthy).unwrap_or_else(pv::empty);
    if !is(g(&last, "provider"), "claude_code")
        || !is(g(&last, "end_reason"), "budget_exhausted")
        || !truthy(g(&last, "session_id"))
        || pv::is_false(g(&last, "resume_honored"))
        || truthy(g(&last, "permission_denials"))
        || !continuation_stop_reasons(doc, false, None).is_empty()
    {
        return false;
    }
    (|| -> Option<bool> {
        let req = instruction_request(repo_root, doc, &last).ok()?;
        let (issued, result) = load_dispatch_outcome(repo_root, &reservation_of(&last, &req)?, None).ok()?;
        Some(
            eq(&issued, &req)
                && is(g(&req, "provider"), "claude_code")
                && eq(&permission_binding(&req), g(&last, "session_binding"))
                && eq(g(&result, "session_id"), g(&last, "session_id"))
                && is(g(&result, "budget_outcome"), "exhausted")
                && !items(g(&result, "reason_codes")).iter().any(|c| is_any(c, &["PERMISSION_DENIED", "TIMEOUT"])),
        )
    })()
    .unwrap_or(false)
}

/// The command-execution events of a result: `None` = malformed (the original returns False), else whether no command is
/// left open. A completed item still `in_progress`/`running` is malformed in both callers.
fn command_events_ok(result: &V) -> Option<bool> {
    let mut open: Vec<String> = vec![];
    for event in items(g(result, "raw_events")) {
        let item = if pv::is_dict(event) { g(event, "item") } else { &pv::NONE };
        if !pv::is_dict(item) || !is(g(item, "type"), "command_execution") {
            continue;
        }
        let Some(ident) = pv::st(g(item, "id")).filter(|x| !x.is_empty()) else { return None };
        if is(g(event, "type"), "item.completed") {
            if is_any(g(item, "status"), &["in_progress", "running"]) {
                return None;
            }
            open.retain(|x| *x != ident);
        } else if is_any(g(event, "type"), &["item.started", "item.updated"]) {
            if !open.contains(&ident) {
                open.push(ident);
            }
        } else {
            return None;
        }
    }
    Some(open.is_empty())
}

pub fn controlled_deadline_ready(repo_root: &str, doc: &V) -> bool {
    let last = last_attempt(doc).cloned().filter(truthy).unwrap_or_else(pv::empty);
    let binding = if truthy(g(&last, "session_binding")) { g(&last, "session_binding").clone() } else { pv::empty() };
    if !truthy(g(&last, "session_id"))
        || pv::is_false(g(&last, "resume_honored"))
        || !is_any(g(g(&last, "failure_evidence"), "signal"), &["controlled_local_deadline", "controlled_remote_deadline"])
    {
        return false;
    }
    (|| -> Option<bool> {
        let req = instruction_request(repo_root, doc, &last).ok()?;
        let (issued, result) = load_dispatch_outcome(repo_root, &reservation_of(&last, &req)?, None).ok()?;
        let signal = g(g(&result, "failure_evidence"), "signal").clone();
        let transport = g(g(&req, "target"), "transport");
        if ne(&issued, &req)
            || ne(&permission_binding(&req), &binding)
            || !is(g(&result, "status"), "timeout")
            || ne(g(&result, "session_id"), g(&last, "session_id"))
            || ne(&signal, g(g(&last, "failure_evidence"), "signal"))
            || (is(&signal, "controlled_local_deadline") && !is(transport, "local"))
            || (is(&signal, "controlled_remote_deadline") && !is(transport, "ssh"))
        {
            return Some(false);
        }
        if is(&signal, "controlled_remote_deadline") {
            let events = items(g(&result, "raw_events"));
            let want = obj(vec![("type", s("adapter.deadline_reaped")), ("request_digest", s(&crate::provider_codex_cli::request_digest(&req)))]);
            if events.is_empty() || ne(events.last().unwrap(), &want) {
                return Some(false);
            }
        }
        if !is_any(&signal, &["controlled_local_deadline", "controlled_remote_deadline"]) {
            return Some(false);
        }
        // An unfinished tool is evidence to reconcile, not evidence of completion (malformed events still refuse).
        command_events_ok(&result)?;
        Some(true)
    })()
    .unwrap_or(false)
}

pub fn capacity_retry_ready(a: &Args, doc: &V, bud: &V, runner: &V, policy: Option<&V>, check_deadline: bool) -> bool {
    let Some(policy) = policy else { return false };
    if truthy(g(doc, "inflight")) {
        return false;
    }
    let atts = attempts(doc);
    if atts.len() as i64 - pv::int(g(policy, "start_attempt")).unwrap_or(0) >= pv::int(g(policy, "allocated_slots")).unwrap_or(0) {
        return false;
    }
    let last = atts.last().cloned().unwrap_or_else(pv::empty);
    if !is(g(g(&last, "failure_evidence"), "signal"), "provider_capacity")
        || !truthy(g(&last, "session_id"))
        || pv::is_false(g(&last, "resume_honored"))
        || ne(g(&last, "runner"), runner)
        || !is(g(runner, "provider"), "codex_cli")
    {
        return false;
    }
    if !continuation_stop_reasons(doc, true, None).is_empty() {
        return false;
    }
    (|| -> Option<bool> {
        let path = owned_artifact(&a.repo_root, &pv::st(g(&last, "outcome_path"))?).ok()?;
        let wrapper = read_json_text(&path).ok()?;
        if !has(&wrapper, "request") || !has(&wrapper, "result") {
            return None;
        }
        let (req, result) = (g(&wrapper, "request").clone(), g(&wrapper, "result").clone());
        let rp = owned_artifact(&a.repo_root, &pv::st(g(&last, "request_path"))?).ok()?;
        let rb = std::fs::read(&rp).ok()?;
        if !is(g(&last, "request_digest"), &pv::sha256_hex(&rb))
            || ne(&pv::loads(&String::from_utf8(rb).ok()?).ok()?, &req)
            || ne(g(&result, "session_id"), g(&last, "session_id"))
            || !is(g(g(&result, "failure_evidence"), "signal"), "provider_capacity")
        {
            return Some(false);
        }
        let mut wk = pv::keys(&wrapper);
        wk.sort();
        let rs = crate::agent_control::load_schema(&crate::agent_control::result_schema_path()).ok()?;
        if !eq(g(&wrapper, "schema_version"), &pv::i(1))
            || wk != ["dispatch_id", "request", "result", "schema_version"]
            || !crate::agent_control::request_schema_violations(&req).is_empty()
            || !crate::agent_control::schema_violations(&result, &rs, "$").is_empty()
            || !crate::agent_control::result_contract_violations(&result, &req).is_empty()
            || ne(g(&result, "provider"), g(&req, "provider"))
            || ne(g(&result, "model_requested"), g(&req, "model"))
            || !is(g(&result, "status"), "execution_failed")
            || ne(g(&result, "status"), g(&last, "control_status"))
            || ne(g(&result, "raw_exit_code"), g(&last, "raw_exit_code"))
            || ne(g(&result, "reason_codes"), g(&last, "reason_codes"))
            || ne(g(&wrapper, "dispatch_id"), g(&last, "dispatch_id"))
            || ne(&permission_binding(&req), g(&last, "session_binding"))
            || ne(g(&req, "model"), g(runner, "model"))
            || !effort_matches(a, runner, &req)
        {
            return Some(false);
        }
        if !command_events_ok(&result)? {
            return Some(false);
        }
        let deadline = parse_utc(&pv::st(g(policy, "deadline_utc"))?)? as f64;
        if check_deadline
            && deadline - now_f() < (pv::num(g(policy, "backoff_seconds")).unwrap_or(0.0) + pv::num(g(bud, "timeout_seconds")).unwrap_or(0.0))
        {
            return Some(false);
        }
        let pending = pending_for(&a.repo_root, g(doc, "context_id")).ok()?;
        let recovery = codex_recovery_requirement(&last)?;
        if pending.iter().any(|e| entry_blocking(e) && ne(g(e, "recovery_required"), &recovery) && !truthy(g(e, "answer"))) {
            return Some(false);
        }
        Some(true)
    })()
    .unwrap_or(false)
}

pub fn capacity_answer(repo_root: &str, context_id: &str, last: &V, policy: &V) -> Res<()> {
    let path = join(&relay_root(repo_root)?, PENDING_HITL);
    let _l = state_lock(&path, true)?;
    let mut doc = load_json_or_raise(&path)?;
    let rec = codex_recovery_requirement(last).unwrap_or(V::None);
    let idx: Vec<usize> = items(g(&doc, "pending"))
        .iter()
        .enumerate()
        .filter(|(_, e)| is(g(e, "context_id"), context_id) && eq(g(e, "recovery_required"), &rec))
        .map(|(i, _)| i)
        .collect();
    if idx.len() != 1 || truthy(g(&items(g(&doc, "pending"))[idx[0]], "answer")) {
        return Err(Exit::msg("[relay] STOP: capacity recovery pending does not match"));
    }
    if let Some(V::List(l)) = pv::get_mut(&mut doc, "pending") {
        set(&mut l[idx[0]], "answer", g(policy, "authorization").clone());
        set(&mut l[idx[0]], "answer_source", g(policy, "source").clone());
    }
    atomic_json(&path, &doc)
}

fn reconcile_first(a: &Args, doc: &V) -> Res<String> {
    Ok(format!(
        "먼저 기존 실행 중 서비스와 명령 및 산출물을 확인하고 부분 파일을 보존하라. 이미 실행된 벤치를 다시 시작하거나 부분 파일을 덮어쓰지 마라.\n{}",
        assemble_continuation(&a.repo_root, doc, &pending_for(&a.repo_root, &pv::opt_s(a.context_id.as_deref()))?)?
    ))
}

pub fn run_attempt(a: &mut Args, doc: &mut V, lp: &str, task0: &str, bud: &V, resume_declared: &str) -> Res<i32> {
    if let Some(r) = super::selftest::hook_run_attempt(task0) {
        return r;
    }
    let ladder = a.ladder.clone();
    for slot in &ladder {
        let effort = runner_effort(a, slot).0;
        if let Some(v) = crate::agent_control::provider_reasoning_violations(&provider_of(slot), &obj(vec![("reasoning_effort", s(&effort))]))? {
            if !v.is_empty() {
                return Err(Exit::msg(format!("[relay] STOP: invalid effort for {}: {}", py_str(g(slot, "name")), v.join("; "))));
            }
        }
    }
    let requested_policy = a.capacity_resume_policy.as_ref().is_some_and(truthy)
        || a.capacity_retries.is_some()
        || a.capacity_backoff_seconds.is_some()
        || a.capacity_deadline_utc.is_some()
        || a.capacity_approval_file.is_some();
    if requested_policy && (ladder.len() != 1 || provider_of(&ladder[0]) != "codex_cli") {
        return Err(Exit::msg("[relay] STOP: capacity retry requires one Codex runner"));
    }
    let policy = capacity_policy(a, doc, bud)?;
    let prior = last_attempt(doc).cloned();
    if policy.is_some()
        && prior.as_ref().is_some_and(|p| truthy(p) && is(g(p, "status"), "input-required"))
        && a.task_file.as_deref().is_some_and(|t| !t.is_empty())
        && !capacity_handoff_reentry(a, doc, &ladder[0], resume_declared)
    {
        return Err(Exit::msg("[relay] STOP: explicit handoff lacks durable same-session evidence"));
    }
    a.prepared_capacity_policy = policy.clone();
    let start = next_runner_index(doc, &ladder);
    if ladder.len() > 1 {
        say(&format!("[relay] 사다리 {}칸: {} · 시작 칸 {}", ladder.len(), ladder_names(&ladder), py_str(g(&ladder[start], "name"))));
    }
    let mut task = task0.to_string();
    let mut tried: Vec<String> = vec![];
    let mut rc = 0;
    for step in 0..ladder.len() {
        let runner = ladder[(start + step) % ladder.len()].clone();
        let last = last_attempt(doc).cloned();
        if policy.is_some()
            && !a.emit_only
            && last.as_ref().is_some_and(|l| truthy(l) && is(g(g(l, "failure_evidence"), "signal"), "provider_capacity"))
        {
            let last = last.clone().unwrap();
            if !capacity_retry_ready(a, doc, bud, &runner, policy.as_ref(), true) {
                return Err(Exit::msg("[relay] STOP: capacity continuation lacks session, evidence, or remaining wall time"));
            }
            if !is(g(&last, "session_id"), resume_declared) {
                return Err(Exit::msg("[relay] STOP: capacity continuation must resume the same session"));
            }
            sleep_s(g(policy.as_ref().unwrap(), "backoff_seconds"));
            let mut zero = policy.clone().unwrap();
            set(&mut zero, "backoff_seconds", pv::i(0));
            if !capacity_retry_ready(a, doc, bud, &runner, Some(&zero), true) {
                return Err(Exit::msg("[relay] STOP: capacity continuation deadline or evidence changed"));
            }
            let pending = pending_for(&a.repo_root, &pv::opt_s(a.context_id.as_deref()))?;
            let rec = codex_recovery_requirement(&last).unwrap_or(V::None);
            if pending.iter().any(|e| eq(g(e, "recovery_required"), &rec) && !truthy(g(e, "answer"))) {
                capacity_answer(&a.repo_root, &a.context_id.clone().unwrap_or_default(), &last, policy.as_ref().unwrap())?;
            }
            a.continue_ = true;
            a.apply = true;
            a.resume = Some(resume_declared.to_string());
            require_dispatch_recovery(a, doc, resume_declared)?;
            a.continuation_source_attempt = pv::int(g(&continuation_instruction(&a.repo_root, doc)?, "attempt"));
            task = reconcile_first(a, doc)?;
        }
        rc = run_attempt_once(a, doc, lp, &task, bud, resume_declared, &runner)?;
        if a.emit_only {
            return Ok(rc);
        }
        a.historical = None;
        let mut last = last_attempt(doc).cloned().unwrap_or_else(pv::empty);
        while capacity_retry_ready(a, doc, bud, &runner, policy.as_ref(), true) {
            sleep_s(g(policy.as_ref().unwrap(), "backoff_seconds"));
            let mut zero = policy.clone().unwrap();
            set(&mut zero, "backoff_seconds", pv::i(0));
            if !capacity_retry_ready(a, doc, bud, &runner, Some(&zero), true) {
                break;
            }
            capacity_answer(&a.repo_root, &a.context_id.clone().unwrap_or_default(), &last, policy.as_ref().unwrap())?;
            a.continue_ = true;
            a.apply = true;
            a.resume = pv::st(g(&last, "session_id"));
            let r = a.resume.clone().unwrap_or_default();
            require_dispatch_recovery(a, doc, &r)?;
            a.continuation_source_attempt = pv::int(g(&continuation_instruction(&a.repo_root, doc)?, "attempt"));
            task = reconcile_first(a, doc)?;
            rc = run_attempt_once(a, doc, lp, &task, bud, &r, &runner)?;
            last = last_attempt(doc).cloned().unwrap_or_else(pv::empty);
        }
        if let Some(p) = &policy {
            if is(g(g(&last, "failure_evidence"), "signal"), "provider_capacity") {
                let mut probe = p.clone();
                set(&mut probe, "allocated_slots", pv::i(attempts(doc).len() as i64 - pv::int(g(p, "start_attempt")).unwrap_or(0) + 1));
                if capacity_retry_ready(a, doc, bud, &runner, Some(&probe), false) {
                    let pending = pending_for(&a.repo_root, &pv::opt_s(a.context_id.as_deref()))?;
                    let rec = codex_recovery_requirement(&last).unwrap_or(V::None);
                    if pending.iter().any(|e| eq(g(e, "recovery_required"), &rec) && !truthy(g(e, "answer"))) {
                        capacity_answer(&a.repo_root, &a.context_id.clone().unwrap_or_default(), &last, p)?;
                    }
                    set(doc, "capacity_deferred", obj(vec![("dispatch_id", g(&last, "dispatch_id").clone()), ("burst_id", g(p, "burst_id").clone())]));
                    save_ledger(lp, doc)?;
                    say("[relay] capacity burst deferred; a later bounded burst may continue the same session");
                    return Ok(2);
                }
            }
        }
        if !is(g(&last, "end_reason"), "runner_unavailable") {
            return Ok(rc);
        }
        let ev = if truthy(g(&last, "runner_evidence")) { g(&last, "runner_evidence").clone() } else { pv::empty() };
        let detail = if truthy(g(&ev, "detail")) { py_str(g(&ev, "detail")) } else { py_str(g(&ev, "signal")) };
        tried.push(format!("{}({detail})", py_str(g(&runner, "name"))));
        if !pv::is_true(g(&ev, "rotate")) || pv::is_true(g(&last, "execution_started")) {
            say(&format!("[relay] ⚠ 회전하지 않는다 — {} 의 실패는 러너를 바꿔도 같다. 사람이 봐야 한다.", py_str(g(&runner, "name"))));
            return Ok(rc);
        }
        if step + 1 < ladder.len() {
            say(&format!(
                "[relay] ↻ 회전 {}/{}: {} → {} (같은 과업·같은 예산·같은 재개 선언 — 회전은 예산 사건이 아니다)",
                step + 1,
                ladder.len() - 1,
                py_str(g(&runner, "name")),
                py_str(g(&ladder[(start + step + 1) % ladder.len()], "name"))
            ));
        }
    }
    let _ = rc;
    let n = attempts(doc).len() as i64;
    let lks = py_str(g(&last_known_session(doc), "session_id"));
    let lks_v = g(&last_known_session(doc), "session_id").clone();
    let names: Vec<String> = ladder.iter().map(|r| py_str(g(r, "name"))).collect();
    let crash = obj(vec![
        ("attempt", pv::i(n)),
        ("end_reason", s("runner_ladder_exhausted")),
        ("control_status", s("execution_failed")),
        ("reason_codes", pv::strs(&[RUNNER_UNAVAILABLE.to_string()])),
        ("blocking", V::Bool(true)),
        ("runner_ladder", pv::strs(&names)),
        ("runner_attempts", pv::strs(&tried)),
        (
            "prompt",
            s(&format!(
                "러너 사다리가 소진됐다 — 선언된 {}칸({})이 **전부 실행되지 않았다**. 서브가 기동되지 않는 것이고, 예산도 과업도 아니다.\n칸별 관측: {}\n마지막 알려진 세션 = {}.\n→ 한도 리셋을 기다릴지, 사다리에 칸을 더할지, 다른 노드로 옮길지는 사람이 정한다. 답을 `answer` 에 적으면 잇는다.",
                ladder.len(),
                ladder_names(&ladder),
                tried.join(" · "),
                if truthy(&lks_v) { lks.clone() } else { "(없음)".into() }
            )),
        ),
        ("last_known_session", lks_v),
    ]);
    let hp = surface_requests(&a.repo_root, &a.context_id.clone().unwrap_or_default(), &pv::empty(), n, &crash)?;
    say(&format!("[relay] ⛔ 러너 사다리 소진 — {}칸 전부 실행하지 못했다:", ladder.len()));
    for t in &tried {
        say(&format!("[relay]     · {t}"));
    }
    say("[relay]   서브가 기동되지 않는다. 이것은 예산도 과업도 아닌 **실행자 평면**의 문제다.");
    if let Some(hp) = hp {
        say(&format!("[relay]   차단성 HITL 표면화 → {hp} (답을 적기 전까지 --continue 는 진행하지 않는다)"));
    }
    Ok(3)
}

fn sleep_s(v: &V) {
    if let Some(x) = pv::num(v) {
        if x > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(x));
        }
    }
}
