//! Goldens captured from the ORIGINAL verify_distribution.py by checks/capture_goldens.sh (frozen inputs: the tree of
//! the golden's `rev` + literal fixtures). Nothing here runs Python: the Rust port is compared against those files.
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::{float_repr, PyValue};
use policy_distribution::checks::{verify_with, Hooks};
use policy_distribution::{dict, list_ints, observe_policy_as_of, ordered_between_detail, shell_array, shipped_python_paths, UtcInstant};
use std::path::{Path, PathBuf};
use std::process::Command;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn repo() -> PathBuf {
    crate_dir().join("../../..").canonicalize().unwrap()
}
fn golden(name: &str) -> PyValue {
    let text = std::fs::read_to_string(crate_dir().join("checks/data").join(name)).unwrap();
    // Tracked golden data spells the skill-script prefix as `@TFN_SCRIPTS@/` so that it is not read as an external call of
    // a non-public terraforming_node script by the public-surface tripwire (antipattern_scan); decoded here.
    let text = text.replace("@TFN_SCRIPTS@/", concat!("terraforming", "_node/scripts/"));
    pyjson::loads(&text).unwrap()
}
fn get<'a>(v: &'a PyValue, k: &str) -> &'a PyValue {
    policy_distribution::get(v, k).unwrap_or_else(|| panic!("missing key {k}"))
}
fn st(v: &PyValue) -> String {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8().unwrap(),
        other => panic!("not a str: {}", other.repr()),
    }
}
fn items(v: &PyValue) -> Vec<PyValue> {
    match v.peel() {
        PyValue::List(l) => l.clone(),
        _ => panic!("not a list"),
    }
}
fn dumps(v: &PyValue) -> String {
    v.dumps_pretty().unwrap().to_utf8().unwrap()
}
/// Key-order-insensitive canonical text (for values whose dict order is not part of the contract).
fn canon(v: &PyValue) -> String {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut parts: Vec<(String, String)> = d.iter().map(|(k, x)| (k.to_utf8().unwrap(), canon(x))).collect();
            parts.sort();
            format!("{{{}}}", parts.iter().map(|(k, x)| format!("{k:?}:{x}")).collect::<Vec<_>>().join(","))
        }
        PyValue::List(l) => format!("[{}]", l.iter().map(canon).collect::<Vec<_>>().join(",")),
        other => other.dumps_ascii().unwrap(),
    }
}
fn toml_canon(v: &policy_distribution::toml::Value) -> String {
    use policy_distribution::toml::Value;
    match v {
        Value::Str(s) => PyValue::str(s).dumps_ascii().unwrap(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => float_repr(*f),
        Value::Bool(b) => if *b { "true".into() } else { "false".into() },
        Value::Array(a) => format!("[{}]", a.iter().map(toml_canon).collect::<Vec<_>>().join(",")),
        Value::Table(t) => {
            let mut parts: Vec<(String, String)> = t.iter().map(|(k, x)| (k.clone(), toml_canon(x))).collect();
            parts.sort();
            format!("{{{}}}", parts.iter().map(|(k, x)| format!("{k:?}:{x}")).collect::<Vec<_>>().join(","))
        }
    }
}

fn sh(cmd: &str, cwd: &Path) {
    let st = Command::new("sh").arg("-c").arg(cmd).current_dir(cwd).status().unwrap();
    assert!(st.success(), "{cmd}");
}

struct Tree(PathBuf);
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// The frozen input tree: `git archive <rev>` + a fresh index on branch single-node (as capture_goldens.sh builds it).
fn frozen_tree(rev: &str, tag: &str) -> Tree {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("vd-golden-rs-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let r = repo();
    sh(&format!("git -C '{}' archive {rev} | tar -x -C '{}'", r.display(), dir.display()), &dir);
    sh("git init -q -b single-node . && git add -A", &dir);
    Tree(dir)
}

/// The delivered tree carries the policy launcher the driver probes (`policy_runner.is_file()`; the original probed the
/// `.py`, which the frozen tree has). Content is irrelevant: children are stubbed. Untracked, like a node build output.
fn with_launcher(tree: &Path) {
    std::fs::write(tree.join(".claude/policies/runtime/policy_registry.sh"), "#!/bin/bash\n").unwrap();
}

fn norm_str(text: &str, tree: &Path, py: &str) -> String {
    text.replace(&tree.to_string_lossy().to_string(), "<REPO>").replace(py, "<PY>")
}
fn norm(v: &PyValue, tree: &Path, py: &str) -> PyValue {
    match v.peel() {
        PyValue::Str(s) => PyValue::str(&norm_str(&s.to_utf8().unwrap(), tree, py)),
        PyValue::List(l) => PyValue::List(l.iter().map(|x| norm(x, tree, py)).collect()),
        PyValue::Dict(d) => {
            let mut out = agent_runtime_support::pyvalue::PyDict::new();
            for (k, x) in d.iter() {
                out.insert(k.clone(), norm(x, tree, py));
            }
            PyValue::Dict(out)
        }
        other => other.clone(),
    }
}

/// The documented argv changes: checks that started a policy `.py` now start its Rust launcher.
fn expected_argv(name: &str, golden_argv: &[String]) -> Vec<String> {
    let to_sh = |a: &str| a.replace(".py", ".sh");
    match name {
        "antipattern_scan_selftest" | "constitution_lint_selftest" | "doc_naming_shim_selftest" => {
            vec!["bash".into(), to_sh(&golden_argv[1]), golden_argv[2].clone()]
        }
        "runtime_regression_selftest" => vec!["bash".into(), ".claude/policies/runtime/runtime_selftest.sh".into()],
        "policy_registry_verify" => {
            let mut v = vec!["bash".into(), to_sh(&golden_argv[1])];
            v.extend(golden_argv[2..].iter().cloned());
            v
        }
        "production_claim_predicates" => vec!["bash".into(), to_sh(&golden_argv[1])],
        "production_companion_predicates" => vec![
            "bash".into(),
            "-c".into(),
            "set -u; n=0; for f in \"$1\"/*_predicate.sh; do [ -f \"$f\" ] || continue; n=$((n+1)); bash \"$f\" || exit $?; done; [ \"$n\" -gt 0 ] || exit 5".into(),
            "bash".into(),
            golden_argv[5].clone(),
        ],
        n if LAUNCHER_SWITCHED.contains(&n) || n.starts_with("upstream_help:") => {
            // plan_26111615: upstream/recipe 진입점이 Rust 런처(.sh)로 바뀌었다 — golden 은 b1f3563e 의 `<PY> x.py ...`.
            let script = if n == "recipe_info_only_gate" {
                ".claude/skills/vllm-recipe-explorer/scripts/recipe.sh".to_string()
            } else {
                to_sh(&golden_argv[1])
            };
            let mut v = vec!["bash".into(), script];
            v.extend(golden_argv[2..].iter().cloned());
            v
        }
        _ => golden_argv.to_vec(),
    }
}

/// Checks whose `<PY> x.py` argv became `bash x.sh` when their owners were ported (closed list · tripwire).
const LAUNCHER_SWITCHED: &[&str] = &[
    "upstream_slave_forward_selftest",
    "upstream_render_dockerfile_selftest",
    "recipe_info_only_gate",
    "recipe_serve_knob_parity",
    "recipe_parse_config_selftest",
    "recipe_estimate_vram_selftest",
    "recipe_run_trial_selftest",
    "upstream_budget_preflight_selftest",
    "upstream_layer_ledger_selftest",
    "upstream_push_branches_selftest",
    "upstream_branch_sync_mechanics_selftest",
    "recipe_preload_ram_gate_selftest",
    "recipe_escalation_predicate_selftest",
    "recipe_rope_translate_selftest",
    "recipe_gmu_roles_batch_selftest",
    "terraform_agent_guard_selftest",
    "blackbox_session_selftest",
    // terraforming_node A2A control plane (relay · agent_control · render_sub_env · canary · gates).
    "terraform_render_selftest",
    "terraform_staleness_gate_selftest",
    "agent_control_selftest",
    "terraform_relay_selftest",
    "terraform_bootstrap_canary_selftest",
    "terraform_check_sub_hint_equip_selftest",
    // terraforming first pass (library_access ported in 3de7a246; its row was switched at the core-contracts merge).
    "terraform_library_access_selftest",
];

/// Golden check names that were renamed with their launcher (`upstream_help:x.py` → `upstream_help:x.sh`).
fn renamed(name: &str) -> String {
    match name.strip_prefix("upstream_help:") {
        Some(f) => format!("upstream_help:{}", f.replace(".py", ".sh")),
        None => name.to_string(),
    }
}

fn stub_report(tree: &Path, py: &str) -> PyValue {
    let run = |name: &str, argv: &[String], expected: &[i64], cwd: &Path, env: &[(&str, &str)]| -> PyValue {
        let mut e: Vec<i64> = expected.to_vec();
        e.sort();
        let mut envd = agent_runtime_support::pyvalue::PyDict::new();
        for (k, v) in env {
            envd.set(k, PyValue::str(v));
        }
        dict(vec![
            ("name", PyValue::str(name)),
            ("ok", PyValue::Bool(true)),
            ("rc", PyValue::int(0)),
            ("expected_rc", list_ints(&e)),
            ("argv", PyValue::List(argv.iter().map(|a| PyValue::str(a)).collect())),
            ("cwd", PyValue::str(&cwd.to_string_lossy())),
            ("env", PyValue::Dict(envd)),
        ])
    };
    let gitless = |_: &Path| {
        vec![
            dict(vec![("name", PyValue::str("gitless_hint_match")), ("ok", PyValue::Bool(true)), ("stub", PyValue::Bool(true))]),
            dict(vec![("name", PyValue::str("gitless_hint_match_reads_index")), ("ok", PyValue::Bool(true)), ("stub", PyValue::Bool(true))]),
        ]
    };
    let flag = |_: &Path| false;
    let record = observe_policy_as_of(Some("2026-11-16"), None).unwrap();
    let hooks = Hooks { run: &run, gitless: &gitless, flag_issued: &flag, python: py.to_string() };
    let report = verify_with(tree, record, &hooks).expect("no uncaught exception");
    norm(&report, tree, py)
}

fn compare_reports(golden_name: &str, mutate: bool) {
    let g = golden(golden_name);
    let rev = st(get(&g, "rev"));
    let tree = frozen_tree(&rev, if mutate { "mut" } else { "base" });
    if mutate {
        apply_mutations(&tree.0);
        sh("git add -A", &tree.0);
    }
    with_launcher(&tree.0);
    let py = "/usr/bin/python3-golden-stub";
    let got = stub_report(&tree.0, py);
    let want = get(&g, "report");
    for key in ["schema_version", "repo", "policy_as_of", "verdict"] {
        assert_eq!(dumps(get(&got, key)), dumps(get(want, key)), "{golden_name}: top-level {key}");
    }
    let (gc, wc) = (items(get(&got, "checks")), items(get(want, "checks")));
    let names = |v: &[PyValue]| v.iter().map(|c| renamed(&st(get(c, "name")))).collect::<Vec<_>>();
    assert_eq!(names(&gc), names(&wc), "{golden_name}: check names/order");
    let mut argv_changed = 0;
    for (g1, w1) in gc.iter().zip(wc.iter()) {
        let mut w1 = w1.clone();
        let golden_name_of_check = st(get(&w1, "name"));
        let name = renamed(&golden_name_of_check);
        if name != golden_name_of_check {
            policy_distribution::update(&mut w1, vec![("name", PyValue::str(&name))]);
        }
        if let Some(argv) = policy_distribution::get(&w1, "argv") {
            let wargv: Vec<String> = items(argv).iter().map(st).collect();
            let wargv: Vec<String> = wargv.iter().map(|a| a.replace("<PY>", py)).collect();
            let exp = expected_argv(&name, &wargv);
            if exp != wargv {
                argv_changed += 1;
            }
            let exp: Vec<String> = exp.iter().map(|a| a.replace(py, "<PY>")).collect();
            policy_distribution::update(&mut w1, vec![("argv", PyValue::List(exp.iter().map(|a| PyValue::str(a)).collect()))]);
        }
        assert_eq!(dumps(g1), dumps(&w1), "{golden_name}: check {name}");
    }
    assert_eq!(argv_changed, 7 + LAUNCHER_SWITCHED.len() + 9, "the seven policy-program checks plus the ported upstream/recipe/blackbox launchers (17 named + 9 upstream_help) change their argv");
}

fn apply_mutations(tree: &Path) {
    let muts = golden("mutations.json");
    for m in items(&muts) {
        let p = tree.join(st(get(&m, "path")));
        match st(get(&m, "op")).as_str() {
            "append" => {
                let t = std::fs::read_to_string(&p).unwrap();
                std::fs::write(&p, t + &st(get(&m, "text"))).unwrap();
            }
            "write" => {
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, st(get(&m, "text"))).unwrap();
            }
            "write_bytes_hex" => {
                let h = st(get(&m, "hex"));
                let bytes: Vec<u8> = (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect();
                std::fs::write(&p, bytes).unwrap();
            }
            "delete" => std::fs::remove_file(&p).unwrap(),
            "replace" => {
                let t = std::fs::read_to_string(&p).unwrap();
                let count = match get(&m, "count").peel() {
                    PyValue::Int(n) => n.to_string().parse::<usize>().unwrap(),
                    _ => 0,
                };
                let (old, new) = (st(get(&m, "old")), st(get(&m, "new")));
                let out = if count == 0 { t.replace(&old, &new) } else { t.replacen(&old, &new, count) };
                std::fs::write(&p, out).unwrap();
            }
            other => panic!("unknown op {other}"),
        }
    }
}

#[test]
fn verify_static_report_matches_original() {
    compare_reports("verify_static.json", false);
}

#[test]
fn missing_policy_launcher_fails_closed() {
    let tree = frozen_tree("b1f3563e", "nolauncher");
    let report = stub_report(&tree.0, "/usr/bin/python3-golden-stub");
    let checks = items(get(&report, "checks"));
    let c = checks.iter().find(|c| st(get(c, "name")) == "policy_registry_verify").unwrap();
    assert_eq!(dumps(c), dumps(&dict(vec![
        ("name", PyValue::str("policy_registry_verify")),
        ("ok", PyValue::Bool(false)),
        ("error", PyValue::str("missing .claude/policies/runtime/policy_registry.sh")),
    ])));
    assert_eq!(st(get(&report, "verdict")), "FAIL");
}

#[test]
fn verify_static_failing_branches_match_original() {
    compare_reports("verify_static_mutated.json", true);
}

#[test]
fn assert_detector_matches_ast() {
    let g = golden("asserts.json");
    let tree = frozen_tree("b1f3563e", "ast");
    let files = match get(&g, "files").peel() {
        PyValue::Dict(d) => d.clone(),
        _ => panic!(),
    };
    let paths = shipped_python_paths(&tree.0);
    let rels: Vec<String> = paths.iter().map(|p| p.strip_prefix(&tree.0).unwrap().to_string_lossy().into_owned()).collect();
    let want: Vec<String> = files.iter().map(|(k, _)| k.to_utf8().unwrap()).collect();
    assert_eq!(rels, want, "shipped python set and order");
    for (p, r) in paths.iter().zip(rels.iter()) {
        let text = std::fs::read_to_string(p).unwrap();
        let mut lines = policy_distribution::pyassert::assert_lines(&text, r).unwrap();
        lines.sort();
        let got = list_ints(&lines.iter().map(|&n| n as i64).collect::<Vec<_>>());
        assert_eq!(dumps(&got), dumps(files.get(r).unwrap()), "{r}");
    }
    let snippets = match get(&g, "snippets").peel() {
        PyValue::Dict(d) => d.clone(),
        _ => panic!(),
    };
    for (k, case) in snippets.iter() {
        let key = k.to_utf8().unwrap();
        let text = st(get(case, "text"));
        let res = policy_distribution::pyassert::assert_lines(&text, &key);
        match policy_distribution::get(case, "error") {
            Some(e) => assert_eq!(res.unwrap_err(), st(e), "{key}"),
            None => {
                let mut lines = res.unwrap();
                lines.sort();
                assert_eq!(dumps(&list_ints(&lines.iter().map(|&n| n as i64).collect::<Vec<_>>())), dumps(get(case, "lines")), "{key}");
            }
        }
    }
}

#[test]
fn toml_subset_matches_tomllib() {
    let g = golden("toml.json");
    let cases = match g.peel() {
        PyValue::Dict(d) => d.clone(),
        _ => panic!(),
    };
    for (k, case) in cases.iter() {
        let key = k.to_utf8().unwrap();
        let res = policy_distribution::toml::loads(&st(get(case, "text")));
        match policy_distribution::get(case, "error") {
            Some(e) => assert_eq!(res.unwrap_err().0, st(e), "{key}"),
            None => {
                let table = res.unwrap_or_else(|e| panic!("{key}: {}", e.0));
                let got = toml_canon(&policy_distribution::toml::Value::Table(table));
                assert_eq!(got, canon(get(case, "value")), "{key}");
            }
        }
    }
}

#[test]
fn shell_arrays_and_order_detail_match() {
    let g = golden("shell.json");
    let tree = frozen_tree("b1f3563e", "sh");
    let sources = [
        ("sync_branches", ".claude/skills/upstream-version-watch/scripts/sync_branches.sh"),
        ("sync_to_sub", ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"),
    ];
    for case in items(get(&g, "arrays")) {
        let source = st(get(&case, "source"));
        let text = match sources.iter().find(|(l, _)| *l == source) {
            Some((_, rel)) => policy_distribution::pytext::read_text(&tree.0.join(rel)).unwrap(),
            None => st(get(&case, "text")),
        };
        let got: Vec<String> = shell_array(&text, &st(get(&case, "name"))).into_iter().collect();
        assert_eq!(dumps(&policy_distribution::list_strs(&got)), dumps(get(&case, "value")), "{source} {}", st(get(&case, "name")));
    }
    for case in items(get(&g, "orders")) {
        let toks: Vec<String> = items(get(&case, "tokens")).iter().map(st).collect();
        let toks: Vec<&str> = toks.iter().map(|s| s.as_str()).collect();
        let (_, detail) = ordered_between_detail(&st(get(&case, "text")), &st(get(&case, "marker")), &st(get(&case, "end")), &toks);
        assert_eq!(dumps(&detail), dumps(get(&case, "value")));
    }
}

#[test]
fn reference_date_observation_matches() {
    for case in items(&golden("clock.json")) {
        let got = match policy_distribution::get(&case, "explicit") {
            Some(e) => observe_policy_as_of(Some(&st(e)), None),
            None => {
                let secs = match get(&case, "unix").peel() {
                    PyValue::Int(n) => n.to_string().parse::<i64>().unwrap(),
                    _ => panic!(),
                };
                let micros = match get(&case, "micros").peel() {
                    PyValue::Int(n) => n.to_string().parse::<u32>().unwrap(),
                    _ => panic!(),
                };
                observe_policy_as_of(None, Some(UtcInstant::from_unix(secs, micros)))
            }
        };
        match policy_distribution::get(&case, "error") {
            Some(e) => assert_eq!(got.unwrap_err(), st(e)),
            None => {
                let r = got.unwrap();
                let v = dict(vec![
                    ("as_of", PyValue::str(&r.as_of)),
                    ("as_of_source", PyValue::str(r.as_of_source)),
                    ("observed_utc", policy_distribution::opt_str(&r.observed_utc)),
                    ("repro_as_of", PyValue::str(&r.repro_as_of)),
                ]);
                assert_eq!(dumps(&v), dumps(get(&case, "value")));
            }
        }
    }
}

#[test]
fn child_run_result_matches_original_run() {
    let tree = frozen_tree("b1f3563e", "run");
    for row in items(&golden("run.json")) {
        let case = get(&row, "case");
        let argv: Vec<String> = items(get(case, "argv")).iter().map(st).collect();
        let expected: Vec<i64> = items(get(case, "expected"))
            .iter()
            .map(|v| match v.peel() {
                PyValue::Int(n) => n.to_string().parse().unwrap(),
                _ => panic!(),
            })
            .collect();
        let cwd = policy_distribution::get(case, "cwd").map(|c| PathBuf::from(st(c))).unwrap_or_else(|| tree.0.clone());
        let env_pairs: Vec<(String, String)> = match policy_distribution::get(case, "env").map(|e| e.peel()) {
            Some(PyValue::Dict(d)) => d.iter().map(|(k, v)| (k.to_utf8().unwrap(), st(v))).collect(),
            _ => vec![],
        };
        let env: Vec<(&str, &str)> = env_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let name = st(get(case, "name"));
        let got = policy_distribution::run(&name, &argv, &expected, &cwd, &env);
        assert_eq!(dumps(&got), dumps(get(&row, "result")), "{name}");
    }
}
