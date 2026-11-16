//! The four hint-publisher tests of the full `runtime_selftest.py` suite (`_test_hint_binding_source`,
//! `_test_hint_cli_publication`, `_test_hint_cli_refute_paths`, `_test_hint_map_only_publication`), same assertions and
//! messages.
//!
//! The original imported `hint.py`/`hintlib` in-process for the fixture assembly (`hint._fx_repo` · `_fx_legacy_remote` ·
//! `_fx_author` · `_fx_tree`) and the unit judges, and drove the CLI under test as a **subprocess** (a copy of the CLI in
//! the deployment layout of the fixture). Here the fixture assemblers and judges are the `hintlib` crate's (the same
//! assemblers the hint CLI self-test uses — `hintlib::cli::fixtures`), and the CLI under test is the deployed launcher
//! `scripts/hint.sh` copied into the fixture's deployment layout, executing this checkout's native `hint` binary
//! (`HINT_BIN`) with the fixture's skill directory (`HINT_SKILL_DIR`). The fake `docker` is a bash executable with the same
//! answers as the original's Python shim (no Python is started). Intended difference: the programmatic authoring step runs
//! in-process (the original used a child interpreter only to escape its own `-S` flag; there is no interpreter here).
use crate::tripwires::{require, Fail, R};
use hintlib::cli::fixtures;
use hintlib::value::{self, J};
use hintlib::{artifacts, branch, catalog, core, evidence, tag, template};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const HINT_SCRIPTS_REL: &str = ".claude/skills/hint-publisher/scripts";
const HINT_SKILL_REL: &str = ".claude/skills/hint-publisher";
const HINT_BIN_REL: &str = ".claude/skills/hint-publisher/rust/target/release/hint";
const HINT_VOCAB_REL: &str = "hints/vocab.json";
const SWEEP_META_PRODUCER_REL: &str = ".claude/skills/adversarial-benchmark/rust/src/assemble_sweep_meta.rs";
const RENDER_REPORT_REL: &str = ".claude/skills/adversarial-benchmark/scripts/render_report.sh";
const PUBLISH_RECORD_REL: &str = ".claude/skills/adversarial-benchmark/scripts/publish_benchmark_record.sh";
const FX_CAMPAIGN: &str = "c1";
const FX_CELL: &str = "c1-a";
const FX_LITE_CELL: &str = "c1-l";
const FX_BASE_TAG: &str = "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len4096-kvauto";
const FX_LITE_TAG: &str = "hint/0.9.0/fixture-model-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len2048-kvauto";
const FX_PUBLISH_UTC: &str = "2026-01-02T06:00:00Z";
const FX_CONTINUE_UTC: &str = "2026-01-02T07:00:00Z";
const FX_LITE_MEASURED_UTC: &str = "2026-01-02T14:00:00Z";
const FX_LITE_REPORT: &str = "docs/benchmark/bench_report_26010214_fixture-model-nvfp4_GB10_0.9.0.md";
const FX_LITE_CERT: &str = "docs/benchmark/benchmark_26010214_fixture-model-nvfp4_GB10_0.9.0.yaml";
const PROMO_RUBRIC: &str = r#"{"rubric_authority": "explore", "floor_tps": 22.1, "ratio_M_over_primary": 0.719, "primary_source": "E(external_reference)", "rubric_source": "verdict_json"}"#;

fn fx_tag() -> String {
    format!("{FX_BASE_TAG}-eager")
}
fn fx_tail() -> J {
    jv(r#"[{"token": "eager", "meaning": "CUDA graph 대신 eager 실행 — 같은 q·len·kv 의 graph 셀과 가른다",
            "evidence": {"file": "output/multi/configs/c1-a.yaml", "key": "enforce-eager", "value": "true"}}]"#)
}

// ───────────────────────────────────────────── plumbing
fn jv(text: &str) -> J {
    value::loads(text).expect("hint_suite JSON literal")
}
/// An owner error surfaced where the original let the exception propagate (a crash of the test, not a verdict).
fn crash<E: std::fmt::Display>(e: E) -> Fail {
    Fail::Crash(e.to_string())
}
fn fail(msg: impl Into<String>) -> Fail {
    Fail::Selftest(msg.into())
}
fn ps(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
/// Python `s[-n:]` over characters (messages only).
fn tail(s: &str, n: usize) -> String {
    let c: Vec<char> = s.chars().collect();
    c[c.len().saturating_sub(n)..].iter().collect()
}
fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
fn read(p: &Path) -> R<String> {
    std::fs::read_to_string(p).map_err(|e| crash(format!("FileNotFoundError: {}: {e}", ps(p))))
}
fn read_b(p: &Path) -> R<Vec<u8>> {
    std::fs::read(p).map_err(|e| crash(format!("FileNotFoundError: {}: {e}", ps(p))))
}
fn write(p: &Path, text: &str) -> R<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| crash(format!("OSError: {}: {e}", ps(d))))?;
    }
    std::fs::write(p, text).map_err(|e| crash(format!("OSError: {}: {e}", ps(p))))
}
fn write_b(p: &Path, b: &[u8]) -> R<()> {
    std::fs::write(p, b).map_err(|e| crash(format!("OSError: {}: {e}", ps(p))))
}
fn load(p: &Path) -> R<J> {
    value::loads(&read(p)?).map_err(|e| crash(format!("JSONDecodeError: {}: {e}", ps(p))))
}
fn copy(src: &Path, dst: &Path) -> R<()> {
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d).map_err(|e| crash(format!("OSError: {}: {e}", ps(d))))?;
    }
    std::fs::copy(src, dst).map(|_| ()).map_err(|e| crash(format!("OSError: copy {} -> {}: {e}", ps(src), ps(dst))))
}
fn s(v: &J) -> String {
    v.as_str().map(str::to_string).unwrap_or_default()
}
fn strs(v: &J) -> Vec<String> {
    v.items().iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
}
fn reason_codes(g: &J) -> Vec<String> {
    strs(&g["reason_codes"])
}

/// `os.environ` edits for a scope (set and pop), restored in reverse on drop.
struct EnvScope(Vec<(String, Option<std::ffi::OsString>)>);
impl EnvScope {
    fn new() -> Self {
        EnvScope(Vec::new())
    }
    fn set(&mut self, k: &str, v: &str) {
        self.0.push((k.to_string(), std::env::var_os(k)));
        std::env::set_var(k, v);
    }
    fn pop(&mut self, k: &str) {
        self.0.push((k.to_string(), std::env::var_os(k)));
        std::env::remove_var(k);
    }
}
impl Drop for EnvScope {
    fn drop(&mut self) {
        for (k, v) in self.0.drain(..).rev() {
            match v {
                Some(v) => std::env::set_var(&k, v),
                None => std::env::remove_var(&k),
            }
        }
    }
}

/// A temporary directory (`tempfile.TemporaryDirectory(prefix=...)`), resolved, removed on drop.
struct Td(PathBuf);
impl Td {
    fn new(prefix: &str) -> R<Td> {
        let base = std::env::temp_dir();
        for i in 0..1000u32 {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let p = base.join(format!("{prefix}{}{nanos:x}{i}", std::process::id()));
            if std::fs::create_dir(&p).is_ok() {
                return Ok(Td(std::fs::canonicalize(&p).unwrap_or(p)));
            }
        }
        Err(crash("FileExistsError: could not create a temporary directory"))
    }
}
impl Drop for Td {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(argv: &[String], cwd: Option<&Path>, env: &[(String, String)], env_remove: &[&str], input: Option<&[u8]>) -> R<Out> {
    let o = core::run_capture(argv, cwd, env, env_remove, input).map_err(|e| crash(format!("OSError: {e}")))?;
    Ok(Out {
        code: o.returncode,
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    })
}

/// `_hint_isolated_env(td)`: isolated git/credential environment for fixture assembly and the CLI subprocesses.
fn isolated_env(td: &Path) -> R<EnvScope> {
    let gcfg = td.join("gitconfig");
    let mail = ["selftest", "example.invalid"].join("@");
    write(
        &gcfg,
        &format!("[user]\n\tname = selftest\n\temail = {mail}\n[commit]\n\tgpgsign = false\n[tag]\n\tgpgsign = false\n[core]\n\thooksPath = /dev/null\n"),
    )?;
    let mut env = EnvScope::new();
    for k in [
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_TERMINAL_PROMPT",
        "GITHUB_TOKEN",
        "QUANT_MODEL_PATH",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        env.pop(k);
    }
    env.set("GIT_CONFIG_GLOBAL", &ps(&gcfg));
    env.set("GIT_CONFIG_NOSYSTEM", "1");
    env.set("GIT_TERMINAL_PROMPT", "0");
    Ok(env)
}

/// The in-process `hintlib` calls locate the skill from `HINT_SKILL_DIR` (this executable is not the hint binary).
fn hint_lib_env(root: &Path) -> EnvScope {
    let mut env = EnvScope::new();
    env.set("HINT_SKILL_DIR", &ps(&root.join(HINT_SKILL_REL)));
    env
}

fn hgit(repo: &Path, args: &[&str]) -> R<String> {
    let mut argv = vec!["git".to_string(), "-C".to_string(), ps(repo)];
    argv.extend(args.iter().map(|a| a.to_string()));
    let o = run(&argv, None, &[], &[], None)?;
    if o.code != 0 {
        let h: Vec<String> = args.iter().take(3).map(|a| format!("'{a}'")).collect();
        let h = if h.len() == 1 { format!("({},)", h[0]) } else { format!("({})", h.join(", ")) };
        return Err(fail(format!("hint fixture git {h} rc={}: {}", o.code, tail(o.stderr.trim(), 400))));
    }
    Ok(o.stdout.trim().to_string())
}
fn refs(repo: &Path) -> R<String> {
    hgit(repo, &["for-each-ref", "--format=%(refname) %(objectname)"])
}
fn remote_ref(bare: &Path, r: &str) -> R<Option<String>> {
    let out = hgit(bare, &["ls-remote", &ps(bare), r])?;
    let hits: Vec<String> = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let mut it = l.splitn(2, '\t');
            let sha = it.next()?;
            let name = it.next()?;
            (name == r).then(|| sha.to_string())
        })
        .collect();
    Ok(if hits.len() == 1 { Some(hits[0].clone()) } else { None })
}
fn tag_body(repo: &Path, t: &str) -> R<String> {
    hgit(repo, &["cat-file", "tag", &format!("refs/tags/{t}")])
}

// ───────────────────────────────────────────── fake docker
/// `_hint_shim_cid(n)`.
fn shim_cid(n: usize) -> String {
    format!("c0ffee{n:058x}")
}

fn sq(t: &str) -> String {
    format!("'{}'", t.replace('\'', "'\\''"))
}

/// The fake `docker` (`_HINT_DOCKER_SHIM`): read-only `image inspect`/`history`, a non-starting container `create`
/// (deterministic id = n-th create), `cp` = file absent (old image without a ledger/patch copy), `rm` succeeds, `run` (ledger
/// cat) rc 1, anything else rc 1. Every call's argv is logged (one line, arguments terminated by U+001F).
fn write_docker_shim(bindir: &Path, state_dir: &Path, log: &Path, docker: &J) -> R<()> {
    std::fs::create_dir_all(state_dir).map_err(|e| crash(format!("OSError: {e}")))?;
    let mut inspect = String::new();
    let mut create = String::new();
    if let Some(m) = docker["images"].as_dict() {
        for (i, (k, img)) in m.iter().enumerate() {
            let base = state_dir.join(format!("img{i}"));
            write(&base.with_extension("json"), &format!("{}\n", value::dumps_default(&J::List(vec![img.clone()]))))?;
            for (ext, key) in [("id", "Id"), ("created", "Created")] {
                let v = &img[key];
                write(&base.with_extension(ext), &if v.truthy() { format!("{}\n", v.display()) } else { String::new() })?;
            }
            inspect.push_str(&format!("    {}) d={} ;;\n", sq(k), sq(&ps(&base))));
            create.push_str(&format!("    {}) ;;\n", sq(k)));
        }
    }
    let mut history = String::new();
    if let Some(m) = docker["history"].as_dict() {
        for (i, (k, h)) in m.iter().enumerate() {
            let f = state_dir.join(format!("hist{i}.txt"));
            write(&f, &h.display())?;
            history.push_str(&format!("    {}) cat {}; exit 0 ;;\n", sq(k), sq(&ps(&f))));
        }
    }
    let script = format!(
        "#!/bin/bash\n\
         LOG={log}\n\
         {{ for a in \"$@\"; do printf '%s\\037' \"$a\"; done; printf '\\n'; }} >> \"$LOG\"\n\
         last=\"${{@: -1}}\"\n\
         if [ \"$1\" = create ]; then\n  case \"$last\" in\n{create}    *) printf 'Error response from daemon: No such image\\n' >&2; exit 1 ;;\n  esac\n\
         \x20 n=$(grep -c $'^create\\037' \"$LOG\")\n  printf 'c0ffee%058x\\n' \"$n\"; exit 0\nfi\n\
         if [ \"$1\" = cp ]; then printf 'Error response from daemon: Could not find the file in container\\n' >&2; exit 1; fi\n\
         if [ \"$1\" = rm ]; then printf '%s\\n' \"$last\"; exit 0; fi\n\
         if [ \"$1\" = image ] && [ \"$2\" = inspect ]; then\n  case \"$last\" in\n{inspect}    *) printf 'Error: No such image\\n' >&2; exit 1 ;;\n  esac\n\
         \x20 fmt=''; seen=0; prev=''; for a in \"$@\"; do if [ \"$prev\" = --format ] && [ \"$seen\" = 0 ]; then fmt=\"$a\"; seen=1; fi; prev=\"$a\"; done\n\
         \x20 if [ \"$seen\" = 1 ] || [ \"$prev\" = --format ]; then\n\
         \x20   case \"$fmt\" in '{{{{.Id}}}}') f=\"$d.id\" ;; '{{{{.Created}}}}') f=\"$d.created\" ;; *) f=/nonexistent ;; esac\n\
         \x20   if [ -s \"$f\" ]; then cat \"$f\"; exit 0; fi\n    exit 1\n  fi\n  cat \"$d.json\"; exit 0\nfi\n\
         if [ \"$1\" = history ]; then\n  case \"$last\" in\n{history}    *) exit 1 ;;\n  esac\nfi\n\
         if [ \"$1\" = run ]; then printf 'cat: /opt/easy-vllm/build_ledger.json: No such file or directory\\n' >&2; exit 1; fi\n\
         printf 'runtime_selftest docker shim: unexpected call\\n' >&2\nexit 1\n",
        log = sq(&ps(log)),
    );
    let exe = bindir.join("docker");
    write(&exe, &script)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).map_err(|e| crash(format!("OSError: {e}")))
}

fn docker_calls(log: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut v: Vec<String> = l.split('\u{1f}').map(str::to_string).collect();
            v.pop();
            v
        })
        .collect()
}

/// `_hint_docker_calls_non_starting(calls)`: the violating calls (empty = pass).
pub fn docker_calls_non_starting(calls: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut made: Vec<String> = Vec::new();
    let mut removed: std::collections::BTreeSet<String> = Default::default();
    let mut bad: Vec<Vec<String>> = Vec::new();
    for a in calls {
        let pair = |flag: &str, val: &str| a.windows(2).any(|w| w[0] == flag && w[1] == val);
        let first = a.first().map(String::as_str);
        if (first == Some("image") && a.get(1).map(String::as_str) == Some("inspect")) || first == Some("history") {
            continue;
        }
        if first == Some("create") {
            made.push(shim_cid(made.len() + 1));
            if !(pair("--pull", "never") && pair("--network", "none") && pair("--entrypoint", "/bin/true")) {
                bad.push(a.clone());
            }
            continue;
        }
        if first == Some("cp") {
            let srcs: Vec<&String> = a[1..].iter().filter(|x| !x.starts_with('-')).collect();
            let cid = match srcs.first() {
                Some(x) if x.contains(':') => x.split(':').next().unwrap_or("").to_string(),
                _ => String::new(),
            };
            if !made.contains(&cid) || removed.contains(&cid) {
                bad.push(a.clone());
            }
            continue;
        }
        if first == Some("rm") {
            let ids: Vec<String> = a[1..].iter().filter(|x| !x.starts_with('-')).cloned().collect();
            if ids.is_empty() || ids.iter().any(|x| !made.contains(x)) {
                bad.push(a.clone());
            }
            removed.extend(ids);
            continue;
        }
        bad.push(a.clone());
    }
    for cid in &made {
        if !removed.contains(cid) {
            bad.push(vec!["<left-behind>".into(), cid.clone()]);
        }
    }
    bad
}

// ───────────────────────────────────────────── CLI fixture
struct Fx {
    root: PathBuf,
    repo: PathBuf,
    fx: J,
    bare: PathBuf,
    legacy: J,
    bin: PathBuf,
    docker_log: PathBuf,
}

/// `_hint_cli_fixture(td, hint, legacy=...)`: the hint self-test's own fixture repository, plus the deployment-layout CLI
/// launcher, the tracked vocabulary + producer anchor, and the fake docker.
fn cli_fixture(root: &Path, td: &Path, legacy: bool) -> R<Fx> {
    hint_bin(root)?;
    let (repo, fx) = fixtures::fx_repo(td).map_err(crash)?;
    let dst = repo.join(HINT_SCRIPTS_REL);
    copy(&root.join(HINT_SCRIPTS_REL).join("hint.sh"), &dst.join("hint.sh"))?;
    for rel in [HINT_VOCAB_REL, SWEEP_META_PRODUCER_REL] {
        copy(&root.join(rel), &repo.join(rel))?;
    }
    let bare = PathBuf::from(s(&fx["bare"]));
    let old = if legacy { fixtures::fx_legacy_remote(td, &repo, &bare).map_err(crash)? } else { J::dict() };
    let bindir = td.join("bin");
    std::fs::create_dir(&bindir).map_err(|e| crash(format!("OSError: {e}")))?;
    let log = td.join("docker_calls.log");
    write_docker_shim(&bindir, &td.join("docker_state"), &log, &fx["docker"])?;
    Ok(Fx { root: root.to_path_buf(), repo, fx, bare, legacy: old, bin: bindir, docker_log: log })
}

/// `_hint_cli(f, *args)`: the fixture's deployed CLI launcher as a subprocess (global `--repo` before the subcommand).
/// The native hint executable the deployed launcher runs (absent = fail closed with the build hint, no fallback).
fn hint_bin(root: &Path) -> R<PathBuf> {
    let bin = root.join(HINT_BIN_REL);
    if !bin.is_file() {
        return Err(fail(format!("hint native executable is absent: {} (build hint-publisher first)", ps(&bin))));
    }
    Ok(bin)
}

fn cli(f: &Fx, args: &[&str]) -> R<Out> {
    let bin = hint_bin(&f.root)?;
    let mut argv = vec!["bash".to_string(), ps(&f.repo.join(HINT_SCRIPTS_REL).join("hint.sh")), "--repo".into(), ps(&f.repo)];
    argv.extend(args.iter().map(|a| a.to_string()));
    let path = format!("{}:{}", ps(&f.bin), std::env::var("PATH").unwrap_or_default());
    let env = vec![
        ("PATH".to_string(), path),
        ("HINT_BIN".to_string(), ps(&bin)),
        ("HINT_SKILL_DIR".to_string(), ps(&f.repo.join(HINT_SKILL_REL))),
    ];
    run(&argv, Some(&f.repo), &env, &[], None)
}

fn cli_ok(f: &Fx, what: &str, args: &[&str]) -> R<Out> {
    let o = cli(f, args)?;
    require(o.code == 0, || {
        format!("hint.py {what} failed: rc={} stdout={:?} stderr={:?}", o.code, tail(&o.stdout, 900), tail(&o.stderr, 1500))
    })?;
    Ok(o)
}

fn single_draft(repo: &Path) -> R<PathBuf> {
    let base = repo.join("hints/.drafts");
    let mut drafts: Vec<PathBuf> = std::fs::read_dir(&base)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.join("state.json").is_file()).collect())
        .unwrap_or_default();
    drafts.sort();
    require(drafts.len() == 1, || {
        format!("expected exactly one published draft, found {:?}", drafts.iter().map(|d| ps(d)).collect::<Vec<_>>())
    })?;
    Ok(drafts.remove(0))
}

fn has_drafts(repo: &Path) -> bool {
    std::fs::read_dir(repo.join("hints/.drafts")).map(|rd| rd.flatten().any(|e| e.path().join("state.json").is_file())).unwrap_or(false)
}

/// `_hint_edit_json(path, fn)`: rewrite a JSON file; returns the original bytes.
fn edit_json(p: &Path, f: impl FnOnce(&mut J)) -> R<Vec<u8>> {
    let before = read_b(p)?;
    let mut doc = value::loads(&String::from_utf8_lossy(&before)).map_err(|e| crash(format!("JSONDecodeError: {e}")))?;
    f(&mut doc);
    write(p, &format!("{}\n", value::dumps_pretty(&doc)))?;
    Ok(before)
}

fn author(f: &Fx, draft: &Path) -> R<()> {
    let st = load(&draft.join("state.json"))?;
    let _lib = hint_lib_env(&f.root);
    fixtures::fx_author(&f.repo, draft, &st).map_err(|e| fail(format!("programmatic authoring failed: {e}")))
}

fn lint_json(f: &Fx, draft: &Path) -> R<(Out, Option<J>)> {
    let o = cli(f, &["lint", "--draft", &ps(draft), "--json"])?;
    let parsed = value::loads(&o.stdout).ok();
    Ok((o, parsed))
}

/// `_hint_publish_and_author(f, *extra)` -> (draft, state.json).
fn publish_and_author(f: &Fx, extra: &[&str]) -> R<(PathBuf, J)> {
    let mut args = vec!["publish", "--campaign", FX_CAMPAIGN];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--generated-utc", FX_PUBLISH_UTC]);
    cli_ok(f, "publish", &args)?;
    let draft = single_draft(&f.repo)?;
    let st = load(&draft.join("state.json"))?;
    author(f, &draft)?;
    let (o, issues) = lint_json(f, &draft)?;
    let clean = matches!(&issues, Some(J::List(l)) if l.is_empty());
    require(o.code == 0 && clean, || {
        format!(
            "programmatic authoring left lint issues: rc={} {} {:?}",
            o.code,
            head(&issues.as_ref().map(|i| i.repr()).unwrap_or_else(|| "None".into()), 900),
            tail(&o.stderr, 400)
        )
    })?;
    Ok((draft, st))
}

// ───────────────────────────────────────────── _test_hint_binding_source
pub fn hint_binding_source(root: &Path) -> R<()> {
    let _lib = hint_lib_env(root);
    let bench_report_rel = "../benchmark/bench_report_selftest.md";
    let cert_rel = "../benchmark/benchmark_selftest.yaml";
    let rubric = |over: &[(&str, J)]| -> J {
        let mut b = jv(PROMO_RUBRIC);
        for (k, v) in over {
            b.set(*k, v.clone());
        }
        b
    };
    let shape = |benchmark: J, with_cert: bool, task_class: &str| -> J {
        let mut ev = J::dict();
        ev.set("bench_report", jv(&format!(r#"{{"path": "{bench_report_rel}"}}"#)));
        if with_cert {
            ev.set("certificate", jv(&format!(r#"{{"path": "{cert_rel}"}}"#)));
        }
        let mut m = J::dict();
        m.set("task_class", task_class);
        m.set("benchmark", benchmark);
        m.set("evidence", ev);
        m
    };
    let src = |m: &J| -> R<Option<String>> { evidence::no_cert_binding_source(m, Some(root)).map_err(crash) };
    let path = |m: &J| -> R<Option<String>> { evidence::binding_artifact_path(m, Some(root)).map_err(crash) };
    let st = |t: &str| J::str(t);
    let explore = shape(rubric(&[("verdict", st("REFUTE"))]), false, "full_benchmark");
    let weak_refute = shape(rubric(&[("rubric_authority", st("weak")), ("verdict", st("REFUTE"))]), false, "full_benchmark");
    let explicit_refute = shape(rubric(&[("rubric_authority", st("explicit")), ("verdict", st("REFUTE"))]), false, "full_benchmark");
    let observation = shape(
        jv(r#"{"verdict": "OBSERVATION-ONLY", "rubric_authority": "weak", "floor_tps": null, "ratio_M_over_primary": null, "primary_source": null, "rubric_source": "verdict_json"}"#),
        false,
        "full_benchmark",
    );
    let explicit_pass = shape(rubric(&[("rubric_authority", st("explicit")), ("verdict", st("PASS"))]), false, "full_benchmark");
    let bare = shape(jv(r#"{"verdict": "REFUTE"}"#), false, "full_benchmark");
    let cert = shape(rubric(&[("verdict", st("REFUTE"))]), true, "full_benchmark");
    let map_only = shape(jv(r#"{"mode": "lite", "verdict": null}"#), false, "hint_map_only");
    let map_only_cert = shape(jv(r#"{"mode": "lite", "verdict": null}"#), true, "hint_map_only");
    let is = |v: Option<String>, want: &str| v.as_deref() == Some(want);
    require(is(src(&explore)?, "explore"), || "explore rubric authority did not open the no-certificate binding source".into())?;
    require(is(src(&weak_refute)?, "weak") && is(src(&explicit_refute)?, "explicit"), || {
        "a weak/explicit REFUTE with a valid rubric did not open the bench_report binding (unified gate)".into()
    })?;
    require(is(src(&observation)?, "observation_only"), || "an OBSERVATION-ONLY full cell did not open the bench_report binding".into())?;
    require(src(&explicit_pass)?.is_none(), || {
        "explicit PASS opened a no-certificate binding (certificate = explicit ∧ PASS only)".into()
    })?;
    require(is(src(&map_only)?, "hint_map_only"), || "hint_map_only (lite-only cell) did not open the no-certificate binding source".into())?;
    require(src(&bare)?.is_none(), || "a manifest with no valid rubric / observation / map_only opened a binding source".into())?;
    for (field, value, shown) in [("floor_tps", J::from(0i64), "0"), ("rubric_source", J::Null, "None"), ("primary_source", st("N/A"), "'N/A'")] {
        let broken = shape(rubric(&[("rubric_authority", st("weak")), ("verdict", st("REFUTE")), (field, value)]), false, "full_benchmark");
        require(src(&broken)?.is_none(), || format!("rubric contract violation {field}={shown} still opened the binding source"))?;
    }
    require(path(&cert)?.as_deref() == Some(cert_rel), || "certificate lost priority over the manifest-declared rubric fallback".into())?;
    require(path(&map_only_cert)?.as_deref() == Some(cert_rel), || "a bound certificate lost priority over the hint_map_only fallback".into())?;
    for (label, man) in [("explore", &explore), ("weak-REFUTE", &weak_refute), ("OBSERVATION-ONLY", &observation), ("hint_map_only", &map_only)] {
        require(path(man)?.as_deref() == Some(bench_report_rel), || format!("{label} path did not bind the bench_report artifact"))?;
    }
    require(path(&bare)?.is_none(), || {
        "a manifest with no certificate/rubric/observation/map_only still produced a binding artifact".into()
    })
}

// ───────────────────────────────────────────── _test_hint_cli_publication
fn gate_denied(o: &Out, action: &str) -> bool {
    let Ok(g) = value::loads(&o.stdout) else { return false };
    o.code != 0
        && g.is_dict()
        && g["allowed"].as_bool() == Some(false)
        && g["action"].as_str() == Some(action)
        && g["exit_code"].as_i64() == Some(o.code as i64)
        && reason_codes(&g).iter().any(|c| c == "RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED")
}

fn ineligible(d: &mut J) {
    if let Some(r) = d.get_mut("runtime") {
        r.set("functional_smoke_passed", false);
    }
}

pub fn hint_cli_publication(root: &Path) -> R<()> {
    let tdg = Td::new("hint-cli-e2e.")?;
    let td = tdg.0.clone();
    let _env = isolated_env(&td)?;
    let _lib = hint_lib_env(root);
    let _owners = fixtures::owner_bin_env();
    let f = cli_fixture(root, &td, true)?;
    let (repo, bare) = (f.repo.clone(), f.bare.clone());
    let refs0 = refs(&repo)?;
    let tagname = fx_tag();

    // ── publish(스캐폴드 · 정지) ──
    cli_ok(&f, "publish", &["publish", "--campaign", FX_CAMPAIGN, "--cell", FX_CELL, "--generated-utc", FX_PUBLISH_UTC])?;
    let draft = single_draft(&repo)?;
    let st = load(&draft.join("state.json"))?;
    require(s(&st["tag"]) == FX_BASE_TAG && s(&st["stage"]) == "scaffolded", || {
        format!("publish did not scaffold under the v7 base name (q·len·kv, no tail): {} {}", st["tag"].repr(), st["stage"].repr())
    })?;
    require(refs(&repo)? == refs0, || "publish moved a ref (publish makes no tag or branch)".into())?;
    let calls = docker_calls(&f.docker_log);
    let ledger = artifacts::LEDGER_IN_IMAGE;
    let c1 = shim_cid(1);
    let v = |x: &[&str]| x.iter().map(|a| a.to_string()).collect::<Vec<String>>();
    let probe_ok = vec![
        v(&["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "fixture"]),
        v(&["cp", "-L", &format!("{c1}:{ledger}"), "/tmp/x"]),
        v(&["rm", &c1]),
    ];
    let vacuous: Vec<Vec<Vec<String>>> = vec![
        vec![v(&["image", "rm", "fixture:0.9.0-source"])],
        vec![v(&["run", "--rm", "--network", "none", "fixture", "sh"])],
        vec![v(&["run", "--rm", "--network", "none", "--pull", "never", "--entrypoint", "cat", "fixture", ledger])],
        vec![v(&["create", "--network", "none", "--entrypoint", "/bin/true", "fixture"]), v(&["rm", &c1])],
        vec![v(&["create", "--pull", "never", "--network", "none", "--entrypoint", "/bin/true", "fixture"]), v(&["start", &c1]), v(&["rm", &c1])],
        vec![v(&["cp", "-L", &format!("{}:{ledger}", shim_cid(9)), "/tmp/x"])],
        probe_ok[..2].to_vec(),
        {
            let mut x = probe_ok.clone();
            x.push(v(&["cp", "-L", &format!("{c1}:{ledger}"), "/tmp/x"]));
            x
        },
    ];
    require(docker_calls_non_starting(&probe_ok).is_empty() && vacuous.iter().all(|c| !docker_calls_non_starting(c).is_empty()), || {
        "the non-starting docker predicate admits a starting/mutating call, an unpulled-never create, a cp from a container it did not create, a container left behind, or rejects the plain probe sequence (vacuous check)".into()
    })?;
    let bad_calls = docker_calls_non_starting(&calls);
    require(!calls.is_empty() && bad_calls.is_empty(), || {
        format!("publish made a docker call outside the non-starting probe contract: {:?}", &bad_calls[..bad_calls.len().min(6)])
    })?;
    require(calls.iter().any(|c| c.first().map(String::as_str) == Some("create")) && calls.iter().any(|c| c.first().map(String::as_str) == Some("cp")), || {
        format!("flagless publish did not probe the measured image (create+cp absent): {:?}", &calls[..calls.len().min(8)])
    })?;

    // ── ★승인 부재 = 어느 ref 도 움직이지 않는다 ──
    let draft_s = ps(&draft);
    let decl_p = repo.join(format!("campaigns/{FX_CAMPAIGN}/campaign.yaml"));
    let decl0 = edit_json(&decl_p, |d| d.set("hint_targets", J::list()))?;
    let o = cli(&f, &["continue", "--draft", &draft_s, "--generated-utc", FX_CONTINUE_UTC])?;
    require(o.code != 0 && o.stderr.contains("HINT_APPROVAL_ABSENT") && refs(&repo)? == refs0, || {
        format!("continue without a recorded approval was not refused before any ref moved: rc={} {:?}", o.code, tail(&o.stderr, 600))
    })?;
    write_b(&decl_p, &decl0)?;

    // ── ★미저작 = 린트 차단 ──
    let o = cli(&f, &["continue", "--draft", &draft_s, "--generated-utc", FX_CONTINUE_UTC])?;
    require(o.code != 0 && o.stderr.contains("HINT_LINT_FAILED") && refs(&repo)? == refs0, || {
        format!("an unauthored draft was not refused before the payload commit: rc={} {:?}", o.code, tail(&o.stderr, 600))
    })?;

    // ── 저작 → 이름 꼬리 → lint 0 ──
    author(&f, &draft)?;
    let tail_p = draft.join("inputs/tail.json");
    let mut wrong = fx_tail();
    if let Some(first) = wrong.as_list_mut().and_then(|l| l.get_mut(0)) {
        if let Some(e) = first.get_mut("evidence") {
            e.set("value", "false");
        }
    }
    write(&tail_p, &value::dumps_unicode(&wrong))?;
    let o = cli(&f, &["lint", "--draft", &draft_s, "--json"])?;
    require(o.code != 0 && o.stdout.contains("HINT_TAIL_UNGROUNDED"), || {
        format!("a tail whose evidence disagrees with the serving config passed lint: {:?}", tail(&o.stdout, 600))
    })?;
    write(&tail_p, &value::dumps_unicode(&fx_tail()))?;
    let o = cli(&f, &["lint", "--draft", &draft_s, "--json"])?;
    require(o.code == 0 && o.stdout.trim() == "[]", || format!("programmatic authoring left lint issues: {:?}", tail(&o.stdout, 900)))?;

    // ── ★승격 게이트 거부 = 페이로드 커밋 전에 멈춘다 ──
    let man_p = repo.join(s(&st["manifest"]));
    let man0 = edit_json(&man_p, ineligible)?;
    let o = cli(&f, &["continue", "--draft", &draft_s, "--generated-utc", FX_CONTINUE_UTC, "--remote", "origin"])?;
    write_b(&man_p, &man0)?;
    require(gate_denied(&o, "hint_finalize") && refs(&repo)? == refs0, || {
        format!(
            "a gate denial at continue did not stop before the payload commit (or lost its gate JSON): rc={} stdout={:?} stderr={:?}",
            o.code,
            tail(&o.stdout, 500),
            tail(&o.stderr, 400)
        )
    })?;

    cli_ok(&f, "continue", &["continue", "--draft", &draft_s, "--generated-utc", FX_CONTINUE_UTC, "--remote", "origin"])?;

    let tag_ref = format!("refs/tags/{tagname}");
    let local_obj = hgit(&repo, &["rev-parse", &tag_ref])?;
    require(remote_ref(&bare, &tag_ref)?.as_deref() == Some(local_obj.as_str()), || {
        "remote tag object differs from the locally sealed tag object (AC9)".into()
    })?;
    let guide = s(&f.fx["guide"]);
    require(remote_ref(&bare, "refs/heads/hint")?.as_deref() == Some(guide.as_str()), || {
        "publish moved the remote hint branch -- the publication push is the exact tag refspec only (O2 · the branch = the guide commit, moved only by an approved branch-transition)".into()
    })?;
    require(s(&load(&draft.join("state.json"))?["tag"]) == tagname, || "continue did not confirm the final v7 name (base + authored tail)".into())?;
    let tip = hgit(&repo, &["rev-parse", &format!("{tag_ref}^{{commit}}")])?;
    let parents = hgit(&repo, &["rev-parse", &format!("{tip}^@")])?;
    require(
        parents.split_whitespace().collect::<Vec<_>>() == vec![guide.as_str()]
            && remote_ref(&bare, "refs/heads/hint")?.as_deref() == Some(guide.as_str())
            && !refs(&repo)?.contains("refs/heads/hint"),
        || "the payload commit's parent is not the remote guide commit, or the publication moved a hint branch".into(),
    )?;
    let body = tag_body(&repo, &tagname)?;
    require(
        body.contains(&format!("anchor: {tip}")) && body.contains("bench_ref: ../benchmark/benchmark_") && body.contains("bench_kind: certificate") && !body.contains("certificate_ref:"),
        || format!("tag footer v2 does not bind the payload commit + certificate: {:?}", tail(&body, 600)),
    )?;
    let ident = branch::synthetic_identity().map_err(crash)?;
    require(hgit(&repo, &["log", "-1", "--format=%an <%ae>|%cn <%ce>", &tip])? == format!("{ident}|{ident}"), || {
        "payload commit is not authored by the synthetic identity (PII · branch.commit_payload)".into()
    })?;
    let tree: Vec<String> = hgit(&repo, &["ls-tree", "--name-only", &tip])?.lines().map(str::to_string).collect();
    let need = ["README.md", "00-hint.md", "01-artifacts.md", "02-narrative.md", "03-benchmark.md", "PAYLOAD.json", "LINEAGE.json", "PROVENANCE.json"];
    require(need.iter().all(|n| tree.iter().any(|t| t == n)) && !tree.iter().any(|t| t == ".claude"), || {
        format!("payload tree is not the v6 allowlist: {tree:?}")
    })?;
    let idx = load(&repo.join("hints/index.json"))?;
    let mut rows: BTreeMap<String, J> = BTreeMap::new();
    for h in idx["hints"].items() {
        if h.is_dict() {
            rows.insert(h["tag"].display(), h.clone());
        }
    }
    let row = rows.get(&tagname).cloned().unwrap_or_else(J::dict);
    require(row["grammar"].as_str() == Some("v7") && row["verdict"].as_str() == Some("PASS"), || {
        format!("catalog did not derive a v7 row with the PASS verdict column for the new tag: {}", rows.get(&tagname).map(|r| r.repr()).unwrap_or_else(|| "None".into()))
    })?;
    let mut changed: Vec<String> = hgit(&repo, &["show", "--name-only", "--format=", "HEAD"])?.split_whitespace().map(str::to_string).collect();
    changed.sort();
    require(
        hgit(&repo, &["log", "-1", "--format=%s"])? == format!("chore(hint): 카탈로그 파생 — {tagname} 발행 반영")
            && changed == vec!["HINTS.md".to_string(), "hints/index.json".to_string()],
        || "continue did not auto-commit exactly the two catalog paths with the tool message".into(),
    )?;
    let legacy = &f.legacy;
    require(["old", "remote_only", "lightweight"].iter().all(|k| rows.contains_key(&s(&legacy[*k]))), || {
        format!("AC6: legacy/foreign tags did not coexist as catalog rows: {:?}", rows.keys().collect::<Vec<_>>())
    })?;
    let ro = rows.get(&s(&legacy["remote_only"])).cloned().unwrap_or_else(J::dict);
    require(ro["object"].as_str() == Some(catalog::OBJECT_ABSENT_LOCAL), || {
        format!("AC6: a remote-only tag must be recorded as absent-local (no synthesis): {}", ro.repr())
    })?;
    let phase = load(&repo.join(format!("campaigns/{FX_CAMPAIGN}/phases/main/publish.status.json")))?;
    require(phase["state"].as_str() == Some("done") && phase["proof"]["source"].as_str() == Some(format!("{tag_ref}@{local_obj}").as_str()), || {
        format!("campaign publish phase was not recorded with the remote SHA proof: {}", phase.repr())
    })?;
    let o = cli_ok(&f, "verify", &["verify", "--tag", &tagname])?;
    require(o.stdout.contains("PASS"), || format!("verify did not report PASS: {:?}", tail(&o.stdout, 400)))?;

    // ── ★단독 verify·push 도 게이트를 먼저 묻는다 ──
    let remote_obj0 = remote_ref(&bare, &tag_ref)?;
    let man0 = edit_json(&man_p, ineligible)?;
    let vp = cli(&f, &["verify", "--tag", &tagname])?;
    let pp = cli(&f, &["push", "--tag", &tagname, "--remote", "origin", "--apply"])?;
    write_b(&man_p, &man0)?;
    require(gate_denied(&vp, "hint_verify"), || {
        format!("standalone verify ran without asking the gate (hint_verify): rc={} stdout={:?}", vp.code, tail(&vp.stdout, 400))
    })?;
    require(gate_denied(&pp, "hint_push") && remote_ref(&bare, &tag_ref)? == remote_obj0 && !pp.stdout.contains("\"refspec\""), || {
        format!("standalone push ran without asking the gate (hint_push): rc={} stdout={:?}", pp.code, tail(&pp.stdout, 400))
    })?;

    // ── ★소스 트리 커밋에 봉인한 태그 — verify FAIL · push 거부 · 원격 불변 ──
    let forged = format!("{tagname}-forged");
    let headc = hgit(&repo, &["rev-parse", "HEAD"])?;
    let obj = tag::read_tag(&repo, &tagname).map_err(crash)?.ok_or_else(|| crash("KeyError: tag object absent"))?;
    let ann = tag::parse_annotation(&s(&obj["body"])).map_err(crash)?;
    let bound = tag::bench_binding(&ann["footer"]).map_err(crash)?;
    let mut fields = J::dict();
    fields.set("version", tag::FOOTER_VERSION);
    fields.set("tag", forged.as_str());
    fields.set("topology", st["topology_label"].clone());
    fields.set("anchor", headc.as_str());
    fields.set("manifest_ref", st["manifest"].clone());
    fields.set("bench_ref", bound["ref"].clone());
    fields.set("bench_kind", bound["kind"].clone());
    let brief = template::brief(&draft.join("payload")).map_err(crash)?;
    let message = tag::annotation(&brief, &fields).map_err(crash)?;
    let tagger = vec![
        ("GIT_COMMITTER_NAME".to_string(), core::SYNTHETIC_NAME.to_string()),
        ("GIT_COMMITTER_EMAIL".to_string(), core::synthetic_email()),
        ("GIT_COMMITTER_DATE".to_string(), core::git_date(FX_CONTINUE_UTC).map_err(crash)?),
    ];
    let argv: Vec<String> = ["git", "-C", &ps(&repo), "tag", "-a", "-F", "-", "--cleanup=verbatim", &forged, &headc].iter().map(|x| x.to_string()).collect();
    let o = run(&argv, None, &tagger, &[], Some(message.as_bytes()))?;
    require(o.code == 0, || format!("fixture could not create the source-anchored tag: {:?}", tail(&o.stderr, 300)))?;
    let o = cli(&f, &["verify", "--tag", &forged])?;
    require(o.code != 0 && o.stdout.contains("HINT_ANCHOR_PARENT_NOT_GUIDE"), || {
        format!("a tag sealed on a source-tree commit passed verify: rc={} {:?}", o.code, tail(&o.stdout, 600))
    })?;
    let o = cli(&f, &["push", "--tag", &forged, "--remote", "origin", "--apply"])?;
    require(
        o.code != 0
            && remote_ref(&bare, &format!("refs/tags/{forged}"))?.is_none()
            && o.stderr.contains("HINT_PUSH_UNVERIFIED")
            && o.stderr.contains("HINT_ANCHOR_PARENT_NOT_GUIDE"),
        || format!("a source-anchored tag was pushed (or refused for an unrelated reason): rc={} {:?}", o.code, tail(&o.stderr, 600)),
    )?;

    // ── 봉인된 draft 는 자기 manifest 에 대해 린트 0 ──
    let (o, parsed) = lint_json(&f, &draft)?;
    let codes = match &parsed {
        Some(J::List(l)) => l.iter().map(|i| i["code"].display()).collect::<Vec<_>>(),
        _ => return Err(fail(format!("hint.py lint --json produced no JSON: {:?}", tail(&o.stderr, 500)))),
    };
    require(codes.is_empty(), || "sealed draft no longer lints clean against its own manifest".into())?;

    // ── ★관측 없는 셀 = publish 가 첫 쓰기 전에 막는다 ──
    let ph = repo.join(format!("output/multi/benchlog/sweep_{FX_CELL}/level_01/post_health_{FX_CELL}.json"));
    let ph_bytes = read_b(&ph)?;
    std::fs::remove_file(&ph).map_err(|e| crash(format!("OSError: {e}")))?;
    let (tree0, refs1) = (fixtures::fx_tree(&repo), refs(&repo)?);
    let unobserved = td.join("unobserved");
    let o = cli(&f, &["publish", "--campaign", FX_CAMPAIGN, "--cell", FX_CELL, "--generated-utc", "2026-01-02T10:00:00Z", "--out", &ps(&unobserved)])?;
    require(
        o.code != 0 && o.stderr.contains("HINT_QUALIFICATION_UNOBSERVED") && fixtures::fx_tree(&repo) == tree0 && refs(&repo)? == refs1 && !unobserved.exists(),
        || format!("a cell without observed health+inference was not refused before any write: rc={} {:?}", o.code, tail(&o.stderr, 500)),
    )?;
    write_b(&ph, &ph_bytes)?;

    // ── ★측정 뒤 바뀐 트리플렛 = 이름을 짓지 않는다 ──
    let yaml_p = repo.join(format!("output/multi/configs/{FX_CELL}.yaml"));
    let y0 = read_b(&yaml_p)?;
    write(&yaml_p, &String::from_utf8_lossy(&y0).replace("max-model-len: 4096", "max-model-len: 8192"))?;
    let o = cli(&f, &["name", "--campaign", FX_CAMPAIGN, "--cell", FX_CELL])?;
    require(o.code != 0 && o.stderr.contains("HINT_TRIPLET_DRIFT"), || {
        format!("a serving triplet changed after measurement still yielded a name: rc={} {:?} {:?}", o.code, tail(&o.stdout, 200), tail(&o.stderr, 400))
    })?;
    write_b(&yaml_p, &y0)?;
    let o = cli_ok(&f, "name", &["name", "--campaign", FX_CAMPAIGN, "--cell", FX_CELL])?;
    require(o.stdout.split_whitespace().next() == Some(FX_BASE_TAG), || {
        format!("read-only `name` disagrees with the published base name: {:?}", head(&o.stdout, 200))
    })
}

// ───────────────────────────────────────────── _test_hint_cli_refute_paths
/// `_hint_fx_refute(repo, authority, floor)`.
fn fx_refute(repo: &Path, authority: &str, floor: f64) -> R<()> {
    let sweep = repo.join(format!("output/multi/benchlog/sweep_{FX_CELL}"));
    let mut rubric = J::dict();
    rubric.set("authority", authority);
    rubric.set("floor", if floor.fract() == 0.0 { J::from(floor as i64) } else { J::from(floor) });
    rubric.set("ratio_M_over_primary", 0.9);
    rubric.set("source", "expected_achievable(roofline×MBU)");
    let mut doc = J::dict();
    doc.set("verdict", "REFUTE");
    doc.set("rubric", rubric);
    write(&sweep.join("verdict.json"), &value::dumps_default(&doc))?;
    edit_json(&repo.join(format!("campaigns/{FX_CAMPAIGN}/evidence_pointers.json")), |d| {
        let kept: Vec<J> = d["pointers"].items().iter().filter(|p| p["kind"].as_str() != Some("certificate")).cloned().collect();
        d.set("pointers", J::List(kept));
    })?;
    Ok(())
}

pub fn hint_cli_refute_paths(root: &Path) -> R<()> {
    {
        let tdg = Td::new("hint-cli-refute-vacuous.")?;
        let td = tdg.0.clone();
        let _env = isolated_env(&td)?;
        let _lib = hint_lib_env(root);
        let _owners = fixtures::owner_bin_env();
        let f = cli_fixture(root, &td, false)?;
        let repo = f.repo.clone();
        fx_refute(&repo, "weak", 0.0)?;
        let refs0 = refs(&repo)?;
        let o = cli(&f, &["publish", "--campaign", FX_CAMPAIGN, "--cell", FX_CELL, "--generated-utc", FX_PUBLISH_UTC])?;
        let gate = value::loads(&o.stdout).unwrap_or_else(|_| J::dict());
        require(
            o.code != 0
                && gate["allowed"].as_bool() == Some(false)
                && reason_codes(&gate).iter().any(|c| c == "PROMOTION_GATE_NOT_ELIGIBLE")
                && gate["action"].as_str() == Some("hint_finalize"),
            || {
                format!(
                    "a vacuous-floor REFUTE run passed the publish gate pre-check: rc={} stdout={:?} stderr={:?}",
                    o.code,
                    tail(&o.stdout, 600),
                    tail(&o.stderr, 400)
                )
            },
        )?;
        require(refs(&repo)? == refs0 && !has_drafts(&repo), || "a gate denial at publish left a tag/branch or a scaffolded draft behind".into())?;
    }
    for authority in ["weak", "explore"] {
        refute_publishes(root, authority)?;
    }
    Ok(())
}

/// `_hint_refute_publishes(hint, authority)`.
fn refute_publishes(root: &Path, authority: &str) -> R<()> {
    let tdg = Td::new(&format!("hint-cli-refute-{authority}."))?;
    let td = tdg.0.clone();
    let _env = isolated_env(&td)?;
    let _lib = hint_lib_env(root);
    let _owners = fixtures::owner_bin_env();
    let f = cli_fixture(root, &td, false)?;
    let repo = f.repo.clone();
    fx_refute(&repo, authority, 8.5)?;
    let (draft, st) = publish_and_author(&f, &["--cell", FX_CELL])?;
    let man = load(&repo.join(s(&st["manifest"])))?;
    require(
        man["benchmark"]["verdict"].as_str() == Some("REFUTE")
            && man["benchmark"]["rubric_authority"].as_str() == Some(authority)
            && !man["evidence"]["certificate"]["path"].truthy(),
        || {
            format!(
                "fixture did not produce a {authority} REFUTE manifest without a certificate: {} {}",
                man["benchmark"].repr(),
                man["evidence"]["certificate"].repr()
            )
        },
    )?;
    let t = s(&st["tag"]);
    cli_ok(&f, &format!("continue({authority})"), &["continue", "--draft", &ps(&draft), "--generated-utc", FX_CONTINUE_UTC, "--remote", "origin"])?;
    let body = tag_body(&repo, &t)?;
    require(body.contains("bench_ref: ../benchmark/bench_report_") && body.contains("bench_kind: bench_report"), || {
        format!("{authority} footer v2 did not bind the bench_report (bench_kind=bench_report): {:?}", tail(&body, 500))
    })?;
    let payload = value::loads(&hgit(&repo, &["show", &format!("refs/tags/{t}^{{commit}}:PAYLOAD.json")])?).map_err(crash)?;
    require(payload["measurement"]["verdict"].as_str() == Some("REFUTE"), || {
        format!("a REFUTE cell did not carry its verdict on the machine surface: {}", payload["measurement"].repr())
    })?;
    let hint00 = hgit(&repo, &["show", &format!("refs/tags/{t}^{{commit}}:00-hint.md")])?;
    require(hint00.contains("> **REFUTE** —"), || format!("a {authority} REFUTE tag did not carry the REFUTE banner in 00-hint.md"))?;
    let local = hgit(&repo, &["rev-parse", &format!("refs/tags/{t}")])?;
    require(remote_ref(&f.bare, &format!("refs/tags/{t}"))?.as_deref() == Some(local.as_str()), || {
        format!("{authority}-authority REFUTE tag did not reach the remote with the sealed object")
    })
}

// ───────────────────────────────────────────── _test_hint_map_only_publication
fn bench_env_remove() -> &'static [&'static str] {
    &["EASY_VLLM_VERSION"]
}

/// `_render_lite_report(raw_json)`: the deployed renderer's lite-only report.
fn render_lite_report(root: &Path, raw: &Path) -> R<String> {
    let argv: Vec<String> = vec!["bash".into(), ps(&root.join(RENDER_REPORT_REL)), "--lite-only".into(), "--lite-raw-json".into(), ps(raw), "--stdout".into()];
    let o = run(&argv, None, &[], bench_env_remove(), None)?;
    require(o.code == 0 && !o.stdout.trim().is_empty(), || {
        format!("render_report --lite-only failed on a synthetic lite raw: rc={} stderr={:?}", o.code, tail(&o.stderr, 600))
    })?;
    Ok(o.stdout)
}

/// `_hint_fx_lite_cell(repo, fx, with_report_pointer=...)`.
fn fx_lite_cell(root: &Path, repo: &Path, fx: &J, with_report_pointer: bool) -> R<()> {
    let cell = FX_LITE_CELL;
    let out = repo.join("output/multi");
    write(
        &out.join(format!("configs/{cell}.yaml")),
        "# lite 셀\nmodel: /app/quant_models/Org/Fixture-Model-NVFP4\ntensor-parallel-size: 2\nmax-model-len: 2048\nkv-cache-dtype: auto\nenforce-eager: true\n",
    )?;
    write(&out.join(format!("configs/{cell}.sh")), "#!/bin/bash\nvllm serve --config \"/app/configs/${CONFIG_FILE}.yaml\"\n")?;
    write(
        &out.join(format!("envs/.env.{cell}")),
        &format!(
            "CONFIG_FILE={cell}\nIMAGE_TAG=fixture:0.9.0-source\nBUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_REPO=https://github.com/vllm-project/vllm.git\nVLLM_REF=v0.9.0\nVLLM_PLE_MMAP=1\n"
        ),
    )?;
    let bl = out.join("benchlog");
    let (warm, cold, elog) = (bl.join(format!("lite_warm_{cell}.json")), bl.join(format!("lite_cold_{cell}.json")), bl.join(format!("lite_engine_{cell}.log")));
    write(&warm, r#"{"completed": 3, "failed": 0, "median_tpot_ms": 81.3, "output_throughput": 12.3}"#)?;
    write(&cold, r#"{"completed": 1, "failed": 0, "median_ttft_ms": 140.0}"#)?;
    write(&elog, "INFO Starting vLLM API server v0.9.0\nINFO GPU KV cache size: 54,192 tokens\nINFO Available KV cache memory: 8.5 GiB\n")?;
    let raw = bl.join(format!("lite_raw_{cell}.json"));
    let mut node = J::dict();
    node.set("role", "main");
    node.set("gpu_smi_used_mib", J::Null);
    node.set("gpu_smi_total_mib", J::Null);
    node.set("ram_total_kib", 128000000i64);
    node.set("ram_avail_kib", 64000000i64);
    let mut rd = J::dict();
    for (k, v) in [
        ("topology", J::from("multi")),
        ("burst_n", J::from(3i64)),
        ("config_name", J::from(cell)),
        ("measured_utc", J::from(FX_LITE_MEASURED_UTC)),
        ("backend", J::from("openai")),
        ("endpoint", J::from("/v1/completions")),
        ("config_yaml", J::from(ps(&out.join(format!("configs/{cell}.yaml"))))),
        ("env_file", J::from(ps(&out.join(format!("envs/.env.{cell}"))))),
        ("manifest", J::from(ps(&out.join("manifest.yaml")))),
        ("bench_warm_json", J::from(ps(&warm))),
        ("bench_cold_json", J::from(ps(&cold))),
        ("engine_log", J::from(ps(&elog))),
        ("lite_verdict", J::from("pass")),
        ("lite_exit", J::from(0i64)),
        ("lite_verdict_source", J::from("selftest fixture(lite_bench 판정 결과 모양)")),
        ("nodes", J::List(vec![node])),
    ] {
        rd.set(k, v);
    }
    write(&raw, &value::dumps_default(&rd))?;
    let text = render_lite_report(root, &raw)?;
    require(
        text.contains("mode: lite") && text.contains("| bench_mode | lite |") && text.contains(&format!("생성일 {FX_LITE_MEASURED_UTC}")),
        || format!("real lite report lost its mode header / measurement-config row / join key: {:?}", head(&text, 700)),
    )?;
    write(&repo.join(FX_LITE_REPORT), &text)?;
    let argv: Vec<String> = vec!["bash".into(), ps(&root.join(PUBLISH_RECORD_REL)), "--lite-raw-json".into(), ps(&raw), "--stdout".into()];
    let o = run(&argv, None, &[], bench_env_remove(), None)?;
    require(o.code == 0 && o.stdout.contains("benchmark_mode: lite"), || {
        format!("publish_benchmark_record --lite-raw-json failed on the lite fixture: rc={} stderr={:?}", o.code, tail(&o.stderr, 600))
    })?;
    write(&repo.join(FX_LITE_CERT), &o.stdout)?;

    let camp = repo.join(format!("campaigns/{FX_CAMPAIGN}"));
    edit_json(&camp.join("campaign.yaml"), |d| {
        let mut a = J::dict();
        a.set("cell", cell);
        a.set("mode", "AUTO");
        if let Some(main) = d.get_mut("assignments").and_then(|x| x.get_mut("main")) {
            main.push(a);
        }
        if let Some(cells) = d.get_mut("hint_targets").and_then(|x| x.as_list_mut()).and_then(|l| l.get_mut(0)).and_then(|t| t.get_mut("cells")) {
            cells.push(J::from(cell));
        }
    })?;
    let cdir = camp.join(format!("cells/{cell}"));
    write(&cdir.join("cell.status.json"), &format!(r#"{{"cell_id": "{cell}", "cell_outcome": "measured"}}"#))?;
    write(&cdir.join("lockset.json"), &format!(r#"{{"id": "{cell}", "provenance": "hand-authored"}}"#))?;
    copy(&camp.join(format!("cells/{FX_CELL}/config.yaml")), &cdir.join("config.yaml"))?;
    let ptr = |kind: &str, path: &str, node: &str| -> J {
        let mut p = J::dict();
        p.set("kind", kind);
        p.set("path", path);
        p.set("cell_id", cell);
        p.set("node_id", node);
        p
    };
    let mut ptrs = vec![ptr("devlog", &s(&fx["devlog"]), "main"), ptr("testlog", &s(&fx["testlog"]), "main")];
    if with_report_pointer {
        ptrs.push(ptr("bench_report", FX_LITE_REPORT, "cluster"));
        ptrs.push(ptr("certificate", FX_LITE_CERT, "cluster"));
    }
    edit_json(&camp.join("evidence_pointers.json"), |d| {
        if let Some(l) = d.get_mut("pointers").and_then(|x| x.as_list_mut()) {
            l.extend(ptrs);
        }
    })?;
    Ok(())
}

fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn hint_map_only_publication(root: &Path) -> R<()> {
    let tdg = Td::new("hint-cli-lite.")?;
    let td = tdg.0.clone();
    let _env = isolated_env(&td)?;
    let _lib = hint_lib_env(root);
    let _owners = fixtures::owner_bin_env();
    let f = cli_fixture(root, &td, false)?;
    let repo = f.repo.clone();
    fx_lite_cell(root, &repo, &f.fx, false)?;
    let (tree0, refs0) = (fixtures::fx_tree(&repo), refs(&repo)?);
    let o = cli(&f, &["publish", "--campaign", FX_CAMPAIGN, "--cell", FX_LITE_CELL, "--node", "cluster", "--generated-utc", FX_PUBLISH_UTC])?;
    require(
        o.code != 0 && o.stderr.contains("HINT_QUALIFICATION_UNOBSERVED") && fixtures::fx_tree(&repo) == tree0 && refs(&repo)? == refs0,
        || format!("a lite cell with no bound lite report was not refused before any write: rc={} {:?}", o.code, tail(&o.stderr, 600)),
    )?;
    edit_json(&repo.join(format!("campaigns/{FX_CAMPAIGN}/evidence_pointers.json")), |d| {
        let mk = |kind: &str, path: &str| {
            let mut p = J::dict();
            p.set("kind", kind);
            p.set("path", path);
            p.set("cell_id", FX_LITE_CELL);
            p.set("node_id", "cluster");
            p
        };
        if let Some(l) = d.get_mut("pointers").and_then(|x| x.as_list_mut()) {
            l.push(mk("bench_report", FX_LITE_REPORT));
            l.push(mk("certificate", FX_LITE_CERT));
        }
    })?;

    cli_ok(&f, "publish(lite)", &["publish", "--campaign", FX_CAMPAIGN, "--cell", FX_LITE_CELL, "--generated-utc", FX_PUBLISH_UTC])?;
    let draft = single_draft(&repo)?;
    let st = load(&draft.join("state.json"))?;
    require(s(&st["tag"]) == FX_LITE_TAG, || format!("lite cell did not derive the expected v7 base name: {}", st["tag"].repr()))?;
    let man = load(&repo.join(s(&st["manifest"])))?;
    require(
        man["task_class"].as_str() == Some("hint_map_only")
            && man["benchmark"]["mode"].as_str() == Some("lite")
            && man["evidence"]["bench_report"]["path"].display().ends_with(basename(FX_LITE_REPORT))
            && man["evidence"]["certificate"]["path"].display().ends_with(basename(FX_LITE_CERT)),
        || {
            format!(
                "lite publication was not driven as hint_map_only + bound lite report + lite certificate: {} {} {}",
                man["task_class"].repr(),
                man["benchmark"].repr(),
                man["evidence"].repr()
            )
        },
    )?;

    let hint00 = draft.join("payload/00-hint.md");
    let text00 = read(&hint00)?;
    let marker = template::MAP_ONLY_MARKER;
    let draft_s = ps(&draft);
    let lint_codes = || -> R<Vec<String>> {
        let o = cli(&f, &["lint", "--draft", &draft_s, "--json"])?;
        let v = value::loads(&o.stdout).map_err(|e| crash(format!("JSONDecodeError: {e}")))?;
        Ok(v.items().iter().map(|i| i["code"].display()).collect())
    };
    require(
        marker == "OBSERVATION-ONLY" && text00.contains(marker) && !lint_codes()?.iter().any(|c| c == "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING"),
        || "a hint_map_only scaffold does not carry the machine-rendered OBSERVATION-ONLY marker".into(),
    )?;
    write(&hint00, &text00.replace(marker, "OBSERVATION"))?;
    require(lint_codes()?.iter().any(|c| c == "HINT_MAP_ONLY_OBSERVATION_MARKER_MISSING"), || {
        "a hint_map_only body without the OBSERVATION-ONLY marker passed lint".into()
    })?;
    write(&hint00, &text00)?;

    author(&f, &draft)?;
    let o = cli(&f, &["lint", "--draft", &draft_s, "--json"])?;
    require(o.code == 0 && o.stdout.trim() == "[]", || {
        format!("programmatic authoring left lint issues on the lite draft: {:?}", tail(&o.stdout, 900))
    })?;
    cli_ok(&f, "continue(lite)", &["continue", "--draft", &draft_s, "--generated-utc", FX_CONTINUE_UTC, "--remote", "origin"])?;
    let tag_ref = format!("refs/tags/{FX_LITE_TAG}");
    let local = hgit(&repo, &["rev-parse", &tag_ref])?;
    require(remote_ref(&f.bare, &tag_ref)?.as_deref() == Some(local.as_str()), || "lite tag did not reach the remote with the sealed object".into())?;
    let body = tag_body(&repo, FX_LITE_TAG)?;
    require(body.contains(&format!("bench_ref: ../benchmark/{}", basename(FX_LITE_CERT))) && body.contains("bench_kind: certificate"), || {
        format!("map_only footer v2 did not bind the lite-grade certificate: {:?}", tail(&body, 500))
    })?;
    let payload = value::loads(&hgit(&repo, &["show", &format!("{tag_ref}^{{commit}}:PAYLOAD.json")])?).map_err(crash)?;
    require(payload["measurement"]["verdict"].as_str() == Some("OBSERVATION-ONLY"), || {
        format!("a lite-only cell did not carry OBSERVATION-ONLY on the machine surface: {}", payload["measurement"].repr())
    })?;
    let mc = &payload["measurement_config"];
    let missing = strs(&payload["missing"]);
    require(
        missing.iter().any(|m| m == "BENCH_MODE_LITE")
            && !missing.iter().any(|m| m == "HINT_MISSING_LITE")
            && mc["bench_mode"].as_str() == Some("lite")
            && mc["bench_mode_kind"].as_str() == Some("declared-lite")
            && mc["repeats"].as_i64() == Some(1)
            && mc["bench_tool"].as_str() == Some("vllm-bench-serve")
            && mc["downgrade_reason"].is_null(),
        || format!("PAYLOAD must carry BENCH_MODE_LITE and the parsed measurement config: {} {}", payload["missing"].repr(), mc.repr()),
    )?;
    let idx = load(&repo.join("hints/index.json"))?;
    let entries: Vec<&J> = idx["hints"].items().iter().filter(|e| e["tag"].as_str() == Some(FX_LITE_TAG)).collect();
    require(
        entries.len() == 1 && entries[0]["bench_mode"].as_str() == Some("lite(선언)") && strs(&entries[0]["missing"]).iter().any(|m| m == "BENCH_MODE_LITE"),
        || format!("catalog did not derive the bench_mode column from the remote tag payload: {:?}", entries.iter().map(|e| e.repr()).collect::<Vec<_>>()),
    )?;
    let hints_md = read(&repo.join("HINTS.md"))?;
    require(hints_md.contains("| lite(선언) |"), || format!("HINTS.md row lacks the derived bench_mode column: {:?}", tail(&hints_md, 600)))
}
