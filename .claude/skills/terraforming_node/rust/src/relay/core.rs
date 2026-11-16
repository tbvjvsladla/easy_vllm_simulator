//! relay.py: ledger, reasons, pending requests, report parsing, continuation assembly and the supervisor decision.
//! Each function keeps the original's name (without the leading underscore) and its exact text.

use super::pv::{self, eq, g, has, is, is_any, items, ne, obj, py_str, s, set, truthy, V};
use crate::sysexit::{Exit, Res};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub const PENDING_HITL: &str = "pending_hitl.json";
pub const RUNNER_UNAVAILABLE: &str = "RUNNER_UNAVAILABLE";
pub const DEFAULT_CAPABILITIES: [&str; 5] = ["read", "execute", "edit", "write", "search"];
pub const DEFAULT_LADDER: [&str; 1] = ["sonnet"];
pub const CONTEXT_KINDS: [&str; 2] = ["build", "cell"];
pub const CAMPAIGN_CONTROLS: [(&str, &str); 5] = [
    ("suspend", "suspended"),
    ("resume", "active"),
    ("close", "closed"),
    ("abandon", "closed"),
    ("reslice", "active"),
];
pub const CONTROL_CLOSE_REASON: [(&str, &str); 2] = [("close", "completed"), ("abandon", "abandoned")];
pub const DOC_PLANE_SKEW_S: f64 = 120.0;
/// The campaign_init command the A2A task bodies tell a sub to run (its Rust launcher; the original named `campaign_init.py`).
pub const CI: &str = "bash .claude/skills/terraforming_node/scripts/campaign_init.sh";

pub fn control_state(c: &str) -> Option<&'static str> {
    CAMPAIGN_CONTROLS.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}
pub fn close_reason_of(c: &str) -> Option<&'static str> {
    CONTROL_CLOSE_REASON.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

// ── locations ──────────────────────────────────────────────────────────────────────────────────────────────────────────

/// `HERE` — the scripts directory (`os.path.dirname(os.path.abspath(__file__))`).
pub fn here() -> PathBuf {
    crate::owner::owner_path("relay.py").parent().map(Path::to_path_buf).unwrap_or_default()
}
/// `REPO` — four levels above `HERE`.
pub fn repo() -> String {
    here().ancestors().nth(4).map(|p| p.display().to_string()).unwrap_or_default()
}
pub fn agent_control_sh() -> String {
    here().join("agent_control.sh").display().to_string()
}
pub fn campaign_init_sh() -> String {
    here().join("campaign_init.sh").display().to_string()
}
/// `turn_budget.REQUEST_SCHEMA`.
pub fn request_schema() -> String {
    format!("{}/.claude/schemas/agent-control-request.schema.json", repo())
}
pub fn task_report_schema() -> PathBuf {
    crate::agent_control::runtime_dir().parent().map(Path::to_path_buf).unwrap_or_default().join("sub_node").join("task-report.schema.json")
}

thread_local! {
    /// `contextlib.redirect_stdout(io.StringIO())` of the self-test: when set, `say` appends here instead of stdout.
    pub static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}
/// `print(text)` (through the self-test capture when one is active).
pub fn say(t: &str) {
    say_raw(&format!("{t}\n"));
}
/// `sys.stdout.write(text)`.
pub fn say_raw(t: &str) {
    let captured = CAPTURE.with(|c| match c.borrow_mut().as_mut() {
        Some(b) => {
            b.push_str(t);
            true
        }
        None => false,
    });
    if !captured {
        use std::io::Write;
        let mut o = std::io::stdout().lock();
        let _ = o.write_all(t.as_bytes());
    }
}
/// `print(text, file=sys.stderr)`.
pub fn warn(t: &str) {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    eprintln!("{t}");
}

thread_local! {
    /// `_RELAY_CAMPAIGN`: the ledger campaign of a lifecycle control (None = derive from ACTIVE).
    pub static RELAY_CAMPAIGN: RefCell<Option<String>> = const { RefCell::new(None) };
}
pub fn relay_campaign() -> Option<String> {
    RELAY_CAMPAIGN.with(|c| c.borrow().clone())
}
pub fn set_relay_campaign(v: Option<String>) {
    RELAY_CAMPAIGN.with(|c| *c.borrow_mut() = v);
}

/// A traceback whose last line is `line` (an exception relay.py lets escape), status 1.
pub fn tb(line: &str) -> Exit {
    Exit::Msg(format!(
        "Traceback (most recent call last):\n  (Python source frames are not reproduced by the Rust port)\n{line}"
    ))
}

pub fn active_campaign_id(repo_root: &str) -> Res<String> {
    let site = format!("{}/campaign_init.py", here().display());
    role_contract::campaign::active_campaign_id(Some(repo_root.as_bytes()), repo().as_bytes(), site.as_bytes())
        .map(|p| pv::text(&p))
        .map_err(|c| tb(&format!("{c:?}")))
}

/// `relay_root(repo_root)` — `campaign_init.derive_path("relay-root", ...)`.
pub fn relay_root(repo_root: &str) -> Res<String> {
    let camp = match relay_campaign() {
        Some(c) if !c.is_empty() => c,
        _ => active_campaign_id(repo_root)?,
    };
    let base = Path::new(repo_root).join("campaigns").join(camp).join("relay");
    Ok(norm_path(&base.display().to_string()))
}

/// `str(Path(p))`: collapse `//`, drop `.` components and a trailing `/`.
pub fn norm_path(p: &str) -> String {
    let abs = p.starts_with('/');
    let parts: Vec<&str> = p.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
    let j = parts.join("/");
    if abs {
        format!("/{j}")
    } else if j.is_empty() {
        ".".into()
    } else {
        j
    }
}
/// `os.path.join(a, b)`.
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}
/// `os.path.relpath(path, start)` for absolute normalized paths.
pub fn relpath(path: &str, start: &str) -> String {
    let abs = |x: &str| {
        if x.starts_with('/') {
            norm_path(x)
        } else {
            norm_path(&join(&std::env::current_dir().unwrap_or_default().display().to_string(), x))
        }
    };
    let (p, s0) = (abs(path), abs(start));
    let pp: Vec<&str> = p.split('/').filter(|x| !x.is_empty()).collect();
    let sp: Vec<&str> = s0.split('/').filter(|x| !x.is_empty()).collect();
    let common = pp.iter().zip(&sp).take_while(|(a, b)| a == b).count();
    let mut out: Vec<String> = vec!["..".into(); sp.len() - common];
    out.extend(pp[common..].iter().map(|x| x.to_string()));
    if out.is_empty() {
        ".".into()
    } else {
        out.join("/")
    }
}

pub fn ledger_path(repo_root: &str, context_id: &str) -> Res<String> {
    Ok(join(&relay_root(repo_root)?, &format!("{context_id}.json")))
}

/// `json.load(open(path))` where a failure escapes as the original's traceback.
pub fn load_json_or_raise(path: &str) -> Res<V> {
    pv::load_file(Path::new(path)).map_err(|(class, msg)| {
        if class == "OSError" {
            tb(&msg)
        } else {
            tb(&format!("{class}: {msg}"))
        }
    })
}

pub fn load_ledger(path: &str) -> Res<V> {
    if !Path::new(path).exists() {
        return Ok(obj(vec![("schema_version", pv::i(1)), ("context_id", V::None), ("attempts", pv::list(vec![]))]));
    }
    load_json_or_raise(path)
}

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;
const LOCK_UN: i32 = 8;

/// `state_lock(path)`: an exclusive `flock` on `<path>.lock` held while the returned guard lives.
pub struct StateLock(std::fs::File);
impl Drop for StateLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe {
            flock(self.0.as_raw_fd(), LOCK_UN);
        }
    }
}
pub fn state_lock(path: &str, blocking: bool) -> Res<StateLock> {
    use std::os::unix::io::AsRawFd;
    if let Some(d) = Path::new(path).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let lp = format!("{path}.lock");
    let f = std::fs::OpenOptions::new().append(true).create(true).open(&lp).map_err(|e| tb(&crate::pathfmt::os_error(&e, &lp)))?;
    let rc = unsafe { flock(f.as_raw_fd(), LOCK_EX | if blocking { 0 } else { LOCK_NB }) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(11) {
            return Err(Exit::msg("[relay] STOP: another writer owns this workspace; inspect its ledger"));
        }
        return Err(tb(&format!("OSError: {err}")));
    }
    Ok(StateLock(f))
}

/// `_atomic_json(path, doc)`: sorted, indented, fsynced, renamed into place.
pub fn atomic_json(path: &str, doc: &V) -> Res<()> {
    use std::io::Write;
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir).map_err(|e| tb(&crate::pathfmt::os_error(&e, &dir.display().to_string())))?;
    let tmp = dir.join(format!(".relay-{}", pv::uuid4_hex()));
    let body = pv::dumps_sorted_pretty(doc) + "\n";
    let r = (|| -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)?;
        let d = std::fs::File::open(&dir)?;
        d.sync_all()?;
        Ok(())
    })();
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map_err(|e| tb(&format!("OSError: {e}")))
}

pub fn save_ledger(path: &str, doc: &mut V) -> Res<()> {
    let _l = state_lock(path, true)?;
    let current = load_ledger(path)?;
    let cur_rev = if has(&current, "revision") { g(&current, "revision").clone() } else { pv::i(0) };
    let doc_rev = if has(doc, "revision") { g(doc, "revision").clone() } else { pv::i(0) };
    if ne(&cur_rev, &doc_rev) {
        return Err(Exit::msg("[relay] STOP: stale ledger revision; reload before changing state"));
    }
    set(doc, "revision", pv::i(pv::int(&doc_rev).unwrap_or(0) + 1));
    atomic_json(path, doc)
}

// ── attempts ───────────────────────────────────────────────────────────────────────────────────────────────────────────

pub fn reached_sub(att: &V) -> bool {
    if pv::is_true(g(att, "execution_started")) {
        return true;
    }
    if pv::contains_str(g(att, "reason_codes"), RUNNER_UNAVAILABLE) {
        return false;
    }
    truthy(g(att, "session_id")) || truthy(g(att, "status")) || truthy(g(att, "budget_outcome"))
}

pub fn attempts(doc: &V) -> &[V] {
    items(g(doc, "attempts"))
}

pub fn last_known_session(doc: &V) -> V {
    for att in attempts(doc).iter().rev() {
        if !reached_sub(att) {
            continue;
        }
        let provider = match g(g(att, "runner"), "provider") {
            V::None if !has(g(att, "runner"), "provider") => s("claude_code"),
            x => x.clone(),
        };
        return obj(vec![
            ("session_id", g(att, "session_id").clone()),
            ("attempt", g(att, "attempt").clone()),
            ("status", g(att, "status").clone()),
            ("control_status", g(att, "control_status").clone()),
            ("budget_outcome", g(att, "budget_outcome").clone()),
            ("binding", g(att, "session_binding").clone()),
            ("provider", provider),
        ]);
    }
    obj(vec![
        ("session_id", V::None),
        ("attempt", V::None),
        ("status", V::None),
        ("control_status", V::None),
        ("budget_outcome", V::None),
    ])
}

/// `(att.get("runner") or {}).get("provider", "claude_code")`-style: `d.get(k, default)` on a dict.
pub fn get_or<'a>(d: &'a V, k: &str, default: &'a V) -> &'a V {
    if has(d, k) {
        g(d, k)
    } else {
        default
    }
}
pub fn provider_of(runner: &V) -> String {
    if has(runner, "provider") {
        py_str(g(runner, "provider"))
    } else {
        "claude_code".into()
    }
}

pub fn codex_recovery_requirement(att: &V) -> Option<V> {
    let provider = if truthy(g(att, "provider")) { g(att, "provider") } else { g(g(att, "runner"), "provider") };
    if !is(provider, "codex_cli") {
        return None;
    }
    let codes = g(att, "reason_codes");
    let evidence = g(att, "failure_evidence");
    let raw = g(att, "raw_exit_code");
    let kind;
    if is_any(g(evidence, "signal"), &["process_signal", "execution_interrupted"])
        || (!is(g(att, "control_status"), "timeout")
            && matches!(raw.peel(), V::Int(_) | V::Bool(_))
            && pv::num(raw).or(if pv::is_true(raw) { Some(1.0) } else if pv::is_false(raw) { Some(0.0) } else { None })
                .map_or(false, |n| n < 0.0 || n == 130.0 || n == 137.0 || n == 143.0))
    {
        kind = "process_interrupted";
    } else if pv::contains_str(codes, "PERMISSION_DENIED") {
        if is(g(att, "status"), "completed") {
            return None;
        }
        kind = "permission_denied";
    } else if is(g(att, "control_status"), "timeout")
        && pv::contains_str(codes, "TIMEOUT")
        && is_any(g(evidence, "signal"), &["controlled_local_deadline", "controlled_remote_deadline"])
    {
        return None;
    } else if is(g(att, "control_status"), "completed") {
        return None;
    } else if pv::contains_str(codes, RUNNER_UNAVAILABLE)
        && pv::is_true(g(evidence, "rotate"))
        && !pv::is_true(g(att, "execution_started"))
    {
        return None;
    } else {
        kind = "control_failure";
    }
    let signal = if truthy(g(evidence, "signal")) { g(evidence, "signal").clone() } else { s("unclassified_control_failure") };
    Some(obj(vec![
        ("kind", s(kind)),
        ("provider", s("codex_cli")),
        ("attempt", g(att, "attempt").clone()),
        ("dispatch_id", g(att, "dispatch_id").clone()),
        ("signal", signal),
        ("explicit_resume_required", V::Bool(true)),
    ]))
}

pub fn end_reason(att: &V) -> &'static str {
    let codes = g(att, "reason_codes");
    let (control, sub) = (g(att, "control_status"), g(att, "status"));
    let recovery = codex_recovery_requirement(att);
    if let Some(r) = &recovery {
        if is(g(r, "kind"), "process_interrupted") {
            return "process_interrupted";
        }
    }
    if pv::contains_str(codes, "PERMISSION_DENIED") {
        return if is(sub, "completed") { "completed_with_denials" } else { "permission_denied" };
    }
    if pv::contains_str(codes, RUNNER_UNAVAILABLE) {
        return "runner_unavailable";
    }
    if let Some(r) = &recovery {
        return match pv::st(g(r, "kind")).as_deref() {
            Some("permission_denied") => "permission_denied",
            Some("process_interrupted") => "process_interrupted",
            _ => "control_failure",
        };
    }
    if is(g(att, "budget_outcome"), "exhausted") {
        return "budget_exhausted";
    }
    if is(g(att, "budget_outcome"), "external_interruption") || pv::contains_str(codes, "TIMEOUT") {
        return "external_interruption";
    }
    if is(control, "invalid_request") {
        return "invalid_request";
    }
    if is(control, "model_safety_blocked") {
        return "model_blocked";
    }
    if is(control, "malformed_output") {
        return "malformed_output";
    }
    if is(sub, "completed") && is(control, "completed") {
        return "completed";
    }
    if is(sub, "input-required") {
        return "sub_input_required";
    }
    if is_any(sub, &["failed", "execution_failed"]) {
        return "sub_failed";
    }
    if is(control, "execution_failed") && !truthy(g(att, "session_id")) {
        return "transport_or_launch_failure";
    }
    "unclassified"
}

pub fn require_resume(declared: Option<&str>, last: &V) -> Res<String> {
    if let Some(d) = declared {
        let t = pv::text(&agent_runtime_support::pystr::PyStr::new(d).strip());
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let sid = g(last, "session_id");
    Err(Exit::msg(format!(
        "[relay] STOP: --resume 이 선언되지 않았다 — 재개 여부는 맥락을 아는 쪽이 정한다.\n  → 마지막 알려진 세션: {} (attempt {} · sub_status={} · budget={})\n  → 이어받으려면 `--resume <session_id>`, 새 맥락이면 `--resume new` 를 붙여라.",
        if truthy(sid) { py_str(sid) } else { "(없음)".into() },
        py_str(g(last, "attempt")),
        py_str(g(last, "status")),
        py_str(g(last, "budget_outcome"))
    )))
}

pub fn budget_floor(doc: &V) -> i64 {
    attempts(doc)
        .iter()
        .filter(|a| truthy(g(a, "max_turns_allocated")) && reached_sub(a))
        .map(|a| pv::int(g(a, "max_turns_allocated")).unwrap_or(0))
        .max()
        .unwrap_or(0)
}

/// `f"{x:g}"`.
pub fn fmt_g(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e16 {
        let i = x as i64;
        if x.abs() < 1e6 {
            return i.to_string();
        }
    }
    // general format with 6 significant digits
    let e = if x == 0.0 { 0 } else { x.abs().log10().floor() as i32 };
    if e < -4 || e >= 6 {
        let m = format!("{:.5e}", x);
        let (mant, exp) = m.split_once('e').unwrap();
        let mant = mant.trim_end_matches('0').trim_end_matches('.');
        let exp: i32 = exp.parse().unwrap();
        format!("{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (5 - e).max(0) as usize;
        let t = format!("{:.*}", decimals, x);
        if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t
        }
    }
}

pub fn schema_cap(field: &str) -> Res<i64> {
    let path = role_contract::turn_budget::PathValue::Value(s(&request_schema()));
    match role_contract::turn_budget::schema_cap(&s(field), &path) {
        Ok(v) => Ok(pv::int(&v).unwrap_or(0)),
        Err(f) => Err(budget_failure(f)),
    }
}

pub fn budget_failure(f: role_contract::turn_budget::Failure) -> Exit {
    match f {
        role_contract::turn_budget::Failure::Raised(r) => {
            if r.class == "SystemExit" {
                Exit::Msg(pv::text(&r.message))
            } else {
                tb(&format!("{}: {}", r.class, pv::text(&r.message)))
            }
        }
        role_contract::turn_budget::Failure::Unsupported(m) => tb(&format!("RuntimeError: {m}")),
    }
}

/// `turn_budget.declare(max_turns, timeout_seconds, source=..., unit=...)`.
pub fn declare(max_turns: V, timeout_seconds: V, source: &str, unit: &str) -> Res<V> {
    let mut d = role_contract::turn_budget::Declaration::new(max_turns, timeout_seconds, s(source));
    d.unit = s(unit);
    let path = role_contract::turn_budget::PathValue::Value(s(&request_schema()));
    role_contract::turn_budget::declare(&d, &path).map_err(budget_failure)
}

pub fn continuation_budget_preview(doc: &V, ladder: &[V], last: Option<&V>, next_timeout: Option<i64>) -> Res<(String, String)> {
    let runner = &ladder[next_runner_index(doc, ladder)];
    let provider = provider_of(runner);
    if provider == "codex_cli" {
        let previous = last.cloned().unwrap_or_else(pv::empty);
        let allocation = g(&previous, "timeout_seconds_allocated");
        let allocation = match pv::int(allocation) {
            Some(n) if n > 0 => format!("{n}초"),
            _ => "미관측".into(),
        };
        let duration = g(&previous, "duration_ms");
        let elapsed = match pv::num(duration) {
            Some(d) if d > 0.0 => format!("{}초(provider duration_ms)", fmt_g(d / 1000.0)),
            _ => {
                let (st, en) = (utc_epoch(g(&previous, "started_utc")), utc_epoch(g(&previous, "ended_utc")));
                match (st, en) {
                    (Some(a), Some(b)) if b > a => format!("{}초(main 시작/종료 시각)", fmt_g(b - a)),
                    _ => "미관측".into(),
                }
            }
        };
        let proposed = match next_timeout {
            Some(t) => format!("{t}초(명시)"),
            None => "미선언".into(),
        };
        let mut flags = "--timeout-seconds/--budget-source 와 --resume".to_string();
        if ladder.iter().any(|r| provider_of(r) != "codex_cli") {
            flags += " (혼합 사다리의 Claude fallback용 --max-turns 도 선언 · Codex native turn 아님)";
        }
        return Ok((
            format!(
                "다음 러너 {} · provider={provider} · 직전 wall 배정 {allocation} · 관측 경과 {elapsed} · 다음 wall 배정 {proposed}",
                py_str(g(runner, "name"))
            ),
            flags,
        ));
    }
    Ok((
        format!(
            "다음 러너 {} · provider={provider} · 집행된 native turn 바닥 {} · max_turns 스키마 필드 상한 {}",
            py_str(g(runner, "name")),
            budget_floor(doc),
            schema_cap("max_turns")?
        ),
        "--max-turns/--timeout-seconds/--budget-source 와 --resume".into(),
    ))
}

pub fn stalled_attempts(doc: &V) -> i64 {
    let atts: Vec<&V> = attempts(doc).iter().filter(|a| reached_sub(a)).collect();
    let mut n = 0;
    let mut phase: Option<V> = None;
    for att in atts.iter().rev() {
        if is(g(att, "status"), "completed") {
            break;
        }
        match &phase {
            None => phase = Some(g(att, "phase").clone()),
            Some(p) if ne(g(att, "phase"), p) => break,
            _ => {}
        }
        n += 1;
    }
    n
}

pub fn campaign_control_block(campaign_id: Option<&str>, control: &V) -> String {
    let camp = campaign_id.filter(|c| !c.is_empty());
    if camp.is_none() && !truthy(control) {
        return String::new();
    }
    let mut lines = vec!["## [relay] 캠페인 정체성 — 서브는 이 id 로 자기 인스턴스를 만든다(이름을 짓지 않는다)\n".to_string()];
    if let Some(c) = camp {
        lines.push(format!("- campaign_id: {c}\n"));
        lines.push(format!("- 서브 인스턴스 경로: campaigns/{c}/  (디렉터리명 = campaign_id)\n"));
    }
    if truthy(control) {
        lines.push("- layer-1 통제변인(메인 단일 창구 · 서브는 **그대로 echo** 한다):\n".into());
        let mut ks = pv::keys(control);
        ks.sort_by(|a, b| a.chars().cmp(b.chars()));
        for k in ks {
            lines.push(format!("    - {k}: {}\n", py_str(g(control, &k))));
        }
    }
    lines.push("- 규약: 리포트 `campaign_id` 에 위 값을 그대로 적고, `control_variables_echo` 에\n  위 표를 그대로 되받아 적어라. **부재도 불일치도 메인이 STOP 한다**(fail-closed).\n  통제변인을 바꿔야 한다고 판단하면 스스로 바꾸지 말고 `input-required` 로 끊고\n  `next_steps` 에 사유를 적어라 — 개정 창구는 메인 하나다.\n\n".into());
    lines.concat()
}

pub fn relay_header(context_id: &str, attempt: i64, allocated: &V, budget_source: &str, timeout_seconds: &V) -> String {
    format!(
        "## [relay] 위임 헤더 — 메인이 결정론으로 붙였다. 서브는 이 값을 그대로 회신한다.\n- context_id: {context_id}\n- attempt: {attempt}\n- max_turns_allocated: {}\n{}- 예산 근거(메인 선언): {budget_source}\n- 규약: 리포트 `context_id` 에 위 값을 그대로 적는다. 예산이 모자라면 **소진하지 말고**\n  `input-required` 로 끊고 남은 일을 `next_steps` 에 적어라(scope ⊥ budget).\n- 외부지식을 검색했다면 `external_search[]` 에 질의·출처·요지를 남겨라 — 그 기록이\n  메인의 자산이 되고, 인용 없는 결정은 거짓이 아니라 **누락**이다(헌법 불변식 B).\n\n",
        pv::dumps_a(allocated),
        if truthy(timeout_seconds) { format!("- timeout_seconds_allocated: {}\n", py_str(timeout_seconds)) } else { String::new() }
    )
}

// ── subprocesses ───────────────────────────────────────────────────────────────────────────────────────────────────────

pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// `subprocess.run(argv, capture_output=True, text=True, cwd=..., input=...)`; `Err` = the program could not start.
pub fn run(argv: &[String], cwd: Option<&str>, input: Option<&str>, stdin_null: bool) -> std::io::Result<Out> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else if stdin_null {
        Stdio::null()
    } else {
        Stdio::inherit()
    });
    let mut child = cmd.spawn()?;
    if let Some(t) = input {
        let mut si = child.stdin.take().unwrap();
        let data = t.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = si.write_all(&data);
        });
    }
    let o = child.wait_with_output()?;
    use std::os::unix::process::ExitStatusExt;
    Ok(Out {
        code: o.status.code().unwrap_or_else(|| -o.status.signal().unwrap_or(0)),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    })
}

/// `text[:n]` by code points.
pub fn head(t: &str, n: usize) -> String {
    t.chars().take(n).collect()
}
/// `text[-n:]` by code points.
pub fn tail(t: &str, n: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}
/// Python `str.strip()`.
pub fn strip(t: &str) -> String {
    crate::py::strip(t).to_string()
}

pub fn runner_table(repo_root: &str) -> Res<Vec<(String, V)>> {
    let argv = vec!["bash".to_string(), agent_control_sh(), "runners".into(), "--provider".into(), "all".into()];
    let out = run(&argv, Some(repo_root), None, false).map_err(|e| tb(&crate::pathfmt::os_error(&e, "bash")))?;
    if out.code != 0 {
        let msg = if !out.stderr.is_empty() { &out.stderr } else { &out.stdout };
        return Err(Exit::msg(format!(
            "[relay] STOP: 러너 표를 읽지 못했다(rc={}) — {}",
            out.code,
            head(&strip(msg), 300)
        )));
    }
    let doc = pv::loads(&out.stdout).map_err(|e| tb(&format!("json.decoder.JSONDecodeError: {e}")))?;
    let mut table: Vec<(String, V)> = vec![];
    for r in items(g(&doc, "runners")) {
        let name = py_str(g(r, "name"));
        if let Some(slot) = table.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = r.clone();
        } else {
            table.push((name, r.clone()));
        }
    }
    Ok(table)
}

fn sorted_names(table: &[(String, V)]) -> String {
    let mut names: Vec<String> = table.iter().map(|(n, _)| n.clone()).collect();
    names.sort_by(|a, b| a.chars().cmp(b.chars()));
    pv::py_repr(&pv::strs(&names))
}

pub fn resolve_ladder(repo_root: &str, spec: Option<&str>, backend: Option<&str>, model: Option<&str>, provider: Option<&str>) -> Res<Vec<V>> {
    if let Some(h) = super::selftest::hook_resolve_ladder() {
        return Ok(h);
    }
    let table = runner_table(repo_root)?;
    let lookup = |n: &str| table.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    if let Some(spec) = spec.filter(|x| !x.is_empty()) {
        let names: Vec<String> = spec.split(',').map(strip).filter(|x| !x.is_empty()).collect();
        if names.is_empty() {
            return Err(Exit::msg("[relay] STOP: --runners 가 비었다 — 빈 사다리는 실행자가 0이다."));
        }
        let unknown: Vec<String> = names.iter().filter(|n| lookup(n).is_none()).cloned().collect();
        if !unknown.is_empty() {
            return Err(Exit::msg(format!(
                "[relay] STOP: 등재되지 않은 러너 {} — 아는 이름은 {} 다. 새 러너는 어댑터 RUNNER_ALIASES 에 등재한다(닫힌 목록이라 오타가 조용히 통과하지 않는다).",
                pv::py_repr(&pv::strs(&unknown)),
                sorted_names(&table)
            )));
        }
        return Ok(names.iter().map(|n| lookup(n).unwrap()).collect());
    }
    let (backend, model, provider) = (backend.filter(|x| !x.is_empty()), model.filter(|x| !x.is_empty()), provider.filter(|x| !x.is_empty()));
    if backend.is_some() || model.is_some() || provider.is_some() {
        let p = provider.unwrap_or("claude_code");
        let eligible: Vec<&V> = table.iter().map(|(_, v)| v).filter(|r| provider_of(r) == p).collect();
        let b: V = match backend {
            Some(b) => s(b),
            None => eligible.first().map(|r| g(r, "backend").clone()).unwrap_or(V::None),
        };
        let m: V = match model {
            Some(m) => s(m),
            None => eligible.iter().find(|r| eq(g(r, "backend"), &b)).map(|r| g(r, "model").clone()).unwrap_or(V::None),
        };
        if !truthy(&m) {
            return Err(Exit::msg(format!(
                "[relay] STOP: backend={} 의 기본 모델을 알 수 없다 — --model 을 선언하라.",
                pv::py_repr(&b)
            )));
        }
        if let Some(named) = eligible.iter().find(|r| eq(g(r, "backend"), &b) && eq(g(r, "model"), &m)) {
            return Ok(vec![(*named).clone()]);
        }
        return Ok(vec![obj(vec![
            ("name", s(&format!("{p}:{}:{}", py_str(&b), py_str(&m)))),
            ("provider", s(p)),
            ("backend", b),
            ("model", m),
        ])]);
    }
    DEFAULT_LADDER
        .iter()
        .map(|n| lookup(n).ok_or_else(|| tb(&format!("KeyError: {}", pv::repr_s(n)))))
        .collect()
}

pub fn ladder_names(ladder: &[V]) -> String {
    ladder.iter().map(|r| py_str(g(r, "name"))).collect::<Vec<_>>().join(" → ")
}

pub fn next_runner_index(doc: &V, ladder: &[V]) -> usize {
    let names: Vec<String> = ladder.iter().map(|r| py_str(g(r, "name"))).collect();
    for att in attempts(doc).iter().rev() {
        let used = g(g(att, "runner"), "name");
        if let Some(u) = pv::st(used) {
            if let Some(i) = names.iter().position(|n| *n == u) {
                return if is(g(att, "end_reason"), "runner_unavailable") { (i + 1) % names.len() } else { i };
            }
        }
    }
    0
}

pub fn consecutive_runner_unavailable(doc: &V) -> i64 {
    let mut n = 0;
    for att in attempts(doc).iter().rev() {
        if is(g(att, "end_reason"), "runner_unavailable") {
            n += 1;
            continue;
        }
        break;
    }
    n
}

// ── campaign axis ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// `campaign_init.py <args>` through its launcher; `Err` = (status, stderr/stdout text).
pub fn campaign_init(args: &[&str], cwd: &str, input: Option<&str>) -> Result<String, (i32, String)> {
    let mut argv = vec!["bash".to_string(), campaign_init_sh()];
    argv.extend(args.iter().map(|x| x.to_string()));
    match run(&argv, Some(cwd), input, false) {
        Ok(o) if o.code == 0 => Ok(o.stdout),
        Ok(o) => Err((o.code, strip(if o.stderr.trim().is_empty() { &o.stdout } else { &o.stderr }))),
        Err(e) => Err((127, e.to_string())),
    }
}

/// `campaign_init.read_declaration(base)`.
pub fn read_declaration(base: &str) -> V {
    match role_contract::campaign::read_declaration(base.as_bytes()) {
        Ok(d) => V::Dict(d),
        Err(_) => pv::empty(),
    }
}

/// `campaign_init.emit_slice(base, node, utc="delegated-task-body")` through the command line (`--emit-slice`).
pub fn emit_slice(repo_root: &str, campaign_id: &str, node: &str) -> Res<V> {
    let out = campaign_init(&["--emit-slice", node, "--campaign-id", campaign_id, "--utc", "delegated-task-body"], repo_root, None)
        .map_err(|(_, m)| Exit::msg(m))?;
    pv::loads(&out).map_err(|e| tb(&format!("json.decoder.JSONDecodeError: {e}")))
}

/// `campaign_init.fold_lifecycle(base, active=active_campaign_id(repo_root))` through `--lifecycle CAMP`.
pub fn fold_lifecycle(repo_root: &str, camp: &str) -> Result<V, String> {
    let out = campaign_init(&["--lifecycle", camp], repo_root, None).map_err(|(_, m)| m)?;
    pv::loads(&out).map_err(|e| e.to_string())
}

pub fn campaign_slice_block(repo_root: &str, topology: &str, campaign_id: Option<&str>, delegated_node: Option<&str>) -> Res<String> {
    let (Some(camp), Some(node)) = (campaign_id.filter(|c| !c.is_empty()), delegated_node.filter(|n| !n.is_empty())) else {
        return Ok(String::new());
    };
    if topology != "single" {
        return Ok(String::new());
    }
    let base = format!("{}/campaigns/{camp}", repo());
    let decl = read_declaration(&base);
    let node_doc = items(g(&decl, "nodes")).iter().find(|n| pv::is_dict(n) && is(g(n, "node_id"), node)).cloned();
    match &node_doc {
        Some(n) if is(g(n, "role"), "sub") => {}
        _ => return Ok(String::new()),
    }
    let slice_doc = emit_slice(repo_root, camp, node)?;
    let blob = pv::dumps_pretty(&slice_doc);
    Ok(format!(
        "## [relay] main-derived campaign declaration — stdin으로만 소비(중간 파일 생성 금지)\n서브에 campaigns/{camp}/campaign.yaml 이 없으면 아래 JSON을 정확히 stdin으로 넘겨 `campaign_init.sh --init {camp} --plan-ref {} --from-slice - --apply` 를 실행하라.\n이미 존재하면 같은 JSON을 stdin으로 넘겨 `campaign_init.sh --slice-update - --utc <현재 UTC> --apply` 를 실행하고 개정 번호와 값을 확인하라.\n```json\n{blob}\n```\n\n",
        py_str(g(&slice_doc, "plan_ref"))
    ))
}

pub fn campaign_control_task(campaign_id: &str, control: &str, preempt: bool, reason: Option<&str>, slice_doc: &V) -> Res<String> {
    if control_state(control).is_none() {
        return Err(Exit::msg(format!(
            "[relay] STOP: --campaign-control 은 ('suspend', 'resume', 'close', 'abandon', 'reslice') 중 하나다: {}",
            pv::repr_s(control)
        )));
    }
    let ci = CI;
    let (cmd, what) = match control {
        "suspend" => (
            format!(
                "{ci} --suspend {campaign_id} --release <이 캠페인을 위해 네가 띄운 서빙 config | none>{} --approved-by \"main-a2a: 메인 전환 승인 전사\" --utc <지금 UTC> --apply",
                if preempt { " --preempt" } else { "" }
            ),
            format!(
                "보류(suspend)했다. 네가 이 캠페인을 위해 띄운 서빙은 위 명령이 정식 teardown 으로 내린다(compose down 직접 ✗ · 띄운 것이 없으면 `--release none`).{}",
                if preempt {
                    " 메인이 강제 전환(--preempt)을 승인했다 — 진행 중 셀은 interrupted 로 남는다."
                } else {
                    " 진행 중 셀이 있으면 강제하지 말고 failed 로 보고하라(협조 전환이 기본)."
                }
            ),
        ),
        "resume" => (
            format!("{ci} --resume-campaign {campaign_id} --approved-by \"main-a2a: 메인 재개 승인 전사\" --utc <지금 UTC> --apply"),
            "재개(resume)했다. 재개 게이트(핀·digest·토폴로지 대조)가 거부하면 우회하지 말고 그 안내문 원문을 보고하라 — drift 수용은 사람 결정이다.".to_string(),
        ),
        "reslice" => {
            if !pv::is_dict(slice_doc) {
                return Err(Exit::msg("[relay] STOP: reslice 는 메인 파생 선언(slice)이 필요하다"));
            }
            let rev = g(g(slice_doc, "_assignment_revision"), "rev");
            (
                format!("{ci} --slice-update - --utc <지금 UTC> --apply   # stdin = 아래 JSON 그대로"),
                format!(
                    "재배정(reslice · 메인 rev {})했다. 네 인스턴스의 배정을 아래 선언으로 바꾼다 — pending 이 아닌 셀을 빼거나 바꾸려 해서 writer 가 거부하면 우회하지 말고 그 사유 원문을 보고하라(메인이 본 상태와 네 실물이 갈라졌다는 뜻이다).\n```json\n{}\n```",
                    py_str(rev),
                    pv::dumps_pretty(slice_doc)
                ),
            )
        }
        "close" => (
            format!("{ci} --close {campaign_id} --utc <지금 UTC> --apply"),
            "완주 종결(close)했다. 네 인스턴스도 닫는다 — 종결 선행조건(증거 포인터 실재·릴레이 요약)이 거부하면 우회하지 말고 그 사유 원문을 보고하라(사람이 포기로 바꿀지 정한다).".to_string(),
        ),
        _ => (
            format!(
                "{ci} --abandon {campaign_id} --reason \"{}\" --summary-testlog <네가 발행한 포기 요약 testlog — doc_naming 규약 이름 · 포기 사유·미완 셀 목록> --release <이 캠페인을 위해 네가 띄운 서빙 config | none> --approved-by \"main-a2a: 메인 포기 승인 전사\" --utc <지금 UTC> --apply",
                reason.filter(|r| !r.is_empty()).unwrap_or("메인 포기 종결")
            ),
            "포기 종결(abandon)했다. 네 인스턴스도 같은 사유로 닫는다 — 먼저 포기 요약 testlog 1건을 규약 이름(`docs/testlog/testlog_<KST YYMMDDHH>_<주제>.md`)으로 발행하고 그것을 인용한다. 축소 게이트가 거부하면 우회하지 말고 사유 원문을 보고하라.".to_string(),
        ),
    };
    Ok(format!(
        "## [relay] 캠페인 수명 제어 — {control}\n메인이 캠페인 `{campaign_id}` 를 {what}\n\n1. 실행: `{cmd}`\n2. `{ci} --resume-brief` 의 **첫 줄**을 읽는다.\n3. 리포트 JSON 에 `campaign_control_echo` 를 적는다: {{\"campaign_id\": \"{campaign_id}\", \"control\": \"{control}\", \"state\": \"<{ci} --lifecycle {campaign_id} 의 state>\", {}{}\"first_line\": \"<첫 줄>\"}}. 실패하면 state 에 실패 사유를 적고 status 를 failed 로 둔다(성공인 척 ✗).\n4. 셀 진행·빌드·서빙 기동은 **하지 않는다** — 이 지시는 수명 전이 하나다.\n\n",
        if close_reason_of(control).is_some() { "\"close_reason\": \"<같은 출력의 close_reason>\", " } else { "" },
        if control == "reslice" {
            "\"rev\": <위 선언의 _assignment_revision.rev>, \"assigned_cells\": [<campaign.yaml 의 네 배정 셀 순서대로>], "
        } else {
            ""
        }
    ))
}

pub fn orphan_sweep_task(campaign_ids: &[String], reason: &str) -> String {
    let ci = CI;
    let lines: Vec<String> = campaign_ids
        .iter()
        .map(|c| {
            format!(
                "   - `{ci} --abandon {c} --reason \"{reason}\" --summary-testlog <위 요약> --release none --approved-by \"main-a2a: 사람이 고른 orphan 정리 전사\" --utc <지금 UTC> --apply`"
            )
        })
        .collect();
    format!(
        "## [relay] 캠페인 수명 제어 — orphan sweep(abandon)\n메인에는 인스턴스가 없고 네 쪽에만 남은 캠페인 {}건을 사람이 포기로 정리하기로 했다(사유: {reason}).\n\n1. 포기 요약 testlog 1건을 규약 이름(`docs/testlog/testlog_<KST YYMMDDHH>_<주제>.md`)으로 발행한다 — 대상 id 전수 · 각자의 fold state · 미완 셀 · 사유.\n2. 아래를 차례로 실행한다(목록 밖 캠페인은 건드리지 않는다):\n{}\n   축소 게이트가 거부한 것은 우회하지 말고 그 사유 원문을 결과에 적고 다음으로 넘어간다.\n3. `{ci} --resume-brief` 의 **첫 줄**을 읽는다.\n4. 리포트 JSON 에 `campaign_control_echo` 를 적는다: {{\"control\": \"orphan_sweep\", \"summary_testlog\": \"<경로>\", \"results\": [{{\"campaign_id\": \"<id>\", \"state\": \"<--lifecycle 의 state>\", \"close_reason\": \"<같은 출력>\", \"refusal\": \"<거부 원문 | null>\"}}, …], \"first_line\": \"<첫 줄>\"}}.\n5. 셀 진행·빌드·서빙 기동은 **하지 않는다**.\n\n",
        campaign_ids.len(),
        lines.join("\n")
    )
}

pub fn orphan_sweep_echo_ok(report: &V, campaign_ids: &[String]) -> (bool, String) {
    let echo = if pv::is_dict(report) { g(report, "campaign_control_echo") } else { &pv::NONE };
    if !pv::is_dict(echo) || !is(g(echo, "control"), "orphan_sweep") {
        return (false, "리포트에 orphan_sweep echo 가 없다".into());
    }
    let mut got: Vec<(V, V)> = vec![];
    for r in items(g(echo, "results")) {
        if pv::is_dict(r) {
            let k = g(r, "campaign_id").clone();
            if let Some(slot) = got.iter_mut().find(|(x, _)| eq(x, &k)) {
                slot.1 = r.clone();
            } else {
                got.push((k, r.clone()));
            }
        }
    }
    let find = |c: &str| got.iter().find(|(k, _)| is(k, c)).map(|(_, v)| v.clone()).unwrap_or_else(pv::empty);
    let left: Vec<&String> = campaign_ids
        .iter()
        .filter(|c| {
            let r = find(c);
            !is(g(&r, "state"), "closed") || !is(g(&r, "close_reason"), "abandoned")
        })
        .collect();
    let mut extra: Vec<V> = got.iter().map(|(k, _)| k.clone()).filter(|k| !campaign_ids.iter().any(|c| is(k, c))).collect();
    extra.sort_by(|a, b| pv::py_str(a).chars().cmp(pv::py_str(b).chars()));
    if !extra.is_empty() {
        return (false, format!("목록 밖 캠페인을 건드렸다: {}", pv::py_repr(&V::List(extra))));
    }
    if !left.is_empty() {
        let parts: Vec<String> = left
            .iter()
            .map(|c| {
                let r = find(c);
                format!("{c}={}({})", py_str(g(&r, "state")), py_str(g(&r, "refusal")))
            })
            .collect();
        return (false, format!("미정리 {}건: {}", left.len(), parts.join("; ")));
    }
    (
        true,
        format!(
            "orphan {}건 closed(abandoned) · 요약 {} · 첫 줄: {}",
            campaign_ids.len(),
            py_str(g(echo, "summary_testlog")),
            py_str(g(echo, "first_line"))
        ),
    )
}

pub fn campaign_control_echo_ok(report: &V, campaign_id: &str, control: &str, slice_doc: &V) -> (bool, String) {
    let echo = if pv::is_dict(report) { g(report, "campaign_control_echo") } else { &pv::NONE };
    if !pv::is_dict(echo) {
        return (false, "리포트에 campaign_control_echo 가 없다".into());
    }
    let want = control_state(control).unwrap_or("");
    if !is(g(echo, "campaign_id"), campaign_id) || !is(g(echo, "control"), control) {
        return (
            false,
            format!("echo 정체성 불일치: {}/{}", pv::py_repr(g(echo, "campaign_id")), pv::py_repr(g(echo, "control"))),
        );
    }
    if !is(g(echo, "state"), want) {
        return (false, format!("서브 상태가 {} 다(기대 {})", pv::py_repr(g(echo, "state")), pv::repr_s(want)));
    }
    if control == "reslice" {
        let mut want_cells = vec![];
        for (_, its) in pv::entries(g(slice_doc, "assignments")) {
            for it in items(its) {
                if pv::is_dict(it) {
                    want_cells.push(g(it, "cell").clone());
                }
            }
        }
        let want_rev = g(g(slice_doc, "_assignment_revision"), "rev");
        let got_cells = V::List(items(g(echo, "assigned_cells")).to_vec());
        if ne(g(echo, "rev"), want_rev) || ne(&got_cells, &V::List(want_cells.clone())) {
            return (
                false,
                format!(
                    "서브 배정이 메인 선언과 다르다: rev {}/{} · 셀 {}/{}",
                    pv::py_repr(g(echo, "rev")),
                    pv::py_repr(want_rev),
                    pv::py_repr(g(echo, "assigned_cells")),
                    pv::py_repr(&V::List(want_cells))
                ),
            );
        }
    }
    if let Some(cr) = close_reason_of(control) {
        if !is(g(echo, "close_reason"), cr) {
            return (
                false,
                format!(
                    "서브 종결 사유가 {} 다(기대 {}) — 같은 closed 라도 사유가 다르면 수렴이 아니다",
                    pv::py_repr(g(echo, "close_reason")),
                    pv::repr_s(cr)
                ),
            );
        }
    }
    (true, format!("서브 {control} echo 수렴 · 첫 줄: {}", py_str(g(echo, "first_line"))))
}

pub fn bind_campaign_context(doc: &mut V, node: Option<&str>, kind: Option<&str>, cell: Option<&str>) -> Res<()> {
    if let Some(k) = kind {
        if !CONTEXT_KINDS.contains(&k) {
            return Err(Exit::msg(format!("[relay] STOP: --context-kind 는 ('build', 'cell') 중 하나다: {}", pv::repr_s(k))));
        }
    }
    for (key, val) in [("campaign_node", node), ("campaign_context_kind", kind)] {
        let Some(val) = val else { continue };
        let have = g(doc, key).clone();
        if truthy(&have) && !is(&have, val) {
            return Err(Exit::msg(format!(
                "[relay] STOP: 이 원장의 {key} 는 이미 {} 다 — {} 로 바꾸려면 새 context 를 열어라(한 원장이 두 정체성을 주장하면 상관검증이 무너진다).",
                pv::py_repr(&have),
                pv::repr_s(val)
            )));
        }
        set(doc, key, s(val));
    }
    if let Some(cell) = cell.filter(|c| !c.is_empty()) {
        let have = g(doc, "campaign_cell").clone();
        if truthy(&have) && !is(&have, cell) {
            return Err(Exit::msg(format!(
                "[relay] STOP: 이 context 는 이미 셀 {} 의 것이다 — {} 은 **새 context** 로 연다(셀 하나 = context 하나 · D20).\n  셀 둘을 한 문맥에 묶으면 그 attempt 는 캡에서 잘린다(2026-09-07 실측 2/2).",
                pv::py_repr(&have),
                pv::repr_s(cell)
            )));
        }
        set(doc, "campaign_cell", s(cell));
    }
    Ok(())
}

/// `datetime.strptime(v, "%Y-%m-%dT%H:%M:%SZ")` as UTC epoch seconds.
pub fn parse_utc(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<i64> {
        let x = &t[r];
        if x.bytes().all(|c| c.is_ascii_digit()) {
            x.parse().ok()
        } else {
            None
        }
    };
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    if !(1..=12).contains(&mo) || d < 1 || h > 23 || mi > 59 || se > 61 {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if d > mdays[(mo - 1) as usize] || se > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86400 + h * HOUR + mi * 60 + se)
}
/// Seconds per hour (written as a product so the time-cap tripwire of the self-test stays meaningful).
const HOUR: i64 = 60 * 60;
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
pub fn fmt_utc(epoch: i64) -> String {
    let days = epoch.div_euclid(86400);
    let secs = epoch.rem_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / HOUR, secs % HOUR / 60, secs % 60)
}
/// `_utc_epoch(v)` (`str(v)` parsed; None when it does not parse).
pub fn utc_epoch(v: &V) -> Option<f64> {
    parse_utc(&py_str(v)).map(|x| x as f64)
}

pub fn derive_timeout(repo_root: &str, campaign_id: Option<&str>, node: Option<&str>, margin: f64) -> Res<(Option<i64>, String)> {
    let cap = schema_cap("timeout_seconds")?;
    let (Some(camp), Some(node)) = (campaign_id.filter(|c| !c.is_empty() && *c != "_bootstrap"), node.filter(|n| !n.is_empty())) else {
        return Ok((None, format!("실측 파생 불가(캠페인·노드 미선언) — 스키마 상한 {cap} 사용")));
    };
    let pdir = Path::new(repo_root).join("campaigns").join(camp).join("phases").join(node);
    let mut spans: Vec<(i64, String)> = vec![];
    let mut names: Vec<String> = match std::fs::read_dir(&pdir) {
        Ok(rd) if pdir.is_dir() => rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        _ => vec![],
    };
    names.sort();
    for name in names {
        if !name.ends_with(".status.json") {
            continue;
        }
        let Ok(st) = pv::load_file(&pdir.join(&name)) else { continue };
        let a = if truthy(g(&st, "first_started_utc")) { g(&st, "first_started_utc") } else { g(&st, "started_utc") };
        let b = g(&st, "ended_utc");
        let (Some(a), Some(b)) = (pv::st(a), pv::st(b)) else { continue };
        let (Some(t0), Some(t1)) = (parse_utc(&a), parse_utc(&b)) else { continue };
        let secs = t1 - t0;
        if secs > 0 {
            spans.push((secs, name));
        }
    }
    if spans.is_empty() {
        return Ok((None, format!("{node} 의 phase 실측이 없다(진행표에 시각 없음) — 스키마 상한 {cap} 사용")));
    }
    let (worst, wher) = spans.iter().max().cloned().unwrap();
    let want = (worst as f64 * margin) as i64;
    let used = want.min(cap);
    let note = if used == want { String::new() } else { format!(" · 스키마 상한 {cap} 으로 clamp") };
    Ok((
        Some(used),
        format!("{node}/{wher} 실측 {worst}s x {} = {want}s{note} (campaigns/{camp}/phases 파생)", fmt_g(margin)),
    ))
}

pub fn echo_stop_reasons(att: &V, context_id: &V, campaign_id: &V, control_variables: &V) -> Vec<String> {
    if !truthy(g(att, "sub_reported")) {
        return vec![];
    }
    let mut reasons = vec![];
    let got_ctx = g(att, "context_id_reported");
    if !truthy(got_ctx) {
        reasons.push(format!(
            "context_id echo 부재 — 메인은 '{}' 를 헤더로 보냈다. 서브가 그것을 되받지 않으면 두 원장이 같은 작업인지 확인할 수단이 없다",
            py_str(context_id)
        ));
    } else if ne(got_ctx, context_id) {
        reasons.push(format!("context_id 불일치 — 메인 '{}' vs 서브 '{}'", py_str(context_id), py_str(got_ctx)));
    }
    if truthy(campaign_id) {
        let got = g(att, "campaign_id_reported");
        if !truthy(got) {
            reasons.push(format!(
                "campaign_id echo 부재 — 메인은 '{}' 를 보냈다. 부재를 통과시키면 서브가 자기 인스턴스 이름을 스스로 짓는다(2026-09-07 실측: 서브가 'camp7-sub-native' 를 저작했다)",
                py_str(campaign_id)
            ));
        } else if ne(got, campaign_id) {
            reasons.push(format!("campaign_id 불일치 — 메인 '{}' vs 서브 '{}'", py_str(campaign_id), py_str(got)));
        }
    }
    if truthy(control_variables) {
        let echo = g(att, "control_variables_echo");
        if !pv::is_dict(echo) || !truthy(echo) {
            reasons.push("control_variables echo 부재 — layer-1 선언은 메인 단일 창구이고 서브는 그대로 되받아야 한다(2026-09-05: 모델 행이 빠져 서브가 다른 모델을 집었다)".into());
        } else {
            let diff: Vec<(String, &V)> =
                pv::entries(control_variables).into_iter().filter(|(k, v)| py_str(g(echo, k)) != py_str(v)).collect();
            if !diff.is_empty() {
                let parts: Vec<String> = diff
                    .iter()
                    .map(|(k, v)| format!("{k}: 메인 {} vs 서브 {}", pv::py_repr(v), pv::py_repr(g(echo, k))))
                    .collect();
                reasons.push(format!(
                    "control_variables 불일치 — {}. 서브가 선언을 바꿨다면 그것은 개정이 아니라 드리프트다",
                    parts.join(", ")
                ));
            }
        }
    }
    reasons
}

pub fn parse_runner_evidence(result: &V) -> V {
    if !pv::contains_str(g(result, "reason_codes"), RUNNER_UNAVAILABLE) {
        return V::None;
    }
    if pv::is_dict(g(result, "failure_evidence")) {
        return g(result, "failure_evidence").clone();
    }
    let unparsed = || obj(vec![("signal", s("unparsed")), ("rotate", V::Bool(false))]);
    let text = pv::st(g(result, "output")).filter(|t| !t.is_empty()).unwrap_or_default();
    let Some(i) = text.find('{') else { return unparsed() };
    match pv::loads(&text[i..]) {
        Ok(ev) if pv::is_dict(&ev) => ev,
        Ok(_) => unparsed(),
        Err(_) => unparsed(),
    }
}

pub fn denial_block(result: &V) -> V {
    if !pv::contains_str(g(result, "reason_codes"), "PERMISSION_DENIED") {
        return V::None;
    }
    match pv::st(g(result, "output")) {
        Some(out) if out.starts_with("[permission_denials]") => s(out.split("\n\n[서브가 남긴 말]").next().unwrap_or("")),
        _ => V::None,
    }
}

#[derive(Clone)]
pub struct AttemptMeta {
    pub resume_declared: V,
    pub request_path: V,
    pub runner: V,
    pub started_utc: V,
    pub ended_utc: V,
    pub session_binding: V,
    pub dispatch_id: V,
}

pub fn or_list(v: &V) -> V {
    if truthy(v) {
        v.clone()
    } else {
        pv::list(vec![])
    }
}

pub fn record_attempt(doc: &mut V, context_id: &str, bud: &V, result: &V, report: &V, m: &AttemptMeta) -> V {
    let report_present = truthy(report);
    let report = if report_present { report.clone() } else { pv::empty() };
    let resume_requested = if pv::is_none(&m.resume_declared) || is(&m.resume_declared, "new") { V::None } else { m.resume_declared.clone() };
    let session = g(result, "session_id").clone();
    let evidence = g(result, "failure_evidence");
    let compact = if pv::is_dict(evidence) {
        let mut d = pv::empty();
        for k in ["signal", "rotate", "returncode", "termination", "lock_acquired", "malformed_jsonl"] {
            if has(evidence, k) {
                set(&mut d, k, g(evidence, k).clone());
            }
        }
        d
    } else {
        V::None
    };
    let provider = if truthy(g(result, "provider")) { g(result, "provider").clone() } else { g(&m.runner, "provider").clone() };
    let budget_unit = if has(bud, "budget_unit") { g(bud, "budget_unit").clone() } else { s("native_turns") };
    let model_used = or_list(g(result, "model_used"));
    let artifact_paths = if report_present {
        V::List(
            items(g(&report, "artifacts"))
                .iter()
                .filter(|a| pv::is_dict(a) && truthy(g(a, "path")))
                .map(|a| g(a, "path").clone())
                .collect(),
        )
    } else {
        V::None
    };
    let resume_honored = if pv::is_none(&resume_requested) { V::None } else { V::Bool(eq(&session, &resume_requested)) };
    let n = attempts(doc).len() as i64 + 1;
    let mut att = obj(vec![
        ("attempt", pv::i(n)),
        ("max_turns_allocated", g(bud, "max_turns").clone()),
        ("timeout_seconds_allocated", g(bud, "timeout_seconds").clone()),
        ("budget_source", g(bud, "source").clone()),
        ("budget_unit", budget_unit),
        ("execution_started", g(result, "execution_started").clone()),
        ("raw_exit_code", g(result, "raw_exit_code").clone()),
        ("usage", g(result, "usage").clone()),
        ("provider", provider),
        ("failure_evidence", compact),
        ("session_binding", m.session_binding.clone()),
        ("dispatch_id", m.dispatch_id.clone()),
        ("budget_recommended", g(&report, "budget_recommendation").clone()),
        ("request_path", m.request_path.clone()),
        ("started_utc", m.started_utc.clone()),
        ("ended_utc", m.ended_utc.clone()),
        ("duration_ms", g(result, "duration_ms").clone()),
        ("duration_api_ms", g(result, "duration_api_ms").clone()),
        ("resume_declared", m.resume_declared.clone()),
        ("max_turns_used", g(result, "num_turns").clone()),
        ("budget_outcome", g(result, "budget_outcome").clone()),
        ("session_id", session),
        ("resume_requested", resume_requested),
        ("resume_honored", resume_honored),
        ("model_declared", g(result, "model_requested").clone()),
        ("model_used", model_used),
        ("reasoning_selection", g(result, "reasoning_selection").clone()),
        ("runner", if pv::is_dict(&m.runner) { m.runner.clone() } else { V::None }),
        ("runner_evidence", parse_runner_evidence(result)),
        ("control_status", g(result, "status").clone()),
        ("reason_codes", or_list(g(result, "reason_codes"))),
        ("permission_denials", denial_block(result)),
        ("status", g(&report, "status").clone()),
        ("phase", g(&report, "phase").clone()),
        ("context_id_reported", g(&report, "context_id").clone()),
        ("campaign_id_reported", g(&report, "campaign_id").clone()),
        ("running_services", g(&report, "running_services").clone()),
        ("control_variables_echo", g(&report, "control_variables_echo").clone()),
        ("external_search", or_list(g(&report, "external_search"))),
        ("sub_reported", V::Bool(report_present)),
        ("next_steps", if report_present { g(&report, "next_steps").clone() } else { V::None }),
        ("artifact_paths", artifact_paths),
        ("hitl_needed", if report_present { V::Bool(truthy(g(g(&report, "hitl"), "needed"))) } else { V::None }),
        (
            "budget_recommendation",
            if report_present && pv::is_dict(g(&report, "budget_recommendation")) {
                g(&report, "budget_recommendation").clone()
            } else {
                V::None
            },
        ),
    ]);
    let er = end_reason(&att);
    set(&mut att, "end_reason", s(er));
    if let Some(r) = codex_recovery_requirement(&att) {
        set(&mut att, "recovery_required", r);
    }
    set(doc, "context_id", s(context_id));
    pv::append(pv::setdefault(doc, "attempts", pv::list(vec![])), att.clone());
    att
}

pub fn entry_blocking(entry: &V) -> bool {
    truthy(g(entry, "blocking"))
}

pub fn sort_pending(pending: Vec<V>) -> Vec<V> {
    let mut idx: Vec<(bool, f64, String, usize)> = pending
        .iter()
        .enumerate()
        .map(|(n, e)| {
            let att = if truthy(g(e, "attempt")) { pv::num(g(e, "attempt")).unwrap_or(0.0) } else { 0.0 };
            let rid = if truthy(g(e, "request_id")) { py_str(g(e, "request_id")) } else { String::new() };
            (!entry_blocking(e), att, rid, n)
        })
        .collect();
    idx.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.2.chars().cmp(b.2.chars()))
    });
    idx.into_iter().map(|(_, _, _, n)| pending[n].clone()).collect()
}

pub fn surface_requests(repo_root: &str, context_id: &str, report: &V, attempt: i64, crash: &V) -> Res<Option<String>> {
    let path = join(&relay_root(repo_root)?, PENDING_HITL);
    let _l = state_lock(&path, true)?;
    surface_requests_unlocked(repo_root, context_id, report, attempt, crash)
}

fn surface_requests_unlocked(repo_root: &str, context_id: &str, report: &V, attempt: i64, crash: &V) -> Res<Option<String>> {
    let hitl = g(report, "hitl");
    let hitl = if truthy(hitl) { hitl.clone() } else { pv::empty() };
    let path = join(&relay_root(repo_root)?, PENDING_HITL);
    let mut doc = obj(vec![("schema_version", pv::i(1)), ("pending", pv::list(vec![]))]);
    if Path::new(&path).exists() {
        doc = load_json_or_raise(&path)?;
    }
    let pending_all: Vec<V> = items(g(&doc, "pending")).to_vec();
    let mut previous: Vec<(V, V)> = vec![];
    for e in &pending_all {
        if is(g(e, "context_id"), context_id) {
            let k = g(e, "request_id").clone();
            if let Some(slot) = previous.iter_mut().find(|(x, _)| eq(x, &k)) {
                slot.1 = e.clone();
            } else {
                previous.push((k, e.clone()));
            }
        }
    }
    let prior = |rid: &V| previous.iter().find(|(k, _)| eq(k, rid)).map(|(_, e)| g(e, "answer").clone()).unwrap_or(V::None);
    let mut kept: Vec<V> = pending_all.into_iter().filter(|e| !is(g(e, "context_id"), context_id)).collect();
    let mut entry: Option<V> = None;
    let recovery = g(crash, "recovery_required");
    if truthy(recovery) {
        let rid = format!("relay-recovery-a{attempt:02}");
        let old = previous.iter().find(|(k, _)| is(k, &rid)).map(|(_, e)| e.clone()).unwrap_or_else(pv::empty);
        let answer = if eq(g(&old, "recovery_required"), recovery) { g(&old, "answer").clone() } else { V::None };
        let mut prompt = format!(
            "Codex execution remains unresolved ({}; signal={}). Inspect the preserved outcome and effects. Record the human recovery decision in answer, then explicitly use --continue --apply --resume <session_id|new>. A signal does not identify who stopped the process, and this answer grants no additional tool permissions.",
            py_str(g(recovery, "kind")),
            py_str(g(recovery, "signal"))
        );
        if truthy(g(&hitl, "needed")) {
            prompt += &format!("\nSub request: {}", py_str(g(&hitl, "prompt")));
        }
        let e = obj(vec![
            ("context_id", s(context_id)),
            ("attempt", pv::i(attempt)),
            ("request_id", s(&rid)),
            ("source", s("relay-control-recovery")),
            ("blocking", V::Bool(true)),
            ("recovery_required", recovery.clone()),
            ("answer", answer),
            ("prompt", s(&prompt)),
        ]);
        kept.push(e.clone());
        entry = Some(e);
    } else if truthy(crash) && !truthy(g(&hitl, "needed")) {
        let rid = format!("relay-crash-a{attempt:02}");
        let prompt = if truthy(g(crash, "prompt")) {
            g(crash, "prompt").clone()
        } else {
            let codes: Vec<String> = items(g(crash, "reason_codes")).iter().map(py_str).collect();
            let codes = if codes.join(",").is_empty() { "코드 없음".to_string() } else { codes.join(",") };
            let lks = g(crash, "last_known_session");
            s(&format!(
                "attempt {} 가 리포트 없이 끝났다(end_reason={} · control={} · {}). 마지막 알려진 세션 = {}. 이어받으려면 `--resume <session_id>`, 새로 열려면 `--resume new` 를 선언하라.",
                py_str(g(crash, "attempt")),
                py_str(g(crash, "end_reason")),
                py_str(g(crash, "control_status")),
                codes,
                if truthy(lks) { py_str(lks) } else { "(없음)".into() }
            ))
        };
        let mut e = obj(vec![
            ("context_id", s(context_id)),
            ("attempt", pv::i(attempt)),
            ("request_id", s(&rid)),
            ("source", s("relay-crash")),
            ("prompt", prompt),
        ]);
        if truthy(g(crash, "blocking")) {
            set(&mut e, "blocking", V::Bool(true));
        }
        set(&mut e, "answer", prior(&s(&rid)));
        kept.push(e.clone());
        entry = Some(e);
    } else if truthy(g(&hitl, "needed")) {
        let rid = g(&hitl, "request_id").clone();
        let source = if truthy(g(&hitl, "source")) { g(&hitl, "source").clone() } else { s("sub-relay") };
        let mut e = obj(vec![
            ("context_id", s(context_id)),
            ("attempt", pv::i(attempt)),
            ("request_id", rid.clone()),
            ("source", source),
            ("prompt", g(&hitl, "prompt").clone()),
        ]);
        if truthy(g(&hitl, "blocking")) {
            set(&mut e, "blocking", V::Bool(true));
        }
        set(&mut e, "answer", prior(&rid));
        kept.push(e.clone());
        entry = Some(e);
    }
    set(&mut doc, "pending", V::List(sort_pending(kept)));
    atomic_json(&path, &doc)?;
    Ok(entry.map(|_| path))
}

pub fn pending_for(repo_root: &str, context_id: &V) -> Res<Vec<V>> {
    let path = join(&relay_root(repo_root)?, PENDING_HITL);
    if !Path::new(&path).exists() {
        return Ok(vec![]);
    }
    let doc = load_json_or_raise(&path)?;
    Ok(sort_pending(items(g(&doc, "pending")).iter().filter(|e| eq(g(e, "context_id"), context_id)).cloned().collect()))
}

pub fn dispatch_crash(att: &V, report: &V) -> V {
    let recovery = codex_recovery_requirement(att);
    if recovery.is_none()
        && truthy(report)
        && (is(g(att, "control_status"), "completed") || is(g(att, "end_reason"), "completed_with_denials"))
    {
        return V::None;
    }
    let mut d = obj(vec![
        ("attempt", g(att, "attempt").clone()),
        ("end_reason", g(att, "end_reason").clone()),
        ("control_status", g(att, "control_status").clone()),
        ("reason_codes", g(att, "reason_codes").clone()),
        ("last_known_session", g(att, "session_id").clone()),
    ]);
    if let Some(r) = recovery {
        set(&mut d, "recovery_required", r);
        set(&mut d, "blocking", V::Bool(true));
    }
    d
}

pub fn last_attempt(doc: &V) -> Option<&V> {
    attempts(doc).last()
}

pub fn ensure_recovery_pending(repo_root: &str, doc: &V) -> Res<Vec<V>> {
    let mut pending = pending_for(repo_root, g(doc, "context_id"))?;
    let last = last_attempt(doc).filter(|a| truthy(a));
    let recovery = last.and_then(codex_recovery_requirement);
    if let (Some(recovery), Some(last)) = (recovery, last) {
        if !pending.iter().any(|e| eq(g(e, "recovery_required"), &recovery) && entry_blocking(e)) {
            let report = load_report(repo_root, last)?;
            let rep = if truthy(&report) { report.clone() } else { pv::empty() };
            surface_requests(
                repo_root,
                &py_str(g(doc, "context_id")),
                &rep,
                pv::int(g(last, "attempt")).unwrap_or(0),
                &dispatch_crash(last, &report),
            )?;
            pending = pending_for(repo_root, g(doc, "context_id"))?;
        }
    }
    Ok(pending)
}

/// `parse_report(output)`: the last top-level JSON object found by decoding at every `{`.
pub fn parse_report(output: &V) -> V {
    let Some(text) = pv::st(output) else { return V::None };
    let c: Vec<char> = text.chars().collect();
    let find = |from: usize| c.iter().skip(from).position(|&x| x == '{').map(|p| p + from);
    let mut best = V::None;
    let mut i = find(0);
    while let Some(st) = i {
        match raw_decode(&c, st) {
            Some((v, end)) => {
                if pv::is_dict(&v) {
                    best = v;
                }
                i = find(end);
            }
            None => i = find(st + 1),
        }
    }
    best
}

/// `JSONDecoder().raw_decode(text, i)` for an object at `i`: (value, end) or None.
fn raw_decode(c: &[char], st: usize) -> Option<(V, usize)> {
    let (mut depth, mut in_str, mut esc) = (0i64, false, false);
    let mut end = None;
    for (k, &ch) in c.iter().enumerate().skip(st) {
        if in_str {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(k + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let slice: String = c[st..end].iter().collect();
    pv::loads(&slice).ok().map(|v| (v, end))
}

pub fn load_report(repo_root: &str, att: &V) -> Res<V> {
    let rel = g(att, "report_path");
    if !truthy(rel) {
        return Ok(V::None);
    }
    let path = join(repo_root, &py_str(rel));
    if !Path::new(&path).exists() {
        return Ok(V::None);
    }
    let d = load_json_or_raise(&path)?;
    let d = if truthy(&d) { d } else { pv::empty() };
    Ok(g(&d, "report").clone())
}

pub fn instruction_scope(doc: &V) -> V {
    let mut d = pv::empty();
    for k in ["context_id", "campaign_id", "topology", "campaign_node", "campaign_context_kind", "campaign_cell", "control_variables"] {
        set(&mut d, k, g(doc, k).clone());
    }
    d
}

pub fn instruction_refusal(reason: &str) -> Exit {
    Exit::msg(format!(
        "[relay] TASK_INSTRUCTION_SOURCE_UNRESOLVED: {reason}; main must review the issued requests and supply a complete explicit --task-file under the existing dispatch guards; do not reset the context or budget"
    ))
}

/// `_owned_artifact(repo_root, relative)`.
pub fn owned_artifact(repo_root: &str, relative: &str) -> Res<PathBuf> {
    let root = std::fs::canonicalize(repo_root).unwrap_or_else(|_| PathBuf::from(repo_root));
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(Exit::msg("[relay] STOP: recovery artifact must be workspace-relative"));
    }
    let mut path = root.clone();
    for part in rel.components() {
        if let std::path::Component::Normal(p) = part {
            path = path.join(p);
            if path.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
                return Err(Exit::msg("[relay] STOP: recovery artifact path cannot contain symlinks"));
            }
        }
    }
    let resolved = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if !resolved.starts_with(&root) || !path.is_file() {
        return Err(Exit::msg("[relay] STOP: recovery artifact must be an existing regular file inside this workspace"));
    }
    Ok(path)
}

/// `permission_binding(request)`.
pub fn permission_binding(request: &V) -> V {
    let mut d = obj(vec![("provider", g(request, "provider").clone()), ("target", g(request, "target").clone())]);
    for k in ["sandbox_mode", "permission_profile"] {
        if has(request, k) {
            set(&mut d, k, g(request, k).clone());
        }
    }
    d
}

pub fn instruction_request(repo_root: &str, doc: &V, att: &V) -> Res<V> {
    let number = g(att, "attempt");
    let context = g(doc, "context_id");
    let ok_num = pv::int(number).is_some_and(|n| n >= 1);
    let ctx = pv::st(context).filter(|c| !c.is_empty());
    if !ok_num || ctx.is_none() {
        return Err(instruction_refusal("instruction attempt/context is absent or malformed"));
    }
    let n = pv::int(number).unwrap();
    let camp = if truthy(g(doc, "campaign_id")) { py_str(g(doc, "campaign_id")) } else { "_bootstrap".into() };
    let expected = norm_path(&format!("campaigns/{camp}/relay/{}.requests/attempt-{n:02}.json", ctx.unwrap()));
    if !is(g(att, "request_path"), &expected) {
        return Err(instruction_refusal("instruction request path does not belong to this context/attempt"));
    }
    let loaded = (|| -> Result<(Vec<u8>, V), String> {
        let path = owned_artifact(repo_root, &expected).map_err(|_| "SystemExit".to_string())?;
        let raw = std::fs::read(&path).map_err(|_| "OSError".to_string())?;
        let t = String::from_utf8(raw.clone()).map_err(|_| "UnicodeDecodeError".to_string())?;
        let v = pv::loads(&t).map_err(|_| "JSONDecodeError".to_string())?;
        Ok((raw, v))
    })();
    let (raw, request) = match loaded {
        Ok(x) => x,
        Err(k) => return Err(instruction_refusal(&format!("issued instruction request is unavailable: {k}"))),
    };
    let digest = g(att, "request_digest");
    if !pv::is_none(digest) {
        if !is(digest, &pv::sha256_hex(&raw)) {
            return Err(instruction_refusal("issued instruction request bytes changed"));
        }
    } else if !pv::is_none(g(att, "task_instruction")) {
        return Err(instruction_refusal("issued instruction request digest is absent"));
    } else {
        let op = if truthy(g(att, "outcome_path")) { py_str(g(att, "outcome_path")) } else { String::new() };
        let wrapper = (|| -> Result<V, String> {
            let p = owned_artifact(repo_root, &op).map_err(|_| "SystemExit".to_string())?;
            let b = std::fs::read(&p).map_err(|_| "OSError".to_string())?;
            let t = String::from_utf8(b).map_err(|_| "UnicodeDecodeError".to_string())?;
            pv::loads(&t).map_err(|_| "JSONDecodeError".to_string())
        })();
        let wrapper = match wrapper {
            Ok(w) => w,
            Err(k) => return Err(instruction_refusal(&format!("legacy instruction outcome is unavailable: {k}"))),
        };
        if !pv::is_dict(&wrapper)
            || !eq(g(&wrapper, "schema_version"), &pv::i(1))
            || ne(g(&wrapper, "dispatch_id"), g(att, "dispatch_id"))
            || !truthy(g(att, "dispatch_id"))
            || ne(g(&wrapper, "request"), &request)
        {
            return Err(instruction_refusal("legacy instruction outcome does not match the issued dispatch"));
        }
    }
    if !pv::is_dict(&request)
        || !crate::agent_control::request_schema_violations(&request).is_empty()
        || !is(g(&request, "intent"), "delegate")
        || ne(&permission_binding(&request), g(att, "session_binding"))
        || ne(g(&request, "provider"), g(att, "provider"))
        || ne(g(&request, "campaign_id"), g(doc, "campaign_id"))
        || ne(g(&request, "control_variables"), g(doc, "control_variables"))
        || !pv::is_str(g(&request, "task"))
    {
        return Err(instruction_refusal("issued instruction target/provider/campaign binding differs"));
    }
    Ok(request)
}

/// `continuation_instruction(repo_root, doc)` → {attempt, body, request_path, legacy}.
pub fn continuation_instruction(repo_root: &str, doc: &V) -> Res<V> {
    let atts = attempts(doc).to_vec();
    if atts.is_empty() {
        return Err(instruction_refusal("there is no issued task"));
    }
    if atts.iter().enumerate().any(|(i, a)| !pv::is_dict(a) || !eq(g(a, "attempt"), &pv::i(i as i64 + 1))) {
        return Err(instruction_refusal("instruction attempt sequence is malformed"));
    }
    let latest = atts.last().unwrap();
    let instruction = g(latest, "task_instruction");
    if pv::is_none(instruction) {
        if atts.len() != 1 {
            return Err(instruction_refusal("legacy multiple attempts have no explicit task-source declaration"));
        }
        let request = instruction_request(repo_root, doc, latest)?;
        let body = g(doc, "task");
        let ok = pv::st(body).filter(|b| !b.is_empty()).is_some_and(|b| pv::st(g(&request, "task")).unwrap_or_default().ends_with(&b));
        if !ok {
            return Err(instruction_refusal("legacy initial task does not match its issued request"));
        }
        return Ok(obj(vec![
            ("attempt", pv::i(1)),
            ("body", body.clone()),
            ("request_path", g(latest, "request_path").clone()),
            ("legacy", V::Bool(true)),
        ]));
    }
    if !pv::is_dict(instruction) {
        return Err(instruction_refusal("instruction metadata is malformed"));
    }
    let source = g(instruction, "source_attempt");
    let src = match pv::int(source) {
        Some(n) if n >= 1 && (n as usize) <= atts.len() => n as usize,
        _ => return Err(instruction_refusal("instruction source attempt is invalid")),
    };
    let origin = atts[src - 1].clone();
    let declared = g(&origin, "task_instruction");
    let scope = instruction_scope(doc);
    let resolved = if pv::is_none(declared) {
        if src != 1 {
            return Err(instruction_refusal("a legacy feedback attempt cannot be selected implicitly"));
        }
        let mut sub = doc.clone();
        set(&mut sub, "attempts", V::List(vec![origin.clone()]));
        continuation_instruction(repo_root, &sub)?
    } else {
        if !pv::is_dict(declared)
            || !eq(g(declared, "schema_version"), &pv::i(1))
            || !is(g(declared, "kind"), "explicit")
            || !eq(g(declared, "source_attempt"), &pv::i(src as i64))
            || ne(g(declared, "scope"), &scope)
        {
            return Err(instruction_refusal("explicit task source scope/kind is invalid"));
        }
        let body = g(declared, "body");
        let request = instruction_request(repo_root, doc, &origin)?;
        let ok = pv::st(body).filter(|b| !b.is_empty()).is_some_and(|b| {
            is(g(declared, "body_digest"), &pv::sha256_hex(b.as_bytes()))
                && pv::st(g(&request, "task")).unwrap_or_default().ends_with(&b)
        });
        if !ok {
            return Err(instruction_refusal("explicit task body differs from its issued request"));
        }
        obj(vec![
            ("attempt", pv::i(src as i64)),
            ("body", body.clone()),
            ("request_path", g(&origin, "request_path").clone()),
            ("legacy", V::Bool(false)),
        ])
    };
    for att in &atts[src..] {
        let r = g(att, "task_instruction");
        let mut ks = pv::keys(r);
        ks.sort();
        let want = ["kind", "schema_version", "scope", "source_attempt"];
        if !pv::is_dict(r)
            || ks != want
            || !eq(g(r, "schema_version"), &pv::i(1))
            || !is(g(r, "kind"), "continuation")
            || !eq(g(r, "source_attempt"), &pv::i(src as i64))
            || ne(g(r, "scope"), &scope)
        {
            return Err(instruction_refusal("later task source is missing, stale, or ambiguous"));
        }
        let request = instruction_request(repo_root, doc, att)?;
        if ne(g(&request, "target"), g(g(&origin, "session_binding"), "target")) {
            return Err(instruction_refusal("continuation target differs from its instruction source"));
        }
    }
    Ok(resolved)
}

pub fn assemble_continuation(repo_root: &str, doc: &V, pending: &[V]) -> Res<String> {
    let instruction = continuation_instruction(repo_root, doc)?;
    let atts: Vec<&V> = attempts(doc).iter().filter(|a| reached_sub(a)).collect();
    let last = atts.last().copied();
    let report = match last {
        Some(l) => load_report(repo_root, l)?,
        None => V::None,
    };
    let mut out: Vec<String> = vec!["## 이어받기 — 아래는 메인이 원장에서 **그대로 옮긴** 사실이다(추측 없음).".into(), String::new()];
    if let Some(l) = last {
        out.push(format!(
            "- 직전 attempt {}: status={} · phase={} · turns={}/{} · budget={}",
            py_str(g(l, "attempt")),
            py_str(g(l, "status")),
            py_str(g(l, "phase")),
            py_str(g(l, "max_turns_used")),
            py_str(g(l, "max_turns_allocated")),
            py_str(g(l, "budget_outcome"))
        ));
        if is(g(l, "budget_outcome"), "exhausted") {
            out.push("- ⚠ 직전 attempt 는 **예산 소진**으로 끊겼다. 같은 일을 처음부터 하지 말고 아래 산출물을 신뢰해 그 다음부터 이어라.".into());
        }
    }
    if pv::is_none(&report) {
        out.push("- ⚠ 직전 리포트(JSON)가 없다 — 서브가 산문만 보냈거나 전송이 실패했다. 무엇이 끝났는지 **실물로 확인한 뒤** 진행하라.".into());
    }
    let rep = if truthy(&report) { report.clone() } else { pv::empty() };
    for art in items(g(&rep, "artifacts")) {
        out.push(format!("  - 산출물[{}]: {}", py_str(g(art, "kind")), py_str(g(art, "path"))));
    }
    for step in items(g(&rep, "next_steps")) {
        out.push(format!("  - 직전 턴이 남긴 다음 단계: {}", py_str(step)));
    }
    if truthy(g(&rep, "notes")) {
        out.push(format!("  - 직전 턴 메모: {}", py_str(g(&rep, "notes"))));
    }
    let answered: Vec<&V> = pending.iter().filter(|e| truthy(g(e, "answer"))).collect();
    if !answered.is_empty() {
        out.push(String::new());
        out.push("## 네 질문에 대한 메인의 답".into());
        for e in answered {
            if truthy(g(e, "prompt")) {
                out.push(format!("- (질문) {}", py_str(g(e, "prompt"))));
            }
            out.push(format!("- **(답)** {}", py_str(g(e, "answer"))));
        }
    }
    let open: Vec<&V> = pending.iter().filter(|e| !truthy(g(e, "answer")) && !entry_blocking(e)).collect();
    if !open.is_empty() {
        out.push(String::new());
        out.push("## 아직 답이 없는 비차단 질문(진행을 막지 않는다 — 없이 갈 수 있으면 가라)".into());
        for e in open {
            let p = if truthy(g(e, "prompt")) {
                py_str(g(e, "prompt"))
            } else {
                format!("(질문 문구 없음 · request_id={})", py_str(g(e, "request_id")))
            };
            out.push(format!("- {p}"));
        }
    }
    out.push(String::new());
    out.push("- 이번 attempt 의 시간·턴이 남은 일에 모자라 보이면, 필요한 값을 산문이 아니라 리포트 `budget_recommendation{max_turns,timeout_seconds,why}` 에 **수치로** 적어라 — 감독 재발급은 그 필드만 읽는다.".into());
    out.push(String::new());
    out.push("## 현재 적용할 명시 지시".into());
    out.push(String::new());
    out.push(format!(
        "- 원문 출처: attempt {} · {}",
        py_str(g(&instruction, "attempt")),
        py_str(g(&instruction, "request_path"))
    ));
    out.push("- 최초 지시는 원장 이력에 보존돼 있다. 이번 재개에는 아래 최신 명시 원문을 적용한다.".into());
    out.push(String::new());
    out.push(py_str(g(&instruction, "body")));
    Ok(out.join("\n") + "\n")
}

pub fn last_reached(doc: &V) -> Option<V> {
    attempts(doc).iter().rev().find(|x| reached_sub(x)).cloned()
}

pub fn continuation_stop_reasons(doc: &V, allow_control_recovery: bool, historical: Option<&V>) -> Vec<String> {
    if truthy(g(doc, "inflight")) {
        return vec!["unresolved in-flight dispatch; reconcile before replay".into()];
    }
    let mut reasons = vec![];
    if let Some(latest) = last_attempt(doc) {
        if truthy(latest) && codex_recovery_requirement(latest).is_some() && !allow_control_recovery {
            reasons.push("unresolved Codex control outcome requires a human answer and explicit manual resume".into());
        }
    }
    let Some(last) = last_reached(doc) else { return reasons };
    let cv = match historical {
        Some(h) => h.clone(),
        None => g(doc, "control_variables").clone(),
    };
    reasons.extend(echo_stop_reasons(&last, g(doc, "context_id"), g(doc, "campaign_id"), &cv));
    if pv::is_list(g(&last, "running_services")) && !items(g(&last, "running_services")).is_empty() {
        reasons.push("previous attempt left running services; use explicit A2A cleanup or a declared new context".into());
    }
    reasons
}

pub fn budget_yield(att: &V) -> bool {
    pv::is_false(g(att, "hitl_needed")) && truthy(g(att, "next_steps"))
}

pub fn ledger_progress(last: &V, prev: Option<&V>) -> Option<String> {
    let (arts, steps) = (g(last, "artifact_paths"), g(last, "next_steps"));
    if pv::is_none(arts) && pv::is_none(steps) {
        return None;
    }
    let Some(prev) = prev else {
        let used = if truthy(g(last, "max_turns_used")) { pv::num(g(last, "max_turns_used")).unwrap_or(0.0) } else { 0.0 };
        if used > 0.0 && (truthy(arts) || truthy(steps)) {
            return Some("첫 중단 · 산출물/남은 일 기록".into());
        }
        return None;
    };
    let prev_arts: Vec<String> = items(g(prev, "artifact_paths")).iter().map(py_str).collect();
    let mut new_arts: Vec<String> = items(arts).iter().map(py_str).filter(|a| !prev_arts.contains(a)).collect();
    new_arts.sort();
    new_arts.dedup();
    if !new_arts.is_empty() {
        return Some(format!("새 산출물 {}건", new_arts.len()));
    }
    if !pv::is_none(steps) && !pv::is_none(g(prev, "next_steps")) && ne(steps, g(prev, "next_steps")) {
        return Some("남은 일 변화".into());
    }
    None
}

pub fn doc_plane_progress(repo_root: &str, doc: &V) -> Option<String> {
    let last = last_reached(doc)?;
    let ctx = g(doc, "context_id");
    if !truthy(ctx) {
        return None;
    }
    let (t0, t1) = (utc_epoch(g(&last, "started_utc"))?, utc_epoch(g(&last, "ended_utc"))?);
    let root = Path::new(repo_root).join("sync_staging").join("sub_docs");
    let ctx = py_str(ctx);
    let mut hits: Vec<String> = vec![];
    walk(&root, &root, &mut |fp: &Path| {
        if fp.extension().map_or(true, |e| e != "md") {
            return;
        }
        let Ok(md) = std::fs::metadata(fp) else { return };
        let mt = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        if !(t0 - DOC_PLANE_SKEW_S <= mt && mt <= t1 + DOC_PLANE_SKEW_S) {
            return;
        }
        let Ok(bytes) = std::fs::read(fp) else { return };
        let text = String::from_utf8_lossy(&bytes);
        let head8: String = text.chars().take(8192).collect();
        if has_word(&head8, &ctx) {
            hits.push(fp.strip_prefix(&root).unwrap().display().to_string());
        }
    });
    if hits.is_empty() {
        return None;
    }
    hits.sort();
    Some(format!("문서 평면 {}건({} 등)", hits.len(), hits[0]))
}

fn walk(dir: &Path, root: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<_> = rd.flatten().collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            if dir == root && e.file_name() == "logs" {
                continue;
            }
            walk(&p, root, f);
        } else {
            f(&p);
        }
    }
}

/// `re.search(r"(?<![\w-])" + re.escape(ctx) + r"(?![\w-])", text)`.
fn has_word(text: &str, ctx: &str) -> bool {
    let wordish = |c: char| crate::py::is_word(c) || c == '-';
    let mut from = 0;
    while let Some(p) = text[from..].find(ctx) {
        let at = from + p;
        let before = text[..at].chars().next_back();
        let after = text[at + ctx.len()..].chars().next();
        if !before.is_some_and(wordish) && !after.is_some_and(wordish) {
            return true;
        }
        from = at + text[at..].chars().next().map_or(1, |c| c.len_utf8());
    }
    false
}

pub fn decision(action: &str, reason: &str) -> V {
    obj(vec![("action", s(action)), ("reason", s(reason))])
}

pub fn supervise_decide(doc: &V, brief: &V, ladder: &[V], doc_progress: Option<&str>) -> V {
    if truthy(g(doc, "inflight")) {
        return decision("popup", "unresolved in-flight dispatch; reconcile before replay");
    }
    let atts = attempts(doc);
    if atts.is_empty() {
        return decision("await_dispatch", "이 context 에 아직 아무것도 나가지 않았다 — 지시서를 열어라");
    }
    let lastatt = atts.last().unwrap();
    if let Some(recovery) = codex_recovery_requirement(lastatt) {
        let deferred = g(doc, "capacity_deferred");
        if is(g(&recovery, "signal"), "provider_capacity") && eq(g(deferred, "dispatch_id"), g(lastatt, "dispatch_id")) {
            return decision("resume", "capacity burst deferred; continue with a new bounded burst");
        }
        return decision(
            "popup",
            &format!(
                "unresolved Codex outcome ({}; {}); human answer and explicit manual resume required; progress does not authorize replay",
                py_str(g(&recovery, "kind")),
                py_str(g(&recovery, "signal"))
            ),
        );
    }
    if is(g(lastatt, "end_reason"), "runner_unavailable") {
        if !pv::is_true(g(g(lastatt, "runner_evidence"), "rotate")) {
            return decision("popup", "runner failure does not authorize rotation");
        }
        if pv::is_true(g(lastatt, "execution_started")) {
            return decision("popup", "runner failed after execution; reconcile effects first");
        }
        let rungs = if ladder.is_empty() { 1 } else { ladder.len() as i64 };
        let burned = consecutive_runner_unavailable(doc);
        if burned < rungs {
            return decision(
                "rotate_runner",
                &format!("러너 평면 실패({burned}/{rungs}칸 소모) — 다음 칸으로 회전한다. 예산도 범위도 바꾸지 않는다(바뀌는 것은 실행자 하나)"),
            );
        }
        return decision(
            "popup",
            &format!("러너 사다리 소진 — 선언된 {rungs}칸이 연속 {burned}회 전부 실행되지 않았다. 서브가 기동되지 않는 것이고 예산·과업 문제가 아니다"),
        );
    }
    let Some(last) = last_reached(doc) else {
        return decision("popup", "attempt 가 있으나 서브에 닿은 것이 없다(전송·기동 실패) — 통신 평면 확인");
    };
    let reason = py_str(g(&last, "end_reason"));
    let cell = g(doc, "campaign_cell").clone();
    if reason == "completed" {
        let mut d = decision("next_cell", "직전 attempt 가 completed 다 — 이 셀은 끝났다");
        set(&mut d, "cell", cell);
        return d;
    }
    if reason == "completed_with_denials" {
        let mut d = decision(
            "next_cell",
            "직전 attempt 가 completed 다(권한 거부 사건 동반 — 원장 permission_denials 참조) — 이 셀은 끝났다",
        );
        set(&mut d, "cell", cell);
        return d;
    }
    if reason == "permission_denied" {
        return decision(
            "popup",
            "권한 평면에서 거부된 호출이 있다 — 무엇이 막혔는지 사람이 봐야 한다(리포트 raw_output 의 permission_denials 를 읽어라). 그냥 재개하면 같은 벽에 다시 닿는다",
        );
    }
    if ["transport_or_launch_failure", "model_blocked", "invalid_request", "malformed_output"].contains(&reason.as_str()) {
        return decision("popup", &format!("모델·통신 평면이 깨졌다(end_reason={reason}) — 재개로 낫는 종류가 아니다"));
    }
    if reason == "sub_input_required" && !budget_yield(&last) {
        return decision("popup", "서브가 input-required 로 끊었다 — 답이 필요하다(차단성이면 답이 승인이다)");
    }
    if reason == "budget_exhausted" {
        return decision("resume", "native invocation budget exhausted; reconcile and resume");
    }
    if reason == "external_interruption"
        && is_any(g(g(&last, "failure_evidence"), "signal"), &["controlled_local_deadline", "controlled_remote_deadline"])
    {
        return decision("resume", "controlled local invocation deadline; reconcile and resume");
    }
    if ["external_interruption", "budget_exhausted", "unclassified", "sub_input_required"].contains(&reason.as_str()) {
        let reached: Vec<&V> = atts.iter().filter(|a| reached_sub(a)).collect();
        let prev: Vec<&V> = reached[..reached.len() - 1].to_vec();
        let moved_phase = !prev.is_empty() && ne(g(prev.last().unwrap(), "phase"), g(&last, "phase"));
        let seen = g(doc, "supervisor_last_seen_utc");
        let blu = g(brief, "last_utc");
        let moved_brief = truthy(brief) && truthy(blu) && (!truthy(seen) || py_str(blu).chars().cmp(py_str(seen).chars()) == std::cmp::Ordering::Greater);
        let moved_ledger = ledger_progress(&last, prev.last().copied());
        if moved_phase || moved_brief || moved_ledger.is_some() || doc_progress.is_some_and(|d| !d.is_empty()) {
            let why = if moved_phase {
                "phase 전진".to_string()
            } else if moved_brief {
                format!("브리핑 last_utc 전진({})", py_str(blu))
            } else if let Some(m) = &moved_ledger {
                format!("원장 전진({m})")
            } else {
                doc_progress.unwrap_or("").to_string()
            };
            let r = if reason == "sub_input_required" { "sub_input_required(예산 양보)".to_string() } else { reason.clone() };
            let note = if r == "unclassified" { " · 종료 사유는 판정 불가지만 일한 흔적이 있다" } else { "" };
            return decision("resume", &format!("{r} 이지만 전진이 보인다({why}){note} — 자동 재발급(D9: 항상 자동)"));
        }
        return decision("popup", &format!("{reason} 이고 전진이 없다 — 같은 벽에 부딪히는 중이다. 예산을 키우기 전에 사람에게 묻는다"));
    }
    decision("popup", &format!("처음 보는 종료(end_reason={reason}) — 모르면 묻는다"))
}

pub fn record_supervisor_step(doc: &mut V, decision: &V, utc: &str, brief: &V) -> V {
    let mut entry = decision.clone();
    set(&mut entry, "utc", s(utc));
    set(&mut entry, "brief_last_utc", g(brief, "last_utc").clone());
    pv::append(pv::setdefault(doc, "supervisor_steps", pv::list(vec![])), entry.clone());
    if truthy(g(brief, "last_utc")) {
        let v = g(brief, "last_utc").clone();
        set(doc, "supervisor_last_seen_utc", v);
    }
    entry
}

pub fn read_brief(repo_root: &str, node: &V) -> V {
    if !truthy(node) {
        return V::None;
    }
    let path = Path::new(repo_root).join("sync_staging/sub_docs/logs").join(py_str(node)).join("campaign_brief.json");
    if !path.is_file() {
        return V::None;
    }
    match pv::load_file(&path) {
        Ok(d) if pv::is_dict(&d) => d,
        _ => V::None,
    }
}

pub fn campaign_ledgers(repo_root: &str, camp: &str) -> Vec<(String, V)> {
    let root = Path::new(repo_root).join("campaigns").join(camp).join("relay");
    let mut names: Vec<String> = match std::fs::read_dir(&root) {
        Ok(rd) if root.is_dir() => rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        _ => vec![],
    };
    names.sort();
    let mut out = vec![];
    for name in names {
        if !name.ends_with(".json") || name == PENDING_HITL {
            continue;
        }
        let path = root.join(&name);
        let Ok(doc) = pv::load_file(&path) else { continue };
        if pv::is_dict(&doc) && truthy(g(&doc, "campaign_node")) {
            out.push((path.display().to_string(), doc));
        }
    }
    out
}

pub fn lifecycle_state(repo_root: &str, camp: &str) -> String {
    match fold_lifecycle(repo_root, camp) {
        Ok(f) => py_str(g(&f, "state")),
        Err(e) => format!("unknown(fold 실패: {e})"),
    }
}

pub fn ledger_runner_spec(doc: &V) -> Option<String> {
    if truthy(g(doc, "runner_ladder")) {
        return Some(py_str(g(doc, "runner_ladder")));
    }
    let first = attempts(doc).iter().find(|a| truthy(g(g(a, "runner"), "name")))?;
    Some(py_str(g(g(first, "runner"), "name")))
}

pub fn recommended_timeout(doc: &V) -> Option<i64> {
    let last = last_reached(doc).unwrap_or_else(pv::empty);
    let v = g(g(&last, "budget_recommendation"), "timeout_seconds");
    match pv::num(v) {
        Some(x) if x > 0.0 => Some(x as i64),
        _ => None,
    }
}

pub fn resume_blockers(pending: &[V]) -> Vec<V> {
    pending.iter().filter(|e| entry_blocking(e) && !truthy(g(e, "answer"))).cloned().collect()
}
