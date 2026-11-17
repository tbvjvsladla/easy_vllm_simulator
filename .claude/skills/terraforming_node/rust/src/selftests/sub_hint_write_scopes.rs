//! Port of `selftest_sub_hint_write_scopes.py` — offline hint write-scope declaration/rendering checks; no claim of
//! native permission enforcement.
//!
//! Calls the Rust renderer (`render_sub_env`) and hint-equip judge (`check_sub_hint_equip`) libraries directly, as the
//! original imported their Python modules. Two documented differences follow the port itself:
//! * the renderer delivers the probe launcher `harness_permission_probe.sh` (not the `.py`), so the delivery check
//!   compares that file's bytes;
//! * `render_tree` returns the scope verdict (`status`, `write_paths`), not the whole validated declaration, so the
//!   "approved_by carried through" check asserts the declared verdict and its write paths.
//!
//! The original wrapped `subprocess.run` to see each git environment; here a `git` wrapper first on `PATH` records the
//! environment of every call and then runs the real git.

use super::{git, j, path_with, read, snapshot, stub, Checks, TempDir};
use crate::check_sub_hint_equip::check_clone_value;
use crate::relay::pv::{self, g, py_str, s, set, V};
use crate::render_sub_env::{
    apply_mode_blocks, build_placeholders, load_hint_write_scopes, map_set, parse_manifest_text, render_text,
    render_tree, validate_hint_write_scopes, Ctx, MVal, Ph,
};
use crate::render_sub_env::toml::{self, Toml};
use std::path::Path;

/// The renderer's own fixture manifest (`render_sub_env.FIXTURE_MANIFEST`, also in `render_sub_env/selftest.rs`).
const FIXTURE_MANIFEST: &str = "topology: multi
cpu_arch: \"aarch64\"
cuda_version: \"132\"
gpus_per_node: 1
gpu_model: \"TEST-GPU\"
nas_model_path: /srv/test-models
model_source: nas
interconnect:
  type: RoCE v2
  hca_devices: [testhca0, testhca1]
  gid_index: 3
  socket_iface: testif0
  mtu: 9000
  bandwidth_gbps: 200
  platform_preset: test-preset
nodes:
  - role: main
    host: 203.0.113.10
    ssh_user: tester
    work_dir: /srv/tester/ws
  - role: sub
    host: 203.0.113.11
    ssh_user: tester
    work_dir: /srv/tester/ws
";

fn placeholders(ctx: &Ctx, topology: &str) -> Ph {
    let mut data = parse_manifest_text(FIXTURE_MANIFEST).unwrap_or_else(|e| panic!("fixture manifest: {}", e.text()));
    map_set(&mut data.0, "topology", MVal::Str(topology.into()));
    let (ph, missing) = build_placeholders(ctx, &data, None).unwrap_or_else(|e| panic!("placeholders: {}", e.text()));
    assert!(missing.is_empty(), "fixture incomplete: {missing:?}");
    ph
}

fn declaration(ph: &Ph) -> V {
    let q = |k: &str| s(ph.get(k).map(String::as_str).unwrap_or(""));
    let mut d = j(r#"{"schema_version": 1, "topology": "single", "target": {},
        "observed_home_dir": "/Users/tester", "hint_clone_dir": "/srv/hint checkout",
        "git_dir": "/srv/hint checkout/.git", "git_common_dir": "/srv/hint checkout/.git",
        "observation_source": "operator terminal report fixture", "observed_utc": "2026-10-27T04:00:00Z",
        "approved_by": "existing user scope decision fixture", "approved_utc": "2026-10-27T03:00:00Z",
        "evidence": "existing approved task fixture"}"#);
    let target = pv::obj(vec![("host", q("SUB_HOST")), ("ssh_user", q("SSH_USER")), ("work_dir", q("WORKSPACE_PATH"))]);
    set(&mut d, "target", target);
    d
}

/// `profile(ph, scopes)`: the rendered Codex profile as `(key, value-text)` pairs, after the scope validation.
fn profile(ctx: &Ctx, ph: &Ph, scopes: Option<&V>) -> Result<Vec<(String, String)>, String> {
    let txt = read(&Path::new(&ctx.subnode_dir()).join("codex.config.template.toml"));
    let txt = apply_mode_blocks(&txt, ph.get("SUB_MODE").map(String::as_str).unwrap_or("")).map_err(|e| e.text())?;
    validate_hint_write_scopes(ph, scopes).map_err(|e| e.text())?;
    flat(&toml::loads(&render_text(&txt, ph)).map_err(|e| format!("{e:?}"))?)
}

fn flat(t: &Toml) -> Result<Vec<(String, String)>, String> {
    Ok(t.keys().into_iter().map(|k| {
        let v = match t.get(&k) {
            Some(Toml::Str(x)) => x.clone(),
            Some(other) => format!("{other:?}"),
            None => String::new(),
        };
        (k, v)
    }).collect())
}

fn builtin() -> Vec<(String, String)> {
    vec![("default_permissions".into(), ":danger-full-access".into()), ("approval_policy".into(), "never".into())]
}

fn rejected(ph: &Ph, value: Option<&V>) -> bool {
    matches!(validate_hint_write_scopes(ph, value), Err(e) if e.text().contains("HINT_WRITE_SCOPES_INVALID"))
}

fn paths(ph: &Ph, d: &V) -> Option<Vec<String>> {
    validate_hint_write_scopes(ph, Some(d)).ok().map(|x| x.write_paths)
}

fn with(d: &V, k: &str, v: V) -> V {
    let mut x = d.clone();
    set(&mut x, k, v);
    x
}

pub fn run() -> i32 {
    let mut c = Checks::new("selftest_sub_hint_write_scopes");
    let ctx = Ctx::from_owner();
    let ph = placeholders(&ctx, "single");
    let multi = placeholders(&ctx, "multi");
    let decl = declaration(&ph);
    let st = |k: &str| py_str(g(&decl, k));

    c.group("common builtin permissions preserve scope records");
    for (name, p, scopes) in [("single/none", &ph, None), ("single/declared", &ph, Some(&decl)), ("multi/none", &multi, None)] {
        let got = profile(&ctx, p, scopes);
        c.ck(&format!("{name}: profile = built-in full access"), got.as_ref() == Ok(&builtin()), || format!("{got:?}"));
    }
    c.ck("validated write paths = clone + git dir", paths(&ph, &decl) == Some(vec![st("hint_clone_dir"), st("git_dir")]),
         || format!("{:?}", paths(&ph, &decl)));

    c.group("linked worktree metadata is explicit, not inferred");
    {
        let d = with(&with(&decl, "git_dir", s("/srv/hint-meta.git/worktrees/publish")), "git_common_dir", s("/srv/hint-meta.git"));
        c.ck("linked worktree: three write paths", paths(&ph, &d) == Some(vec![st("hint_clone_dir"),
             "/srv/hint-meta.git/worktrees/publish".into(), "/srv/hint-meta.git".into()]), || format!("{:?}", paths(&ph, &d)));
        c.ck("unrelated git dir rejected", rejected(&ph, Some(&with(&d, "git_dir", s("/srv/unrelated.git")))), String::new);
    }

    c.group("multi rejects a single hint scope");
    c.ck("multi + declaration rejected", rejected(&multi, Some(&decl)), String::new);

    c.group("target identity is exact");
    for key in ["host", "ssh_user", "work_dir"] {
        let mut d = decl.clone();
        let mut t = g(&d, "target").clone();
        let changed = s(&format!("{}-other", py_str(g(&t, key))));
        set(&mut t, key, changed);
        set(&mut d, "target", t);
        c.ck(&format!("target.{key} changed → rejected"), rejected(&ph, Some(&d)), String::new);
    }

    c.group("observation and approval are both required");
    for key in ["observation_source", "observed_utc", "approved_by", "approved_utc", "evidence"] {
        c.ck(&format!("{key}=pending rejected"), rejected(&ph, Some(&with(&decl, key, s("pending")))), String::new);
    }
    c.ck("unexpected key rejected", rejected(&ph, Some(&with(&decl, "unexpected_permission", V::Bool(true)))), String::new);
    c.ck("schema_version=True rejected", rejected(&ph, Some(&with(&decl, "schema_version", V::Bool(true)))), String::new);

    c.group("roots, home, workspace, NAS and private config are rejected");
    {
        let ws = ph.get("WORKSPACE_PATH").cloned().unwrap_or_default();
        let parent = Path::new(&ws).parent().map(|p| p.display().to_string()).unwrap_or_default();
        let nas = format!("{}/hint", ph.get("NAS_MOUNT").cloned().unwrap_or_default());
        let mut list: Vec<String> = ["/", "/tmp", "/root", "/unlisted-root", "/home", "/Users/tester", "/Users/other", "/srv",
            "/etc/policy", "/usr/local/tool"].iter().map(|x| x.to_string()).collect();
        list.extend([ws.clone(), format!("{ws}/nested"), parent, nas]);
        list.extend(["/Users/tester/.ssh/clone", "/Users/tester/.codex/clone", "/Users/tester/.config/clone"].iter().map(|x| x.to_string()));
        for path in list {
            c.ck(&format!("hint_clone_dir {path:?} rejected"), rejected(&ph, Some(&with(&decl, "hint_clone_dir", s(&path)))), String::new);
        }
    }

    c.group("ambiguous path syntax fails closed");
    for path in ["relative/repo", "~/repo", "//srv/repo", "/srv/../hint", "/srv/./hint", "/srv/hint/", "/srv/*", "/srv/a?",
                 "/srv/[ab]", "/srv/{ab}", "/srv/a\\b", "/srv/a\nb"] {
        c.ck(&format!("hint_clone_dir {path:?} rejected"), rejected(&ph, Some(&with(&decl, "hint_clone_dir", s(path)))), String::new);
    }

    c.group("scope paths do not become permission profiles");
    {
        let clone = "/srv/hint \"출판\" # key = \"deny\"";
        let d = with(&with(&with(&decl, "hint_clone_dir", s(clone)), "git_dir", s(&format!("{clone}/.git"))),
                     "git_common_dir", s(&format!("{clone}/.git")));
        c.ck("quoted clone path validates verbatim", paths(&ph, &d) == Some(vec![clone.into(), format!("{clone}/.git")]),
             || format!("{:?}", paths(&ph, &d)));
        let prof = profile(&ctx, &ph, Some(&d));
        c.ck("profile has no permissions table", prof.as_ref().is_ok_and(|p| !p.iter().any(|(k, _)| k == "permissions")),
             || format!("{prof:?}"));
    }

    c.group("a bad declaration cannot delete existing staging");
    {
        let td = TempDir::new("hint-scopes-");
        let marker = td.path().join("keep");
        std::fs::write(&marker, "existing staging").expect("marker");
        let out = render_tree(&ctx, &ph, &td.path().display().to_string(), false, None, None, Some(&j("{}")));
        c.ck("render_tree with {} fails · staging kept", out.is_err() && read(&marker) == "existing staging", String::new);
    }

    c.group("the real renderer wires scopes and both-role probe delivery");
    {
        let td = TempDir::new("hint-scopes-render-");
        let single = render_tree(&ctx, &ph, &td.path().join("single").display().to_string(), false, None, None, Some(&decl));
        let multi_out = render_tree(&ctx, &multi, &td.path().join("multi").display().to_string(), false, None, None, None);
        match (single, multi_out) {
            (Ok(single), Ok(multi_out)) => {
                let cfg = toml::loads(&read(&Path::new(&single.out_dir).join(".codex/config.toml"))).map_err(|e| format!("{e:?}")).and_then(|t| flat(&t));
                c.ck("rendered .codex/config.toml = built-in full access", cfg.as_ref() == Ok(&builtin()), || format!("{cfg:?}"));
                c.ck("render_tree returns the declared scope verdict and its write paths",
                     single.scopes.status == "declared" && single.scopes.write_paths == vec![st("hint_clone_dir"), st("git_dir")],
                     || format!("{:?}", single.scopes));
                let helper = ".claude/skills/terraforming_node/scripts/harness_permission_probe.sh";
                let real = std::fs::read(super::scripts_dir().join("harness_permission_probe.sh")).unwrap_or_default();
                for (name, r) in [("single", &single), ("multi", &multi_out)] {
                    let joined = r.produced.join("\n");
                    c.ck(&format!("{name}: probe launcher delivered byte-identical · no bootstrap-permissions selftest"),
                         r.produced.iter().any(|p| p == helper)
                             && std::fs::read(Path::new(&r.out_dir).join(helper)).ok().as_deref() == Some(&real[..])
                             && !joined.contains("selftest_bootstrap_permissions.sh")
                             && !joined.contains("scripts/selftest_bootstrap_permissions.sh"),
                         || joined.lines().filter(|l| l.contains("probe") || l.contains("selftest_bootstrap")).collect::<Vec<_>>().join(", "));
                }
            }
            (a, b) => c.ck("render_tree single and multi succeed", false, || format!("{:?} {:?}",
                a.err().map(|e| e.text()), b.err().map(|e| e.text()))),
        }
    }

    c.group("JSON duplicates and non-objects are rejected");
    {
        let td = TempDir::new("hint-scopes-json-");
        let path = td.path().join("scopes.json");
        let ps = path.display().to_string();
        for text in ["null", "[]", r#"{"target":{},"target":{}}"#] {
            std::fs::write(&path, text).expect("scopes");
            let r = load_hint_write_scopes(Some(&ps));
            c.ck(&format!("{text} → HINT_WRITE_SCOPES_INVALID"), matches!(&r, Err(e) if e.text().contains("HINT_WRITE_SCOPES_INVALID")), String::new);
        }
        std::fs::write(&path, super::dumps(&decl)).expect("scopes");
        let r = load_hint_write_scopes(Some(&ps));
        c.ck("declaration round-trips", matches!(&r, Ok(Some(v)) if pv::eq(v, &decl)), String::new);
    }

    c.group("real local git observation does not invent approval");
    {
        let td = TempDir::new("hint-scopes-git-");
        let base = td.path();
        let (ws, clone, worktree) = (base.join("ws"), base.join("hint"), base.join("linked"));
        std::fs::create_dir_all(ws.join("envs")).expect("ws");
        git(base, &["init", "-q", "-b", "hint", &clone.display().to_string()]);
        git(&clone, &["commit", "--allow-empty", "-qm", "fixture"]);
        git(&clone, &["remote", "add", "origin", "https://example.invalid/project"]);
        git(&clone, &["worktree", "add", "-qb", "fixture", &worktree.display().to_string()]);
        std::fs::write(ws.join("envs/.env"), format!("HINT_CLONE_DIR={}\nGITHUB_TOKEN=fixture_not_a_secret\n", worktree.display())).expect("env");
        let obs = check_clone_value(&ws).unwrap_or(V::None);
        c.ck("linked worktree observed: ok · clone · common dir · git dir · no approval · no secret",
             py_str(g(&obs, "status")) == "ok"
                 && py_str(g(&obs, "hint_clone_dir")) == worktree.display().to_string()
                 && py_str(g(&obs, "git_common_dir")) == clone.join(".git").display().to_string()
                 && py_str(g(&obs, "git_dir")) == clone.join(".git/worktrees/linked").display().to_string()
                 && !pv::has(&obs, "approved_by")
                 && !super::dumps(&obs).contains("fixture_not_a_secret"),
             || super::dumps(&obs));
    }

    c.group("inherited git location and config cannot redirect the observation");
    {
        let td = TempDir::new("hint-scopes-poison-");
        let base = td.path();
        let (ws, clone, other) = (base.join("ws"), base.join("hint"), base.join("unrelated"));
        std::fs::create_dir_all(ws.join("envs")).expect("ws");
        for p in [&clone, &other] {
            git(base, &["init", "-q", "-b", "hint", &p.display().to_string()]);
            git(p, &["commit", "--allow-empty", "-qm", "fixture"]);
        }
        git(&clone, &["remote", "add", "origin", "https://example.invalid/approved"]);
        git(&other, &["remote", "add", "origin", "http://example.invalid/unrelated"]);
        std::fs::write(ws.join("envs/.env"), format!("HINT_CLONE_DIR={}\n", clone.display())).expect("env");
        let o = |rel: &str| other.join(rel).display().to_string();
        let poison: Vec<(&str, String)> = vec![
            ("GIT_DIR", o(".git")), ("GIT_WORK_TREE", other.display().to_string()), ("GIT_COMMON_DIR", o(".git")),
            ("GIT_INDEX_FILE", o(".git/index")), ("GIT_OBJECT_DIRECTORY", o(".git/objects")),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", o(".git/objects")), ("GIT_CONFIG_COUNT", "1".into()),
            ("GIT_CONFIG_KEY_0", "remote.origin.url".into()), ("GIT_CONFIG_VALUE_0", "http://example.invalid/injected".into()),
            ("GIT_CONFIG_PARAMETERS", "'remote.origin.url'='http://example.invalid/injected'".into()),
            ("GIT_CONFIG_GLOBAL", o(".git/config")), ("GIT_CONFIG_SYSTEM", o(".git/config")),
            ("GIT_SSH_COMMAND", "fixture-preserve-ssh -F /fixture/config".into()), ("GIT_SSH_VARIANT", "ssh".into()),
            ("SSH_AUTH_SOCK", "/fixture/agent.sock".into()),
        ];
        // The negative control actually observes the unrelated repository.
        let redirected = std::process::Command::new("git").args(["rev-parse", "--absolute-git-dir"]).current_dir(&clone)
            .envs(poison.iter().map(|(k, v)| (*k, v.as_str()))).output().expect("git");
        c.ck("negative control: the poison redirects plain git", String::from_utf8_lossy(&redirected.stdout).trim() == o(".git"),
             || String::from_utf8_lossy(&redirected.stdout).into_owned());
        let before = snapshot(&[&clone, &other, &ws], base);
        // A git wrapper first on PATH records each call's environment, then runs the real git.
        let real_git = crate::proc::which("git").unwrap_or_else(|| "/usr/bin/git".into());
        let bin = TempDir::new("hint-scopes-bin-");
        let log = bin.path().join("env.log");
        stub(&bin.path().join("git"), &format!("{{ printf -- '---\\n'; env; }} >> {}\nexec {} \"$@\"\n",
             crate::agent_control::pyhelp::shlex_quote(&log.display().to_string()), crate::agent_control::pyhelp::shlex_quote(&real_git)));
        let saved_path = std::env::var_os("PATH");
        let saved: Vec<(&str, Option<std::ffi::OsString>)> = poison.iter().map(|(k, _)| (*k, std::env::var_os(k))).collect();
        std::env::set_var("PATH", path_with(bin.path()));
        for (k, v) in &poison {
            std::env::set_var(k, v);
        }
        let result = check_clone_value(&ws).unwrap_or(V::None);
        for (k, v) in saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        match saved_path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        c.ck("observation: ok · https · clone · own git dirs",
             py_str(g(&result, "status")) == "ok" && py_str(g(&result, "origin_scheme")) == "https"
                 && py_str(g(&result, "hint_clone_dir")) == clone.display().to_string()
                 && py_str(g(&result, "git_dir")) == clone.join(".git").display().to_string()
                 && py_str(g(&result, "git_common_dir")) == clone.join(".git").display().to_string(),
             || super::dumps(&result));
        let calls: Vec<Vec<(String, String)>> = read(&log).split("---\n").filter(|b| !b.trim().is_empty()).map(|b| {
            b.lines().filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect()
        }).collect();
        let allowed = ["GIT_SSH_COMMAND", "GIT_SSH_VARIANT", "SSH_AUTH_SOCK"];
        let envs_ok = !calls.is_empty() && calls.iter().all(|env| {
            let get = |k: &str| env.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            !env.iter().any(|(k, _)| k.starts_with("GIT_CONFIG"))
                && !poison.iter().any(|(k, _)| !allowed.contains(k) && get(k).is_some())
                && get("GIT_SSH_COMMAND").as_deref() == Some("fixture-preserve-ssh -F /fixture/config")
                && get("SSH_AUTH_SOCK").as_deref() == Some("/fixture/agent.sock")
                && get("GIT_OPTIONAL_LOCKS").as_deref() == Some("0")
        });
        c.ck(&format!("every git call ({}) drops location/config, keeps ssh, sets GIT_OPTIONAL_LOCKS=0", calls.len()), envs_ok, String::new);
        c.ck("nothing in the three trees changed", snapshot(&[&clone, &other, &ws], base) == before, String::new);
    }
    c.finish()
}
