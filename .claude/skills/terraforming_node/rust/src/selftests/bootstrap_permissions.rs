//! Port of `selftest_bootstrap_permissions.py` — offline canary evidence regressions. No native CLI, network, Docker,
//! GPU or sudo.
//!
//! Native envelopes/tool receipts below are explicit fixtures, not runtime policy evidence. One helper case really
//! writes/commits only within a disposable local fixture checkout; the Docker/library answers are substituted there and
//! never counted as live observations.
//!
//! The original patched `harness_permission_probe._command`; here the probe's real command runner meets stub `docker`
//! and `node` executables first on `PATH` (a recording `git` wrapper where the original asserted "not called"). The
//! canary and probe are called as the Rust libraries (`bootstrap_canary`, `harness_permission_probe`).

use super::{dumps, git, j, path_with, snapshot, stub, Checks, TempDir};
use crate::agent_control::pyhelp::shlex_quote;
use crate::agent_control::request_schema_violations;
use crate::bootstrap_canary::{build_request, permission_plan, verify_result, RequestOpts, PLAN_PREFIX};
use crate::harness_permission_probe::{self as probe, BUILD_FILES, MARKER, PROTECTED_FILES};
use crate::relay::pv::{self, g, items, py_str, s, set, V};
use std::path::{Path, PathBuf};

struct Fx {
    tmp: TempDir,
    manifest: PathBuf,
}

/// `PATH` replaced for the duration of one closure (the probe's runner resolves `docker`/`node`/`git` through it).
fn with_path<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", path_with(dir));
    let out = f();
    match saved {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
    out
}

/// Stub `docker` (`version` → a JSON server version, anything else → a compose version) and `node` (a library answer).
fn fixture_tools(dir: &Path, library: &str) {
    stub(&dir.join("docker"), "if [ \"$1\" = version ]; then echo '\"fixture-server\"'; else echo fixture-compose; fi\n");
    stub(&dir.join("node"), &format!("echo {}\n", shlex_quote(library)));
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

impl Fx {
    fn new() -> Fx {
        let tmp = TempDir::new("bootstrap-permissions-");
        let manifest = tmp.path().join("manifest.yaml");
        std::fs::write(&manifest, "topology: multi\nnodes:\n  - role: sub\n    host: 203.0.113.11\n    ssh_user: tester\n    work_dir: /srv/fixture\n").expect("manifest");
        Fx { tmp, manifest }
    }
    fn root(&self) -> &Path {
        self.tmp.path()
    }

    /// `request(topology, role, provider, **extra)`; `extra` edits the keyword arguments before the build.
    fn try_request(&self, topology: &str, role: &str, provider: &str, extra: impl FnOnce(&mut RequestOpts)) -> Result<V, String> {
        let codex = provider == "codex_cli";
        let mut o = RequestOpts {
            phase: "permissions".into(),
            target_role: role.into(),
            provider: provider.into(),
            model: Some(if codex { "gpt-6.1-sol" } else { "sonnet" }.into()),
            max_turns: if codex { V::None } else { V::int(10) },
            timeout_seconds: V::int(120),
            budget_source: Some("declared: offline permission receipt fixture".into()),
            main_work_dir: Some(self.root().display().to_string()),
            ..RequestOpts::default()
        };
        if codex {
            o.max_dispatches = Some(1);
            o.permission_profile = Some(":danger-full-access".into());
        }
        extra(&mut o);
        let text = std::fs::read_to_string(&self.manifest).unwrap_or_default();
        std::fs::write(&self.manifest, text.replace("topology: multi", &format!("topology: {topology}"))).expect("manifest");
        build_request(topology, &self.manifest.display().to_string(), &o).map_err(|e| format!("{e:?}"))
    }
    fn request(&self, topology: &str, role: &str, provider: &str) -> V {
        self.try_request(topology, role, provider, |_| {}).unwrap_or_else(|e| panic!("request: {e}"))
    }

    fn packet(&self, request: &V) -> V {
        let plan = permission_plan(request).expect("permission plan");
        let topo = py_str(g(&plan, "topology"));
        let role = py_str(g(g(request, "target"), "role"));
        let mut functional = strs(&["scratch_write_cleanup", "scratch_local_git", "scratch_removed", "workspace_git_metadata",
            "docker_daemon_read", "docker_compose_read", "library_query", "model_asset_read", "buildkit_declaration"]);
        if topo == "single" && role == "sub" {
            functional.extend(strs(&["hint_clone_write", "hint_clone_git_discovery", "hint_clone_write_git_1"]));
        }
        let mut checks: Vec<V> = functional.iter().map(|n| pv::obj(vec![("name", s(n)), ("axis", s("functional")), ("outcome", s("allowed"))])).collect();
        let denied_build = topo == "multi" && role == "sub";
        let mut build: Vec<String> = strs(&BUILD_FILES);
        build.push("Dockerfile.source-build".into());
        for n in &build {
            checks.push(pv::obj(vec![("name", s(&format!("buildkit:{n}"))), ("axis", s("os_enforcement")), ("preserved", V::Bool(true)),
                ("outcome", s(if denied_build { "denied" } else { "allowed" }))]));
        }
        for (n, outcome) in probe::protected_file_expectations(&role) {
            checks.push(pv::obj(vec![("name", s(&format!("protected:{n}"))), ("axis", s("os_enforcement")), ("outcome", s(outcome)),
                ("preserved", V::Bool(true))]));
        }
        if py_str(g(request, "provider")) == "codex_cli" {
            for ch in checks.iter_mut() {
                if py_str(g(ch, "axis")) == "os_enforcement" {
                    set(ch, "outcome", s("allowed"));
                }
            }
        }
        pv::obj(vec![("schema_version", V::int(1)), ("nonce", g(&plan, "nonce").clone()), ("topology", s(&topo)), ("role", s(&role)),
            ("workspace", g(g(request, "target"), "work_dir").clone()), ("provenance", s("observed_local_process")),
            ("checks", V::List(checks)), ("build_files", pv::strs(&build))])
    }

    fn result(&self, request: &V, packet: Option<&V>) -> V {
        let plan = permission_plan(request).expect("permission plan");
        let p = packet.cloned().unwrap_or_else(|| self.packet(request));
        let text = format!("{MARKER}{}\n", dumps(&p));
        let argv: Vec<String> = items(g(&plan, "argv")).iter().map(py_str).collect();
        let command = argv.iter().map(|a| shlex_quote(a)).collect::<Vec<_>>().join(" ");
        let nonce = py_str(g(&plan, "nonce"));
        let report = pv::obj(vec![("task_id", s(&format!("harness-canary-{nonce}"))), ("context_id", s(&format!("harness-canary-{nonce}"))),
            ("turn", V::int(1)), ("node_id", g(g(request, "target"), "role").clone()), ("phase", s("inspect")), ("status", s("completed")),
            ("self_verification", j(r#"{"checks_run": ["harness_permission_probe"], "schema_valid": true}"#))]);
        let claude = py_str(g(request, "provider")) == "claude_code";
        let events = if claude {
            V::List(vec![
                pv::obj(vec![("type", s("assistant")), ("message", pv::obj(vec![("content", V::List(vec![pv::obj(vec![
                    ("type", s("tool_use")), ("id", s("use-1")), ("name", s("Bash")), ("input", pv::obj(vec![("command", s(&command))]))])]))]))]),
                pv::obj(vec![("type", s("user")), ("message", pv::obj(vec![("content", V::List(vec![pv::obj(vec![
                    ("type", s("tool_result")), ("tool_use_id", s("use-1")), ("content", s(&text))])]))]))]),
            ])
        } else {
            V::List(vec![pv::obj(vec![("type", s("item.completed")), ("item", pv::obj(vec![("type", s("command_execution")),
                ("command", s(&command)), ("status", s("completed")), ("exit_code", V::int(0)), ("aggregated_output", s(&text))]))])])
        };
        let mut r = pv::obj(vec![("schema_version", g(request, "schema_version").clone()), ("provider", g(request, "provider").clone()),
            ("intent", s("bootstrap_canary")), ("status", s("completed")), ("exit_code", V::int(0)),
            ("model_requested", g(request, "model").clone()), ("model_used", V::List(vec![])), ("reason_codes", V::List(vec![])),
            ("output", s(&dumps(&report))), ("session_id", s("fixture-session")), ("num_turns", V::None),
            ("budget_outcome", s("within_budget")), ("duration_ms", V::None), ("duration_api_ms", V::None), ("raw_events", events)]);
        if !claude {
            pv::update(&mut r, &j(r#"{"raw_exit_code": 0, "execution_started": true, "usage": null, "failure_evidence": null}"#));
        }
        r
    }

    fn status(&self, req: &V, res: &V) -> String {
        match verify_result(req, res) {
            Ok(v) => py_str(g(&v, "status")),
            Err(e) => format!("raised {e:?}"),
        }
    }
    fn check(&self, req: &V, res: &V) -> V {
        verify_result(req, res).unwrap_or(V::None)
    }
}

fn without(packet: &V, pred: impl Fn(&str) -> bool) -> V {
    let mut p = packet.clone();
    let kept: Vec<V> = items(g(&p, "checks")).iter().filter(|c| !pred(&py_str(g(c, "name")))).cloned().collect();
    set(&mut p, "checks", V::List(kept));
    p
}

fn edit_check(packet: &V, name: &str, changes: &V) -> V {
    let mut p = packet.clone();
    let mut list = items(g(&p, "checks")).to_vec();
    for c in list.iter_mut() {
        if py_str(g(c, "name")) == name {
            pv::update(c, changes);
            break;
        }
    }
    set(&mut p, "checks", V::List(list));
    p
}

/// `res["raw_events"][i] ... ` edits via a JSON pointer-like path of keys/indices.
fn edit(v: &mut V, path: &[&str], value: V) {
    if path.len() == 1 {
        match v {
            V::List(l) => {
                let i: usize = path[0].parse().expect("index");
                l[i] = value;
            }
            _ => set(v, path[0], value),
        }
        return;
    }
    let child = match v {
        V::List(l) => &mut l[path[0].parse::<usize>().expect("index")],
        _ => pv::get_mut(v, path[0]).expect("path"),
    };
    edit(child, &path[1..], value);
}

/// One fresh fixture per original test method (`setUp`) with its default codex multi/sub request.
fn fresh() -> (Fx, V) {
    let fx = Fx::new();
    let req = fx.request("multi", "sub", "codex_cli");
    (fx, req)
}

pub fn run() -> i32 {
    let mut c = Checks::new("selftest_bootstrap_permissions");

    c.group("legacy discovery is unchanged and cannot prove permissions");
    {
        let fx = Fx::new();
        let req = build_request("multi", &fx.manifest.display().to_string(), &RequestOpts {
            max_turns: V::int(10), timeout_seconds: V::int(120), budget_source: Some("declared: discovery".into()), ..RequestOpts::default()
        });
        c.ck("discovery request: no capture · no permission plan · read-only task", req.as_ref().is_ok_and(|r|
            !pv::has(r, "capture_native_events") && permission_plan(r).is_none() && py_str(g(r, "task")).contains("아무 파일도 고치지 마라")),
            || format!("{req:?}"));
    }

    c.group("main local does not require a sub");
    {
        let fx = Fx::new();
        std::fs::write(&fx.manifest, "topology: single\nnodes: []\n").expect("manifest");
        let req = fx.request("single", "main", "codex_cli");
        let want = pv::obj(vec![("role", s("main")), ("transport", s("local")), ("work_dir", s(&fx.root().display().to_string()))]);
        c.ck("main target is local in the main work dir · schema-valid", pv::eq(g(&req, "target"), &want)
            && request_schema_violations(&req).is_empty(), || dumps(&req));
    }

    c.group("codex permission phase requires an explicit model and the built-in profile");
    {
        let fx = Fx::new();
        type Edit = fn(&mut RequestOpts);
        let cases: [(&str, Edit); 4] = [
            ("model None", |o| o.model = None),
            ("permission_profile None", |o| o.permission_profile = None),
            ("sandbox_mode read-only", |o| o.sandbox_mode = Some("read-only".into())),
            ("permission_profile vllm-main", |o| o.permission_profile = Some("vllm-main".into())),
        ];
        for (label, e) in cases {
            c.ck(&format!("{label} → SystemExit"), fx.try_request("multi", "sub", "codex_cli", e).is_err(), String::new);
        }
    }

    c.group("codex exact receipt and role work complete");
    for (topology, role) in [("single", "sub"), ("multi", "sub"), ("single", "main"), ("multi", "main")] {
        let fx = Fx::new();
        let req = fx.request(topology, role, "codex_cli");
        let out = fx.check(&req, &fx.result(&req, None));
        c.ck(&format!("{topology}/{role}: completed · no observed model"), py_str(g(&out, "status")) == "completed"
            && pv::eq(g(g(&out, "native_cli"), "model_observed"), &V::List(vec![])), || dumps(&out));
    }


    c.group("read-only self report without a tool receipt never passes");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mut res = fx.result(&req, None);
        set(&mut res, "raw_events", V::List(vec![]));
        let mut report = pv::loads(&py_str(g(&res, "output"))).expect("report");
        set(&mut report, "notes", s(&format!("{MARKER}{}", dumps(&fx.packet(&req)))));
        set(&mut res, "output", s(&dumps(&report)));
        c.ck("notes-only marker → input-required", fx.status(&req, &res) == "input-required", String::new);
    }

    c.group("missing work and OS failure do not pass");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    for name in ["scratch_removed", "docker_daemon_read", "library_query", "workspace_git_metadata", "protected:AGENTS.md"] {
        let p = without(&fx.packet(&req), |n| n == name);
        c.ck(&format!("without {name} → input-required"), fx.status(&req, &fx.result(&req, Some(&p))) == "input-required", String::new);
    }

    c.group("full access does not accept denied or modified files");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    for change in [r#"{"outcome": "denied"}"#, r#"{"preserved": false}"#] {
        let p = edit_check(&fx.packet(&req), "protected:AGENTS.md", &j(change));
        c.ck(&format!("AGENTS.md {change} → input-required"), fx.status(&req, &fx.result(&req, Some(&p))) == "input-required", String::new);
    }

    c.group("single external clone required, multi has no hint grant");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let single = fx.request("single", "sub", "codex_cli");
        let p = without(&fx.packet(&single), |n| n.starts_with("hint_clone"));
        c.ck("single without hint_clone checks → input-required", fx.status(&single, &fx.result(&single, Some(&p))) == "input-required", String::new);
        let multi = fx.request("multi", "sub", "codex_cli");
        c.ck("multi packet has no hint_clone checks", !items(g(&fx.packet(&multi), "checks")).iter().any(|x| py_str(g(x, "name")).starts_with("hint_clone")), String::new);
    }

    c.group("binding rejects a wrong nonce, workspace and report identity");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    for field in ["nonce", "workspace", "topology", "role", "provenance"] {
        let mut p = fx.packet(&req);
        set(&mut p, field, s("other"));
        c.ck(&format!("packet {field}=other → input-required"), fx.status(&req, &fx.result(&req, Some(&p))) == "input-required", String::new);
    }
    {
        let mut res = fx.result(&req, None);
        let mut report = pv::loads(&py_str(g(&res, "output"))).expect("report");
        set(&mut report, "context_id", s("other"));
        set(&mut res, "output", s(&dumps(&report)));
        c.ck("report context_id=other → input-required", fx.status(&req, &res) == "input-required", String::new);
    }

    c.group("ambiguous and failed tool receipts are rejected");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mut res = fx.result(&req, None);
        let mut doubled = items(g(&res, "raw_events")).to_vec();
        doubled.extend(doubled.clone());
        set(&mut res, "raw_events", V::List(doubled));
        c.ck("doubled receipts → input-required", fx.status(&req, &res) == "input-required", String::new);
        let mut res = fx.result(&req, None);
        edit(&mut res, &["raw_events", "0", "item", "exit_code"], V::int(1));
        c.ck("receipt exit_code 1 → input-required", fx.status(&req, &res) == "input-required", String::new);
    }

    c.group("the receipt command cannot be echo or append shell code");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mut res = fx.result(&req, None);
        let original = py_str(g(g(&items(g(&res, "raw_events"))[0], "item"), "command"));
        for command in ["echo fake".to_string(), format!("{original}; echo fake")] {
            edit(&mut res, &["raw_events", "0", "item", "command"], s(&command));
            c.ck(&format!("command {command:?} → input-required"), fx.status(&req, &res) == "input-required", String::new);
        }
    }

    c.group("a tampered plan command is rejected");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mut plan = permission_plan(&req).expect("plan");
        set(&mut plan, "argv", pv::strs(&strs(&["echo", "fake"])));
        let mut r = req.clone();
        set(&mut r, "task", s(&format!("{PLAN_PREFIX}{}", dumps(&plan))));
        c.ck("tampered plan → no permission plan", permission_plan(&r).is_none(), String::new);
    }

    c.group("claude linked native receipt and the OS surface are separate");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let creq = fx.request("multi", "sub", "claude_code");
        c.ck("claude permission request captures native events · schema-valid",
             pv::is_true(g(&creq, "capture_native_events")) && request_schema_violations(&creq).is_empty(), || dumps(&creq));
        let mut p = fx.packet(&creq);
        let list: Vec<V> = items(g(&p, "checks")).iter().map(|x| {
            let mut x = x.clone();
            if py_str(g(&x, "axis")) == "os_enforcement" {
                set(&mut x, "outcome", s("allowed"));
            }
            x
        }).collect();
        set(&mut p, "checks", V::List(list));
        let out = fx.check(&creq, &fx.result(&creq, Some(&p)));
        c.ck("allowed OS surface: completed · enforcement unproven", py_str(g(&out, "status")) == "completed"
            && py_str(g(g(&out, "enforcement_evidence"), "status")) == "unproven", || dumps(&out));
        let mut res = fx.result(&creq, None);
        edit(&mut res, &["raw_events", "1", "message", "content", "0", "tool_use_id"], s("unrelated"));
        c.ck("unlinked tool_result → input-required", fx.status(&creq, &res) == "input-required", String::new);
    }

    c.group("native failure and an invalid report are separate from the probe");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mut res = fx.result(&req, None);
        set(&mut res, "status", s("timeout"));
        set(&mut res, "exit_code", V::int(124));
        c.ck("timeout result → input-required", fx.status(&req, &res) == "input-required", String::new);
        let mut res = fx.result(&req, None);
        set(&mut res, "output", s("not json"));
        c.ck("invalid report → task_report unproven", py_str(g(g(&fx.check(&req, &res), "task_report"), "status")) == "unproven", String::new);
    }

    c.group("probe: real local fixture only, no canonical mutation");
    {
        let fx = Fx::new();
        let (ws, clone) = (fx.root().join("workspace"), fx.root().join("external-hint"));
        std::fs::create_dir_all(&ws).expect("ws");
        std::fs::create_dir_all(&clone).expect("clone");
        for p in [&ws, &clone] {
            git(fx.root(), &["init", "--quiet", &p.display().to_string()]);
        }
        super::write(&ws.join("envs/.env"), &format!("HINT_CLONE_DIR={}\nGITHUB_TOKEN=DO_NOT_EXPOSE\n", clone.display()));
        let out = ws.join("output/single");
        for rel in BUILD_FILES.iter().chain(["Dockerfile"].iter()) {
            std::fs::create_dir_all(&out).expect("out");
            std::fs::write(out.join(rel), b"fixture\x00bytes").expect("build file");
        }
        std::fs::write(out.join("docker-compose.yaml"), "services:\n  vllm:\n    build:\n      dockerfile: Dockerfile\n").expect("compose");
        std::fs::write(out.join("manifest.yaml"), format!("topology: single\nmodel_source: managed\nnas_model_path: {}\n", clone.display())).expect("manifest");
        let saved = snapshot(&[&out], &out);
        let bin = TempDir::new("bootstrap-permissions-bin-");
        fixture_tools(bin.path(), r#"{"query_tokens":["fixture"],"results":[]}"#);
        let packet = with_path(bin.path(), || probe::observe(&ws, "single", "main", &"ab".repeat(16))).unwrap_or(V::None);
        let checks = items(g(&packet, "checks")).to_vec();
        c.ck("every non-protected check allowed", !checks.is_empty() && checks.iter()
            .filter(|x| !py_str(g(x, "name")).starts_with("protected:")).all(|x| py_str(g(x, "outcome")) == "allowed"), || dumps(&packet));
        c.ck("protected files (absent) unobserved", checks.iter().filter(|x| py_str(g(x, "name")).starts_with("protected:"))
            .all(|x| py_str(g(x, "outcome")) == "unobserved"), || dumps(&packet));
        let leftovers = |d: &Path, prefix: &str| std::fs::read_dir(d).map(|r| r.flatten().any(|e| e.file_name().to_string_lossy().starts_with(prefix))).unwrap_or(false);
        let clone_markers = snapshot(&[&clone], &clone).iter().any(|(p, _)| p.contains(".harness-permission-"));
        c.ck("no secret · no markers or scratch left · build files unchanged · clone clean",
             !dumps(&packet).contains("DO_NOT_EXPOSE") && !leftovers(&ws, ".harness-permission-")
                 && !leftovers(&ws.join("docs/logs"), "harness-permission-") && !clone_markers
                 && snapshot(&[&out], &out) == saved
                 && git(&clone, &["status", "--porcelain"]).is_empty(), String::new);
    }

    c.group("a pre-existing marker is preserved and a missing file is not a denial");
    {
        let fx = Fx::new();
        let marker = fx.root().join(format!(".harness-permission-{}", "ab".repeat(16)));
        std::fs::write(&marker, "existing").expect("marker");
        c.ck("existing marker → unobserved, kept", probe::write_marker(fx.root(), &"ab".repeat(16)).ok() == Some("unobserved")
            && super::read(&marker) == "existing", String::new);
        c.ck("absent file → unobserved", probe::write_open(&fx.root().join("absent")).0 == "unobserved", String::new);
    }

    c.group("ephemeral not_applicable is explicit, not a fake model read");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let p = edit_check(&fx.packet(&req), "model_asset_read", &j(r#"{"outcome": "not_applicable", "model_source": "ephemeral"}"#));
        c.ck("ephemeral not_applicable → completed", fx.status(&req, &fx.result(&req, Some(&p))) == "completed", String::new);
        let p = edit_check(&p, "model_asset_read", &j(r#"{"model_source": "managed"}"#));
        c.ck("managed not_applicable → input-required", fx.status(&req, &fx.result(&req, Some(&p))) == "input-required", String::new);
    }

    c.group("provider event surface and unique tool ids");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let codex = fx.request("multi", "sub", "codex_cli");
        let claude = fx.request("multi", "sub", "claude_code");
        for (r, other, label) in [(&codex, &claude, "codex with claude events"), (&claude, &codex, "claude with codex events")] {
            let mut res = fx.result(r, None);
            set(&mut res, "raw_events", g(&fx.result(other, None), "raw_events").clone());
            c.ck(&format!("{label} → input-required"), fx.status(r, &res) == "input-required", String::new);
        }
        let mut res = fx.result(&claude, None);
        for ident in [V::None, s("")] {
            edit(&mut res, &["raw_events", "0", "message", "content", "0", "id"], ident.clone());
            edit(&mut res, &["raw_events", "1", "message", "content", "0", "tool_use_id"], ident.clone());
            c.ck(&format!("tool id {} → input-required", ident.repr()), fx.status(&claude, &res) == "input-required", String::new);
        }
        let mut res = fx.result(&claude, None);
        let mut ev = items(g(&res, "raw_events")).to_vec();
        ev.insert(1, ev[0].clone());
        set(&mut res, "raw_events", V::List(ev));
        c.ck("duplicated tool_use → input-required", fx.status(&claude, &res) == "input-required", String::new);
    }

    c.group("result intent and version are bound to the request");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    for (k, v) in [("intent", s("probe")), ("schema_version", V::int(1))] {
        let mut res = fx.result(&req, None);
        set(&mut res, k, v);
        c.ck(&format!("result {k} changed → input-required"), fx.status(&req, &res) == "input-required", String::new);
    }
    c.ck("non-object result → input-required", fx.status(&req, &V::List(vec![])) == "input-required", String::new);

    c.group("main requires owned canonical writes and preserved bytes");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let mreq = fx.request("multi", "main", "codex_cli");
        c.ck("main expectations: .codex/config.toml and wiki-desk SKILL.md allowed",
             probe::protected_file_expectations("main") == vec![(".codex/config.toml", "allowed"), (".agents/skills/wiki-desk/SKILL.md", "allowed")], String::new);
        let out = fx.check(&mreq, &fx.result(&mreq, None));
        let lim = items(g(g(&out, "enforcement_evidence"), "limitations")).first().map(py_str).unwrap_or_default();
        c.ck("main completed · limitation says it does not prove role ownership", py_str(g(&out, "status")) == "completed"
            && lim.contains("do not prove role ownership"), || dumps(&out));
        for name in ["protected:.codex/config.toml", "protected:.agents/skills/wiki-desk/SKILL.md"] {
            for change in [r#"{"outcome": "denied"}"#, r#"{"outcome": "unobserved"}"#, r#"{"preserved": false}"#] {
                let p = edit_check(&fx.packet(&mreq), name, &j(change));
                let r = fx.check(&mreq, &fx.result(&mreq, Some(&p)));
                c.ck(&format!("{name} {change}: domain pass · enforcement unproven · input-required"),
                     py_str(g(g(&r, "domain_evidence"), "status")) == "pass"
                         && py_str(g(g(&r, "enforcement_evidence"), "status")) == "unproven"
                         && py_str(g(&r, "status")) == "input-required", || dumps(&r));
            }
            let p = without(&fx.packet(&mreq), |n| n == name);
            c.ck(&format!("without {name} → input-required"), fx.status(&mreq, &fx.result(&mreq, Some(&p))) == "input-required", String::new);
        }
    }

    c.group("sub full access still requires preserved observations");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    for topology in ["single", "multi"] {
        let sreq = fx.request(topology, "sub", "codex_cli");
        for rel in PROTECTED_FILES {
            let p = edit_check(&fx.packet(&sreq), &format!("protected:{rel}"), &j(r#"{"outcome": "denied", "expected_outcome": "allowed"}"#));
            let r = fx.check(&sreq, &fx.result(&sreq, Some(&p)));
            c.ck(&format!("{topology} {rel} denied: enforcement unproven · input-required"),
                 py_str(g(g(&r, "enforcement_evidence"), "status")) == "unproven" && py_str(g(&r, "status")) == "input-required", || dumps(&r));
        }
    }

    c.group("main write-open observation preserves the canonical fixture bytes");
    {
        let fx = Fx::new();
        for rel in [".codex/config.toml", ".agents/skills/wiki-desk/SKILL.md"] {
            let path = fx.root().join(rel);
            super::write(&path, "canonical fixture\n");
            c.ck(&format!("{rel}: allowed · preserved · bytes kept"), probe::write_open(&path) == ("allowed", Some(true))
                && super::read(&path) == "canonical fixture\n", String::new);
        }
    }

    c.group("wheel track requires the active file, not the absent source-build file");
    let (fx, req) = fresh();
    let _ = (&fx, &req);
    {
        let req = fx.request("multi", "sub", "codex_cli");
        let mut p = fx.packet(&req);
        let mut build = strs(&BUILD_FILES);
        build.push("Dockerfile".into());
        set(&mut p, "build_files", pv::strs(&build));
        let list: Vec<V> = items(g(&p, "checks")).iter().map(|x| {
            let mut x = x.clone();
            if py_str(g(&x, "name")) == "buildkit:Dockerfile.source-build" {
                set(&mut x, "name", s("buildkit:Dockerfile"));
            }
            x
        }).collect();
        set(&mut p, "checks", V::List(list));
        c.ck("wheel track with buildkit:Dockerfile → completed", fx.status(&req, &fx.result(&req, Some(&p))) == "completed", String::new);
        let p = without(&p, |n| n == "buildkit:Dockerfile");
        c.ck("without buildkit:Dockerfile → input-required", fx.status(&req, &fx.result(&req, Some(&p))) == "input-required", String::new);
    }

    c.group("a hint parent repository is rejected");
    {
        let fx = Fx::new();
        let ws = fx.root().join("child");
        super::write(&ws.join("envs/.env"), &format!("HINT_CLONE_DIR={}\n", fx.root().display()));
        let bin = TempDir::new("bootstrap-permissions-bin-");
        let log = bin.path().join("calls.log");
        stub(&bin.path().join("git"), &format!("echo \"$@\" >> {}\nexit 1\n", shlex_quote(&log.display().to_string())));
        let got = with_path(bin.path(), || probe::hint_root(&ws));
        c.ck("parent clone → None · no command run", got.is_none() && !log.exists(), String::new);
    }

    c.group("interpolated build files use observed compose profiles");
    {
        let fx = Fx::new();
        let output = fx.root().join("output/multi");
        super::write(&output.join("docker-compose.yaml"), "services:\n  worker:\n    build:\n      dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}\n");
        super::write(&output.join(".env"), "BUILD_DOCKERFILE=Dockerfile.source-build\n");
        let bin = TempDir::new("bootstrap-permissions-bin-");
        let log = bin.path().join("docker.log");
        let answer = bin.path().join("answer");
        let rcf = bin.path().join("rc");
        stub(&bin.path().join("docker"), &format!(
            "{{ pwd; for a in \"$@\"; do printf '%s\\n' \"$a\"; done; echo ---; }} >> {log}\ncat {answer}\nexit \"$(cat {rcf})\"\n",
            log = shlex_quote(&log.display().to_string()), answer = shlex_quote(&answer.display().to_string()), rcf = shlex_quote(&rcf.display().to_string())));
        let set_answer = |rc: i32, out: &str| {
            std::fs::write(&answer, out).expect("answer");
            std::fs::write(&rcf, rc.to_string()).expect("rc");
        };
        set_answer(0, r#"{"services": {"worker": {"build": {"dockerfile": "Dockerfile.source-build"}}}}"#);
        let got = with_path(bin.path(), || probe::build_files(fx.root(), "multi")).ok().flatten();
        let mut want = strs(&BUILD_FILES);
        want.push("Dockerfile.source-build".into());
        let call: Vec<String> = super::read(&log).split("---\n").next().unwrap_or("").lines().map(String::from).collect();
        let env_path = output.join(".env").display().to_string();
        c.ck("observed compose config → build files · argv has --profile * and the .env · cwd = workspace",
             got.as_ref() == Some(&want) && call.first().map(String::as_str) == Some(&fx.root().display().to_string()[..])
                 && call.iter().any(|a| a == "*") && call.iter().any(|a| a == "--profile") && call.iter().any(|a| *a == env_path),
             || format!("{got:?} {call:?}"));
        for (rc, out) in [(1, ""), (0, "not-json"), (0, r#"{"services": {}}"#), (0, r#"{"services": {"worker": {"build": {"dockerfile": "${UNRESOLVED}"}}}}"#)] {
            set_answer(rc, out);
            let got = with_path(bin.path(), || probe::build_files(fx.root(), "multi")).ok().flatten();
            c.ck(&format!("compose answer ({rc}, {out:?}) → None"), got.is_none(), || format!("{got:?}"));
        }
    }

    c.group("ambient git location and template cannot touch an existing repo");
    {
        let fx = Fx::new();
        let outside = fx.root().join("existing");
        std::fs::create_dir_all(&outside).expect("outside");
        git(fx.root(), &["init", "--quiet", "--template=", &outside.display().to_string()]);
        git(&outside, &["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "Existing"]);
        let gitdir = outside.join(".git");
        let snap = snapshot(&[&gitdir], &outside);
        let own = fx.root().join("own");
        std::fs::create_dir_all(&own).expect("own");
        let template = fx.root().join("bad-template");
        super::write(&template.join("must-not-copy"), "fixture");
        let o = |rel: &str| outside.join(rel).display().to_string();
        let poison: Vec<(&str, String)> = vec![("GIT_DIR", o(".git")), ("GIT_WORK_TREE", outside.display().to_string()),
            ("GIT_INDEX_FILE", o(".git/index")), ("GIT_TEMPLATE_DIR", template.display().to_string()), ("GIT_CONFIG_COUNT", "1".into()),
            ("GIT_CONFIG_KEY_0", "core.worktree".into()), ("GIT_CONFIG_VALUE_0", outside.display().to_string())];
        let saved: Vec<(&str, Option<std::ffi::OsString>)> = poison.iter().map(|(k, _)| (*k, std::env::var_os(k))).collect();
        for (k, v) in &poison {
            std::env::set_var(k, v);
        }
        let init = probe::command(&["git", "init", "--quiet", "--template="], &own).0;
        let dirs = probe::git_dirs(&own);
        let commit = probe::command(&["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "-c", "core.hooksPath=/dev/null",
            "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "Scratch"], &own).0;
        let output = own.join("output/multi");
        super::write(&output.join("docker-compose.yaml"), "services:\n  vllm:\n    build:\n      dockerfile: Dockerfile\n");
        super::write(&output.join("Dockerfile"), "FROM fixture\n");
        super::write(&output.join("requirements.txt"), "fixture\n");
        super::write(&output.join("manifest.yaml"), "model_source: ephemeral\n");
        let bin = TempDir::new("bootstrap-permissions-bin-");
        fixture_tools(bin.path(), r#"{"query_tokens":["fixture"]}"#);
        let observed = with_path(bin.path(), || probe::observe(&own, "multi", "main", &"cd".repeat(16))).unwrap_or(V::None);
        for (k, v) in saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        let scratch = items(g(&observed, "checks")).iter().find(|x| py_str(g(x, "name")) == "scratch_local_git").map(|x| py_str(g(x, "outcome")));
        c.ck("poisoned env: own init/commit rc 0 · git dirs = own/.git · scratch_local_git allowed",
             init == Some(0) && commit == Some(0) && dirs == Some(vec![own.join(".git")]) && scratch.as_deref() == Some("allowed"),
             || format!("{init:?} {commit:?} {dirs:?} {scratch:?}"));
        c.ck("no template copy · existing repo bytes unchanged", !own.join(".git/must-not-copy").exists() && snapshot(&[&gitdir], &outside) == snap, String::new);
    }
    c.finish()
}
