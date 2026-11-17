//! Port of `agent_control.py` — the terraforming_node-owned, provider-neutral agent-control orchestrator.
//!
//! Reads a request JSON conforming to `.claude/schemas/agent-control-request.schema.json`, dispatches it to the
//! provider adapter named by `provider` (v1 `provider_claude_code`, v2 `provider_codex_cli`), and prints the stable
//! sorted, indented result JSON conforming to `.claude/schemas/agent-control-result.schema.json`, exiting with
//! `result["exit_code"]`. This module stays provider-neutral: it never builds a provider CLI command line (the
//! self-test checks that boundary against this file's source).
//!
//! Exit-code <-> status <-> reason_codes table (single source of truth):
//!     0   completed              reason_codes == []
//!     2   invalid_request        REQUEST_SCHEMA_INVALID
//!     4   execution_failed       NONZERO_EXIT | IS_ERROR | PERMISSION_DENIED
//!     5   malformed_output       MALFORMED_JSON
//!     124 timeout                TIMEOUT
//!
//! The `pub fn` API below is what relay and bootstrap_canary call in-process (the Python `_agent_control.<name>`).

pub mod process;
pub mod pyhelp;
mod selftest;

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::raise::Raised;
use crate::sysexit::{Exit, Res};
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use pyhelp::{dict, get, has, int, py_eq, py_in, repr, s, set, str_of};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION_DEFAULT: i64 = 1;
pub const KNOWN_INTENTS: &[&str] = &["delegate", "probe", "bootstrap_canary"];
pub const KNOWN_PROVIDERS: &[&str] = &["claude_code", "codex_cli"];
pub const EXIT_INVALID_REQUEST: i64 = 2;

fn owner() -> PathBuf {
    crate::owner::owner_path("agent_control.sh")
}

/// `REPO_ROOT` (`Path(__file__).resolve().parents[4]`).
pub fn repo_root() -> PathBuf {
    owner().ancestors().nth(5).map(Path::to_path_buf).unwrap_or_default()
}
/// `RUNTIME_DIR` (the scripts directory).
pub fn runtime_dir() -> PathBuf {
    owner().parent().map(Path::to_path_buf).unwrap_or_default()
}
pub fn request_schema_path() -> PathBuf {
    repo_root().join(".claude/schemas/agent-control-request.schema.json")
}
pub fn result_schema_path() -> PathBuf {
    repo_root().join(".claude/schemas/agent-control-result.schema.json")
}

fn provisioned_config_path(role: &str) -> Option<PathBuf> {
    match role {
        "main" => Some(repo_root().join(".codex").join("config.toml")),
        "sub" => Some(runtime_dir().parent()?.join("sub_node").join("codex.config.template.toml")),
        _ => None,
    }
}

/// `PROVISIONED_CODEX_CONFIG[role]` as the f-string shows it.
pub fn provisioned_codex_config(role: &str) -> String {
    provisioned_config_path(role).map(|p| p.display().to_string()).unwrap_or_default()
}

/// The top-level string value of `key` in a TOML document (`tomllib.load(fh).get(key)`); `None` when absent, not a
/// basic/literal string, or the document is unreadable. (Only the root table is consulted, as the original.)
fn toml_root_string(text: &str, key: &str) -> Option<String> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        let k = k.trim().trim_matches('"');
        if k != key {
            continue;
        }
        let v = v.trim();
        if let Some(rest) = v.strip_prefix('"') {
            let mut out = String::new();
            let mut it = rest.chars();
            while let Some(c) = it.next() {
                match c {
                    '"' => return Some(out),
                    '\\' => match it.next()? {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        _ => return None,
                    },
                    c => out.push(c),
                }
            }
            return None;
        }
        if let Some(rest) = v.strip_prefix('\'') {
            return rest.split_once('\'').map(|(a, _)| a.to_string());
        }
        return None;
    }
    None
}

/// `provisioned_permission_profile(role)`: the installed Codex `default_permissions`, `None` when unreadable.
pub fn provisioned_permission_profile(role: &str) -> Option<String> {
    let path = provisioned_config_path(role)?;
    let text = std::fs::read_to_string(path).ok()?;
    toml_root_string(&text, "default_permissions").filter(|v| !v.is_empty())
}

/// `_load_schema(path)`; `Err` = the exception line the original lets escape.
pub fn load_schema(path: &Path) -> Result<PyValue, String> {
    let shown = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| crate::pathfmt::os_error(&e, &shown))?;
    let text = String::from_utf8(bytes).map_err(|_| "UnicodeDecodeError: 'utf-8' codec can't decode".to_string())?;
    pyjson::loads(&text).map_err(|e| format!("json.decoder.JSONDecodeError: {e}"))
}

fn schema_or_raise(path: &Path) -> Res<PyValue> {
    load_schema(path).map_err(|m| Exit::Code(crate::raise::exit(&Raised(m))))
}

fn type_matches(instance: &PyValue, t: &str) -> bool {
    match (instance.peel(), t) {
        (PyValue::Dict(_), "object") => true,
        (PyValue::List(_), "array") => true,
        (PyValue::Str(_), "string") => true,
        (PyValue::Int(_), "integer") => true,
        (PyValue::Int(_) | PyValue::Float(_), "number") => true,
        (PyValue::Bool(_), "boolean") => true,
        (PyValue::None, "null") => true,
        _ => false,
    }
}

fn is_number(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::Int(_) | PyValue::Float(_))
}

fn lt(a: &PyValue, b: &PyValue) -> bool {
    agent_runtime_support::pyops::compare(agent_runtime_support::pyops::Compare::Less, a, b).unwrap_or(false)
}

/// `_schema_violations(instance, schema, path)`: the Draft-07 subset validator of the original.
pub fn schema_violations(instance: &PyValue, schema: &PyValue, path: &str) -> Vec<String> {
    let mut out = vec![];
    if let Some(PyValue::List(branches)) = get(schema, "oneOf") {
        let matches = branches.iter().filter(|b| schema_violations(instance, b, path).is_empty()).count();
        if matches != 1 {
            out.push(format!("{path}: expected exactly one oneOf branch, matched {matches}"));
        }
    }
    if let Some(t) = get(schema, "type") {
        let allowed: Vec<PyValue> = match t {
            PyValue::List(l) => l.clone(),
            other => vec![other.clone()],
        };
        if !allowed.iter().any(|a| matches!(a, PyValue::Str(x) if type_matches(instance, &crate::py::text(x)))) {
            out.push(format!("{path}: expected type {}, got {} ({})", repr(&PyValue::List(allowed)),
                             instance.type_name(), repr(instance)));
            return out;
        }
    }
    if let Some(PyValue::List(e)) = get(schema, "enum") {
        if !py_in(instance, e) {
            out.push(format!("{path}: {} not in enum {}", repr(instance), repr(&PyValue::List(e.clone()))));
        }
    }
    if let Some(c) = get(schema, "const") {
        if !py_eq(instance, c) {
            out.push(format!("{path}: expected const {}, got {}", repr(c), repr(instance)));
        }
    }
    if let (PyValue::Str(t), Some(m)) = (instance.peel(), get(schema, "minLength")) {
        if lt(&int(t.len() as i64), m) {
            out.push(format!("{path}: length {} below minLength {}", t.len(), str_of(m)));
        }
    }
    if is_number(instance) {
        if let Some(m) = get(schema, "minimum") {
            if lt(instance, m) {
                out.push(format!("{path}: {} below minimum {}", repr(instance), str_of(m)));
            }
        }
        if let Some(m) = get(schema, "maximum") {
            if lt(m, instance) {
                out.push(format!("{path}: {} above maximum {}", repr(instance), str_of(m)));
            }
        }
    }
    if let PyValue::List(items) = instance.peel() {
        if let Some(sub) = get(schema, "items") {
            for (i, item) in items.iter().enumerate() {
                out.extend(schema_violations(item, sub, &format!("{path}[{i}]")));
            }
        }
        if let Some(m) = get(schema, "minItems") {
            if lt(&int(items.len() as i64), m) {
                out.push(format!("{path}: {} items below minItems {}", items.len(), str_of(m)));
            }
        }
        if let Some(m) = get(schema, "maxItems") {
            if lt(m, &int(items.len() as i64)) {
                out.push(format!("{path}: {} items above maxItems {}", items.len(), str_of(m)));
            }
        }
        if matches!(get(schema, "uniqueItems"), Some(PyValue::Bool(true))) {
            let mut seen = std::collections::HashSet::new();
            for it in items {
                seen.insert(pyhelp::sort_keys(it).dumps_ascii().unwrap_or_default());
            }
            if seen.len() != items.len() {
                out.push(format!("{path}: duplicate items forbidden"));
            }
        }
    }
    if let PyValue::Dict(inst) = instance.peel() {
        let props = get(schema, "properties").cloned().unwrap_or_else(|| dict(vec![]));
        if let Some(PyValue::List(req)) = get(schema, "required") {
            for k in req {
                let k = str_of(k);
                if !inst.contains_key(&k) {
                    out.push(format!("{path}.{k}: missing required property"));
                }
            }
        }
        if matches!(get(schema, "additionalProperties"), Some(PyValue::Bool(false))) {
            for (k, _) in inst.iter() {
                let k = crate::py::text(k);
                if !has(&props, &k) {
                    out.push(format!("{path}.{k}: unknown property (additionalProperties: false)"));
                }
            }
        }
        if let PyValue::Dict(p) = props.peel() {
            for (k, sub) in p.iter() {
                let k = crate::py::text(k);
                if let Some(v) = inst.get(&k) {
                    out.extend(schema_violations(v, sub, &format!("{path}.{k}")));
                }
            }
        }
    }
    out
}

fn text_at(v: &PyValue, key: &str) -> Option<String> {
    match get(v, key) {
        Some(PyValue::Str(t)) if !t.is_empty() => Some(crate::py::text(t)),
        _ => None,
    }
}

/// `_request_contract_violations(request)`: the provider-neutral cross-field target contract.
fn request_contract_violations(request: &PyValue) -> Vec<String> {
    let target = get(request, "target").cloned().unwrap_or(PyValue::None);
    let role = get(&target, "role").map(str_of).unwrap_or_default();
    let transport = get(&target, "transport").map(str_of).unwrap_or_default();
    let mut out = vec![];
    let intent = get(request, "intent").map(str_of).unwrap_or_default();
    if matches!(get(request, "capture_native_events"), Some(PyValue::Bool(true))) && intent != "probe"
        && intent != "bootstrap_canary"
    {
        out.push("$.capture_native_events: native evidence capture is diagnostic-only".into());
    }
    if !matches!((role.as_str(), transport.as_str()), ("main", "local") | ("sub", "ssh")) {
        out.push("$.target: role/transport mismatch".into());
    }
    let work_dir = text_at(&target, "work_dir");
    if work_dir.is_none() {
        out.push("$.target.work_dir: required".into());
    }
    if transport == "ssh" && text_at(&target, "host").is_none() {
        out.push("$.target.host: required for ssh".into());
    }
    if transport == "ssh" && text_at(&target, "ssh_user").is_none() {
        out.push("$.target.ssh_user: required for ssh (계정 미지정 위임은 권한 평면을 바꾼다)".into());
    }
    if get(request, "schema_version").is_some_and(|v| py_eq(v, &int(2))) {
        let abs = matches!(get(&target, "work_dir"), Some(PyValue::Str(w)) if crate::py::text(w).starts_with('/'));
        if !abs {
            out.push("$.target.work_dir: v2 requires an absolute workspace path".into());
        }
        let resume = get(request, "resume_session_id").cloned();
        let binding = get(request, "resume_binding").cloned();
        let mut expected = dict(vec![
            ("provider", get(request, "provider").cloned().unwrap_or(PyValue::None)),
            ("target", target.clone()),
        ]);
        for k in ["sandbox_mode", "permission_profile"] {
            if let Some(v) = get(request, k) {
                set(&mut expected, k, v.clone());
            }
        }
        match resume {
            Some(r) if !matches!(r, PyValue::None) => {
                let blank = matches!(&r, PyValue::Str(t) if t.strip().is_empty());
                if blank || !binding.as_ref().is_some_and(|b| py_eq(b, &expected)) {
                    out.push("$.resume_binding: native resume requires the exact provider, target and permission selection".into());
                }
            }
            _ => {
                if binding.is_some_and(|b| !matches!(b, PyValue::None)) {
                    out.push("$.resume_binding: forbidden without resume_session_id".into());
                }
            }
        }
        let profile = get(request, "permission_profile").cloned();
        let provisioned = provisioned_permission_profile(if role == "main" || role == "sub" { &role } else { "main" });
        if let Some(p) = profile.filter(|p| p.is_truthy()) {
            let same = matches!((&p, &provisioned), (PyValue::Str(a), Some(b)) if crate::py::text(a) == *b);
            if !same {
                let shown = provisioned.as_deref().map_or("None".to_string(), pyhelp::repr_str);
                out.push(format!("$.permission_profile: must equal the provisioned Codex default_permissions ({shown})"));
            }
        }
        if role == "sub" && matches!(get(request, "sandbox_mode"), Some(PyValue::Str(m)) if m.equals("workspace-write")) {
            out.push("$.sandbox_mode: sub writes require :danger-full-access via permission_profile; do not use a legacy override".into());
        }
    }
    out
}

/// `getattr(_load_provider(name), "reasoning_violations", None)(request)`: `None` when the provider has no such function.
pub fn provider_reasoning_violations(provider: &str, request: &PyValue) -> Res<Option<Vec<String>>> {
    match provider {
        "claude_code" => Ok(Some(crate::provider_claude_code::reasoning_violations(request))),
        "codex_cli" => Ok(None),
        other => Err(Exit::Code(crate::raise::exit(&Raised(format!(
            "FileNotFoundError: [Errno 2] No such file or directory: '{}'",
            runtime_dir().join("providers").join(format!("{other}.py")).display()
        ))))),
    }
}

/// `request_schema_violations(request)`: the public transport gate (structural + cross-field + provider effort).
pub fn request_schema_violations(request: &PyValue) -> Vec<String> {
    let schema = match load_schema(&request_schema_path()) {
        Ok(s) => s,
        Err(m) => {
            std::process::exit(crate::raise::exit(&Raised(m)));
        }
    };
    let mut v = schema_violations(request, &schema, "$");
    if v.is_empty() {
        v.extend(request_contract_violations(request));
        let provider = get(request, "provider").map(str_of).unwrap_or_default();
        match provider_reasoning_violations(&provider, request) {
            Ok(Some(x)) => v.extend(x),
            Ok(None) => {}
            Err(Exit::Code(c)) => std::process::exit(c),
            Err(e) => std::process::exit(e.finish()),
        }
    }
    v
}

fn versioned(mut result: PyValue) -> PyValue {
    if get(&result, "schema_version").is_some_and(|v| py_eq(v, &int(2))) {
        for k in ["raw_exit_code", "execution_started", "usage", "failure_evidence"] {
            set(&mut result, k, PyValue::None);
        }
        set(&mut result, "raw_events", PyValue::List(vec![]));
    }
    result
}

/// `_invalid_request_result(request)` (request `None` when it was not even a dict).
pub fn invalid_request_result(request: Option<&PyValue>) -> PyValue {
    let pick = |key: &str, known: &[&str]| -> String {
        let v = request.and_then(|r| get(r, key));
        match v {
            Some(PyValue::Str(t)) if known.iter().any(|k| t.equals(k)) => crate::py::text(t),
            _ => known[0].to_string(),
        }
    };
    let provider = pick("provider", KNOWN_PROVIDERS);
    let intent = pick("intent", KNOWN_INTENTS);
    let model = match request.and_then(|r| get(r, "model")) {
        Some(m @ PyValue::Str(_)) => m.clone(),
        _ => s(""),
    };
    let version = if provider == "codex_cli" { 2 } else { SCHEMA_VERSION_DEFAULT };
    versioned(dict(vec![
        ("schema_version", int(version)),
        ("provider", s(&provider)),
        ("intent", s(&intent)),
        ("status", s("invalid_request")),
        ("exit_code", int(EXIT_INVALID_REQUEST)),
        ("model_requested", model),
        ("model_used", PyValue::List(vec![])),
        ("reason_codes", PyValue::List(vec![s("REQUEST_SCHEMA_INVALID")])),
        ("output", PyValue::None),
        ("session_id", PyValue::None),
        ("num_turns", PyValue::None),
        ("budget_outcome", PyValue::None),
        ("duration_ms", PyValue::None),
        ("duration_api_ms", PyValue::None),
    ]))
}

/// `_invalid_provider_result(request)`.
pub fn invalid_provider_result(request: &PyValue) -> PyValue {
    let g = |k: &str| get(request, k).cloned().unwrap_or(PyValue::None);
    let mut r = versioned(dict(vec![
        ("schema_version", g("schema_version")),
        ("provider", g("provider")),
        ("intent", g("intent")),
        ("status", s("malformed_output")),
        ("exit_code", int(5)),
        ("model_requested", g("model")),
        ("model_used", PyValue::List(vec![])),
        ("reason_codes", PyValue::List(vec![s("PROVIDER_RESULT_INVALID")])),
        ("output", PyValue::None),
        ("session_id", PyValue::None),
        ("num_turns", PyValue::None),
        ("budget_outcome", PyValue::None),
        ("duration_ms", PyValue::None),
        ("duration_api_ms", PyValue::None),
    ]));
    if matches!(get(request, "capture_native_events"), Some(PyValue::Bool(true))) {
        set(&mut r, "raw_events", PyValue::List(vec![]));
    }
    r
}

/// `_result_contract_violations(result, request)`.
pub fn result_contract_violations(result: &PyValue, request: &PyValue) -> Vec<String> {
    let mut out = vec![];
    for (key, want) in [("schema_version", "schema_version"), ("provider", "provider"), ("intent", "intent"),
                        ("model_requested", "model")] {
        let have = get(result, key).cloned().unwrap_or(PyValue::None);
        let expected = get(request, want).cloned().unwrap_or(PyValue::None);
        if !py_eq(&have, &expected) {
            out.push(format!("$.{key}: provider result does not match the dispatched request"));
        }
    }
    if matches!(get(request, "provider"), Some(PyValue::Str(p)) if p.equals("claude_code")) {
        let capture = matches!(get(request, "capture_native_events"), Some(PyValue::Bool(true)));
        if capture && !has(result, "raw_events") {
            out.push("$.raw_events: requested native event evidence is absent".into());
        } else if !capture && has(result, "raw_events") {
            out.push("$.raw_events: unsolicited native event evidence".into());
        }
    }
    if let Some(sel) = get(result, "reasoning_selection").filter(|v| !matches!(v, PyValue::None)) {
        let req = get(sel, "requested").cloned().unwrap_or(PyValue::None);
        let want = get(request, "reasoning_effort").cloned().unwrap_or(PyValue::None);
        if !py_eq(&req, &want) {
            out.push("$.reasoning_selection: does not match the requested effort".into());
        }
    }
    out
}

/// Route to the provider adapter (`_load_provider(name).invoke(request)`).
pub fn provider_invoke(provider: &str, request: &PyValue) -> PyValue {
    match provider {
        "codex_cli" => crate::provider_codex_cli::invoke(request),
        _ => crate::provider_claude_code::invoke(request),
    }
}

fn emit(result: &PyValue) -> i32 {
    use std::io::Write;
    println!("{}", pyhelp::dumps_sorted_pretty(result));
    let _ = std::io::stdout().flush();
    pyhelp::as_int(get(result, "exit_code")).unwrap_or(1) as i32
}

/// `persist_outcome(path, dispatch_id=, request=, result=)`: unique, fully flushed outcome (link, never overwrite).
pub fn persist_outcome(path: &str, dispatch_id: &str, request: &PyValue, result: &PyValue) -> Result<(), Raised> {
    use std::io::Write;
    let dest = Path::new(path);
    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    std::fs::create_dir_all(&parent).map_err(|e| Raised::os(&e, &parent.display().to_string()))?;
    let mut tmp = None;
    for n in 0..1000u32 {
        let name = format!(".agent-outcome-{:08x}", (std::process::id().wrapping_mul(2654435761)) ^ n.wrapping_mul(40503)
            ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0));
        let p = parent.join(name);
        if let Ok(f) = std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            tmp = Some((p, f));
            break;
        }
    }
    let (tmp, mut f) = tmp.ok_or_else(|| Raised::new("FileExistsError", "No usable temporary file name found"))?;
    let doc = dict(vec![
        ("schema_version", int(1)),
        ("dispatch_id", s(dispatch_id)),
        ("request", request.clone()),
        ("result", result.clone()),
    ]);
    let body = format!("{}\n", pyhelp::dumps_sorted_unicode(&doc));
    let r = (|| {
        f.write_all(body.as_bytes()).map_err(|e| Raised::os(&e, &tmp.display().to_string()))?;
        f.sync_all().map_err(|e| Raised::os(&e, &tmp.display().to_string()))?;
        std::fs::hard_link(&tmp, dest).map_err(|e| {
            Raised(format!("{} -> {}", crate::pathfmt::os_error(&e, &tmp.display().to_string()), pyhelp::repr_str(path)))
        })?;
        if let Ok(d) = std::fs::File::open(&parent) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(&tmp);
    r
}

fn cmd_invoke(p: &argp::Parsed) -> Res<i32> {
    let request_path = p.text(1).unwrap_or("");
    let result_file = p.text(2).filter(|x| !x.is_empty());
    let dispatch_id = p.text(3).filter(|x| !x.is_empty());
    if result_file.is_some() != dispatch_id.is_some() {
        return Err(Exit::msg("--result-file and --dispatch-id must be supplied together"));
    }
    let request = match std::fs::read(request_path).ok().and_then(|b| String::from_utf8(b).ok()) {
        Some(t) => match pyjson::loads(&t.replace("\r\n", "\n").replace('\r', "\n")) {
            Ok(v) => v,
            Err(pyjson::LoadError::TooDeep { .. }) | Err(_) => return Ok(emit(&invalid_request_result(None))),
        },
        None => return Ok(emit(&invalid_request_result(None))),
    };
    let violations = request_schema_violations(&request);
    if !violations.is_empty() {
        for v in &violations {
            eprintln!("[agent-control] REQUEST_SCHEMA_INVALID: {v}");
        }
        let r = if matches!(request.peel(), PyValue::Dict(_)) { Some(&request) } else { None };
        return Ok(emit(&invalid_request_result(r)));
    }
    let provider = get(&request, "provider").map(str_of).unwrap_or_default();
    let mut result = provider_invoke(&provider, &request);
    if has(&request, "reasoning_effort") {
        let effort = get(&request, "reasoning_effort").cloned().unwrap_or(PyValue::None);
        let is_default = matches!(&effort, PyValue::Str(e) if e.equals("default"));
        if !has(&result, "reasoning_selection") {
            set(&mut result, "reasoning_selection", dict(vec![
                ("requested", effort.clone()),
                ("resolved", if is_default { PyValue::None } else { effort.clone() }),
                ("source", s(if is_default { "target-native-policy" } else { "request" })),
                ("mode", s(if is_default { "native-default" } else { "explicit" })),
                ("upstream_observed", PyValue::None),
            ]));
        }
        let source = get(&request, "reasoning_effort_source").cloned().unwrap_or_else(|| s("caller"));
        if let PyValue::Dict(d) = &mut result {
            if let Some(sel) = d.get_mut("reasoning_selection") {
                set(sel, "selection_source", source);
            }
        }
    }
    let result_schema = schema_or_raise(&result_schema_path())?;
    let mut rv = schema_violations(&result, &result_schema, "$");
    if rv.is_empty() {
        rv.extend(result_contract_violations(&result, &request));
    }
    if !rv.is_empty() {
        result = invalid_provider_result(&request);
    }
    if let (Some(f), Some(d)) = (result_file, dispatch_id) {
        if let Err(r) = persist_outcome(f, d, &request, &result) {
            return Err(Exit::Code(crate::raise::exit(&r)));
        }
    }
    Ok(emit(&result))
}

/// `SUPPORTS` / `RUNNER_ALIASES` of a provider adapter.
pub fn provider_tables(provider: &str) -> (PyValue, &'static [(&'static str, (&'static str, &'static str))]) {
    match provider {
        "codex_cli" => (crate::provider_codex_cli::supports(), crate::provider_codex_cli::RUNNER_ALIASES),
        _ => (crate::provider_claude_code::supports(), crate::provider_claude_code::RUNNER_ALIASES),
    }
}

fn cmd_runners(provider: &str) -> Res<i32> {
    let names: Vec<&str> = if provider == "all" { KNOWN_PROVIDERS.to_vec() } else { vec![provider] };
    let mut runners = vec![];
    let mut supports = dict(vec![]);
    let mut seen = std::collections::HashSet::new();
    for name in names {
        let (sup, aliases) = provider_tables(name);
        set(&mut supports, name, sup);
        for (alias, (backend, model)) in aliases {
            if !seen.insert(*alias) {
                return Err(Exit::msg(format!("duplicate runner alias: {alias}")));
            }
            runners.push(dict(vec![("name", s(alias)), ("provider", s(name)), ("backend", s(backend)),
                                   ("model", s(model))]));
        }
    }
    let sup = if provider == "all" { supports } else { get(&supports, provider).cloned().unwrap_or(PyValue::None) };
    let out = dict(vec![("provider", s(provider)), ("supports", sup), ("runners", PyValue::List(runners))]);
    println!("{}", pyhelp::dumps_sorted_pretty(&out));
    Ok(0)
}

static INVOKE: Spec = Spec {
    prog: "agent_control.sh invoke",
    usage: include_str!("../help/agent_control.invoke.usage"),
    help: include_str!("../help/agent_control.invoke.help"),
    args: &[HELP, store(&["--request"]).required(), store(&["--result-file"]), store(&["--dispatch-id"])],
    subs: None,
};

static RUNNERS: Spec = Spec {
    prog: "agent_control.sh runners",
    usage: include_str!("../help/agent_control.runners.usage"),
    help: include_str!("../help/agent_control.runners.help"),
    args: &[HELP, store(&["--provider"]).choices(&["claude_code", "codex_cli", "all"])],
    subs: None,
};

pub static SPEC: Spec = Spec {
    prog: "agent_control.sh",
    usage: include_str!("../help/agent_control.usage"),
    help: include_str!("../help/agent_control.help"),
    args: &[HELP, flag(&["--self-test"])],
    subs: Some(("cmd", &[("invoke", &INVOKE), ("runners", &RUNNERS)])),
};

fn run(argv: &[String]) -> Res<i32> {
    let p = match argp::parse(&SPEC, argv) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return Ok(c),
    };
    if p.flag(1) {
        return Ok(selftest::run());
    }
    match &p.sub {
        Some((name, sp)) if name == "invoke" => cmd_invoke(sp),
        Some((name, sp)) if name == "runners" => cmd_runners(sp.text(1).unwrap_or("claude_code")),
        _ => Ok(argp::error(&SPEC, "a subcommand (invoke | runners) or --self-test is required")),
    }
}

// --self-test (plan_26093022 · the boundary scan of the self-test reads this file only up to this marker)

pub fn main(argv: &[String]) -> i32 {
    crate::sysexit::run(run(argv))
}
