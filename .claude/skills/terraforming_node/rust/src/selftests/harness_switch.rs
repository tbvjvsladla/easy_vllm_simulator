//! Port of `selftest_harness_switch.py` (partial — see "Not ported" below) — offline checks of provider handoff and
//! durable relay ownership with the real request builders, transport validation and ledger writers. No model, SSH or GPU.
//!
//! Ported cases (the original's names): lifecycle_echo_schema_matches_relay_contract · codex_reasoning_effort_contract_and_argv
//! · canary_cli_forwards_effort (through `bootstrap_canary.sh`) · slot_efforts_restore_without_cross_slot_leakage ·
//! claude_effort_argv_and_unsupported_levels · native_effort_default_is_measured_without_forcing_auto and
//! native_effort_status_rejects_fabricated_model_reply_and_timeout (through the provider's `invoke_with` process seam:
//! the original replaced the adapter's inner `_invoke`, here the stand-in process answers the same way; the timeout case
//! uses a one-second request budget instead of a patched `time.monotonic`) · relay_rejects_ambiguous_task_arguments
//! (through `relay.sh`) · continue_preview_reports_selected_provider_budget_without_writes (the `relay.sh` previews and
//! `continuation_budget_preview`; the patched `next_runner_index` call is observed through the `--runners` command line) ·
//! v1_builder · v2_builder · real_runner_registry · native_resume · rejected_rotation (its supervisor half; the
//! `run_attempt_once` stand-in's ledger entry is written by `record_attempt` itself) · late_execution · stale_ledger ·
//! capacity_is_strict_structured_failure · local_deadline · the three supervised-budget cases ·
//! historical_count_caps (the supervisor's own progress reading replaces the patched `ledger_progress`) ·
//! preflight_quotes_apostrophe (through `assert_sub_branch`'s runner seam) · instruction_corruption · legacy_single.
//!
//! Not ported (they replace `relay.subprocess.run` for the A2A dispatch, `assert_sub_branch`, `run_attempt[_once]`,
//! `pending_for`, `continuation_instruction`, `assemble_continuation`, `load_dispatch_outcome`, `_campaign_init`, … inside one
//! interpreter; the Rust relay has no in-process seam for that dispatch path yet): codex_failure_recovery · the capacity
//! batch/handoff/policy/burst/unfinished-command cases · codex_wall_timeout · dispatch_reserves_durably · mixed_rotation ·
//! lost_dispatch_output · concurrent_pending_writers · real_dispatch_entrypoint_rejects_competing_owner ·
//! inflight_stops_replay · remote_deadline_marker · controlled_deadline_requires_durable_bound_outcome ·
//! native_exhaustion_requires_bound_proof · supervisor_restores_context · lost_stdout_recovers · reconciliation_does_not_upgrade
//! · slice_header_distinguishes_existing_campaign · revised_control_axis · explicit_feedback_survives_supervision. The
//! relay's own `--self-test` (`relay.sh --self-test`) covers part of that ground with its thread-local hooks.

use super::{dumps, j, path_with, scripts_dir, Checks, TempDir};
use crate::agent_control::process::{Completed, RunError};
use crate::agent_control::{load_schema, request_schema_violations, result_schema_path, schema_violations};
use crate::relay::cli::Args;
use crate::relay::core::{self as rc, AttemptMeta};
use crate::relay::dispatch as rd;
use crate::relay::pv::{self, g, items, py_str, s, set, V};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "topology: multi\ncpu_arch: aarch64\ngpus_per_node: 1\nnodes:\n  - role: main\n    host: 192.0.2.10\n    ssh_user: tester\n    work_dir: /srv/main\n  - role: sub\n    host: 192.0.2.11\n    ssh_user: tester\n    work_dir: /srv/sub\n";

struct Fx {
    _tmp: TempDir,
    root: PathBuf,
    manifest: String,
    native: V,
    wall: V,
    claude: V,
    codex: V,
}

fn with(v: &V, changes: &str) -> V {
    let mut x = v.clone();
    pv::update(&mut x, &j(changes));
    x
}

fn rejected<T>(r: Result<T, crate::sysexit::Exit>, message: &str) -> bool {
    match r {
        Ok(_) => false,
        Err(e) => message.is_empty() || format!("{e:?}").contains(message),
    }
}

fn meta(runner: &V) -> AttemptMeta {
    AttemptMeta { resume_declared: V::None, request_path: V::None, runner: runner.clone(), started_utc: V::None, ended_utc: V::None,
                  session_binding: V::None, dispatch_id: V::None }
}

impl Fx {
    fn new() -> Fx {
        let tmp = TempDir::new("harness-switch-");
        let root = tmp.path().to_path_buf();
        let manifest = root.join("manifest.yaml");
        std::fs::write(&manifest, MANIFEST).expect("manifest");
        let native = rc::declare(V::int(12), V::int(90), "selftest explicit declaration", "native_turns").expect("native budget");
        let wall = rc::declare(V::None, V::int(90), "selftest explicit declaration", "wall_time").expect("wall budget");
        Fx { _tmp: tmp, root, manifest: manifest.display().to_string(), native, wall,
             claude: j(r#"{"name": "sonnet", "provider": "claude_code", "backend": "anthropic", "model": "sonnet"}"#),
             codex: j(r#"{"name": "codex-gpt-6.1-sol", "provider": "codex_cli", "backend": "openai", "model": "gpt-6.1-sol"}"#) }
    }
    fn repo(&self) -> String {
        self.root.display().to_string()
    }
    fn opts(&self, provider: &str, edit: impl FnOnce(&mut rd::BuildOpts)) -> rd::BuildOpts {
        let runner = if provider == "codex_cli" { &self.codex } else { &self.claude };
        let mut o = rd::BuildOpts { context_id: Some("switch".into()), attempt: 1, model: pv::st(g(runner, "model")), campaign_id: None,
            control_variables: V::None, backend: pv::st(g(runner, "backend")), delegated_node: None, provider: provider.into(),
            sandbox_mode: None, permission_profile: None, reasoning_effort: None };
        edit(&mut o);
        o
    }
    fn try_request(&self, provider: &str, edit: impl FnOnce(&mut rd::BuildOpts)) -> Result<V, crate::sysexit::Exit> {
        let bud = if provider == "codex_cli" { &self.wall } else { &self.native };
        rd::build_request(&self.repo(), "multi", &self.manifest, "inspect provided evidence", bud, &self.opts(provider, edit))
    }
    fn request(&self, provider: &str, edit: impl FnOnce(&mut rd::BuildOpts)) -> V {
        self.try_request(provider, edit).unwrap_or_else(|e| panic!("request: {e:?}"))
    }
    fn relay(&self, args: &[&str]) -> (i32, String, String) {
        let out = std::process::Command::new("bash").arg(scripts_dir().join("relay.sh")).args(args).current_dir(&self.root)
            .stdin(std::process::Stdio::null()).output().expect("relay.sh");
        (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn args_with(ladder: Vec<V>) -> Args {
    Args { ladder, ..Default::default() }
}

fn strs(v: &V) -> Vec<String> {
    items(v).iter().map(py_str).collect()
}

fn lines(events: &[V]) -> String {
    events.iter().map(|e| format!("{}\n", dumps(e))).collect()
}

fn codex_parse(request: &V, events: &[V], rc_: i32, timed_out: bool, cancelled: bool) -> V {
    crate::provider_codex_cli::parse_output(request, &lines(events), "", rc_, timed_out, cancelled, None)
}

fn lifecycle_echo(c: &mut Checks) {
    c.group("lifecycle echo schema matches the relay contract");
    let schema = load_schema(&scripts_dir().parent().expect("skill").join("sub_node/task-report.schema.json")).expect("task report schema");
    let violations = |v: &V| schema_violations(v, &schema, "$");
    let mut report = j(r#"{"task_id": "close", "context_id": "ctx", "turn": 1, "phase": "inspect", "status": "completed",
        "node_id": "sub", "self_verification": {"schema_valid": true, "checks_run": []}}"#);
    let echo = j(r#"{"campaign_id": "campaign", "control": "close", "state": "closed", "close_reason": "completed",
        "first_line": "상태: ACTIVE=_bootstrap"}"#);
    let base_errors = violations(&report);
    c.ck("base report is schema-valid", base_errors.is_empty(), || format!("{base_errors:?}"));
    set(&mut report, "campaign_control_echo", echo.clone());
    c.ck("close echo valid · converged", violations(&report) == base_errors && rc::campaign_control_echo_ok(&report, "campaign", "close", &V::None).0,
         String::new);
    let normal = &items(g(g(g(&schema, "properties"), "campaign_control_echo"), "oneOf"))[0];
    let mut enum_: Vec<String> = strs(g(g(g(normal, "properties"), "control"), "enum"));
    enum_.sort();
    let mut controls: Vec<String> = rc::CAMPAIGN_CONTROLS.iter().map(|(k, _)| k.to_string()).collect();
    controls.sort();
    c.ck("schema control enum = relay CAMPAIGN_CONTROLS", enum_ == controls, || format!("{enum_:?} {controls:?}"));
    for (control, state) in rc::CAMPAIGN_CONTROLS {
        set(&mut report, "campaign_control_echo", pv::obj(vec![("campaign_id", s("campaign")), ("control", s(control)), ("state", s(state)),
            ("first_line", s("fixture"))]));
        c.ck(&format!("echo {control}/{state} valid"), violations(&report) == base_errors, String::new);
    }
    set(&mut report, "campaign_control_echo", j(r#"{"campaign_id": "campaign", "control": "reslice", "state": "active",
        "first_line": "fixture", "rev": 2, "assigned_cells": ["a", "b"]}"#));
    c.ck("reslice echo valid", violations(&report).is_empty(), String::new);
    set(&mut report, "campaign_control_echo", j(r#"{"control": "orphan_sweep", "summary_testlog": "t.md", "first_line": "fixture",
        "results": [{"campaign_id": "old", "state": "closed", "close_reason": "abandoned", "refusal": null}]}"#));
    let ids = vec!["old".to_string()];
    c.ck("orphan sweep echo valid · converged", violations(&report) == base_errors && rc::orphan_sweep_echo_ok(&report, &ids).0, String::new);
    let mut refused = report.clone();
    set(&mut refused, "campaign_control_echo", j(r#"{"control": "orphan_sweep", "summary_testlog": "t.md", "first_line": "fixture",
        "results": [{"campaign_id": "old", "state": "active", "close_reason": null, "refusal": "gate denied"}]}"#));
    c.ck("refused sweep valid · not converged", violations(&refused).is_empty() && !rc::orphan_sweep_echo_ok(&refused, &ids).0, String::new);
    for change in [r#"{"unexpected": 1}"#, r#"{"first_line": 3}"#,
                   r#"{"results": [{"campaign_id": "old", "state": "closed", "close_reason": "abandoned", "refusal": 12}]}"#] {
        let mut bad = report.clone();
        let e = with(g(&bad, "campaign_control_echo"), change);
        set(&mut bad, "campaign_control_echo", e);
        c.ck(&format!("echo {change} → campaign_control_echo violation"), violations(&bad).iter().any(|x| x.contains("campaign_control_echo")), String::new);
    }
    let bad = with(&report, r#"{"unexpected": true}"#);
    c.ck("top-level unexpected → violation", violations(&bad).iter().any(|x| x.contains("unexpected")), String::new);
    let mut bad = report.clone();
    set(&mut bad, "campaign_control_echo", with(&echo, r#"{"rev": true}"#));
    c.ck("rev=true → violation", violations(&bad).iter().any(|x| x.contains("campaign_control_echo")), String::new);
    set(&mut bad, "campaign_control_echo", with(&echo, r#"{"campaign_id": "wrong"}"#));
    c.ck("wrong campaign id: schema-valid but not converged",
         violations(&bad) == base_errors && !rc::campaign_control_echo_ok(&bad, "campaign", "close", &V::None).0, String::new);
}

fn efforts(c: &mut Checks) {
    let fx = Fx::new();
    c.group("codex reasoning effort contract and argv");
    let request = fx.request("codex_cli", |o| o.reasoning_effort = Some("low".into()));
    c.ck("codex low effort request valid", request_schema_violations(&request).is_empty(), || format!("{:?}", request_schema_violations(&request)));
    let argv = crate::provider_codex_cli::inner_argv(&request);
    let m = argv.iter().position(|a| a == "--model").map(|i| argv[i + 1].clone());
    c.ck("argv --model gpt-6.1-sol · model_reasoning_effort=\"low\"", m.as_deref() == Some("gpt-6.1-sol")
         && argv.iter().any(|a| a == "model_reasoning_effort=\"low\""), || format!("{argv:?}"));
    let resumed = with(&request, r#"{"resume_session_id": "native-id"}"#);
    let sb = argv.iter().position(|a| a == "--sandbox").unwrap_or(argv.len());
    let task = py_str(g(&request, "task"));
    let mut want: Vec<String> = argv[..4].to_vec();
    want.push("resume".into());
    want.extend(argv[4..sb].iter().cloned());
    want.extend(["-c".into(), "sandbox_mode=\"read-only\"".into(), "--".into(), "native-id".into(), task]);
    let got = crate::provider_codex_cli::inner_argv(&resumed);
    c.ck("resume argv shape", got == want, || format!("{got:?}\n{want:?}"));
    let old = fx.request("codex_cli", |_| {});
    c.ck("no effort → no reasoning key/flag", !pv::has(&old, "reasoning_effort")
         && !crate::provider_codex_cli::inner_argv(&old).iter().any(|a| a.contains("model_reasoning_effort")), String::new);
    c.ck("invalid effort rejected", !request_schema_violations(&with(&request, r#"{"reasoning_effort": "invalid"}"#)).is_empty(), String::new);
    let claude = fx.request("claude_code", |_| {});
    c.ck("claude low valid · minimal rejected", request_schema_violations(&with(&claude, r#"{"reasoning_effort": "low"}"#)).is_empty()
         && !request_schema_violations(&with(&claude, r#"{"reasoning_effort": "minimal"}"#)).is_empty(), String::new);

    c.group("canary CLI forwards effort");
    let out = std::process::Command::new("bash").arg(scripts_dir().join("bootstrap_canary.sh"))
        .args(["--topology", "multi", "--manifest", &fx.manifest, "--provider", "codex_cli", "--backend", "openai", "--model", "gpt-6.1-terra",
               "--reasoning-effort", "low", "--timeout-seconds", "90", "--max-dispatches", "3", "--budget-source", "offline fixture"])
        .stdin(std::process::Stdio::null()).output().expect("bootstrap_canary.sh");
    let req = pv::loads(&String::from_utf8_lossy(&out.stdout)).unwrap_or(V::None);
    c.ck("canary request: model gpt-6.1-terra · effort low · schema-valid", out.status.success()
         && py_str(g(&req, "model")) == "gpt-6.1-terra" && py_str(g(&req, "reasoning_effort")) == "low" && request_schema_violations(&req).is_empty(),
         || String::from_utf8_lossy(&out.stderr).into_owned());

    c.group("slot efforts restore without cross-slot leakage");
    let mut a = args_with(vec![fx.codex.clone(), fx.claude.clone()]);
    a.reasoning_effort = Some("high".into());
    a.runner_effort = vec!["sonnet=low".into()];
    let mut doc = pv::empty();
    let ok = rd::restore_runner_efforts(&mut a, &mut doc).is_ok();
    c.ck("caller + slot efforts", ok && rd::runner_effort(&a, &fx.claude) == ("low".into(), "runner:sonnet".into())
         && rd::runner_effort(&a, &fx.codex) == ("high".into(), "caller".into()), String::new);
    let mut resumed = args_with(a.ladder.clone());
    let ok = rd::restore_runner_efforts(&mut resumed, &mut doc).is_ok();
    c.ck("resumed: saved slot only · other slot native default · matches", ok
         && rd::runner_effort(&resumed, &fx.claude) == ("low".into(), "runner:sonnet".into())
         && rd::runner_effort(&resumed, &fx.codex) == ("default".into(), "native-default".into())
         && rd::effort_matches(&resumed, &fx.codex, &pv::empty()), String::new);
    for entries in [vec!["typo=low"], vec!["sonnet=typo"], vec!["sonnet=low", "sonnet=high"]] {
        a.runner_effort = entries.iter().map(|x| x.to_string()).collect();
        c.ck(&format!("--runner-effort {entries:?} rejected"), rejected(rd::restore_runner_efforts(&mut a, &mut pv::empty()), ""), String::new);
    }
    let mut changed = args_with(vec![fx.codex.clone()]);
    changed.runners = Some("codex-gpt-6.1-sol".into());
    let ok = rd::restore_runner_efforts(&mut changed, &mut doc).is_ok();
    c.ck("a changed ladder drops foreign slots", ok && pv::eq(g(&doc, "runner_efforts"), &pv::empty()), || dumps(&doc));

    c.group("claude effort argv and unsupported levels");
    for (backend, model) in [("anthropic", "sonnet"), ("claude-sub", "deepseek-flash")] {
        for effort in ["low", "medium", "high", "xhigh", "max"] {
            let r = with(&fx.request("claude_code", |o| o.reasoning_effort = Some(effort.into())),
                         &format!(r#"{{"backend": "{backend}", "model": "{model}"}}"#));
            let argv = crate::provider_claude_code::inner_argv(&r);
            let e = argv.iter().position(|a| a == "--effort").map(|i| argv[i + 1].clone());
            c.ck(&format!("{backend}/{effort}: valid · --effort {effort}"), request_schema_violations(&r).is_empty() && e.as_deref() == Some(effort),
                 || format!("{argv:?}"));
        }
    }
    for effort in ["none", "minimal", "typo"] {
        let r = fx.try_request("claude_code", |o| o.reasoning_effort = Some(effort.into()));
        c.ck(&format!("claude effort {effort} rejected"), r.map(|r| !request_schema_violations(&r).is_empty()).unwrap_or(true), String::new);
    }
    let d = fx.request("claude_code", |o| o.reasoning_effort = Some("default".into()));
    let cd = fx.request("codex_cli", |o| o.reasoning_effort = Some("default".into()));
    c.ck("default effort: no --effort / no model_reasoning_effort", !crate::provider_claude_code::inner_argv(&d).iter().any(|a| a == "--effort")
         && !crate::provider_codex_cli::inner_argv(&cd).iter().any(|a| a.contains("model_reasoning_effort")), String::new);
}

/// The native CLI words of a provider command line: the argv itself plus the words of the remote `bash -lc '<cmd>'` the ssh
/// transport carries as its last argument (two shell levels).
fn tokens(argv: &[String]) -> Vec<String> {
    use crate::agent_control::pyhelp::shlex_split;
    let mut out = argv.to_vec();
    if let Some(outer) = argv.last().map(|l| shlex_split(l)) {
        if let Some(inner) = outer.last().map(|l| shlex_split(l)) {
            out.extend(inner);
        }
        out.extend(outer);
    }
    out
}

/// A provider process stand-in for the claude adapter: answers `/effort status` as a local command (or not) and the work.
fn effort_run<'a>(calls: &'a RefCell<Vec<(Vec<String>, Option<f64>)>>, status_text: &'a str, local: bool, delay_ms: u64)
    -> impl Fn(&[String], Option<&str>, Option<f64>) -> Result<Completed, RunError> + 'a {
    move |argv: &[String], _cwd: Option<&str>, timeout: Option<f64>| {
        calls.borrow_mut().push((argv.to_vec(), timeout));
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let words = tokens(argv);
        let probe = words.iter().any(|a| a == "/effort status");
        let session = words.iter().position(|a| a == "--resume").map(|i| words[i + 1].clone());
        let sid = session.map(|x| s(&x)).unwrap_or(V::None);
        let stdout = if probe {
            let mut terminal = pv::obj(vec![("type", s("result")), ("subtype", s("success")), ("is_error", V::Bool(false)), ("result", s(status_text)),
                ("session_id", sid), ("num_turns", V::int(1)), ("permission_denials", V::List(vec![])), ("modelUsage", pv::empty())]);
            if local {
                set(&mut terminal, "local_command", s("effort"));
            }
            dumps(&terminal)
        } else {
            dumps(&pv::obj(vec![("type", s("result")), ("subtype", s("success")), ("is_error", V::Bool(false)), ("result", s("work result")),
                ("session_id", sid), ("num_turns", V::int(1)), ("permission_denials", V::List(vec![])), ("modelUsage", pv::empty())]))
        };
        Ok(Completed { returncode: 0, stdout: stdout.into_bytes(), stderr: vec![] })
    }
}

fn native_effort(c: &mut Checks) {
    let fx = Fx::new();
    c.group("native effort default is measured without forcing auto");
    let request = fx.request("claude_code", |o| o.reasoning_effort = Some("default".into()));
    let request = with(&request, r#"{"resume_session_id": "existing-native"}"#);
    let calls = RefCell::new(vec![]);
    let run = effort_run(&calls, "Effort level: auto (currently high)", true, 0);
    let result = crate::provider_claude_code::invoke_with(&request, &run);
    drop(run);
    let calls = calls.into_inner();
    let resumes = |argv: &[String]| {
        let w = tokens(argv);
        w.iter().position(|a| a == "--resume").map(|i| w[i + 1].clone())
    };
    c.ck("two calls · both resume existing-native · probe has no --effort · work has no --effort · budget not grown",
         calls.len() == 2 && resumes(&calls[0].0).as_deref() == Some("existing-native") && resumes(&calls[1].0).as_deref() == Some("existing-native")
             && !tokens(&calls[0].0).iter().any(|a| a == "--effort") && !tokens(&calls[1].0).iter().any(|a| a == "--effort")
             && calls[1].1.unwrap_or(f64::MAX) <= 90.0, || format!("{:?}", calls.iter().map(|c| &c.0).collect::<Vec<_>>()));
    let sel = g(&result, "reasoning_selection");
    c.ck("selection resolved high · native-default · upstream not observed",
         py_str(g(sel, "resolved")) == "high" && py_str(g(sel, "mode")) == "native-default" && pv::is_none(g(sel, "upstream_observed")),
         || dumps(&result));

    c.group("native effort status rejects a fabricated model reply and a timeout");
    let request = fx.request("claude_code", |o| o.reasoning_effort = Some("default".into()));
    let calls = RefCell::new(vec![]);
    let run = effort_run(&calls, "Effort level: high", false, 0);
    let result = crate::provider_claude_code::invoke_with(&request, &run);
    c.ck("a model saying the phrase is not native evidence → malformed_output · one call",
         py_str(g(&result, "status")) == "malformed_output" && calls.borrow().len() == 1, || dumps(&result));
    let short = with(&request, r#"{"timeout_seconds": 1}"#);
    let calls = RefCell::new(vec![]);
    let run = effort_run(&calls, "Effort level: high", true, 20);
    let result = crate::provider_claude_code::invoke_with(&short, &run);
    c.ck("the probe used the whole budget → timeout · one call", py_str(g(&result, "status")) == "timeout" && calls.borrow().len() == 1,
         || dumps(&result));
    let capture = with(&request, r#"{"intent": "probe", "capture_native_events": true}"#);
    let calls = RefCell::new(vec![]);
    let run = |argv: &[String], cwd: Option<&str>, t: Option<f64>| {
        calls.borrow_mut().push(argv.to_vec());
        let _ = (cwd, t);
        Ok(Completed { returncode: 0, stdout: b"Effort level: high".to_vec(), stderr: vec![] })
    };
    let result = crate::provider_claude_code::invoke_with(&capture, &run);
    let schema = load_schema(&result_schema_path()).expect("result schema");
    c.ck("captured probe with no native stream → raw_events [] · schema-valid",
         pv::eq(g(&result, "raw_events"), &V::List(vec![])) && schema_violations(&result, &schema, "$").is_empty(), || dumps(&result));
}

fn relay_cli(c: &mut Checks) {
    let fx = Fx::new();
    c.group("relay rejects ambiguous task arguments");
    for args in [vec!["--task", "one", "--task-file", "two"], vec!["--continue", "--task", "one"]] {
        let (code, _, _) = fx.relay(&args);
        c.ck(&format!("relay.sh {args:?} → exit ≠ 0"), code != 0, String::new);
    }

    c.group("continue preview reports the selected provider budget without writes");
    let path = rc::ledger_path(&fx.repo(), "switch").expect("ledger path");
    let mut doc = rc::load_ledger(&path).expect("ledger");
    let result = j(r#"{"status": "completed", "provider": "codex_cli", "session_id": "native", "execution_started": true,
        "reason_codes": [], "budget_outcome": "within_budget"}"#);
    let bud = rc::declare(V::None, V::int(3600), "fixture", "wall_time").expect("budget");
    let mut m = meta(&fx.codex);
    m.started_utc = s("2026-10-28T21:23:51Z");
    m.ended_utc = s("2026-10-28T22:14:00Z");
    rc::record_attempt(&mut doc, "switch", &bud, &result, &j(r#"{"status": "input-required", "context_id": "switch", "running_services": []}"#), &m);
    pv::update(&mut doc, &pv::obj(vec![("topology", s("multi")), ("manifest", s(&fx.manifest)), ("runner_ladder", g(&fx.codex, "name").clone())]));
    rc::save_ledger(&path, &mut doc).expect("save ledger");
    let task = fx.root.join("preview-task.txt");
    std::fs::write(&task, "complete task\n").expect("task");
    let before = std::fs::read(&path).unwrap_or_default();
    let ts = task.display().to_string();
    let repo = fx.repo();
    let argv = vec!["--repo-root", &repo, "--context-id", "switch", "--no-campaign", "--continue", "--task-file", &ts, "--timeout-seconds", "1800"];
    let (code, preview, err) = fx.relay(&argv);
    c.ck("preview rc 0 · codex provider · previous/observed/next wall · no turn flags",
         code == 0 && preview.contains("provider=codex_cli") && preview.contains("직전 wall 배정 3600초")
             && preview.contains("관측 경과 3009초(main 시작/종료 시각)") && preview.contains("다음 wall 배정 1800초(명시)")
             && !preview.contains("max_turns") && !preview.contains("--max-turns") && !preview.contains("전송 상한 100"),
         || format!("{code} {preview} {err}"));
    let requests = Path::new(&rc::relay_root(&repo).unwrap_or_default()).join("switch.requests");
    c.ck("preview wrote nothing", std::fs::read(&path).unwrap_or_default() == before && !requests.exists(), String::new);
    let wall1800 = rc::declare(V::None, V::int(1800), "fixture", "wall_time").expect("budget");
    let req = rd::build_request(&repo, "multi", &fx.manifest, "task", &wall1800, &fx.opts("codex_cli", |o| {
        o.context_id = None;
        o.sandbox_mode = Some("read-only".into());
    })).expect("request");
    c.ck("wall request: wall_time · max_turns None · 1800", py_str(g(&req, "budget_unit")) == "wall_time" && pv::is_none(g(&req, "max_turns"))
         && pv::int(g(&req, "timeout_seconds")) == Some(1800), || dumps(&req));
    let codex = vec![fx.codex.clone()];
    let unknown = rc::continuation_budget_preview(&doc, &codex, Some(&pv::empty()), None).map(|x| x.0).unwrap_or_default();
    c.ck("unobserved elapsed · undeclared next", unknown.contains("관측 경과 미관측") && unknown.contains("다음 wall 배정 미선언"), || unknown.clone());
    let measured = rc::continuation_budget_preview(&doc, &codex, Some(&j(r#"{"duration_ms": 1250}"#)), None).map(|x| x.0).unwrap_or_default();
    c.ck("provider duration_ms shown", measured.contains("1.25초(provider duration_ms)"), || measured.clone());
    let claude_doc = j(r#"{"attempts": [{"max_turns_allocated": 24, "execution_started": true}]}"#);
    let (claude, flags) = rc::continuation_budget_preview(&claude_doc, &[fx.claude.clone()], None, None).unwrap_or_default();
    c.ck("claude preview: native turn · floor 24 · schema cap 100 · --max-turns flag",
         claude.contains("native turn") && claude.contains("바닥 24") && claude.contains("스키마 필드 상한 100") && flags.contains("--max-turns"),
         || format!("{claude} | {flags}"));
    let mut mixed_argv = argv.clone();
    mixed_argv.extend(["--runners", "codex-gpt-6.1-sol,sonnet", "--max-turns", "24"]);
    let (code, out, err) = fx.relay(&mixed_argv);
    c.ck("mixed ladder preview: codex selected · Claude fallback --max-turns noted",
         code == 0 && out.contains("provider=codex_cli") && out.contains("Claude fallback용 --max-turns"), || format!("{out} {err}"));
    let mut emit = mixed_argv.clone();
    emit.extend(["--apply", "--emit-only", "--resume", "new", "--budget-source", "fixture", "--sandbox-mode", "read-only"]);
    let (code, blob, err) = fx.relay(&emit);
    let mixed_request = blob.find("{\n").and_then(|i| pv::loads(&blob[i..]).ok()).unwrap_or(V::None);
    c.ck("emit-only request: codex · max_turns None · 1800", code == 0 && py_str(g(&mixed_request, "provider")) == "codex_cli"
         && pv::is_none(g(&mixed_request, "max_turns")) && pv::int(g(&mixed_request, "timeout_seconds")) == Some(1800), || format!("{blob} {err}"));
    let no_turns: Vec<&str> = emit.iter().copied().filter(|x| *x != "--max-turns" && *x != "24").collect();
    let (code, _, _) = fx.relay(&no_turns);
    c.ck("mixed ladder without --max-turns is refused · ledger unchanged", code != 0 && std::fs::read(&path).unwrap_or_default() == before, String::new);
}

fn builders(c: &mut Checks) {
    let fx = Fx::new();
    c.group("v1 builder remains valid and uses the native budget");
    let request = fx.request("claude_code", |_| {});
    let argv = crate::provider_claude_code::build_argv(&request).unwrap_or_default();
    c.ck("v1: valid · schema 1 · 12 turns · 90 s · no budget_unit/sandbox_mode · ssh argv with --max-turns",
         request_schema_violations(&request).is_empty() && pv::int(g(&request, "schema_version")) == Some(1)
             && pv::int(g(&request, "max_turns")) == Some(12) && pv::int(g(&request, "timeout_seconds")) == Some(90)
             && !pv::has(&request, "budget_unit") && !pv::has(&request, "sandbox_mode")
             && argv.first().map(String::as_str) == Some("ssh") && argv.last().is_some_and(|l| l.contains("--max-turns")), || dumps(&request));

    c.group("v2 builder and the real transport validator");
    let request = fx.request("codex_cli", |o| o.permission_profile = Some(":danger-full-access".into()));
    c.ck("v2: valid · schema 2 · no turns · wall 90 · no max_dispatches · AGENTS.md",
         request_schema_violations(&request).is_empty() && pv::int(g(&request, "schema_version")) == Some(2) && pv::is_none(g(&request, "max_turns"))
             && py_str(g(&request, "budget_unit")) == "wall_time" && pv::int(g(&request, "timeout_seconds")) == Some(90)
             && !pv::has(&request, "max_dispatches") && py_str(g(&request, "task")).contains("AGENTS.md"), || dumps(&request));
    for changes in [r#"{"schema_version": 1}"#, r#"{"provider": "claude_code"}"#, r#"{"max_turns": 12}"#, r#"{"max_dispatches": 0}"#,
                    r#"{"max_dispatches": true}"#, r#"{"timeout_seconds": null}"#, r#"{"permission_profile": "vllm-main"}"#,
                    r#"{"sandbox_mode": "workspace-write"}"#] {
        c.ck(&format!("v2 {changes} rejected"), !request_schema_violations(&with(&request, changes)).is_empty(), String::new);
    }
    c.ck("historical max_dispatches=3 stays readable", request_schema_violations(&with(&request, r#"{"max_dispatches": 3}"#)).is_empty(), String::new);
    c.ck("wall budget with turns rejected · wall declaration has no max_dispatches",
         rejected(rc::declare(V::int(1), V::int(90), "declared", "wall_time"), "")
             && !pv::has(&rc::declare(V::None, V::int(90), "declared", "wall_time").unwrap_or(V::None), "max_dispatches"), String::new);

    c.group("the real runner registry preserves the default and provider identity");
    let repo = super::repo_root().display().to_string();
    let default = rc::resolve_ladder(&repo, None, None, None, None).unwrap_or_default();
    c.ck("default ladder = sonnet/claude_code", default.len() == 1 && py_str(g(&default[0], "name")) == "sonnet"
         && py_str(g(&default[0], "provider")) == "claude_code", || format!("{default:?}"));
    let explicit = rc::resolve_ladder(&repo, Some("sonnet,codex-gpt-6.1-sol"), None, None, None).unwrap_or_default();
    c.ck("explicit ladder providers", explicit.iter().map(|r| py_str(g(r, "provider"))).collect::<Vec<_>>() == ["claude_code", "codex_cli"], String::new);
    let chosen = rc::resolve_ladder(&repo, None, Some("openai"), Some("gpt-6.1-sol"), Some("codex_cli")).unwrap_or_default();
    c.ck("provider/backend/model choice = the codex runner", chosen.len() == 1 && pv::eq(&chosen[0], &fx.codex), || format!("{chosen:?}"));

    c.group("native resume never crosses a provider or permission boundary");
    let claude_request = fx.request("claude_code", |_| {});
    let codex_request = fx.request("codex_cli", |o| o.sandbox_mode = Some("read-only".into()));
    let mut doc = pv::obj(vec![("attempts", V::List(vec![pv::obj(vec![("session_id", s("native-claude")), ("runner", fx.claude.clone()),
        ("session_binding", rc::permission_binding(&claude_request))])]))]);
    let new_ = |r: Result<(String, V), crate::sysexit::Exit>| matches!(r, Ok((x, V::None)) if x == "new");
    c.ck("claude session not resumed by codex", new_(rd::bound_resume(&doc, "native-claude", &codex_request)), String::new);
    let binding = rc::permission_binding(&codex_request);
    pv::append(pv::get_mut(&mut doc, "attempts").expect("attempts"), pv::obj(vec![("session_id", s("native-codex")), ("runner", fx.codex.clone()),
        ("session_binding", binding.clone())]));
    c.ck("codex session resumed with its binding", matches!(rd::bound_resume(&doc, "native-codex", &codex_request), Ok((x, b)) if x == "native-codex" && pv::eq(&b, &binding)),
         String::new);
    let mut resumed = codex_request.clone();
    set(&mut resumed, "resume_session_id", s("native-codex"));
    set(&mut resumed, "resume_binding", binding.clone());
    c.ck("resumed request schema-valid · claude cannot take the codex session",
         request_schema_violations(&resumed).is_empty() && new_(rd::bound_resume(&doc, "native-codex", &claude_request)), String::new);
    let mut target = g(&codex_request, "target").clone();
    set(&mut target, "work_dir", s("/srv/other"));
    for (key, value) in [("sandbox_mode", s("workspace-write")), ("target", target)] {
        let mut changed = codex_request.clone();
        set(&mut changed, key, value);
        let mut cr = changed.clone();
        set(&mut cr, "resume_session_id", s("native-codex"));
        set(&mut cr, "resume_binding", binding.clone());
        c.ck(&format!("changed {key}: binding changed refusal · schema rejects the resumed request"),
             rejected(rd::bound_resume(&doc, "native-codex", &changed), "binding changed") && !request_schema_violations(&cr).is_empty(), String::new);
    }
    c.ck("unrecorded session refused · legacy ledger passes through",
         rejected(rd::bound_resume(&doc, "unrecorded", &codex_request), "")
             && matches!(rd::bound_resume(&j(r#"{"attempts": []}"#), "legacy", &claude_request), Ok((x, V::None)) if x == "legacy"), String::new);
}

fn supervision(c: &mut Checks) {
    let fx = Fx::new();
    c.group("a rejected rotation is not reopened by the supervisor");
    let mut doc = j(r#"{"attempts": []}"#);
    rc::record_attempt(&mut doc, "switch", &fx.native, &j(&format!(r#"{{"status": "execution_failed", "reason_codes": ["{}"],
        "execution_started": false, "failure_evidence": {{"rotate": false, "signal": "unknown"}}}}"#, rc::RUNNER_UNAVAILABLE)), &V::None, &meta(&fx.claude));
    let ladder = vec![fx.claude.clone(), fx.codex.clone()];
    c.ck("rotate=false → popup", py_str(g(&rc::supervise_decide(&doc, &V::None, &ladder, None), "action")) == "popup", String::new);
    if let Some(V::List(l)) = pv::get_mut(&mut doc, "attempts") {
        if let Some(ev) = l.last_mut().and_then(|a| pv::get_mut(a, "runner_evidence")) {
            set(ev, "rotate", V::Bool(true));
        }
    }
    c.ck("rotate=true → rotate_runner", py_str(g(&rc::supervise_decide(&doc, &V::None, &ladder, None), "action")) == "rotate_runner", || dumps(&doc));

    c.group("late execution consumes cost and cannot rotate");
    let mut doc = j(r#"{"attempts": []}"#);
    let att = rc::record_attempt(&mut doc, "switch", &fx.wall, &j(&format!(r#"{{"status": "execution_failed", "reason_codes": ["{}"],
        "execution_started": true, "raw_exit_code": 1, "failure_evidence": {{"rotate": true}}}}"#, rc::RUNNER_UNAVAILABLE)), &V::None, &meta(&fx.codex));
    let ladder = vec![fx.codex.clone(), fx.claude.clone()];
    let d = rc::supervise_decide(&doc, &V::None, &ladder, None);
    c.ck("reached the sub · popup · no count cap text", rc::reached_sub(&att) && py_str(g(&d, "action")) == "popup" && !py_str(g(&d, "reason")).contains("1/1"),
         || dumps(&d));

    c.group("a stale ledger cannot erase a new attempt");
    let path = fx.root.join("state.json").display().to_string();
    let mut initial = rc::load_ledger(&path).expect("ledger");
    rc::save_ledger(&path, &mut initial).expect("save");
    let mut stale = rc::load_ledger(&path).expect("ledger");
    let mut winner = rc::load_ledger(&path).expect("ledger");
    pv::append(pv::get_mut(&mut winner, "attempts").expect("attempts"), j(r#"{"attempt": 1, "execution_started": true}"#));
    rc::save_ledger(&path, &mut winner).expect("save");
    pv::append(pv::get_mut(&mut stale, "attempts").expect("attempts"), j(r#"{"attempt": 99}"#));
    let refused = rejected(rc::save_ledger(&path, &mut stale), "stale ledger");
    c.ck("stale save refused · winner kept", refused && pv::eq(&rc::load_ledger(&path).unwrap_or(V::None), &winner), String::new);

    c.group("capacity is a strict structured failure and keeps the recovery gate");
    let request = fx.request("codex_cli", |_| {});
    let msg = "Selected model is at capacity. Please try a different model.";
    let base = vec![j(r#"{"type": "thread.started", "thread_id": "session"}"#), j(r#"{"type": "turn.started"}"#)];
    let failures = vec![pv::obj(vec![("type", s("error")), ("message", s(msg))]),
                        pv::obj(vec![("type", s("turn.failed")), ("error", pv::obj(vec![("message", s(msg))]))])];
    let cat = |a: &[V], b: &[V]| a.iter().chain(b.iter()).cloned().collect::<Vec<V>>();
    let result = codex_parse(&request, &cat(&base, &failures), 1, false, false);
    let att = rc::record_attempt(&mut j(r#"{"attempts": []}"#), "switch", &fx.wall, &result, &V::None, &meta(&fx.codex));
    c.ck("capacity signal · not completed · recovery required", py_str(g(g(&result, "failure_evidence"), "signal")) == "provider_capacity"
         && py_str(g(&result, "status")) != "completed" && rc::codex_recovery_requirement(&att).is_some(), || dumps(&result));
    let agent = pv::obj(vec![("type", s("item.completed")), ("item", pv::obj(vec![("type", s("agent_message")), ("text", s(msg))]))]);
    let other = j(r#"{"type": "error", "message": "other"}"#);
    let denied = j(r#"{"type": "error", "code": "permission_denied"}"#);
    for (label, events, rc_, cancelled) in [("agent message only", cat(&base, &[agent]), 1, false), ("other error", cat(&cat(&base, &failures), &[other]), 1, false),
        ("rc 143", cat(&base, &failures), 143, false), ("rc 124", cat(&base, &failures), 124, false), ("cancelled", cat(&base, &failures), 1, true),
        ("permission denied", cat(&cat(&base, &failures), &[denied]), 1, false)] {
        let r = codex_parse(&request, &events, rc_, false, cancelled);
        c.ck(&format!("{label}: not provider_capacity"), py_str(g(g(&r, "failure_evidence"), "signal")) != "provider_capacity", String::new);
    }
    let malformed = crate::provider_codex_cli::parse_output(&request, &format!("{}\n{{bad", dumps(&failures[0])), "", 1, false, false, None);
    c.ck("malformed stream: not provider_capacity", py_str(g(g(&malformed, "failure_evidence"), "signal")) != "provider_capacity", String::new);

    c.group("local deadline uses one watchdog and a bare 124 is ambiguous");
    let mut request = fx.request("codex_cli", |o| o.sandbox_mode = Some("read-only".into()));
    set(&mut request, "target", pv::obj(vec![("role", s("main")), ("transport", s("local")), ("work_dir", s(&fx.repo()))]));
    let argv = crate::provider_codex_cli::build_argv(&request);
    c.ck("local argv has no timeout wrapper", argv.first().map(String::as_str) != Some("timeout") && !argv.iter().any(|a| a == "timeout"), || format!("{argv:?}"));
    let events = vec![j(r#"{"type": "adapter.lock_acquired"}"#), j(r#"{"type": "thread.started", "thread_id": "native"}"#), j(r#"{"type": "turn.started"}"#)];
    c.ck("bare 124 → wall_timeout · reaped -15 timed_out → controlled_local_deadline",
         py_str(g(g(&codex_parse(&request, &events, 124, false, false), "failure_evidence"), "signal")) == "wall_timeout"
             && py_str(g(g(&codex_parse(&request, &events, -15, true, false), "failure_evidence"), "signal")) == "controlled_local_deadline", String::new);

    c.group("supervised budget uses current-phase evidence and typed recommendations");
    let mut args = args_with(vec![fx.codex.clone()]);
    let mut old = j(r#"{"attempt": 1, "phase": "serve", "control_status": "completed", "status": "completed", "duration_ms": 900000,
        "execution_started": true, "sub_reported": true, "budget_outcome": "within_budget", "provider": "codex_cli"}"#);
    set(&mut old, "runner", fx.codex.clone());
    let mut last = j(r#"{"attempt": 2, "phase": "publish", "control_status": "completed", "status": "input-required", "execution_started": true,
        "sub_reported": true, "next_steps": ["remaining"], "provider": "codex_cli", "timeout_seconds_allocated": 600,
        "budget_recommended": {"timeout_seconds": 120}}"#);
    set(&mut last, "runner", fx.codex.clone());
    let budget = |a: &Args, attempts: Vec<V>| rd::select_supervised_budget(a, &pv::obj(vec![("attempts", V::List(attempts))]));
    let t = |b: &Result<V, crate::sysexit::Exit>| b.as_ref().ok().and_then(|b| pv::int(g(b, "timeout_seconds")));
    let b = budget(&args, vec![old.clone(), last.clone()]);
    c.ck("typed recommendation 120 · no turns · source says recommendation", t(&b) == Some(120)
         && b.as_ref().is_ok_and(|b| pv::is_none(g(b, "max_turns")) && py_str(g(b, "source")).contains("recommendation")), || format!("{b:?}"));
    let mut text_rec = last.clone();
    set(&mut text_rec, "budget_recommended", j(r#"{"timeout_seconds": "120"}"#));
    c.ck("an untyped recommendation is ignored → 600", t(&budget(&args, vec![old.clone(), text_rec])) == Some(600), String::new);
    let same = with(&old, r#"{"attempt": 3, "phase": "publish", "duration_ms": 80000, "next_steps": ["prior checkpoint"]}"#);
    c.ck("same-phase evidence → 120", t(&budget(&args, vec![old.clone(), same.clone(), last.clone()])) == Some(120), String::new);
    set(&mut last, "failure_evidence", j(r#"{"signal": "controlled_remote_deadline"}"#));
    c.ck("controlled remote deadline → 900", t(&budget(&args, vec![old.clone(), same.clone(), last.clone()])) == Some(900), String::new);
    args.timeout_seconds = Some(77);
    c.ck("explicit timeout wins → 77", t(&budget(&args, vec![old.clone(), same.clone(), last.clone()])) == Some(77), String::new);
    args.timeout_seconds = None;
    let cap_t = rc::schema_cap("timeout_seconds").unwrap_or(0);
    set(&mut last, "timeout_seconds_allocated", V::int(cap_t));
    let capped = budget(&args, vec![old.clone(), same.clone(), last.clone()]);
    c.ck("at the schema cap: cap kept · checkpoint continues", t(&capped) == Some(cap_t)
         && capped.as_ref().is_ok_and(|b| py_str(g(b, "source")).contains("checkpoint continues")), || format!("{capped:?}"));
    args.ladder = vec![fx.claude.clone()];
    pv::pop(&mut last, "failure_evidence");
    pv::update(&mut last, &j(r#"{"max_turns_allocated": 100, "budget_recommended": {"timeout_seconds": 120, "max_turns": 20}}"#));
    let reduced = budget(&args, vec![old.clone(), same.clone(), last.clone()]);
    c.ck("claude reduced → (120, 20)", t(&reduced) == Some(120) && reduced.as_ref().ok().and_then(|b| pv::int(g(b, "max_turns"))) == Some(20),
         || format!("{reduced:?}"));
    set(&mut last, "budget_outcome", s("exhausted"));
    let cap_m = rc::schema_cap("max_turns").unwrap_or(0);
    c.ck("exhausted → max_turns at the schema cap",
         budget(&args, vec![old.clone(), same.clone(), last.clone()]).ok().and_then(|b| pv::int(g(&b, "max_turns"))) == Some(cap_m), String::new);
    args.ladder = vec![fx.codex.clone()];
    args.max_turns = Some(10);
    c.ck("codex with --max-turns refused", rejected(budget(&args, vec![old.clone(), same.clone(), last.clone()]), ""), String::new);

    c.group("supervised budget shrink needs new progress and the same provider");
    let args = args_with(vec![fx.codex.clone()]);
    let mut previous = j(r#"{"attempt": 1, "phase": "bench", "provider": "codex_cli", "execution_started": true, "status": "input-required",
        "control_status": "completed", "sub_reported": true, "artifact_paths": ["same"], "next_steps": ["same"]}"#);
    set(&mut previous, "runner", fx.codex.clone());
    let last = with(&previous, r#"{"attempt": 2, "timeout_seconds_allocated": 600, "budget_recommended": {"timeout_seconds": 120}}"#);
    c.ck("no new progress → 600", t(&budget(&args, vec![previous.clone(), last.clone()])) == Some(600), String::new);
    for change in [r#"{"phase": "publish"}"#, r#"{"artifact_paths": ["same", "new"]}"#, r#"{"next_steps": ["changed"]}"#] {
        c.ck(&format!("progress {change} → 120"), t(&budget(&args, vec![previous.clone(), with(&last, change)])) == Some(120), String::new);
    }
    for other_runner in [fx.claude.clone(), with(&fx.codex, r#"{"model": "other-model"}"#)] {
        let mut other = with(&previous, r#"{"attempt": 3, "status": "completed", "duration_ms": 10000}"#);
        set(&mut other, "provider", g(&other_runner, "provider").clone());
        set(&mut other, "runner", other_runner.clone());
        c.ck(&format!("other runner {} → 600", py_str(g(&other_runner, "model"))), t(&budget(&args, vec![other, last.clone()])) == Some(600), String::new);
    }
    let contradicted = with(&previous, r#"{"attempt": 3, "status": "completed", "duration_ms": 10000, "model_used": ["other-observed-model"]}"#);
    c.ck("contradicting observed model → 600", t(&budget(&args, vec![contradicted, last.clone()])) == Some(600), String::new);

    c.group("a successful worker checkpoint calibrates wall without completing the task");
    let mut checkpoint = j(r#"{"attempt": 1, "phase": "publish", "provider": "codex_cli", "control_status": "completed", "status": "input-required",
        "sub_reported": true, "hitl_needed": false, "budget_outcome": "within_budget", "execution_started": true,
        "started_utc": "2026-10-28T10:46:50Z", "ended_utc": "2026-10-28T10:50:04Z", "duration_ms": null, "next_steps": ["old"]}"#);
    set(&mut checkpoint, "runner", fx.codex.clone());
    let last = with(&checkpoint, r#"{"attempt": 2, "started_utc": null, "ended_utc": null, "next_steps": ["new"], "timeout_seconds_allocated": 600}"#);
    let b = budget(&args, vec![checkpoint.clone(), last.clone()]);
    c.ck("calibrated 291 from main_wall_ms", t(&b) == Some(291) && b.as_ref().is_ok_and(|b| py_str(g(b, "source")).contains("main_wall_ms")), || format!("{b:?}"));
    for bad in [r#"{"control_status": "execution_failed"}"#, r#"{"hitl_needed": true}"#, r#"{"budget_outcome": "exhausted"}"#, r#"{"permission_denials": ["denied"]}"#] {
        c.ck(&format!("checkpoint {bad} → 600"), t(&budget(&args, vec![with(&checkpoint, bad), last.clone()])) == Some(600), String::new);
    }

    c.group("historical count caps do not stop supervision or enter new budgets");
    let mut args = args_with(vec![fx.claude.clone()]);
    args.max_turns = Some(10);
    let b = rd::declared_budget(&args, Some(30), "explicit inherited context cap").unwrap_or(V::None);
    c.ck("declared budget has no max_dispatches", !pv::has(&b, "max_dispatches"), || dumps(&b));
    for runner in [fx.claude.clone(), fx.codex.clone()] {
        for count in [6, 101] {
            let attempts: Vec<V> = (1..=count).map(|n| {
                let mut a = j(&format!(r#"{{"attempt": {n}, "end_reason": "sub_input_required", "status": "input-required", "hitl_needed": false,
                    "next_steps": ["{n}"], "phase": "bench", "execution_started": true, "control_status": "completed"}}"#));
                set(&mut a, "runner", runner.clone());
                a
            }).collect();
            let doc = pv::obj(vec![("max_dispatches", V::int(6)), ("attempts", V::List(attempts))]);
            let d = rc::supervise_decide(&doc, &V::None, &[], Some("progress"));
            c.ck(&format!("{} × {count}: resume · attempts kept", py_str(g(&runner, "name"))),
                 py_str(g(&d, "action")) == "resume" && items(g(&doc, "attempts")).len() == count as usize, || dumps(&d));
        }
    }

    c.group("preflight quotes an apostrophe and has local and remote deadlines");
    let mut request = fx.request("codex_cli", |o| o.sandbox_mode = Some("read-only".into()));
    let mut target = g(&request, "target").clone();
    set(&mut target, "work_dir", s("/tmp/a'; printf injected; #"));
    set(&mut request, "target", target);
    let seen: RefCell<Vec<String>> = RefCell::new(vec![]);
    let ok_runner = |_t: &V, cmd: &str| -> (i32, String, String) {
        seen.borrow_mut().push(cmd.to_string());
        (0, "multi".into(), String::new())
    };
    let got = rd::assert_sub_branch(&request, "multi", Some(&ok_runner));
    let cmd = seen.borrow().first().cloned().unwrap_or_default();
    c.ck("branch read: multi · the work dir is shell-quoted (no injected command)",
         matches!(&got, Ok(b) if b == "multi") && cmd.contains(&rd::shell_quote("/tmp/a'; printf injected; #")) && !cmd.contains("git -C '/tmp/a'; printf"),
         || cmd.clone());
    // The default runner: a stub `ssh` first on PATH records its command line (local deadline = timeout + 3).
    let bin = TempDir::new("harness-switch-bin-");
    let log = bin.path().join("ssh.args");
    super::stub(&bin.path().join("ssh"), &format!("for a in \"$@\"; do printf '%s\\n' \"$a\"; done > {}\necho multi\n",
        crate::agent_control::pyhelp::shlex_quote(&log.display().to_string())));
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", path_with(bin.path()));
    let got = rd::assert_sub_branch(&request, "multi", None);
    match saved {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
    let ssh: Vec<String> = super::read(&log).lines().map(String::from).collect();
    c.ck("ssh: StrictHostKeyChecking=yes · remote timeout with --kill-after=3s · quoted work dir",
         matches!(&got, Ok(b) if b == "multi") && ssh.iter().any(|a| a.contains("StrictHostKeyChecking=yes"))
             && ssh.last().is_some_and(|l| l.contains("--kill-after=3s") && l.contains(&rd::shell_quote("/tmp/a'; printf injected; #"))),
         || format!("{ssh:?}"));
    let timeout_runner = |_t: &V, _cmd: &str| -> (i32, String, String) { (124, String::new(), "timed out".into()) };
    c.ck("a timed-out read is refused (rc=124)", rejected(rd::assert_sub_branch(&request, "multi", Some(&timeout_runner)), "rc=124"), String::new);
}

/// `instruction_fixture(legacy=)`: an actual builder request in fixture storage; no native/remote execution.
fn instruction_fixture(fx: &Fx, legacy: bool) -> V {
    let mut state = j(r#"{"context_id": "switch", "topology": "multi", "task": "initial approved full task", "attempts": []}"#);
    let request = rd::build_request(&fx.repo(), "multi", &fx.manifest, "initial approved full task", &fx.wall, &fx.opts("codex_cli", |o| {
        o.permission_profile = Some(":danger-full-access".into());
    })).expect("request");
    let rel = "campaigns/_bootstrap/relay/switch.requests/attempt-01.json";
    let path = fx.root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("requests dir");
    std::fs::write(&path, dumps(&request)).expect("request file");
    let mut att = pv::obj(vec![("attempt", V::int(1)), ("provider", s("codex_cli")), ("session_binding", rc::permission_binding(&request)),
        ("request_path", s(rel)), ("dispatch_id", s("fixture-dispatch-1")), ("status", s("input-required")), ("phase", s("inspect")),
        ("session_id", s("fixture-session")), ("running_services", V::List(vec![])), ("budget_outcome", s("within_budget")),
        ("max_turns_allocated", V::None)]);
    if legacy {
        let orel = "campaigns/_bootstrap/relay/switch.outcomes/fixture-dispatch-1.json";
        let outcome = fx.root.join(orel);
        std::fs::create_dir_all(outcome.parent().expect("parent")).expect("outcomes dir");
        std::fs::write(&outcome, dumps(&pv::obj(vec![("schema_version", V::int(1)), ("dispatch_id", s("fixture-dispatch-1")),
            ("request", request.clone()), ("result", pv::empty())]))).expect("outcome");
        set(&mut att, "outcome_path", s(orel));
    } else {
        set(&mut att, "request_digest", s(&pv::sha256_hex(&std::fs::read(&path).unwrap_or_default())));
        let task = py_str(g(&state, "task"));
        set(&mut att, "task_instruction", pv::obj(vec![("schema_version", V::int(1)), ("kind", s("explicit")), ("source_attempt", V::int(1)),
            ("scope", rc::instruction_scope(&state)), ("body", s(&task)), ("body_digest", s(&pv::sha256_hex(task.as_bytes())))]));
    }
    pv::append(pv::get_mut(&mut state, "attempts").expect("attempts"), att);
    state
}

fn first_attempt_mut(v: &mut V) -> &mut V {
    match pv::get_mut(v, "attempts") {
        Some(V::List(l)) => &mut l[0],
        _ => panic!("attempts"),
    }
}

fn instructions(c: &mut Checks) {
    let fx = Fx::new();
    let repo = fx.repo();
    c.group("instruction corruption never falls back to the initial task");
    let state = instruction_fixture(&fx, false);
    let unresolved = |st: &V| rejected(rc::continuation_instruction(&repo, st), "TASK_INSTRUCTION_SOURCE_UNRESOLVED");
    c.ck("intact instruction resolves to the task body",
         rc::continuation_instruction(&repo, &state).is_ok_and(|r| py_str(g(&r, "body")) == py_str(g(&state, "task"))), String::new);
    for (field, value) in [("source_attempt", V::int(99)), ("kind", s("continuation")), ("body", s("changed")), ("body_digest", s("bad")), ("scope", pv::empty())] {
        let mut bad = state.clone();
        if let Some(ti) = pv::get_mut(first_attempt_mut(&mut bad), "task_instruction") {
            set(ti, field, value);
        }
        c.ck(&format!("task_instruction.{field} corrupted → unresolved"), unresolved(&bad), String::new);
    }
    for (field, value) in [("request_digest", V::None), ("request_path", s("campaigns/other/relay/switch.requests/attempt-01.json")),
                           ("provider", s("claude_code")), ("session_binding", pv::empty()), ("task_instruction", s("broken"))] {
        let mut bad = state.clone();
        set(first_attempt_mut(&mut bad), field, value);
        c.ck(&format!("attempt.{field} corrupted → unresolved"), unresolved(&bad), String::new);
    }
    for (field, value) in [("campaign_id", s("another-campaign")), ("context_id", s("another-context")), ("campaign_cell", s("other-cell")),
                           ("control_variables", j(r#"{"model": "other"}"#))] {
        let mut bad = state.clone();
        set(&mut bad, field, value);
        c.ck(&format!("state.{field} changed → unresolved"), unresolved(&bad), String::new);
    }
    let path = fx.root.join(py_str(g(&items(g(&state, "attempts"))[0], "request_path")));
    let original = std::fs::read(&path).unwrap_or_default();
    for (label, raw) in [("not json", b"not-json".to_vec()), ("trailing newline", [original.clone(), b"\n".to_vec()].concat())] {
        std::fs::write(&path, raw).expect("request");
        c.ck(&format!("request file {label} → unresolved"), unresolved(&state), String::new);
    }
    let _ = std::fs::remove_file(&path);
    c.ck("request file missing → unresolved", unresolved(&state), String::new);

    c.group("legacy single requires issued evidence; multi needs a main handoff");
    let fx = Fx::new();
    let repo = fx.repo();
    let state = instruction_fixture(&fx, true);
    let resolved = rc::continuation_instruction(&repo, &state);
    c.ck("legacy resolves · body = task", resolved.as_ref().is_ok_and(|r| pv::truthy(g(r, "legacy")) && py_str(g(r, "body")) == py_str(g(&state, "task"))),
         || format!("{resolved:?}"));
    let mut bad = state.clone();
    set(&mut bad, "task", s("unissued replacement"));
    c.ck("unissued replacement → does not match", rejected(rc::continuation_instruction(&repo, &bad), "does not match"), String::new);
    let mut bad = state.clone();
    let second = with(&items(g(&state, "attempts"))[0], r#"{"attempt": 2}"#);
    pv::append(pv::get_mut(&mut bad, "attempts").expect("attempts"), second);
    c.ck("legacy multiple attempts refused", rejected(rc::continuation_instruction(&repo, &bad), "legacy multiple attempts"), String::new);
    c.ck("technical handoff is not a human recovery question",
         rc::pending_for(&repo, &s("switch")).is_ok_and(|p| p.is_empty()) && !pv::has(&items(g(&state, "attempts"))[0], "recovery_required"), String::new);
    let _ = std::fs::remove_file(fx.root.join(py_str(g(&items(g(&state, "attempts"))[0], "outcome_path"))));
    c.ck("missing legacy outcome refused", rejected(rc::continuation_instruction(&repo, &state), "legacy instruction outcome"), String::new);
}

pub fn run() -> i32 {
    let mut c = Checks::new("selftest_harness_switch");
    lifecycle_echo(&mut c);
    efforts(&mut c);
    native_effort(&mut c);
    relay_cli(&mut c);
    builders(&mut c);
    supervision(&mut c);
    instructions(&mut c);
    c.finish()
}
