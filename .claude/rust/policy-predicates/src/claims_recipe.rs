//! Clauses ported against the vllm-recipe-explorer owner (`recipe_runtime`, driven in-process -- the analogue of the
//! original's `import` + monkeypatch of `preload_ram_gate` / `cleanup_docker` / `crosscheck_model_card` / `recipe` /
//! `gen_recipe_set`). Every operating-system seam the original monkeypatched is a trait or a value of the owner library:
//! the gate's `Host`/`Provider` (MemAvailable, the drop-caches helper) and the `Fetch` callable of the HF lookup. The
//! cleanup's process steps are real `subprocess` runs in the owner, so the original's `_run`/`list_images` stand-ins are a
//! stand-in `docker` executable first on `PATH` (the owner fixture's own seam), never the real daemon.
//!
//! Where the original read a `.py` SOURCE as evidence (`run_trial.py` `_start_memwatch`, the `cleanup_docker.py`
//! argparse/imports/source order, `inspect.getsource` of the `recipe.py` functions) the same property is read from the
//! Rust owner source that now implements it.
use crate::support::*;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::cleanup_docker::{self as cd, CleanupConsole, Done, Thrown, World};
use recipe_runtime::cliparse::Kind;
use recipe_runtime::escalation::Raised;
use recipe_runtime::memory::{self as mem, DropSystem, GateArguments, HelperRequest, HelperRun, Host, LogChoice, Provider};
use recipe_runtime::memory_cli::{self, CliHost};
use recipe_runtime::model_cli::Streams;
use recipe_runtime::pyexpr::Res;
use recipe_runtime::transform_cli::Console;
use std::path::Path;

const RECIPE_RT: &str = ".claude/skills/vllm-recipe-explorer/rust/runtime";

fn rt_src(rel: &str) -> R<String> {
    read(&format!("{RECIPE_RT}/{rel}"))
}

fn owner(f: agent_runtime_support::failure::Failure) -> Fail {
    Fail::new(f.class, f.message.to_utf8().unwrap_or_default())
}

fn thrown(t: Box<Thrown>) -> Fail {
    Fail::new("OwnerException", format!("{}: {}", t.raised.class, t.raised.message.to_utf8().unwrap_or_default()))
}

fn ps(s: &str) -> PyStr {
    PyStr::new(s)
}

fn txt(v: &PyStr) -> String {
    v.to_utf8().unwrap_or_default()
}

fn field<'a>(d: &'a PyDict, key: &str) -> &'a PyValue {
    static NONE: PyValue = PyValue::None;
    d.get(key).map(PyValue::peel).unwrap_or(&NONE)
}

fn is_int(v: &PyValue, n: i64) -> bool {
    !matches!(v.peel(), PyValue::Bool(_)) && v.peel().as_plain_int().map(|b| b.to_string()) == Some(n.to_string())
}

fn is_bool(v: &PyValue, b: bool) -> bool {
    matches!(v.peel(), PyValue::Bool(x) if *x == b)
}

fn is_none(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::None)
}

fn root_ps() -> PyStr {
    PyStr::new(&repo_root().to_string_lossy())
}

/// The text of one top-level Rust `fn <name>(` (with or without `pub`) up to its closing column-0 brace.
fn rust_fn_body(src: &str, name: &str) -> R<String> {
    let head = format!("fn {name}(");
    let start = src
        .match_indices(&head)
        .map(|(i, _)| i)
        .find(|&i| i == 0 || src[..i].ends_with('\n') || src[..i].ends_with("pub "))
        .ok_or_else(|| Fail::predicate(format!("Rust function {} not found in the owner source", repr(name))))?;
    let rest = &src[start..];
    let end = rest.find("\n}\n").map(|e| e + 3).unwrap_or(rest.len());
    Ok(rest[..end].to_string())
}

// ---------------------------------------------------------------- the pre-load RAM gate host

/// `os.path.exists(DROP_HELPER)` for a (possibly redirected) helper path; the run is never reached for a missing one.
struct HelperAt(String);

impl DropSystem for HelperAt {
    fn helper_exists(&mut self) -> bool {
        Path::new(&self.0).exists()
    }
    fn run_helper(&mut self, _request: &HelperRequest) -> HelperRun {
        HelperRun::Failed
    }
}

/// The gate's machine with `mem_available_mib` replaced (the original's `lambda: N`), the drop helper redirected, and
/// every write captured.
struct GateHost {
    mem: Option<i64>,
    helper: String,
    stderr: String,
    stdout: String,
}

impl GateHost {
    fn new(mem: Option<i64>, helper: &str) -> Self {
        GateHost { mem, helper: helper.to_string(), stderr: String::new(), stdout: String::new() }
    }
}

impl Provider for GateHost {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        match self.mem {
            Some(m) => Ok(PyValue::int(m)),
            // the unpatched original: the real /proc/meminfo
            None => mem::mem_available_mib(&mut recipe_runtime::memory_system::FileMeminfo::new(PyStr::new(mem::MEMINFO_PATH))),
        }
    }
    fn try_drop_caches(&mut self) -> Res<bool> {
        Ok(mem::try_drop_caches(&mut HelperAt(self.helper.clone())))
    }
}

impl Host for GateHost {
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.stderr.push_str(&txt(text));
        Ok(())
    }
    fn call_log(&mut self, _message: &PyStr) -> Res<()> {
        Err(agent_runtime_support::failure::Failure::new("RuntimeError", "no callable log"))
    }
}

impl CliHost for GateHost {
    fn write_stdout(&mut self, text: &PyStr) {
        self.stdout.push_str(&txt(text));
    }
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.stderr.push_str(&txt(usage));
        self.stderr.push_str(&txt(message));
    }
    fn parser_help(&mut self, help: &PyStr) {
        self.stdout.push_str(&txt(help));
    }
}

fn gate(host: &mut GateHost, checkpoint: PyValue, tp: i64, floor_mib: i64, auto_drop: bool) -> R<PyDict> {
    let tp = PyValue::int(tp);
    let floor = PyValue::int(floor_mib);
    let drop = PyValue::Bool(auto_drop);
    mem::gate(host, &GateArguments { checkpoint_bytes: &checkpoint, tp: &tp, floor_mib: &floor, auto_drop: &drop, log: LogChoice::Default })
        .map_err(owner)
}

fn mib(n: i64) -> PyValue {
    PyValue::int(n * i64::from(mem::MIB))
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C1() -> R {
    let mw = read(".claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh")?;
    req(mw.contains("FILTER=\"${1:-@vllm}\""), "mem_watchdog.sh must default to the broad @vllm filter")?;
    req(mw.contains("THRESH_MIB=\"${2:-10240}\""), "default MemAvailable threshold must be 10240 MiB")?;
    req(mw.contains("# ── BB_TARGET_PREDICATE_V1"), "mem_watchdog.sh must carry the shared BB_TARGET_PREDICATE_V1 kill-target block")?;
    let mut block: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in splitlines(&mw) {
        if strip(line).starts_with("# ── BB_TARGET_PREDICATE_V1") {
            in_block = true;
        }
        if in_block {
            block.push(line);
        }
        if in_block && strip(line).starts_with("# ── /BB_TARGET_PREDICATE_V1") {
            break;
        }
    }
    req(block.last().map_or(false, |l| strip(l).starts_with("# ── /BB_TARGET_PREDICATE_V1")), "BB_TARGET_PREDICATE_V1 block is not terminated in mem_watchdog.sh")?;
    let cases: [(&str, &str, &str, i32); 8] = [
        ("serve_image_local_build", "easy-vllm:0.19.1-cu130-aarch64-wheel", "vllm-serve-container", 0),
        ("serve_image_name_without_vllm", "easy-vllm:0.27.1-cu133-aarch64-source", "mn-hy3-master", 0),
        ("serve_image_upstream", "vllm/vllm-openai:latest", "openai-server", 0),
        ("serve_name_only", "ubuntu:24.04", "vllm_trial01", 0),
        ("case_insensitive", "EASY-VLLM:0.19.1", "SERVE", 0),
        ("measurement_tool_tag", "ghcr.io/vllm-project/guidellm:latest", "guidellm-bench", 1),
        ("measurement_tool_digest", "ghcr.io/vllm-project/guidellm@sha256:00ff", "guidellm-bench", 1),
        ("unrelated_base_image", "nvcr.io/nvidia/pytorch:25.08-py3", "build-helper", 1),
    ];
    let probe = format!(
        "{}\n{}",
        block.join("\n"),
        cases.iter().map(|(l, i, n, _)| format!("bb_target_match id {} {}; echo \"{l}=$?\"", shlex_quote(i), shlex_quote(n))).collect::<Vec<_>>().join("\n")
    );
    let proc_ = run_bash(&probe)?;
    req(proc_.code == 0, format!("kill-target predicate probe failed to run: {}", tail_chars(&proc_.stderr, 400)))?;
    let got: std::collections::HashMap<&str, &str> = proc_.stdout.split_whitespace().filter_map(|t| t.split_once('=')).collect();
    for (label, img, name, want) in cases {
        let g = got.get(label).copied();
        req(g == Some(want.to_string().as_str()), format!(
            "broad-mode kill-target predicate: {label} ({}, {}) expected rc={want}, got rc={}",
            repr(img), repr(name), g.map(repr).unwrap_or_else(|| "None".into())
        ))?;
    }

    let mn = read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")?;
    req(mn.contains("MAIN_WATCHDOG=\"$REPO/.claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh\"")
        && mn.contains("bash \"$MAIN_WATCHDOG\"") && mn.contains("WD_MAIN_PID=$!"),
        "multinode_serve_smoke.sh must resolve and auto-start the owner-local watchdog")?;

    // run_trial's harness-scoped watchdog: the Rust owner of `_start_memwatch` (run_trial_session.rs `start_memwatch`).
    let rt = rt_src("src/run_trial_session.rs")?;
    let start_fn = rust_fn_body(&rt, "start_memwatch")?;
    req(count(&start_fn, "Command::new(") == 1, "_start_memwatch must launch exactly one scoped watchdog subprocess")?;
    let launch = &start_fn[index(&start_fn, "Command::new(")?..];
    let launch = &launch[..launch.find(".spawn()").unwrap_or(launch.len())];
    req(launch.contains(".arg(&script)") && launch.contains(".arg(container)") && start_fn.contains("\"mem_watchdog.sh\""),
        "watchdog Popen must bind the resolved script and this trial's container identity")?;
    let session = rt_src("src/run_trial_session.rs")?;
    req(session.contains("start_memwatch(&root, &container,"), "watchdog Popen must bind the resolved script and this trial's container identity")?;

    let compose = rendered("compose")?;
    req(compose.contains("oom_score_adj: 800"), "container oom_score_adj must be 800 (kernel OOM-killer targeting)")?;

    let ihs = read(".claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh")?;
    req(ihs.contains("earlyoom") && ihs.contains("-m 4"), "earlyoom backstop must be installed at the -m 4 threshold")?;

    let mut real = GateHost::new(None, mem::DROP_HELPER);
    let res = gate(&mut real, PyValue::None, 1, mem::DEFAULT_FLOOR_MIB, true)?;
    req(is_bool(field(&res, "ok"), true) && is_bool(field(&res, "skipped"), true), "predicate requirement failed at original line 201")?;

    let mut h = GateHost::new(Some(5000), mem::DROP_HELPER);
    let r_tp1 = gate(&mut h, mib(8192), 1, 1024, false)?;
    let r_tp4 = gate(&mut h, mib(8192), 4, 1024, false)?;
    req(is_bool(field(&r_tp1, "ok"), false) && is_int(field(&r_tp1, "required_mib"), 9216), "tp=1: ceil(8192/1)+1024=9216 > 5000 MemAvailable must refuse")?;
    req(is_bool(field(&r_tp4, "ok"), true) && is_int(field(&r_tp4, "required_mib"), 3072), "tp=4: ceil(8192/4)+1024=3072 <= 5000 MemAvailable must pass -- proves TP division is real")?;
    let r_floor0 = gate(&mut h, mib(4096), 1, 0, false)?;
    let r_floor_big = gate(&mut h, mib(4096), 1, 1500, false)?;
    req(is_bool(field(&r_floor0, "ok"), true) && is_int(field(&r_floor0, "required_mib"), 4096), "predicate requirement failed at original line 221")?;
    req(is_bool(field(&r_floor_big, "ok"), false) && is_int(field(&r_floor_big, "required_mib"), 5596), "same checkpoint/tp, only floor_mib raised, must flip pass->refuse -- proves the floor is additive")
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C4() -> R {
    let missing = "/nonexistent/vllm-drop-caches";
    req(!mem::try_drop_caches(&mut HelperAt(missing.to_string())), "predicate requirement failed at original line 327")?;
    host_safety_c4_scenario(missing, 100)
}

/// The rest of C4 for one machine (drop helper path, persistent MemAvailable): the predicate runs it on a missing helper
/// and a scarce 100 MiB (the original's monkeypatch); the checks run it on other machines to prove it can fail.
pub fn host_safety_c4_scenario(missing: &str, mem_mib: i64) -> R {
    let mut h = GateHost::new(Some(mem_mib), missing);
    let gib500 = PyValue::int(500i64 * (1i64 << 30));
    let res = gate(&mut h, gib500, 1, 10240, true)?;
    req(is_bool(field(&res, "ok"), false), "refusal must be preserved even when the drop helper is absent")?;
    req(is_bool(field(&res, "dropped"), false) && is_bool(field(&res, "skipped"), false), "predicate requirement failed at original line 332")?;

    let mut cli = GateHost::new(Some(mem_mib), missing);
    let bytes = (500i64 * (1i64 << 30)).to_string();
    let args: Vec<PyStr> = ["--checkpoint-bytes", bytes.as_str(), "--tp", "1"].iter().map(|a| ps(a)).collect();
    let skill_root = ps(&repo_root().join(".claude/skills/vllm-recipe-explorer").to_string_lossy());
    let code = memory_cli::execute(&mut cli, &skill_root, &args, Some("80")).map_err(owner)?;
    req(code == 7, format!("CLI must exit 7 on insufficient memory, got {code}"))?;
    let report = &cli.stderr;
    req(report.contains("drop-caches 헬퍼 미가용"), "the CLI must truthfully report the missing drop-caches helper, never silently proceed")?;
    req(report.contains("REFUSE") && !report.contains("PASS"), "the CLI must truthfully report REFUSE, never a fabricated PASS")
}

// ---------------------------------------------------------------- cleanup_docker

/// The cleanup's console: every write captured, parser sinks captured, no stdin.
#[derive(Default)]
struct Capture {
    out: String,
    err: String,
}

impl Console for Capture {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.err.push_str(&txt(text));
        Ok(())
    }
    fn write_stdout(&mut self, text: &PyStr) {
        self.out.push_str(&txt(text));
    }
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        self.err.push_str(&txt(usage));
        self.err.push_str(&txt(message));
    }
    fn parser_help(&mut self, help: &PyStr) {
        self.out.push_str(&txt(help));
    }
    fn write_uncaught(&mut self, text: &PyStr) {
        self.err.push_str(&txt(text));
    }
    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        Ok(PyStr::default())
    }
}

impl CleanupConsole for Capture {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.out.push_str(&txt(text));
        Ok(())
    }
    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.err.push_str(&txt(text));
        Ok(())
    }
}

/// `PATH` with a stand-in `docker` first, restored on drop (the predicates run one at a time).
struct FakePath(Option<std::ffi::OsString>);

impl FakePath {
    fn install(dir: &Path) -> Self {
        let old = std::env::var_os("PATH");
        let mut p = std::ffi::OsString::from(dir.as_os_str());
        if let Some(o) = &old {
            p.push(":");
            p.push(o);
        }
        std::env::set_var("PATH", p);
        FakePath(old)
    }
}

impl Drop for FakePath {
    fn drop(&mut self) {
        match &self.0 {
            Some(o) => std::env::set_var("PATH", o),
            None => std::env::remove_var("PATH"),
        }
    }
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C8() -> R {
    // `--apply` is a store_true flag (never defaulting to True) -- the owner's parser spec.
    let spec = cd::spec("cleanup_docker");
    let apply: Vec<_> = spec.options.iter().filter(|o| o.flags.contains(&"--apply")).collect();
    req(apply.len() == 1, "--apply must never default to True")?;
    req(apply[0].kind == Kind::Flag, "--apply must never default to True")?;
    let src = rt_src("src/cleanup_docker.rs")?;
    let main_src = rust_fn_body(&src, "main")?;
    let dry_run_idx = index(&main_src, "if !apply {")?;
    let apply_section_idx = index_from(&main_src, "return Ok(());", dry_run_idx)?;
    let rmi_idx = index(&main_src, "PyValue::str(\"rmi\")")?;
    req(dry_run_idx < apply_section_idx && apply_section_idx < rmi_idx, "the dry-run early-return must precede the human-approved delete section in source order")?;
    req(src.contains("Some(\"dry-run 표의 항목을 실제 삭제(사람 승인 후)\")"), "predicate requirement failed at original line 450")?;
    // never periodic: no scheduler dependency of the owner package and no wait/loop timer in the cleanup command.
    let cargo = rt_src("Cargo.toml")?;
    let deps = &cargo[index(&cargo, "[dependencies]")?..];
    let deps = &deps[..deps.find("\n[[").unwrap_or(deps.len())];
    let cleanup = [src.as_str(), &rt_src("src/cleanup_cli.rs")?, &rt_src("src/bin/cleanup_docker.rs")?].concat();
    let banned_crates = ["cron", "schedule", "clokwerk", "job_scheduler", "tokio-cron", "apscheduler", "croniter"];
    req(!deps.lines().filter(|l| !l.trim_start().starts_with('#')).any(|l| banned_crates.iter().any(|b| l.contains(b)))
        && !cleanup.contains("thread::sleep") && !cleanup.contains("crontab"),
        "no scheduling library may be imported -- this routine is never periodic")?;

    let dockerfile = rendered("dockerfile")?;
    let dockerfile_from = findall("(?m)^FROM\\s+(\\S+)", &dockerfile).into_iter().next()
        .ok_or_else(|| Fail::new("AttributeError", "'NoneType' object has no attribute 'group'"))?;
    let repo = PyValue::Str(root_ps());
    let tmp = TempDir::new("pp-cleanup-c8-")?;
    let rmi_log = tmp.path().join("rmi.log");
    let (repo_part, tag_part) = dockerfile_from.split_once(':').unwrap_or((dockerfile_from.as_str(), ""));
    let tag_part = if tag_part.is_empty() { "latest" } else { tag_part };
    let debris_ref = "easy-vllm:leftover-debris-9999";
    let img = |r: &str, t: &str, id: &str, size: &str, created: &str| {
        format!("{{\"Repository\": {}, \"Tag\": {}, \"ID\": {}, \"Size\": {}, \"CreatedSince\": {}}}", json_str(r), json_str(t), json_str(id), json_str(size), json_str(created))
    };
    let listing = format!("{}\n{}\n", img(repo_part, tag_part, "sha256:keep", "1GB", "1 day ago"), img("easy-vllm", "leftover-debris-9999", "sha256:debris", "2GB", "40 days ago"));
    write(&tmp.path().join("listing.jsonl"), &listing)?;
    let docker = tmp.path().join("docker");
    write(&docker, &format!(
        "#!/bin/sh\nif [ \"$1\" = image ] && [ \"$2\" = ls ]; then cat {}; exit 0; fi\nif [ \"$1\" = rmi ]; then echo \"$*\" >> {}; exit 1; fi\necho 'docker unavailable in test harness'\nexit 1\n",
        shlex_quote(&tmp.path().join("listing.jsonl").to_string_lossy()), shlex_quote(&rmi_log.to_string_lossy())
    ))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).map_err(|e| Fail::new("OSError", e.to_string()))?;
    }
    let _path = FakePath::install(tmp.path());

    let mut console = Capture::default();
    let keep = cd::preserve_set(&mut World { console: &mut console, tracing: false }, &repo, &PyValue::Bool(true)).map_err(thrown)?;
    req(keep.contains(&ps(&dockerfile_from)), "preserve_set() must really collect the tracked Dockerfile's FROM base as a preserved ref")?;

    let mut console = Capture::default();
    cd::main(&mut World { console: &mut console, tracing: false }, &repo, &[ps("cleanup_docker.sh")], &PyValue::Bool(true), Some("80")).map_err(thrown)?;
    let report = console.out.clone();
    let rm_lines: Vec<&str> = report.lines().filter(|l| l.trim().starts_with("RM")).collect();
    let keep_lines: Vec<&str> = report.lines().filter(|l| l.trim().starts_with("KEEP")).collect();
    let rmi_called = rmi_log.exists();
    req(!rmi_called, "dry-run must never call docker rmi")?;
    req(rm_lines.iter().any(|l| l.contains(debris_ref)), "an unpreserved debris image must appear as an RM candidate")?;
    req(!rm_lines.iter().any(|l| l.contains(&dockerfile_from)), "the preserved Dockerfile FROM base must never appear as an RM candidate")?;
    req(keep_lines.iter().any(|l| l.contains(&dockerfile_from)), "the preserved Dockerfile FROM base must appear in the KEEP table")?;
    req(report.contains("DRY-RUN 종료"), "predicate requirement failed at original line 510")
}

// ---------------------------------------------------------------- MODEL_ACQUISITION_TERNARY_GATE

pub fn predicate_MODEL_ACQUISITION_TERNARY_GATE_C3() -> R {
    // `inspect.signature(crosscheck)`: the Rust owner's parameter list.
    let src = rt_src("src/model_card.rs")?;
    let sig_start = index(&src, "pub fn crosscheck(")? + "pub fn crosscheck(".len();
    let sig = &src[sig_start..index_from(&src, ") -> ", sig_start)?];
    let params: Vec<&str> = sig.split(',').filter_map(|p| p.split(':').next()).map(str::trim).filter(|p| !p.is_empty()).collect();
    req(params.len() >= 4 && !params.contains(&"model_source") && !params.contains(&"mode"), "predicate requirement failed at original line 1164")?;

    let cfg = json_loads("{\"quantization_config\": {\"quant_method\": \"fp8\"}}")?;
    let cprec = recipe_runtime::model_card::config_precision(&cfg).map_err(owner)?;
    req(txt(&cprec).starts_with("fp8"), "predicate requirement failed at original line 1168")?;
    let card = PyValue::str("| DeepSeek-V4-Flash | FP4 + FP8 Mixed (experts FP4) |\n");
    let cardp = recipe_runtime::model_card::card_precision(&card, &PyValue::str("DeepSeek-V4-Flash")).map_err(owner)?;
    req(field(&cardp, "mixed_note").is_truthy() || field(&cardp, "table_row").is_truthy(), "predicate requirement failed at original line 1171")?;

    let skill_md = read(".claude/skills/vllm-recipe-explorer/SKILL.md")?;
    let orchestration_idx = index(&skill_md, "서빙전략의 외부 교차검증(HF 모델카드·vLLM GitHub)은 허용·의무")?;
    let isolation_idx = index_from(&skill_md, "모델획득 격리 한정", orchestration_idx)?;
    req(orchestration_idx < isolation_idx, "the HF-card + vLLM-GitHub cross-check orchestration must be documented as mandatory ('허용·의무'), immediately scoped as independent of acquisition-mode isolation")?;
    let window: String = skill_md[isolation_idx..].chars().take(40).collect();
    req(window.contains("외부접속 전반 차단 ✗"), "the doc must make explicit that this is NOT blocked by the acquisition-mode egress restriction plane -- it always runs, regardless of managed/ephemeral/custom")
}

pub fn predicate_MODEL_ACQUISITION_TERNARY_GATE_C5() -> R {
    use recipe_runtime::model_card::crosscheck_external_vram;
    let tol = PyValue::Float(0.05);
    let never = |_: &PyValue| -> Res<PyValue> {
        Err(agent_runtime_support::failure::Failure::new("AssertionError", "the network boundary must not be reached without a repo_id"))
    };
    let r = crosscheck_external_vram(&PyValue::None, &PyValue::None, &tol, &never).map_err(owner)?;
    req(jstr(field(&r, "verdict")).as_deref() == Some("UNAVAILABLE"), "predicate requirement failed at original line 1234")?;
    req(is_none(field(&r, "external_total")), "no fabricated external total on lookup failure")?;
    let reason = jstr(field(&r, "reason")).unwrap_or_default();
    req(reason.contains("repo_id") || reason.contains("미지정"), "predicate requirement failed at original line 1236")?;

    let stub = |_: &PyValue| -> Res<PyValue> {
        Ok(pyjson_or_panic("{\"ok\": true, \"total\": 1000000, \"parameters\": {\"BF16\": 500000, \"F8_E4M3\": 500000}}"))
    };
    let r_ok = crosscheck_external_vram(&PyValue::None, &PyValue::str("org/model"), &tol, &stub).map_err(owner)?;
    req(jstr(field(&r_ok, "verdict")).as_deref() == Some("OK") && jstr(field(&r_ok, "mode")).as_deref() == Some("ephemeral-estimate"), "predicate requirement failed at original line 1247")?;
    req(is_int(field(&r_ok, "external_total"), 1_000_000), "predicate requirement failed at original line 1248")?;
    req(!is_none(field(&r_ok, "external_native_weight_bytes_est")), "a successful ephemeral lookup must produce a real (non-None) pre-download size estimate")?;
    req(!is_none(field(&r_ok, "external_native_weight_gib_est")), "predicate requirement failed at original line 1251")?;

    let skill_md = read(".claude/skills/vllm-recipe-explorer/SKILL.md")?;
    req(skill_md.contains("ephemeral 다운로드 승인 요청 시 이"), "the doc must tie the pre-download estimate to the ephemeral approval ask")?;
    req(skill_md.contains("사전추정 결과(대략 GiB)를 사용자에게 함께 제시할 것"), "the doc must mandate presenting the estimate (in GiB) to the user at approval time")
}

fn pyjson_or_panic(text: &str) -> PyValue {
    agent_runtime_support::pyjson::loads(text).unwrap_or(PyValue::None)
}

// ---------------------------------------------------------------- KV_ABSOLUTE_CLAMP_PORTABILITY

fn build_yaml_lines(recipe_json: &str) -> R<Vec<String>> {
    let parsed = json_loads("{\"model_path_container\": \"/app/models/org/model\", \"model_id\": \"org/model\"}")?;
    let recipe = json_loads(recipe_json)?;
    let y = recipe_runtime::recipe_set::build_yaml(&parsed, &recipe, &PyValue::int(8000)).map_err(owner)?;
    Ok(splitlines(&txt(&y)).into_iter().map(str::to_string).collect())
}

fn budget(gpu_model: &str, target_gmu: f64, per_card: Option<f64>) -> R<(f64, f64, String)> {
    let mut streams = Streams::default();
    let per = per_card.map_or(PyValue::None, PyValue::Float);
    recipe_runtime::recipe_cmd::resolve_target_gpu_budget(&mut streams, &PyValue::str(gpu_model), &PyValue::Float(target_gmu), &per, &root_ps())
        .map_err(|d| Fail::new("SystemExit", format!("{}: {}", d.code, txt(&d.text))))
}

pub fn predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C1() -> R {
    // `_reject_target_gpu_phase1`: in the Rust owner the Phase-1 refusal is the first step of both Phase-1 commands
    // (estimate, generate) right after the config is read: a defined `target_gpu` dies with status 7 before any emission.
    let src = rt_src("src/recipe_cmd.rs")?;
    for cmd in ["cmd_estimate", "cmd_generate"] {
        let body = rust_fn_body(&src, cmd)?;
        let load = index(&body, "let cfg = load_config(config_path)?;")? + "let cfg = load_config(config_path)?;".len();
        let check = "if get(&cfg, \"target_gpu\").map_or(false, |v| v.is_truthy()) {";
        let at = index(&body, check).map_err(|_| Fail::predicate("predicate requirement failed at original line 1278"))?;
        req(strip(&body[load..at]).is_empty(), "predicate requirement failed at original line 1278")?;
        let guard = &body[at..];
        let guard = &guard[..guard.find("\n    }\n").unwrap_or(guard.len())];
        req(guard.contains("return Err(die(") && strip(&guard[index(guard, "die(")? + 4..]).starts_with("7,") && guard.contains("Phase-1"), "predicate requirement failed at original line 1278")?;
    }
    let with_kv = build_yaml_lines("{\"quantization\": \"native\", \"gpu_memory_utilization\": 0.85, \"max_model_len\": 8192, \"kv_cache_memory_bytes\": 123456789}")?;
    req(with_kv.iter().any(|l| l == "kv-cache-memory-bytes: 123456789"), "the measured absolute byte value must be emitted verbatim, unmodified")?;
    let no_kv = build_yaml_lines("{\"quantization\": \"native\", \"gpu_memory_utilization\": 0.85, \"max_model_len\": 8192}")?;
    req(!no_kv.iter().any(|l| l.contains("kv-cache-memory-bytes")), "without a measured absolute value, no kv-cache-memory-bytes line (gmu-derived or otherwise) may appear -- KV sizing is never approximated from the gmu ratio alone")
}

pub fn predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C2() -> R {
    let (per_card, gmu, model) = budget("NVIDIA RTX PRO 6000", 0.95, None)?;
    req(per_card == 96.0 && gmu == 0.95 && model == "NVIDIA RTX PRO 6000", "discrete GPU: no hard clamp, per-card VRAM from references.md")?;
    let (_per_card2, gmu2, _model2) = budget("NVIDIA GB10", 0.95, Some(120.0))?;
    req(gmu2 == 0.9, "unified-memory GPU (GB10) must hard-clamp target_gmu to 0.90")?;
    let lines = build_yaml_lines(&format!("{{\"quantization\": \"native\", \"gpu_memory_utilization\": {gmu2:?}, \"max_model_len\": 8192, \"kv_cache_memory_bytes\": 987654321}}"))?;
    req(lines.iter().any(|l| l == "gpu-memory-utilization: 0.9") && lines.iter().any(|l| l == "kv-cache-memory-bytes: 987654321"), "gpu-memory-utilization must be co-emitted alongside kv-cache-memory-bytes, never dropped")?;
    req(lines.iter().any(|l| l.contains("startup free-memory 게이트") && l.contains("cap")), "the generated yaml must carry the real comment marking gmu as gate/cap only once the absolute KV clamp controls sizing")?;
    let lines_no_kv = build_yaml_lines("{\"quantization\": \"native\", \"gpu_memory_utilization\": 0.85, \"max_model_len\": 8192}")?;
    req(lines_no_kv.iter().any(|l| l == "gpu-memory-utilization: 0.85"), "predicate requirement failed at original line 1331")?;
    req(!lines_no_kv.iter().any(|l| l.contains("startup free-memory 게이트")), "predicate requirement failed at original line 1332")
}

pub fn predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C3() -> R {
    let (budget_gib, margin, model) = budget("NVIDIA RTX PRO 6000", 0.90, Some(50.0))?;
    req(budget_gib == 50.0 && margin == 0.9 && model == "NVIDIA RTX PRO 6000", "predicate requirement failed at original line 1353")?;
    kv_c3_scenario(budget_gib, margin, 40.0)
}

/// The transfer half of C3 for one host-measured weight figure (the predicate: 40 GiB; the checks: others).
pub fn kv_c3_scenario(budget_gib: f64, margin: f64, weights_gib: f64) -> R {
    use recipe_runtime::simulate_math::{resolve_clamp_kv, Profile};
    let gib = recipe_runtime::estimate_vram::GIB as f64;
    let profile = Profile { weights_gib: Some(weights_gib), non_kv_overhead_gib: Some(4.0), kv_cache_gib: Some(1.0), kv_cache_tokens: Some(1000), ..Default::default() };
    let expected_kv = (10.0 * gib * recipe_runtime::estimate_vram::KV_BLOCK_ALIGN_BUFFER) as i64;
    // the measured per-token rate is present, so the dims formula (`required_kv_bytes(parsed={}, ...)`) is never consulted
    let formula = |_tokens: i64| -> i64 { i64::MAX };
    let r_tp1 = resolve_clamp_kv(&profile, 10000, Some(1), budget_gib, margin, 1, None, &formula);
    req(r_tp1.kv.is_none() && r_tp1.fail.as_ref().map(|f| f.0) == Some("vram_infeasible"), "tp_divisor=1: undivided host weights/overhead must exceed the target budget for this required KV -- if this doesn't fail, the transfer isn't using the real host measurement")?;
    let r_tp2 = resolve_clamp_kv(&profile, 10000, Some(1), budget_gib, margin, 2, None, &formula);
    req(r_tp2.fail.is_none() && r_tp2.kv == Some(expected_kv), "tp_divisor=2: dividing the SAME host-measured weights/overhead by the target TP must free enough budget to satisfy the identical KV request -- proving the transfer actually applies the host invariants (not a reversal/ignoring of them, not an independent re-approximation)")?;
    let r_tp1_again = resolve_clamp_kv(&profile, 10000, Some(1), budget_gib, margin, 1, None, &formula);
    req(r_tp1_again == r_tp1, "porting the identical host profile through a different tp_divisor and back must not leak state -- each port is an independent recomputation, not a carried-forward verdict")
}

pub fn predicate_KV_ABSOLUTE_CLAMP_PORTABILITY_C4() -> R {
    use recipe_runtime::recipe_cmd::lookup_gpu_spec;
    let src = rt_src("src/recipe_cmd.rs")?;
    let lookup_src = rust_fn_body(&src, "lookup_gpu_spec")?;
    req(!lookup_src.to_lowercase().contains("manifest"), "_lookup_gpu_spec must never read manifest state")?;
    let (per_card, is_unified) = lookup_gpu_spec("NVIDIA GB10", &root_ps());
    req(is_unified == Some(true) && per_card.is_none(), "GB10's references.md section is marked unified-memory but has no per-card VRAM line (quick-win override required) -- proves this is a static reference lookup, not a live scan")?;
    let (per_card2, is_unified2) = lookup_gpu_spec("nonexistent-gpu-xyz", &root_ps());
    req(per_card2.is_none() && is_unified2.is_none(), "predicate requirement failed at original line 1406")?;
    let budget_src = rust_fn_body(&src, "resolve_target_gpu_budget")?;
    req(!budget_src.to_lowercase().contains("manifest") && !budget_src.contains("read_manifest"), "resolve_target_gpu_budget (config-time intent plane) must never read manifest state -- it is a pure function of cfg.target_gpu + the static references.md lookup")?;
    let tp_src = rust_fn_body(&src, "target_tp")?;
    req(tp_src.contains("read_manifest("), "_target_tp must read manifest state (node_count) -- its contrast with resolve_target_gpu_budget's manifest-free source is the proof of a deliberate split, not an accidental absence of manifest access anywhere in the module")
}
