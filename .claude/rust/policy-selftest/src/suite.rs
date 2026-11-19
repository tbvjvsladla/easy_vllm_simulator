//! The full `runtime_selftest.py` regression suite (`main()` without flags), same order and messages.
//!
//! Subjects owned by other packages are exercised through their Rust libraries (policy-gate, policy-audit,
//! policy-distribution) or their launchers (`completion_gate.sh`, `evidence_publisher.sh`, `bench_reporting_api.sh`), never
//! through a Python module. The four hint-publisher tests (`_test_hint_binding_source`, `_test_hint_cli_publication`,
//! `_test_hint_cli_refute_paths`, `_test_hint_map_only_publication`) live in [`crate::hint_suite`]: fixture assembly and
//! unit judges through the `hintlib` crate, the CLI under test through its deployed launcher (`scripts/hint.sh`).
use crate::gitx;
use crate::hint_suite as hs;
use crate::pyast::{self, Kind};
use crate::pytext::{py_repr_list, py_strip};
use crate::tempdir::TempDir;
use crate::tripwires::{self as tw, require, Fail, R};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn ps(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
fn s(v: &str) -> PyValue {
    PyValue::Str(PyStr::new(v))
}
fn obj(items: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}
fn set(v: &mut PyValue, k: &str, x: PyValue) {
    if let PyValue::Dict(d) = v {
        d.set(k, x);
    }
}
fn get<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(k).map(|x| x.peel()),
        _ => None,
    }
}
fn gstr(v: &PyValue, k: &str) -> Option<String> {
    match get(v, k) {
        Some(PyValue::Str(t)) => t.to_utf8(),
        _ => None,
    }
}
fn is_true(v: &PyValue, k: &str) -> bool {
    matches!(get(v, k), Some(PyValue::Bool(true)))
}
fn codes(v: &PyValue) -> Vec<String> {
    match get(v, "reason_codes") {
        Some(PyValue::List(l)) => l.iter().filter_map(|x| if let PyValue::Str(t) = x.peel() { t.to_utf8() } else { None }).collect(),
        _ => vec![],
    }
}
fn has(v: &PyValue, code: &str) -> bool {
    codes(v).iter().any(|c| c == code)
}
fn dump(v: &PyValue) -> String {
    v.dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default()
}
fn write(p: &Path, text: &str) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(p, text).expect("fixture write");
}
fn git_ok(args: &[&str]) -> R<()> {
    let o = gitx::run("git", args, None, None);
    if o.code != 0 {
        return Err(Fail::Crash(format!("subprocess.CalledProcessError: git {} -> {}: {}", args.join(" "), o.code, o.err_text())));
    }
    Ok(())
}
fn sha256_hex(b: &[u8]) -> String {
    tw::hex(&Sha256::digest(b))
}

fn expect_fail(r: R<()>) -> R<Option<String>> {
    match r {
        Ok(()) => Ok(None),
        Err(Fail::Selftest(m)) => Ok(Some(m)),
        Err(c) => Err(c),
    }
}

pub fn assert_offenders(root: &Path) -> R<Vec<String>> {
    let mut out = Vec::new();
    for path in tw::walk_governing_files(root, "*.py") {
        let text = std::fs::read(&path).map_err(|e| Fail::Crash(format!("OSError: {e}: {}", ps(&path))))?;
        let text = String::from_utf8(text).map_err(|_| Fail::Crash(format!("UnicodeDecodeError: {}", ps(&path))))?;
        let tree = pyast::parse(&text).map_err(|e| Fail::Crash(format!("SyntaxError: {e} ({})", ps(&path))))?;
        let rel = ps(path.strip_prefix(root).unwrap_or(&path));
        for n in pyast::walk(&tree) {
            if n.kind == Kind::Assert {
                out.push(format!("{rel}:{}", n.line));
            }
        }
    }
    Ok(out)
}

pub fn no_production_asserts(root: &Path) -> R<()> {
    let off = assert_offenders(root)?;
    require(off.is_empty(), || format!("bare assert is optimization-unsafe: {}", py_repr_list(&off)))
}

pub fn nested_worktree_isolation() -> R<()> {
    let td = TempDir::new();
    let root = std::fs::canonicalize(td.path()).unwrap();
    let r = ps(&root);
    write(&root.join("CLAUDE.md"), "fixture\n");
    write(&root.join(".claude/rules/workflow.md"), "fixture\n");
    write(&root.join(".claude/policies/registry.yaml"), "fixture\n");
    write(&root.join(".gitignore"), "fixture\n");
    git_ok(&["init", "-q", "-b", "single-node", &r])?;
    git_ok(&["-C", &r, "add", "."])?;
    git_ok(&["-C", &r, "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"])?;
    let wt = root.join(".claude/worktrees");
    let nested = wt.join("stale");
    let special = wt.join("space path");
    let trailing = wt.join("trailing-space ");
    let detached = wt.join("detached");
    git_ok(&["-C", &r, "worktree", "add", "-q", "-b", "stale-root", &ps(&nested)])?;
    git_ok(&["-C", &r, "worktree", "add", "-q", "-b", "space-root", &ps(&special)])?;
    git_ok(&["-C", &r, "worktree", "lock", &ps(&special)])?;
    git_ok(&["-C", &r, "worktree", "add", "-q", "-b", "trailing-root", &ps(&trailing)])?;
    use std::os::unix::ffi::OsStrExt;
    let mut raw_b = wt.as_os_str().as_bytes().to_vec();
    raw_b.extend_from_slice(b"/raw-\xff");
    let raw = PathBuf::from(std::ffi::OsStr::from_bytes(&raw_b));
    let raw_res = std::process::Command::new("git").args(["-C", &r, "worktree", "add", "-q", "-b", "raw-root"]).arg(&raw).output();
    let raw_created = match &raw_res {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            eprintln!("[runtime_selftest] SKIPPED undecodable worktree path fixture: {}", py_strip(&String::from_utf8_lossy(&o.stderr)));
            false
        }
        Err(e) => return Err(Fail::Crash(e.to_string())),
    };
    let mut cr_b = wt.as_os_str().as_bytes().to_vec();
    cr_b.extend_from_slice(b"/trailing-cr\r");
    let trailing_cr = PathBuf::from(std::ffi::OsStr::from_bytes(&cr_b));
    let cr_res = std::process::Command::new("git").args(["-C", &r, "worktree", "add", "-q", "-b", "trailing-cr-root"]).arg(&trailing_cr).output();
    let cr_created = match &cr_res {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            eprintln!("[runtime_selftest] SKIPPED trailing-CR worktree path fixture: {}", py_strip(&String::from_utf8_lossy(&o.stderr)));
            false
        }
        Err(e) => return Err(Fail::Crash(e.to_string())),
    };
    git_ok(&["-C", &r, "worktree", "add", "-q", "--detach", &ps(&detached)])?;
    let bare = root.join("bare.git");
    git_ok(&["init", "-q", "--bare", &ps(&bare)])?;
    let body = (|| -> R<()> {
        let mut expected: std::collections::BTreeSet<String> =
            ["single-node", "stale-root", "space-root", "trailing-root"].iter().map(|x| x.to_string()).collect();
        if raw_created {
            expected.insert("raw-root".into());
        }
        if cr_created {
            expected.insert("trailing-cr-root".into());
        }
        let live = tw::live_registered_worktree_branches(&root)?;
        require(live == expected, || "live/locked/special/trailing/raw or detached worktree registration parsed incorrectly".into())?;
        require(tw::live_registered_worktree_branches(&bare)?.is_empty(), || "bare repository yielded a worktree branch".into())?;
        write(&nested.join("stale.py"), "assert False\n");
        write(&nested.join("stale.bak"), "stale\n");
        write(&nested.join("stale.md"), "tracked_index\n");
        let generated = root.join(".claude/runtime/rust/lib");
        write(&generated.join("debugger.py"), "assert False\n");
        write(&generated.join("Cargo.toml.orig"), "third-party Cargo metadata\n");
        no_production_asserts(&root)?;
        tw::no_backup_artifacts(&root)?;
        tw::no_retired_hash_mechanism_prose(&root)?;
        require(policy_distribution::shipped_python_paths(&root).is_empty(), || {
            "distribution scan included nested worktree or installed runtime Python".into()
        })?;
        let current_python = root.join(".claude/current.py");
        write(&current_python, "assert False\n");
        require(policy_distribution::shipped_python_paths(&root) == vec![current_python.clone()], || {
            "distribution scan omitted current-tree Python".into()
        })?;
        if expect_fail(no_production_asserts(&root))?.is_none() {
            return Err(Fail::Selftest("Python scan did not catch current-tree violation".into()));
        }
        let _ = std::fs::remove_file(&current_python);
        let cb = root.join(".claude/current.bak");
        write(&cb, "current violation\n");
        if expect_fail(tw::no_backup_artifacts(&root))?.is_none() {
            return Err(Fail::Selftest("backup scan did not catch current-tree violation".into()));
        }
        let _ = std::fs::remove_file(&cb);
        let pb = root.join(".claude/policies/runtime/current.orig");
        write(&pb, "canonical violation\n");
        if expect_fail(tw::no_backup_artifacts(&root))?.is_none() {
            return Err(Fail::Selftest("backup scan omitted canonical policies/runtime".into()));
        }
        let _ = std::fs::remove_file(&pb);
        let cp = root.join(".claude/current.md");
        write(&cp, "tracked_index\n");
        if expect_fail(tw::no_retired_hash_mechanism_prose(&root))?.is_none() {
            return Err(Fail::Selftest("prose scan did not catch current-tree violation".into()));
        }
        let _ = std::fs::remove_file(&cp);
        let _ = std::fs::remove_dir_all(&nested);
        match expect_fail(tw::no_backup_artifacts(&root))? {
            Some(m) => require(m.contains("stale-root"), || format!("prunable worktree ref was not reported as stray: {m}"))?,
            None => return Err(Fail::Selftest("prunable worktree ref was accepted".into())),
        }
        git_ok(&["-C", &r, "worktree", "prune"])?;
        git_ok(&["-C", &r, "branch", "-D", "stale-root"])?;
        git_ok(&["-C", &r, "branch", "worktree-prefix-orphan"])?;
        match expect_fail(tw::no_backup_artifacts(&root))? {
            Some(m) => require(m.contains("worktree-prefix-orphan"), || format!("prefix-only ref was not reported as stray: {m}"))?,
            None => return Err(Fail::Selftest("prefix-only worktree ref was accepted".into())),
        }
        git_ok(&["-C", &r, "branch", "-D", "worktree-prefix-orphan"])
    })();
    for w in [&nested, &special, &trailing, &raw, &trailing_cr, &detached] {
        if w.exists() {
            let _ = std::process::Command::new("git").args(["-C", &r, "worktree", "unlock"]).arg(w).output();
            let o = std::process::Command::new("git").args(["-C", &r, "worktree", "remove", "--force"]).arg(w).output();
            if body.is_ok() && !matches!(o, Ok(ref x) if x.status.success()) {
                return Err(Fail::Crash("subprocess.CalledProcessError: git worktree remove".into()));
            }
        }
    }
    body
}

fn schema_dir(repo: &Path) -> PathBuf {
    repo.join(".claude/schemas")
}

pub fn completion_gate(repo: &Path) -> R<()> {
    use policy_gate::completion_gate as g;
    let load = |n: &str| g::load_schema(&schema_dir(repo), n).map_err(|e| Fail::Crash(e));
    let work = load("work-manifest.schema.json")?;
    let side = load("side-effect-authorization.schema.json")?;
    let strs = |v: Option<&PyValue>| -> Vec<String> {
        match v {
            Some(PyValue::List(l)) => l.iter().filter_map(|x| if let PyValue::Str(t) = x.peel() { t.to_utf8() } else { None }).collect(),
            _ => vec![],
        }
    };
    let path = |v: &PyValue, ks: &[&str]| -> Option<PyValue> {
        let mut cur = v.clone();
        for k in ks {
            cur = get(&cur, k)?.clone();
        }
        Some(cur)
    };
    let mut work_enum = strs(path(&work, &["definitions", "executionApproval", "properties", "allowed_actions", "items", "enum"]).as_ref());
    let mut side_enum = strs(path(&side, &["properties", "action", "enum"]).as_ref());
    let mut expected: Vec<String> = g::ALLOWED_ACTIONS.iter().map(|x| x.to_string()).collect();
    work_enum.sort();
    work_enum.dedup();
    side_enum.sort();
    side_enum.dedup();
    expected.sort();
    require(work_enum == expected && side_enum == expected, || {
        format!(
            "side-effect action enum drift: runtime={}, work={}, authorization={}",
            py_repr_list(&expected),
            py_repr_list(&work_enum),
            py_repr_list(&side_enum)
        )
    })?;
    let refschema = obj(vec![("$ref", s("https://invalid.example/ref"))]);
    let out = agent_runtime_support::schema::validate_against_schema(&PyValue::None, &refschema);
    require(matches!(&out, Err(e) if e.class == "ValueError"), || "completion gate accepted a non-local $ref".into())?;
    let bad = obj(vec![("title", s("runtime-selftest")), ("type", s("object")), ("required", PyValue::List(vec![s("must_exist")]))]);
    require(g::emit_with_schema(obj(vec![]), 0, &bad).is_err(), || "completion gate emitted schema-invalid own output".into())?;
    let td = TempDir::new();
    let root = std::fs::canonicalize(td.path()).unwrap();
    let mdir = root.join("docs/_evidence");
    std::fs::create_dir_all(&mdir).unwrap();
    require(g::lexical_components(&mdir, &root, "../../../outside.txt").is_none(), || {
        "lexical repo escape was not rejected: ok".into()
    })?;
    write(&root.join("target.txt"), "evidence");
    std::os::unix::fs::symlink(root.join("target.txt"), root.join("link.txt")).unwrap();
    let fd = policy_gate::fsx::open_path(&root, libc_o_dir()).map_err(|e| Fail::Crash(format!("OSError {e}")))?;
    let st = g::walk_nofollow(&fd, &["link.txt".to_string()]);
    require(matches!(st, Err(g::Walk::Symlink)), || format!("symlink evidence was not rejected: {:?}", st.as_ref().err()))?;
    let (unreadable, broken, invalid) = g::check_required_local_links(&fd, &root, &root, b"[required evidence](missing.md)");
    require(!unreadable && broken == ["missing.md"] && invalid.is_empty(), || {
        format!("broken Markdown link was not detected: {unreadable}, {broken:?}, {invalid:?}")
    })
}

fn libc_o_dir() -> i32 {
    libc::O_RDONLY | libc::O_DIRECTORY
}

const PROMO_CERTIFICATE: &str = "schema_version: 1
record_type: benchmark_certificate
verdict: PASS
model: selftest-model
gpu_model: GB10
vllm_version: 0.0.0.dev0
quantization: fp8
topology: single
tensor_parallel_size: 1
benchmark_mode: full
rubric_authority: {authority}
primary_source: E(external_reference)
primary_tps: 26.0
floor_tps: 22.1
tolerance: 0.15
ratio_M_over_primary: 0.719
measured_utc: \"2026-01-01T00:00:00Z\"
";
const PROMO_LITE_CERTIFICATE: &str = "schema_version: 1
record_type: benchmark_certificate
verdict: not_applicable
model: selftest-model
gpu_model: GB10
vllm_version: 0.0.0.dev0
quantization: fp8
topology: single
tensor_parallel_size: 1
benchmark_mode: lite
lite_verdict: pass
entry_path: beta_lite_only_cell
lite_included: true
lite_gen_tps_warm: 40.0
measured_utc: \"2026-01-01T00:00:00Z\"
measured_node: main
";
const LITE_BENCH_REPORT: &str = "# bench_report selftest

## lite 지표 (서빙 성공 시 자동 수집 · 관측용)

| 항목 | 값 |
|---|---|
| gen tokens/sec (warm) | 26.0 |
| TTFT p50 (ms) | 120 |
";

fn cert(authority: &str) -> String {
    PROMO_CERTIFICATE.replace("{authority}", authority)
}

fn identity() -> PyValue {
    obj(vec![
        ("model", s("selftest-model")),
        ("gpu", s("GB10")),
        ("vllm", s("0.0.0.dev0")),
        ("quant", s("fp8")),
        ("topology", s("single")),
        ("tp", PyValue::int(1)),
    ])
}

fn rubric() -> PyValue {
    obj(vec![
        ("rubric_authority", s("explore")),
        ("floor_tps", PyValue::Float(22.1)),
        ("ratio_M_over_primary", PyValue::Float(0.719)),
        ("primary_source", s("E(external_reference)")),
        ("rubric_source", s("verdict_json")),
    ])
}

fn with(mut v: PyValue, items: Vec<(&str, PyValue)>) -> PyValue {
    for (k, x) in items {
        set(&mut v, k, x);
    }
    v
}

fn bench_report_with_verdict_table(verdict: &str, authority: &str) -> String {
    format!(
        "{LITE_BENCH_REPORT}\n## 판정 (표시만 — verdict_rule.sh 결과)\n\n| 항목 | 값 |\n|---|---|\n| verdict | **{verdict}** |\n| 측정 decode t/s (동시성1) | 20.0 t/s |\n| 루브릭 권한 | {authority} |\n| floor (primary×(1−tol)) | 22.1 t/s |\n\n## 다음 절\n"
    )
}

fn relpath(target: &Path, base: &Path) -> String {
    let t: Vec<_> = target.components().collect();
    let b: Vec<_> = base.components().collect();
    let common = t.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".into(); b.len() - common];
    parts.extend(t[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

#[derive(Default)]
struct PromoOpts<'a> {
    certificate: Option<&'a str>,
    smoke_failed: bool,
    report: Option<&'a str>,
    task_class: Option<&'a str>,
    drop: &'a [&'a str],
}

fn write_promotion_manifest(root: &Path, verdict: &str, extra: Option<&PyValue>, o: &PromoOpts) -> PathBuf {
    let evidence_dir = root.join("docs/_evidence");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    let mut paths: Vec<(String, String)> = Vec::new();
    for (kind, sub) in [("plan", "plan"), ("devlog", "devlog"), ("testlog", "testlog"), ("bench_report", "benchmark")] {
        let art = root.join("docs").join(sub).join(format!("{kind}_selftest.md"));
        write(&art, &if kind == "bench_report" { o.report.unwrap_or(LITE_BENCH_REPORT).to_string() } else { format!("# {kind} selftest\n") });
        paths.push((kind.into(), relpath(&art, &evidence_dir)));
    }
    let simlog = root.join("docs/simlog/selftest_run");
    write(&simlog.join("summary.json"), "{}\n");
    write(&simlog.join("run_summary.json"), "{\"run_id\": \"selftest_run\", \"converged\": true, \"trial_count\": 1, \"provenance\": \"mock\"}\n");
    paths.push(("simlog".into(), relpath(&simlog, &evidence_dir)));
    if let Some(c) = o.certificate {
        let art = root.join("docs/benchmark/benchmark_selftest.yaml");
        write(&art, c);
        paths.push(("certificate".into(), relpath(&art, &evidence_dir)));
    }
    let mut bench = obj(vec![("mode", s("full")), ("verdict", s(verdict))]);
    if let Some(PyValue::Dict(d)) = extra.map(|e| e.peel()) {
        for (k, v) in d.iter() {
            if let PyValue::Dict(b) = &mut bench {
                b.insert(k.clone(), v.clone());
            }
        }
    }
    paths.retain(|(k, _)| !o.drop.contains(&k.as_str()));
    let mut ev = PyDict::new();
    for (k, v) in &paths {
        ev.set(k, obj(vec![("path", s(v))]));
    }
    let mut scanned: Vec<String> = paths.iter().map(|(_, v)| v.clone()).collect();
    scanned.sort();
    let manifest = obj(vec![
        ("schema_version", PyValue::int(1)),
        ("task_class", s(o.task_class.unwrap_or("full_benchmark"))),
        ("identity", identity()),
        (
            "runtime",
            obj(vec![
                ("health_ok", PyValue::Bool(true)),
                ("functional_smoke_passed", PyValue::Bool(!o.smoke_failed)),
                ("identity", identity()),
                ("containers", PyValue::List(vec![])),
            ]),
        ),
        ("benchmark", bench),
        ("conditions", obj(vec![])),
        ("evidence", PyValue::Dict(ev)),
        ("pii_scan", obj(vec![("passed", PyValue::Bool(true)), ("scanned_paths", PyValue::List(scanned.iter().map(|x| s(x)).collect()))])),
    ]);
    let mp = evidence_dir.join("selftest.work-manifest.json");
    write(&mp, &manifest.dumps_indented(2).unwrap().to_utf8().unwrap());
    mp
}

fn gate_launcher(repo: &Path) -> PathBuf {
    repo.join(".claude/policies/runtime/completion_gate.sh")
}

fn gate_verify(repo: &Path, root: &Path, manifest: &Path, strict: bool) -> R<PyValue> {
    let o = gitx::run("bash", &[&ps(&gate_launcher(repo)), "verify", "--manifest", &ps(manifest), "--repo-root", &ps(root)], Some(root), None);
    match agent_runtime_support::pyjson::loads(&o.text()) {
        Ok(mut v) => {
            set(&mut v, "_returncode", PyValue::int(o.code as i64));
            Ok(v)
        }
        Err(e) if strict => Err(Fail::Selftest(format!(
            "completion_gate verify produced unparseable stdout: {e}; stderr={}",
            crate::pytext::py_repr_str(&crate::pytext::take_chars(py_strip(&o.err_text()), 400))
        ))),
        Err(_) => Ok(obj(vec![("_rc", PyValue::int(o.code as i64))])),
    }
}

fn promotion_probe(repo: &Path, root: &Path, verdict: &str, extra: Option<&PyValue>, o: &PromoOpts) -> R<PyValue> {
    let mp = write_promotion_manifest(root, verdict, extra, o);
    gate_verify(repo, root, &mp, true)
}

fn fresh_root() -> (TempDir, PathBuf) {
    let td = TempDir::new();
    let root = std::fs::canonicalize(td.path()).unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    (td, root)
}

fn promoted(v: &PyValue) -> bool {
    gstr(v, "state").as_deref() == Some("promotion-ready")
        && is_true(v, "eligible_for_promotion")
        && matches!(get(v, "_returncode"), Some(x) if x.repr() == "0")
}
fn rubric_field(v: &PyValue, k: &str) -> Option<String> {
    get(v, "rubric").and_then(|r| gstr(r, k))
}

pub fn certificate_run_resolution(repo: &Path) -> R<()> {
    let c = cert("weak");
    let run = |plant: &[(&str, String)], certificate: &str, extra: Vec<(&str, PyValue)>| -> R<PyValue> {
        let bench = with(with(rubric(), vec![("rubric_authority", s("weak"))]), extra);
        let (_td, root) = fresh_root();
        let o = PromoOpts { certificate: Some(certificate), ..Default::default() };
        write_promotion_manifest(&root, "PASS", Some(&bench), &o);
        for (name, text) in plant {
            write(&root.join("docs/benchmark").join(name), text);
        }
        promotion_probe(repo, &root, "PASS", Some(&bench), &o)
    };
    let cert_of = |v: &PyValue| get(v, "certificate").cloned().unwrap_or(PyValue::None);
    let out = run(&[], &c, vec![])?;
    let ce = cert_of(&out);
    let last_key = match get(&ce, "run_key") {
        Some(PyValue::List(l)) => l.last().map(|x| x.repr()),
        _ => None,
    };
    require(
        is_true(&out, "eligible_for_promotion")
            && last_key.as_deref() == Some("'2026-01-01T00:00:00Z'")
            && matches!(get(&ce, "duplicates"), Some(PyValue::List(l)) if l.is_empty()),
        || format!("단독 인증서는 키가 식별되고 중복 0 이어야 한다: {:?} {}", codes(&out), dump(&ce)),
    )?;
    let out = run(&[("benchmark_selftest_copy.yaml", c.clone())], &c, vec![])?;
    let ce = cert_of(&out);
    require(
        !is_true(&out, "eligible_for_promotion")
            && has(&out, "CERTIFICATE_RUN_AMBIGUOUS")
            && dump(get(&ce, "duplicates").unwrap_or(&PyValue::None)) == "[\"docs/benchmark/benchmark_selftest_copy.yaml\"]"
            && dump(get(&out, "messages").unwrap_or(&PyValue::None)).contains("byte-identical"),
        || format!("★같은 측정의 사본이 있으면 승격이 막혀야 한다: {:?}", codes(&out)),
    )?;
    let out = run(&[("benchmark_selftest_other.yaml", c.replace("2026-01-01T00:00:00Z", "2026-01-01T00:00:01Z"))], &c, vec![])?;
    require(is_true(&out, "eligible_for_promotion") && !has(&out, "CERTIFICATE_RUN_AMBIGUOUS"), || {
        format!("★음성대조 measured_utc 가 다른 형제는 다른 측정이다: {:?}", codes(&out))
    })?;
    let out = run(&[("benchmark_selftest_diff.yaml", c.replace("ratio_M_over_primary: 0.719", "ratio_M_over_primary: 0.9"))], &c, vec![])?;
    require(
        has(&out, "CERTIFICATE_RUN_AMBIGUOUS") && dump(get(&out, "messages").unwrap_or(&PyValue::None)).contains("DIFFERENT bytes"),
        || format!("같은 측정 다른 내용은 더 나쁜 결함으로 가려야 한다: {:?}", codes(&out)),
    )?;
    let out = run(&[], &c.replace("measured_utc: \"2026-01-01T00:00:00Z\"\n", ""), vec![])?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "CERTIFICATE_RUN_KEY_UNRESOLVABLE"), || {
        format!("★measured_utc 없는 인증서는 식별 불가로 차단: {:?}", codes(&out))
    })?;
    let out = run(&[], &c, vec![("measured_utc", s("2026-01-01T00:00:00Z"))])?;
    require(is_true(&out, "eligible_for_promotion") && !has(&out, "CERTIFICATE_RUN_DRIFT"), || {
        format!("선언과 파일이 같으면 통과: {:?}", codes(&out))
    })?;
    let out = run(&[], &c, vec![("measured_utc", s("2026-01-01T00:00:59Z"))])?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "CERTIFICATE_RUN_DRIFT"), || {
        format!("★manifest 가 다른 측정을 선언하면 DRIFT 로 차단: {:?}", codes(&out))
    })?;
    let out = run(&[("notes.yaml", "x: 1\n".into()), ("benchmark_garbled.yaml", "a:\n  nested: 1\n".into())], &c, vec![])?;
    require(is_true(&out, "eligible_for_promotion"), || {
        format!("비-인증서·파싱불가 형제는 매치 대상이 아니다(승격 유지): {:?}", codes(&out))
    })
}

pub fn promotion_rubric_carrier(repo: &Path) -> R<()> {
    let probe = |verdict: &str, extra: Option<PyValue>, o: PromoOpts| -> R<PyValue> {
        let (_td, root) = fresh_root();
        promotion_probe(repo, &root, verdict, extra.as_ref(), &o)
    };
    let explore_cert = cert("explore");
    let out = probe("PASS", Some(rubric()), PromoOpts { certificate: Some(&explore_cert), ..Default::default() })?;
    require(promoted(&out) && rubric_field(&out, "authority_source").as_deref() == Some("certificate"), || {
        format!("PASS+explore lost its certificate carrier: {:?}", codes(&out))
    })?;
    let out = probe("REFUTE", Some(rubric()), PromoOpts::default())?;
    require(
        promoted(&out) && has(&out, "BENCHMARK_VERDICT_ANNOTATED") && rubric_field(&out, "authority_source").as_deref() == Some("manifest_benchmark"),
        || format!("explore-REFUTE auto-open is dead again (manifest carrier not consulted): {:?}", codes(&out)),
    )?;
    for authority in ["weak", "explicit"] {
        let out = probe("REFUTE", Some(with(rubric(), vec![("rubric_authority", s(authority))])), PromoOpts::default())?;
        require(
            promoted(&out)
                && has(&out, "BENCHMARK_VERDICT_ANNOTATED")
                && matches!(get(&out, "certificate"), None | Some(PyValue::None))
                && rubric_field(&out, "authority").as_deref() == Some(authority),
            || format!("{authority}-authority REFUTE with smoke + valid rubric was not promoted: {:?}", codes(&out)),
        )?;
    }
    let out = probe("FAIL", Some(with(rubric(), vec![("rubric_authority", s("weak"))])), PromoOpts::default())?;
    require(promoted(&out) && has(&out, "BENCHMARK_VERDICT_ANNOTATED"), || {
        format!("FAIL with smoke + valid rubric was not promoted: {:?}", codes(&out))
    })?;
    let out = probe("REFUTE", None, PromoOpts::default())?;
    require(
        !is_true(&out, "eligible_for_promotion")
            && has(&out, "BENCHMARK_MEASUREMENT_INVALID")
            && matches!(get(&out, "rubric"), None | Some(PyValue::None))
            && !codes(&out).iter().any(|c| c.starts_with("MANIFEST_RUBRIC")),
        || format!("legacy manifest without a rubric channel was mis-handled: {:?}", codes(&out)),
    )?;
    let obs_rubric = obj(vec![
        ("rubric_authority", s("weak")),
        ("floor_tps", PyValue::None),
        ("ratio_M_over_primary", PyValue::None),
        ("primary_source", PyValue::None),
        ("rubric_source", s("verdict_json")),
    ]);
    let obs_report = bench_report_with_verdict_table("NEEDS_RUBRIC", "weak");
    let out = probe("OBSERVATION-ONLY", Some(obs_rubric.clone()), PromoOpts { report: Some(&obs_report), ..Default::default() })?;
    require(
        promoted(&out)
            && has(&out, "BENCHMARK_OBSERVATION_ONLY_PROMOTION")
            && matches!(get(&out, "certificate"), None | Some(PyValue::None))
            && !codes(&out).iter().any(|c| c.starts_with("MANIFEST_RUBRIC")),
        || format!("OBSERVATION-ONLY cell with smoke + bench_report was not promoted: {:?}", codes(&out)),
    )?;
    let out = probe("OBSERVATION-ONLY", Some(obs_rubric.clone()), PromoOpts { report: Some(&obs_report), smoke_failed: true, ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion"), || {
        format!("OBSERVATION-ONLY bypassed the functional-smoke requirement: {:?}", codes(&out))
    })?;
    let refute_report = bench_report_with_verdict_table("REFUTE", "weak");
    let out = probe("OBSERVATION-ONLY", Some(obs_rubric.clone()), PromoOpts { report: Some(&refute_report), ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "BENCHMARK_OBSERVATION_ONLY_UNCORROBORATED"), || {
        format!("a REFUTE bench_report was accepted as OBSERVATION-ONLY: {:?}", codes(&out))
    })?;
    for (label, extra) in [
        ("rubric-claimed", with(rubric(), vec![("rubric_authority", s("weak"))])),
        ("unsourced", with(obs_rubric.clone(), vec![("rubric_source", PyValue::None)])),
    ] {
        let out = probe("OBSERVATION-ONLY", Some(extra), PromoOpts { report: Some(&obs_report), ..Default::default() })?;
        require(!is_true(&out, "eligible_for_promotion") && has(&out, "BENCHMARK_OBSERVATION_ONLY_INVALID"), || {
            format!("OBSERVATION-ONLY opened despite {label}: {:?}", codes(&out))
        })?;
    }
    let out = probe("PASS", Some(with(rubric(), vec![("rubric_authority", s("explicit"))])), PromoOpts::default())?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "EVIDENCE_MISSING:certificate"), || {
        format!("explicit PASS without a certificate was promoted: {:?}", codes(&out))
    })?;
    for (field, value, code, rep) in [
        ("floor_tps", PyValue::int(0), "MANIFEST_RUBRIC_FLOOR_INVALID", "0"),
        ("ratio_M_over_primary", PyValue::None, "MANIFEST_RUBRIC_RATIO_MISSING", "None"),
        ("primary_source", s("N/A"), "MANIFEST_RUBRIC_SOURCE_MISSING", "'N/A'"),
        ("rubric_source", PyValue::None, "MANIFEST_RUBRIC_PROVENANCE_MISSING", "None"),
    ] {
        let out = probe("REFUTE", Some(with(rubric(), vec![(field, value)])), PromoOpts::default())?;
        require(!is_true(&out, "eligible_for_promotion") && has(&out, code), || {
            format!("manifest rubric contract did not fail closed on {field}={rep}: {:?}", codes(&out))
        })?;
    }
    let weak_cert = cert("weak");
    let out = probe("PASS", Some(rubric()), PromoOpts { certificate: Some(&weak_cert), ..Default::default() })?;
    require(has(&out, "RUBRIC_AUTHORITY_CARRIER_DISAGREEMENT") && rubric_field(&out, "authority").as_deref() == Some("weak"), || {
        format!("carrier disagreement was silently resolved in the manifest's favor: {:?}", codes(&out))
    })?;
    let out = probe("REFUTE", Some(with(rubric(), vec![("rubric_authority", s("weak"))])), PromoOpts { smoke_failed: true, ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED"), || {
        format!("explore authority bypassed the functional-smoke requirement: {:?}", codes(&out))
    })
}

pub fn certificate_waiver_by_authority(repo: &Path) -> R<()> {
    use policy_gate::completion_gate as g;
    let empty = obj(vec![]);
    for (verdict, authority, expect) in [
        ("PASS", Some("explicit"), true),
        ("PASS", None, true),
        ("PASS", Some("explore"), false),
        ("PASS", Some("weak"), false),
        ("REFUTE", Some("explicit"), false),
        ("REFUTE", Some("explore"), false),
        ("FAIL", None, false),
        ("OBSERVATION-ONLY", Some("weak"), false),
        ("OBSERVATION-ONLY", None, false),
    ] {
        let got = g::required_evidence_for("full_benchmark", Some(&empty), Some(&s(verdict)), authority).iter().any(|x| x == "certificate");
        require(got == expect, || {
            format!(
                "required_evidence_for(full_benchmark, {verdict}, {}) certificate={} (expected {})",
                authority.unwrap_or("None"),
                if got { "True" } else { "False" },
                if expect { "True" } else { "False" }
            )
        })?;
    }
    require(!g::required_evidence_for("hint_map_only", Some(&empty), Some(&s("PASS")), None).iter().any(|x| x == "certificate"), || {
        "hint_map_only must never require a certificate".into()
    })?;
    require(g::certificate_requirement_authority(&rubric()).as_deref() == Some("explore"), || {
        "a verdict_json-sourced explore authority was not recognized".into()
    })?;
    for bad in [
        with(rubric(), vec![("rubric_source", PyValue::None)]),
        with(rubric(), vec![("rubric_source", s("hand"))]),
        with(rubric(), vec![("rubric_authority", s("EXPLORE"))]),
        obj(vec![]),
        PyValue::None,
    ] {
        require(g::certificate_requirement_authority(&bad).is_none(), || {
            format!("an unsourced/invalid rubric opened the certificate waiver: {}", bad.repr())
        })?;
    }
    let table_is = |text: &str, verdict: &str, authority: &str| -> bool {
        let (t, state) = g::bench_report_verdict_table(Some(text));
        state == "parsed"
            && t.map_or(false, |rows| {
                rows.len() == 2
                    && rows.iter().any(|(k, v)| *k == "verdict" && v.as_deref() == Some(verdict))
                    && rows.iter().any(|(k, v)| *k == "rubric_authority" && v.as_deref() == Some(authority))
            })
    };
    require(table_is(&bench_report_with_verdict_table("PASS", "explore"), "PASS", "explore"), || "verdict table parse drift".into())?;
    require(table_is(&bench_report_with_verdict_table("NEEDS_RUBRIC", "weak"), "NEEDS_RUBRIC", "weak"), || {
        "NEEDS_RUBRIC verdict table (OBSERVATION-ONLY 의 교차검증 근거) parse drift".into()
    })?;
    require(g::bench_report_verdict_table(Some(LITE_BENCH_REPORT)).0.is_none(), || "absent verdict section parsed as a table".into())?;
    require(g::bench_report_verdict_table(Some("| verdict | **PASS** |\n| 루브릭 권한 | explore |\n")).0.is_none(), || {
        "rows outside the 판정 section were accepted".into()
    })?;
    let dup = bench_report_with_verdict_table("PASS", "explore").replace("| 루브릭 권한 | explore |", "| 루브릭 권한 | explore |\n| verdict | REFUTE |");
    require(g::bench_report_verdict_table(Some(&dup)).1.starts_with("unparseable"), || "duplicate verdict row not rejected".into())?;
    let writer = repo.join(".claude/skills/adversarial-benchmark/scripts/bench_reporting_api.sh");
    let payload = "{\"id\": \"writer-contract\", \"operation\": \"render_report.build_md\", \"arguments\": {\"index\": {\"meta\": {}, \"levels\": []}, \"verdict\": {\"verdict\": \"PASS\", \"rubric\": {\"authority\": \"explore\"}}, \"roofline\": null}}\n";
    let o = gitx::run("bash", &[&ps(&writer), "evaluate-jsonl"], None, Some(payload.as_bytes()));
    require(o.code == 0 && o.stderr.is_empty(), || format!("native render_report writer failed: {}", crate::pytext::py_repr_str(&o.err_text())))?;
    let written = agent_runtime_support::pyjson::loads(&o.text()).map_err(|e| Fail::Crash(format!("JSONDecodeError: {e}")))?;
    let value = gstr(&written, "value");
    require(gstr(&written, "kind").as_deref() == Some("value") && value.is_some(), || format!("native writer returned no report: {}", written.repr()))?;
    require(table_is(&value.unwrap(), "PASS", "explore"), || "native report and verdict-table reader disagree".into())?;

    let verify = |verdict: &str, rubric_v: PyValue, report: &str, certificate: Option<&str>| -> R<PyValue> {
        let (_td, root) = fresh_root();
        let mp = write_promotion_manifest(&root, verdict, Some(&rubric_v), &PromoOpts { certificate, report: Some(report), ..Default::default() });
        gate_verify(repo, &root, &mp, true)
    };
    let explicit = with(rubric(), vec![("rubric_authority", s("explicit"))]);
    let out = verify("PASS", explicit, &bench_report_with_verdict_table("PASS", "explicit"), None)?;
    require(!promoted(&out) && has(&out, "EVIDENCE_MISSING:certificate"), || format!("explicit PASS without a certificate was not rejected: {:?}", codes(&out)))?;
    let out = verify("PASS", rubric(), &bench_report_with_verdict_table("PASS", "explore"), None)?;
    let cert_required = get(&out, "checked_evidence").and_then(|c| get(c, "certificate")).map_or(false, |c| is_true(c, "required"));
    require(
        promoted(&out)
            && has(&out, "CERTIFICATE_WAIVED_NON_ISSUING_AUTHORITY")
            && !has(&out, "EVIDENCE_MISSING:certificate")
            && !cert_required
            && rubric_field(&out, "authority_source").as_deref() == Some("manifest_benchmark"),
        || format!("explore PASS + bench_report did not satisfy the gate: {:?}", codes(&out)),
    )?;
    let out = verify("REFUTE", rubric(), &bench_report_with_verdict_table("REFUTE", "explore"), None)?;
    require(promoted(&out) && has(&out, "BENCHMARK_VERDICT_ANNOTATED"), || format!("explore REFUTE + bench_report regressed: {:?}", codes(&out)))?;
    let out = verify("PASS", with(rubric(), vec![("rubric_authority", s("weak"))]), &bench_report_with_verdict_table("PASS", "weak"), None)?;
    require(promoted(&out) && has(&out, "CERTIFICATE_WAIVED_NON_ISSUING_AUTHORITY"), || format!("weak PASS + bench_report did not satisfy the gate: {:?}", codes(&out)))?;
    for (label, text) in [
        ("표 없음", LITE_BENCH_REPORT.to_string()),
        ("권한 불일치", bench_report_with_verdict_table("PASS", "explicit")),
        ("판정 불일치", bench_report_with_verdict_table("REFUTE", "explore")),
    ] {
        let out = verify("PASS", rubric(), &text, None)?;
        require(!promoted(&out) && has(&out, "CERTIFICATE_WAIVER_UNCORROBORATED"), || {
            format!("waiver opened without bench_report corroboration ({label}): {:?}", codes(&out))
        })?;
    }
    let out = verify("PASS", with(rubric(), vec![("rubric_source", PyValue::None)]), &bench_report_with_verdict_table("PASS", "explore"), None)?;
    require(!promoted(&out) && has(&out, "EVIDENCE_MISSING:certificate"), || format!("an unsourced explore authority waived the certificate: {:?}", codes(&out)))?;
    let out = verify("PASS", with(rubric(), vec![("floor_tps", PyValue::int(0))]), &bench_report_with_verdict_table("PASS", "explore"), None)?;
    require(
        !promoted(&out) && has(&out, "CERTIFICATE_WAIVED_RUBRIC_CONTRACT_UNMET") && has(&out, "MANIFEST_RUBRIC_FLOOR_INVALID"),
        || format!("a waived PASS with floor<=0 was promoted (vacuous PASS): {:?}", codes(&out)),
    )?;
    let ec = cert("explore");
    let out = verify("PASS", rubric(), &bench_report_with_verdict_table("PASS", "explore"), Some(&ec))?;
    require(promoted(&out) && rubric_field(&out, "authority_source").as_deref() == Some("certificate"), || {
        format!("an optional certificate on a waived PASS lost its carrier role: {:?}", codes(&out))
    })
}

pub fn hint_map_only_promotion(repo: &Path) -> R<()> {
    let (_td, root) = fresh_root();
    let v = |verdict: &str, o: PromoOpts| -> R<PyValue> {
        let mp = write_promotion_manifest(&root, verdict, Some(&rubric()), &o);
        gate_verify(repo, &root, &mp, false)
    };
    let drop2: &[&str] = &["simlog", "bench_report"];
    let cert_grade = |out: &PyValue| get(out, "certificate").and_then(|c| gstr(c, "grade"));
    let out = v("PASS", PromoOpts { certificate: Some(PROMO_LITE_CERTIFICATE), task_class: Some("hint_map_only"), drop: drop2, ..Default::default() })?;
    require(
        gstr(&out, "state").as_deref() == Some("promotion-ready")
            && is_true(&out, "eligible_for_promotion")
            && has(&out, "HINT_MAP_ONLY_PROMOTION")
            && cert_grade(&out).as_deref() == Some("lite"),
        || format!("hint_map_only did not reach promotion-ready with a lite-grade certificate: {}", out.repr()),
    )?;
    let out = v("PASS", PromoOpts { task_class: Some("hint_map_only"), drop: drop2, ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE"), || {
        format!("hint_map_only reached promotion without a lite-pass certificate: {}", out.repr())
    })?;
    let bad_lite = PROMO_LITE_CERTIFICATE.replace("lite_verdict: pass", "lite_verdict: server_failed");
    let full_cert = cert("explore");
    for (bad, code) in [(bad_lite.as_str(), "CERTIFICATE_LITE_VERDICT_NOT_PASS"), (full_cert.as_str(), "CERTIFICATE_GRADE_MISMATCH")] {
        let out = v("PASS", PromoOpts { certificate: Some(bad), task_class: Some("hint_map_only"), drop: drop2, ..Default::default() })?;
        require(!is_true(&out, "eligible_for_promotion") && has(&out, code), || {
            format!("hint_map_only accepted a non-lite-pass certificate (want {code}): {}", out.repr())
        })?;
    }
    let out = v("PASS", PromoOpts { certificate: Some(PROMO_LITE_CERTIFICATE), ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion") && has(&out, "CERTIFICATE_GRADE_NOT_FULL"), || {
        format!("full_benchmark promotion opened on a lite-grade certificate: {}", out.repr())
    })?;
    let legacy = cert("explicit").replace("benchmark_mode: full\n", "");
    let out = v("PASS", PromoOpts { certificate: Some(&legacy), ..Default::default() })?;
    require(!has(&out, "CERTIFICATE_BENCHMARK_MODE_MISMATCH") && cert_grade(&out).as_deref() == Some("full"), || {
        format!("a legacy certificate without benchmark_mode was not read as full: {}", out.repr())
    })?;
    let out = v("FAIL", PromoOpts { certificate: Some(PROMO_LITE_CERTIFICATE), task_class: Some("hint_map_only"), drop: drop2, ..Default::default() })?;
    require(is_true(&out, "eligible_for_promotion") && has(&out, "HINT_MAP_ONLY_PROMOTION"), || {
        format!("a FAIL verdict with a lite-pass certificate was blocked on hint_map_only (verdict must be display-only): {}", out.repr())
    })?;
    let out = v("PASS", PromoOpts { task_class: Some("hint_map_only"), drop: &["simlog", "bench_report", "devlog"], ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion"), || {
        format!("hint_map_only reached promotion without devlog -- the journey evidence is not optional: {}", out.repr())
    })?;
    let out = v("PASS", PromoOpts { drop: &["simlog"], ..Default::default() })?;
    require(!is_true(&out, "eligible_for_promotion"), || {
        format!("full_benchmark stopped requiring simlog -- the new class relaxed the old one: {}", out.repr())
    })?;
    let (_td2, root2) = fresh_root();
    let mp = write_promotion_manifest(&root2, "PASS", Some(&rubric()), &PromoOpts::default());
    let vault = root2.join("docs/simlog/selftest_run");
    let _ = std::fs::remove_file(vault.join("run_summary.json"));
    let out = gate_verify(repo, &root2, &mp, false)?;
    require(has(&out, "EVIDENCE_SIMLOG_RUN_UNTERMINATED"), || format!("a simlog vault with no terminal record still passed the evidence gate: {}", out.repr()))?;
    write(&vault.join("level_01_measured.json"), "{\"measurement_ok\": true, \"decode_tps\": 1.0}\n");
    let out = gate_verify(repo, &root2, &mp, false)?;
    require(!has(&out, "EVIDENCE_SIMLOG_RUN_UNTERMINATED"), || format!("a bench-genre vault was blocked by a trial-genre rule: {}", out.repr()))
}

pub fn policy_and_evidence_lifecycle(repo: &Path) -> R<()> {
    require(policy_audit::scan_policy_citations("policy:SELFTEST_OK") == vec![(1, "SELFTEST_OK".to_string())], || {
        "policy citation broad scanner failed its canonical positive".into()
    })?;
    require(policy_audit::scan_policy_citations("xpolicy:NOT_A_CITATION").is_empty(), || {
        "policy citation scanner accepted an embedded token".into()
    })?;
    let o = gitx::run("bash", &[&ps(&repo.join(".claude/policies/runtime/policy_registry.sh")), "--self-test"], None, None);
    require(o.code == 0, || "policy registry self-test did not return 0".into())?;
    let o = gitx::run("bash", &[&ps(&repo.join(".claude/policies/runtime/evidence_publisher.sh")), "--self-test"], None, None);
    if o.code != 0 {
        return Err(Fail::Crash(format!("evidence_publisher self-test failed (rc={}): {}", o.code, crate::pytext::take_chars(&o.err_text(), 800))));
    }
    Ok(())
}

/// The agent_control owner program, by its Rust launcher (terraforming_node/rust `agent_control`).
pub const AGENT_CONTROL_REL: &str = ".claude/skills/terraforming_node/scripts/agent_control.sh";

pub fn agent_control_owner_selftest(repo: &Path) -> R<()> {
    let script = repo.join(AGENT_CONTROL_REL);
    require(script.is_file(), || format!("agent_control owner script missing: {}", ps(&script)))?;
    let script_s = ps(&script);
    let refs: Vec<&str> = vec![script_s.as_str(), "--self-test"];
    let o = gitx::run("bash", &refs, None, None);
    let text = if o.stderr.is_empty() { o.text() } else { o.err_text() };
    let c: Vec<char> = text.chars().collect();
    let tail: String = c[c.len().saturating_sub(800)..].iter().collect();
    require(o.code == 0, || format!("agent_control --self-test failed rc={}: {tail}", o.code))
}

pub fn execution_approval_authorization(repo: &Path) -> R<()> {
    let gate = gate_launcher(repo);
    let (approved_by, approved_at) = ("selftest", "2026-01-01T00:00:00Z");
    let base_atoms: Vec<String> = vec![format!("approved_by: {approved_by}"), format!("approved_at_utc: {approved_at}"), "allowed_action: sync_to_sub".into()];
    let scope_atoms = [
        "propagation_topology: single",
        "propagation_node_id: sub-1",
        "propagation_ssh_host: probe@node.example",
        "propagation_work_dir: /srv/vllm",
        "propagation_planes: overlay",
    ];
    let mut atoms = base_atoms.clone();
    atoms.extend(scope_atoms.iter().map(|x| x.to_string()));
    let plan_body = format!("# selftest plan\n\n## Execution approval\n{}\n", base_atoms.join("\n"));
    let td = TempDir::new();
    let repo_fx = std::fs::canonicalize(td.path()).unwrap().join("repo");
    std::fs::create_dir_all(repo_fx.join("docs/plan")).unwrap();
    std::fs::create_dir_all(repo_fx.join("docs/_evidence")).unwrap();
    let plan_path = repo_fx.join("docs/plan/p.md");
    write(&plan_path, &plan_body);
    let sha = sha256_hex(plan_body.as_bytes());
    let rel = "../plan/p.md";
    let strl = |v: &[String]| PyValue::List(v.iter().map(|x| s(x)).collect());
    let scope = |work_dir: &str, topology: &str, planes: &[&str]| {
        obj(vec![
            ("status", s("approved")),
            ("topology", s(topology)),
            ("node_id", s("sub-1")),
            ("ssh_host", s("probe@node.example")),
            ("work_dir", s(work_dir)),
            ("planes", PyValue::List(planes.iter().map(|x| s(x)).collect())),
            ("approved_utc", s(approved_at)),
            ("approval_atoms", strl(&atoms)),
        ])
    };
    let ea = |over: Vec<(&str, PyValue)>| {
        with(
            obj(vec![
                ("approved", PyValue::Bool(true)),
                ("approved_by", s(approved_by)),
                ("approved_at_utc", s(approved_at)),
                ("plan_path", s(rel)),
                ("plan_sha256", s(&sha)),
                ("approval_anchor", s("## Execution approval")),
                ("approval_atoms", strl(&base_atoms)),
                ("allowed_actions", PyValue::List(vec![s("sync_to_sub")])),
            ]),
            over,
        )
    };
    let manifest = |ea_v: Option<PyValue>, prop: Option<PyValue>| {
        let plan_rel = ea_v.as_ref().and_then(|e| gstr(e, "plan_path")).unwrap_or_else(|| rel.into());
        let mut items = vec![
            ("schema_version", PyValue::int(1)),
            ("task_class", s("harness_change")),
            (
                "identity",
                obj(vec![("model", s("m")), ("gpu", s("g")), ("vllm", s("v")), ("quant", PyValue::None), ("topology", s("single")), ("tp", PyValue::int(1))]),
            ),
            ("evidence", obj(vec![("plan", obj(vec![("path", s(&plan_rel))]))])),
            ("pii_scan", obj(vec![("passed", PyValue::Bool(true))])),
        ];
        if let Some(e) = ea_v {
            items.push(("execution_approval", e));
        }
        if let Some(p) = prop {
            items.push(("propagation_authorization", p));
        }
        obj(items)
    };
    let run = |mut man: PyValue, name: &str| -> R<PyValue> {
        let has_prop = get(&man, "propagation_authorization").map_or(false, |p| p.is_truthy());
        if let Some(e) = get(&man, "execution_approval").cloned() {
            if has_prop {
                let atoms_now: Vec<String> = match get(&e, "approval_atoms") {
                    Some(PyValue::List(l)) => l.iter().map(|x| if let PyValue::Str(t) = x { t.to_utf8().unwrap_or_default() } else { String::new() }).collect(),
                    _ => vec![],
                };
                let scoped = format!("# selftest plan\n\n## Execution approval\n{}\n", atoms_now.join("\n"));
                write(&plan_path, &scoped);
                let mut e2 = e.clone();
                set(&mut e2, "plan_sha256", s(&sha256_hex(scoped.as_bytes())));
                set(&mut man, "execution_approval", e2);
            } else {
                write(&plan_path, &plan_body);
                if !["bad_sha", "no_anchor", "wrong_action"].contains(&name) {
                    let mut e2 = e.clone();
                    set(&mut e2, "plan_sha256", s(&sha256_hex(plan_body.as_bytes())));
                    set(&mut man, "execution_approval", e2);
                }
            }
        } else {
            write(&plan_path, &plan_body);
        }
        let mp = repo_fx.join("docs/_evidence").join(format!("{name}.json"));
        write(&mp, &man.dumps_unicode().unwrap().to_utf8().unwrap());
        let o = gitx::run(
            "bash",
            &[&ps(&gate), "authorize", "--action", "sync_to_sub", "--mode", "experimental", "--manifest", &ps(&mp), "--repo-root", &ps(&repo_fx)],
            None,
            None,
        );
        require(!py_strip(&o.text()).is_empty(), || {
            let e = o.err_text();
            let c: Vec<char> = e.chars().collect();
            format!("authorize produced no JSON for {name}: {}", c[c.len().saturating_sub(400)..].iter().collect::<String>())
        })?;
        agent_runtime_support::pyjson::loads(&o.text()).map_err(|e| Fail::Crash(format!("JSONDecodeError: {e}")))
    };
    let ok = run(manifest(Some(ea(vec![])), None), "ok")?;
    require(is_true(&ok, "allowed") && gstr(&ok, "authorization_state").as_deref() == Some("execution-approved"), || {
        format!("a genuine execution approval must be admitted, got {:?}", codes(&ok))
    })?;
    let full_ea = || ea(vec![("approval_atoms", strl(&atoms))]);
    let p = run(manifest(Some(full_ea()), Some(scope("/srv/vllm", "single", &["overlay"]))), "propagation")?;
    require(is_true(&p, "allowed"), || format!("approved exact propagation scope must admit sync_to_sub: {}", p.repr()))?;
    let ne = run(manifest(None, Some(scope("/srv/vllm", "single", &["overlay"]))), "no_execution")?;
    require(!is_true(&ne, "allowed") && has(&ne, "PROPAGATION_AUTHORIZATION_EXECUTION_APPROVAL_ABSENT"), || {
        format!("standalone propagation scope must be rejected: {}", ne.repr())
    })?;
    let bp = run(manifest(Some(full_ea()), Some(scope("/srv/vllm", "multi", &["band2", "overlay"]))), "bad_propagation")?;
    require(!is_true(&bp, "allowed") && has(&bp, "PROPAGATION_AUTHORIZATION_TOPOLOGY_MISMATCH"), || {
        format!("propagation scope topology drift must be rejected: {}", bp.repr())
    })?;
    let cd = run(manifest(Some(full_ea()), Some(scope("/srv/other", "single", &["overlay"]))), "changed_destination")?;
    require(!is_true(&cd, "allowed") && has(&cd, "EXECUTION_APPROVAL_ATOMS_INVALID"), || {
        format!("destination drift outside approved plan bytes must invalidate the approval atoms: {}", cd.repr())
    })?;
    let sd = run(manifest(Some(full_ea()), Some(scope("/srv/vllm;id", "single", &["overlay"]))), "shell_destination")?;
    require(!is_true(&sd, "allowed") && has(&sd, "PROPAGATION_AUTHORIZATION_WORK_DIR_INVALID"), || {
        format!("shell metacharacter destination must be rejected: {}", sd.repr())
    })?;
    let sync_script = std::fs::read_to_string(repo.join(".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"))
        .map_err(|e| Fail::Crash(format!("FileNotFoundError: {e}")))?;
    let guard = "STOP(PROPAGATION_SCOPE_BRANCH_TRANSITION)";
    let tx = "begin_remote_transaction \"$t\" 0";
    let gpos = sync_script.find(guard);
    let tpos = sync_script.find(tx);
    if gpos.is_some() && tpos.is_none() {
        return Err(Fail::Crash("ValueError: substring not found".into()));
    }
    require(gpos.is_some() && gpos < tpos, || "established scope must reject branch transition before B1 transaction".into())?;
    let bs = run(manifest(Some(ea(vec![("plan_sha256", s(&"0".repeat(64)))])), None), "bad_sha")?;
    require(!is_true(&bs, "allowed") && has(&bs, "EXECUTION_APPROVAL_PLAN_DIGEST_MISMATCH"), || {
        format!("a plan whose bytes changed after approval must be rejected: {}", bs.repr())
    })?;
    let na = run(manifest(Some(ea(vec![("approved", PyValue::Bool(false))])), None), "not_approved")?;
    require(!is_true(&na, "allowed") && has(&na, "EXECUTION_APPROVAL_NOT_APPROVED"), || format!("approved=false must be rejected: {}", na.repr()))?;
    let mut wrong_atoms = base_atoms[..2].to_vec();
    wrong_atoms.push("allowed_action: hint_push".into());
    let wtext = format!("# selftest plan\n\n## Execution approval\n{}\n", wrong_atoms.join("\n"));
    write(&repo_fx.join("docs/plan/w.md"), &wtext);
    let wa = run(
        manifest(
            Some(ea(vec![
                ("plan_path", s("../plan/w.md")),
                ("plan_sha256", s(&sha256_hex(wtext.as_bytes()))),
                ("allowed_actions", PyValue::List(vec![s("hint_push")])),
                ("approval_atoms", strl(&wrong_atoms)),
            ])),
            None,
        ),
        "wrong_action",
    )?;
    require(!is_true(&wa, "allowed") && codes(&wa) == ["EXECUTION_APPROVAL_ACTION_NOT_ALLOWED"], || {
        format!("an approval for a different (live) action must be refused *because the action differs*: {}", wa.repr())
    })?;
    write(&repo_fx.join("docs/plan/q.md"), "# no anchor here\n");
    let nq = run(
        manifest(Some(ea(vec![("plan_path", s("../plan/q.md")), ("plan_sha256", s(&sha256_hex(b"# no anchor here\n")))])), None),
        "no_anchor",
    )?;
    require(!is_true(&nq, "allowed") && has(&nq, "EXECUTION_APPROVAL_PLAN_ATOMS_MISSING"), || {
        format!("a plan without the anchor/atoms must be rejected: {}", nq.repr())
    })
}

pub fn duplicate_certificate_predicate() -> R<()> {
    let base = "schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: m\ngpu_model: \"NVIDIA GB10\"\nvllm_version: 0.18.0\nquantization: mxfp4\ntopology: single\ntensor_parallel_size: 1\n";
    let a = format!("{base}measured_utc: \"2026-09-03T12:00:00Z\"\n");
    let b = format!("{base}measured_utc: \"2026-09-03T12:00:01Z\"\n");
    let g = |v: Vec<(&str, &str)>| tw::duplicate_certificate_groups(&v.into_iter().map(|(x, y)| (x.to_string(), y.to_string())).collect::<Vec<_>>());
    let (d, u) = g(vec![("x.yaml", &a), ("y.yaml", &a)]);
    require(d.len() == 1 && d[0].1 == ["x.yaml", "y.yaml"] && u.is_empty(), || "같은 키 두 장은 한 그룹으로 잡혀야 한다".into())?;
    let (d, u) = g(vec![("x.yaml", &a), ("y.yaml", &b)]);
    require(d.is_empty() && u.is_empty(), || "★음성대조 measured_utc 가 다르면 다른 측정이다".into())?;
    let (d, u) = g(vec![("x.yaml", &a), ("z.yaml", base)]);
    require(d.is_empty() && u == ["z.yaml"], || "★키 결측은 '매치 없음' 이 아니라 식별불가로 나와야 한다".into())?;
    let (_, u) = g(vec![("n.yaml", "a:\n  nested: 1\n")]);
    require(u == ["n.yaml"], || "★파싱 실패도 식별불가".into())?;
    let f = crate::certkey::CERTIFICATE_RUN_KEY_FIELDS;
    require(f[f.len() - 1] == "measured_utc" && !f.contains(&"image_digest"), || "키는 강한 6키 + measured_utc 뿐 — 소프트 지문은 identity 가 아니다".into())
}

pub fn deployed_pii_predicate(repo: &Path) -> R<()> {
    let sc = tw::TsScanner::new(repo.join(tw::FORBIDDEN_SCANNER_REL))?;
    let private = format!("host {}.{}.0.7\nclean line\n", 192, 168);
    let hits = sc.scan_lines(&private, &["secret-node"])?;
    require(hits.iter().any(|h| h.1.starts_with("generic:")), || "scan_lines must fire on private IPv4".into())?;
    let h = sc.scan_lines("node secret-node", &["secret-node"])?;
    require(h.first().map(|x| x.1.as_str()) == Some("term:secret-node"), || "scan_lines must fire on a literal term".into())?;
    let ex = sc.scan_lines(&format!("{}.{}.0.7  # {}", 192, 168, sc.exempt_mark), &[])?;
    require(!ex.is_empty() && ex[0].2, || "EXEMPT_MARK must mark the row exempt, not drop it".into())?;
    require(sc.scan_lines("GB10 2-node TP=2 · 53.92 t/s", &["secret-node"])?.is_empty(), || "clean text must not produce hits".into())?;
    if tw::is_canonical(repo) {
        let t = tw::tracked_paths(repo)?;
        require(t.len() > 50 && t.iter().any(|x| x == "CLAUDE.md"), || format!("deployed surface enumeration looks wrong: {} paths", t.len()))?;
        require(
            t.iter().any(|r| {
                !(r.starts_with(".claude/") || r.starts_with("docs/report/") || r.starts_with("hints/")) && !["CLAUDE.md", "README.md", "HINTS.md"].contains(&r.as_str())
            }),
            || "tripwire5 reach regressed to the old three-prefix list".into(),
        )?;
    }
    Ok(())
}

pub fn root_registry_predicate(repo: &Path) -> R<()> {
    let names = tw::root_names_of(&["a/b/c".into(), "x.md".into(), "campaigns/_template/t.json".into()]);
    require(names.into_iter().collect::<Vec<_>>() == ["a", "campaigns", "x.md"], || "root-name derivation must cut at the first path segment".into())?;
    require(tw::fnmatch("config.camp1-20b-0180.yaml", "config.camp1-*.yaml"), || "tombstone globs must match the escaped campaign variants that caused the leak".into())?;
    require(!tw::fnmatch("configs", "config*.yaml"), || "the tombstone glob must not swallow the tracked `configs/` directory".into())?;
    if tw::is_canonical(repo) {
        let doc = tw::load_root_registry(repo)?;
        let mut names = std::collections::BTreeSet::new();
        let mut tomb_tasks = false;
        if let Some(PyValue::List(es)) = get(&doc, "entries") {
            for e in es {
                if let Some(n) = gstr(e, "name") {
                    names.insert(n);
                }
            }
        }
        if let Some(PyValue::List(ts)) = get(&doc, "tombstones") {
            tomb_tasks = ts.iter().any(|t| gstr(t, "name").as_deref() == Some("tasks"));
        }
        require(["campaigns", "CLAUDE.md", ".claude"].iter().all(|n| names.contains(*n)), || {
            format!("root registry looks wrong: {}", py_repr_list(&names.iter().take(10).cloned().collect::<Vec<_>>()))
        })?;
        require(tomb_tasks, || "the retired `tasks/` location must stay recorded as a tombstone".into())?;
    }
    Ok(())
}

pub fn topology_layer_parity_predicate() -> R<()> {
    let td = TempDir::new();
    let root = std::fs::canonicalize(td.path()).unwrap();
    let r = ps(&root);
    git_ok(&["init", "-q", "-b", "multi-node", &r])?;
    let spec = root.join(".claude/rules/strategy.topology.md");
    write(&spec, "# s\n\n**topology: single**\n");
    git_ok(&["-C", &r, "add", "-A"])?;
    let (_, c, _, _) = tw::parity_evaluate(&root)?;
    require(c.iter().any(|x| x == "LAYER_HEADER_MISMATCH"), || format!("브랜치 multi-node ↔ 헤더 single 이 발화하지 않았다: {}", py_repr_list(&c)))?;
    write(&spec, "# s\n\n**topology: multi**\n");
    git_ok(&["-C", &r, "add", "-A"])?;
    let (v, _, reasons, doc) = tw::parity_evaluate(&root)?;
    require(v == "PASS", || format!("일치 상태가 PASS 가 아니다: {}", py_repr_list(&reasons)))?;
    let leg = |k: &str| get(&doc, "legs").and_then(|l| get(l, k)).and_then(|x| gstr(x, "status")).unwrap_or_default();
    require(leg("manifest") == "absent" && leg("campaign") == "absent", || {
        format!("비추적 다리의 부재는 absent 로 표시돼야 한다(위반이 아니다) — {}/{}", leg("manifest"), leg("campaign"))
    })?;
    let lc = get(&doc, "legs_checked").map(|x| x.repr()).unwrap_or_default();
    require(lc == "2", || format!("3자·2자 판정을 4자처럼 보고하면 안 된다: legs_checked={lc}"))?;
    git_ok(&["-C", &r, "rm", "-q", "--cached", ".claude/rules/strategy.topology.md"])?;
    let (_, c, _, _) = tw::parity_evaluate(&root)?;
    require(c.iter().any(|x| x == "LAYER_ABSENT"), || format!("특화 파일 0개가 발화하지 않았다: {}", py_repr_list(&c)))?;
    drop(td);
    let td = TempDir::new();
    let root = td.path().to_path_buf();
    std::fs::create_dir_all(root.join(".claude/rules")).unwrap();
    write(&root.join("CLAUDE.md"), "# c\n서브는 Ray 워커다.\n");
    require(!tw::vocabulary_offenders(&root).is_empty(), || "공통층의 멀티 전용 어휘가 발화하지 않았다".into())?;
    write(&root.join("CLAUDE.md"), "# c\n서브의 정체는 토폴로지가 정한다.\n");
    require(tw::vocabulary_offenders(&root).is_empty(), || "음성대조 실패 — 어휘가 없는데 발화했다".into())?;
    write(&root.join(".claude/rules/strategy.topology.md"), "**topology: multi**\n서브는 Ray 워커다.\n");
    require(tw::vocabulary_offenders(&root).is_empty(), || "특화층 파일이 공통층 스캔에 들어왔다 — 접미사 규약이 안 먹었다".into())?;
    require(tw::TOPOLOGY_VOCABULARY_EXEMPTIONS.iter().any(|e| e.0 == ".claude/rules/workflow.md"), || {
        "면제 목록이 비었다 — verbatim 잠금 문장에 대한 면제가 사라지면 라이브가 RED 가 된다".into()
    })
}

pub const WATCHDOG_PREDICATE_FILES: [&str; 3] = [
    ".claude/skills/terraforming_node/scripts/node_blackbox/mem_watchdog_eta.sh",
    ".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh",
    ".claude/skills/terraforming_node/scripts/node_blackbox/thermal_watchdog.sh",
];

fn extract_marked_block(text: &str, marker: &str) -> Option<String> {
    let lines = crate::pytext::py_splitlines(text);
    let (mut start, mut end) = (None, None);
    for (i, l) in lines.iter().enumerate() {
        let st = py_strip(l);
        if st.starts_with(&format!("# ── /{marker}")) {
            end = Some(i);
            break;
        }
        if start.is_none() && st.starts_with(&format!("# ── {marker}")) {
            start = Some(i);
        }
    }
    match (start, end) {
        (Some(a), Some(b)) if b > a => Some(lines[a..=b].join("\n")),
        _ => None,
    }
}

pub fn watchdog_target_predicate_parity(root: &Path) -> R<()> {
    if !tw::is_canonical(root) {
        return Ok(());
    }
    let selftest_files = [WATCHDOG_PREDICATE_FILES[0], WATCHDOG_PREDICATE_FILES[2]];
    for (marker, files) in [("BB_TARGET_PREDICATE_V1", &WATCHDOG_PREDICATE_FILES[..]), ("BB_TARGET_PREDICATE_SELFTEST_V1", &selftest_files[..])] {
        let mut blocks: Vec<(String, String)> = Vec::new();
        for rel in files {
            let p = root.join(rel);
            require(p.is_file(), || format!("watchdog predicate carrier missing: {rel}"))?;
            let text = std::fs::read_to_string(&p).map_err(|e| Fail::Crash(e.to_string()))?;
            let b = extract_marked_block(&text, marker);
            require(b.is_some(), || format!("{rel} lost its {marker} block (guard would silently disarm)"))?;
            blocks.push((rel.to_string(), b.unwrap()));
        }
        let distinct: std::collections::BTreeSet<&String> = blocks.iter().map(|x| &x.1).collect();
        require(distinct.len() == 1, || {
            format!(
                "{marker} diverged across {} carriers: {}",
                files.len(),
                blocks.iter().map(|(r, b)| format!("{r}={}", &sha256_hex(b.as_bytes())[..12])).collect::<Vec<_>>().join(", ")
            )
        })?;
    }
    for rel in selftest_files {
        let o = gitx::run("bash", &[&ps(&root.join(rel)), "--self-test"], None, None);
        let out = o.text();
        let c: Vec<char> = out.chars().collect();
        require(o.code == 0, || format!("{rel} --self-test failed: {}", c[c.len().saturating_sub(800)..].iter().collect::<String>()))?;
        require(out.contains("★음성대조"), || format!("{rel} --self-test ran without the kill-target negative control"))?;
    }
    Ok(())
}

pub fn base_skill_edges_predicate() -> R<()> {
    let sk = format!("skill{}", "s");
    require(tw::skill_edge_targets(&format!("x = \".claude/{sk}/wiki-desk/src/cli.ts\"")).into_iter().collect::<Vec<_>>() == ["wiki-desk/src/cli.ts"], || {
        "slash literal target".into()
    })?;
    require(
        tw::skill_edge_targets(&format!("os.path.join(REPO, \".claude\", \"{sk}\", \"tn\", \"scripts\", \"a.py\")")).into_iter().collect::<Vec<_>>() == ["tn/scripts/a.py"],
        || "join-style target".into(),
    )?;
    require(tw::skill_edge_targets("mentions my-skills/ and reskills/x").is_empty(), || "word-embedded 'skills/' must not match".into())?;
    let td = TempDir::new();
    let r = td.path().to_path_buf();
    let rs = ps(&r);
    git_ok(&["init", "-q", &rs])?;
    let a = r.join(".claude/policies/runtime/a.sh");
    write(&a, &format!("P = \".claude/{sk}/s1/scripts/x.py\"\n"));
    let ledger = |reason: &str| {
        format!("{{\"edges\": [{{\"caller\": \".claude/policies/runtime/a.sh\", \"callee\": \"s1/scripts/x.py\", \"phase\": \"verification_plane\", \"reason\": \"{reason}\"}}]}}")
    };
    write(&r.join(tw::BASE_SKILL_EDGES_REL), &ledger("fixture"));
    git_ok(&["-C", &rs, "add", "-A"])?;
    require(tw::base_skill_edge_violations(&r)?.is_empty(), || "listed edge must pass".into())?;
    write(&a, &format!("P = \".claude/{sk}/s1/scripts/x.py\"\nQ = \".claude/{sk}/s2/scripts/y.py\"\n"));
    let v = tw::base_skill_edge_violations(&r)?;
    require(v.len() == 1 && v[0].contains("미등재") && v[0].contains("s2/scripts/y.py"), || format!("new edge must be RED: {}", py_repr_list(&v)))?;
    write(&a, "P = 1\n");
    let v = tw::base_skill_edge_violations(&r)?;
    require(v.len() == 1 && v[0].contains("사라진"), || format!("stale ledger row must be RED: {}", py_repr_list(&v)))?;
    write(&r.join(tw::BASE_SKILL_EDGES_REL), &ledger(""));
    write(&a, &format!("P = \".claude/{sk}/s1/scripts/x.py\"\n"));
    let v = tw::base_skill_edge_violations(&r)?;
    require(v.iter().any(|x| x.contains("phase/reason")), || format!("reasonless row must be RED: {}", py_repr_list(&v)))?;
    write(&r.join(tw::BASE_SKILL_EDGES_REL), &ledger("fixture"));
    write(&r.join(".claude/policies/runtime/old.py"), &format!("P = \".claude/{sk}/s3/scripts/z.py\"\n"));
    write(&r.join(".claude/rust/policy-x/src/new.rs"), &format!("const P: &str = \".claude/{sk}/s4/src/w.rs\";\n"));
    write(&r.join(".claude/rust/policy-x/checks/data.rs"), &format!("const P: &str = \".claude/{sk}/s5/x\";\n"));
    git_ok(&["-C", &rs, "add", "-A"])?;
    let v = tw::base_skill_edge_violations(&r)?;
    require(v.len() == 1 && v[0].contains("미등재") && v[0].contains(".claude/rust/policy-x/src/new.rs") && v[0].contains("s4/src/w.rs"),
            || format!("only the Rust policy source is an edge source (.py and checks/ are not): {}", py_repr_list(&v)))
}

pub fn router_bindings_predicate() -> R<()> {
    require(
        tw::anchor_token_present("2.7", "## 2.7 노드") && !tw::anchor_token_present("2.7", "### 2.7.0 x") && tw::anchor_token_present("2.7.7a", "→ 2.7.7a 턴제"),
        || "anchor token boundaries".into(),
    )?;
    let tn = format!("terraforming{}", "_node");
    let td = TempDir::new();
    let r = td.path().to_path_buf();
    let rs = ps(&r);
    git_ok(&["init", "-q", &rs])?;
    write(&r.join(tw::TN_DIR_REL).join("references/a.md"), "x\n");
    write(&r.join(tw::TN_SKILL_REL), "## 2.6 호스트\n### 2.7.1 평면\n→ `references/a.md`\n");
    write(&r.join("cite.md"), &format!("`{tn}` SKILL.md §2.7.1 · {tn}/references/a.md\n"));
    write(
        &r.join(".claude/policies/runtime/p.py"),
        &format!("def f():\n    s = _read('.claude/{}/{tn}/SKILL.md')\n    _require('## 2.6 호스트' in s)\n", format!("skill{}", "s")),
    );
    git_ok(&["-C", &rs, "add", "-A"])?;
    let v = tw::router_binding_violations(&r)?;
    require(v.is_empty(), || format!("clean fixture must pass: {}", py_repr_list(&v)))?;
    write(&r.join("cite.md"), &format!("`{tn}` SKILL.md §2.7.9 · {tn}/references/zz.md\n"));
    let v = tw::router_binding_violations(&r)?;
    require(v.iter().any(|x| x.contains("§2.7.9")) && v.iter().any(|x| x.contains("zz.md")), || format!("dangling cites must be RED: {}", py_repr_list(&v)))?;
    write(&r.join("cite.md"), "ok\n");
    write(&r.join(tw::TN_SKILL_REL), "### 2.7.1 평면\n");
    let v = tw::router_binding_violations(&r)?;
    require(v.iter().any(|x| x.contains('④') && x.contains("2.6")), || format!("removed bound literal must be RED: {}", py_repr_list(&v)))
}

fn crash(msg: &str) -> i32 {
    eprintln!("Traceback (most recent call last):\n  File \"runtime_selftest.py\", in main\n{msg}");
    1
}

pub fn run(repo: &Path) -> i32 {
    let steps: Vec<(&str, Box<dyn Fn() -> R<()>>)> = vec![
        ("_test_no_production_asserts", Box::new(|| no_production_asserts(repo))),
        ("_test_nested_worktree_isolation", Box::new(nested_worktree_isolation)),
        ("_test_completion_gate", Box::new(|| completion_gate(repo))),
        ("_test_promotion_rubric_carrier", Box::new(|| promotion_rubric_carrier(repo))),
        ("_test_certificate_waiver_by_authority", Box::new(|| certificate_waiver_by_authority(repo))),
        ("_test_certificate_run_resolution", Box::new(|| certificate_run_resolution(repo))),
        ("_test_hint_binding_source", Box::new(|| hs::hint_binding_source(repo))),
        ("_test_hint_cli_publication", Box::new(|| hs::hint_cli_publication(repo))),
        ("_test_hint_cli_refute_paths", Box::new(|| hs::hint_cli_refute_paths(repo))),
        ("_test_hint_map_only_promotion", Box::new(|| hint_map_only_promotion(repo))),
        ("_test_hint_map_only_publication", Box::new(|| hs::hint_map_only_publication(repo))),
        ("_test_policy_and_evidence_lifecycle", Box::new(|| policy_and_evidence_lifecycle(repo))),
        ("_test_agent_control_owner_selftest", Box::new(|| agent_control_owner_selftest(repo))),
        ("_test_execution_approval_authorization", Box::new(|| execution_approval_authorization(repo))),
        ("_test_duplicate_certificate_predicate", Box::new(duplicate_certificate_predicate)),
        ("_test_deployed_pii_predicate", Box::new(|| deployed_pii_predicate(repo))),
        ("_test_root_registry_predicate", Box::new(|| root_registry_predicate(repo))),
        ("_test_topology_layer_parity_predicate", Box::new(topology_layer_parity_predicate)),
        ("_test_watchdog_target_predicate_parity", Box::new(|| watchdog_target_predicate_parity(repo))),
        ("_test_base_skill_edges_predicate", Box::new(base_skill_edges_predicate)),
        ("_test_router_bindings_predicate", Box::new(router_bindings_predicate)),
    ];
    for (name, step) in steps {
        match step() {
            Ok(()) => {}
            Err(Fail::Selftest(m)) => return crash(&format!("__main__.RuntimeSelftestFailure: {m}  [{name}]")),
            Err(Fail::Crash(t)) => return crash(&format!("{t}  [{name}]")),
        }
    }
    tw::announce_non_canonical(repo, "runtime_selftest");
    let repo_state: Vec<Box<dyn Fn() -> R<()>>> = vec![
        Box::new(|| tw::no_backup_artifacts(repo)),
        Box::new(tw::digest_data_roles),
        Box::new(|| tw::no_tracked_digest_rewrite(repo)),
        Box::new(|| tw::no_retired_hash_mechanism_prose(repo)),
        Box::new(|| tw::no_duplicate_certificates(repo)),
        Box::new(|| tw::no_pii_in_deployed_artifacts(repo, repo)),
        Box::new(|| tw::no_revived_antipatterns(repo)),
        Box::new(|| tw::root_surface_registry(repo)),
        Box::new(|| tw::topology_layer_parity(repo)),
        Box::new(|| tw::base_skill_edges(repo)),
        Box::new(|| tw::router_bindings(repo)),
    ];
    for step in repo_state {
        match step() {
            Ok(()) => {}
            Err(Fail::Selftest(m)) => return crash(&format!("__main__.RuntimeSelftestFailure: {m}")),
            Err(Fail::Crash(t)) => return crash(&t),
        }
    }
    for w in tw::tripwire_executor_wiring(repo) {
        eprintln!("[runtime_selftest] WARN {w}");
    }
    println!("[runtime_selftest] PASS");
    0
}
