//! `selftest_broad_search_cell_args`: the isolated negative control of `broad_search.sh cell`'s argument pre-check
//! (F3) and authority inheritance (F4). A Rust port of `selftest_broad_search_cell_args.py`.
//!
//! Why it exists (plan_26100113 H6, two findings of 2026-10-01):
//!
//! * **F3** a missing `--backend` came back as the measurer's rc 2 *before* measuring, but `cell` took that rc for a
//!   measurement result, wrote a void cell record and spent a cell of the budget.
//! * **F4** the judging call was `${AUTHORITY:-explore}`: it never read the authority `init` wrote into the state
//!   file (weak) and judged as explore, a silent fallback on the judging path.
//!
//! Both corrections stand in the shell script's entry path, so the deployed script is run, byte for byte, in an
//! isolated git checkout (the same rules as the precheck verifier):
//!
//! * the copies are the deployed files at their relative paths, nothing is re-implemented;
//! * the load scripts (`sweep_bench.sh`, `judge_bench.sh`) are stubs that only record their argv and exit with the rc
//!   the environment names, so no real load is possible and no stub output imitates a measurement;
//! * `curl` is a shim that answers `200`, so a call gets past the health probe to the place where the measurer is
//!   invoked.
//!
//! Cases (★ = fails on the behaviour before the correction):
//!
//! * N1★ load path, no `--backend`: exit 2, measurer and health probe not called, no cell record.
//! * N2★ measurer stub rc 2 (argument or precondition refusal): exit 2, no cell record, `cells_remaining` unchanged.
//! * N3★ state-file authority weak + judging knobs: the judge stub's argv carries `--authority weak` and the knobs.
//! * N4★ `--authority` given to `cell`: exit 2, measurer not called (the declaration of `init` is the only source).
//! * N5★ no `rubric_authority` in the state file: exit 2, measurer not called (no default is invented).
//! * N6 negative control: reassembly (`--reassemble-only`) is not blocked without `--backend` (it is no load path).

use crate::conformance::{py_equals, Raised};
use crate::dataset_prep::Console;
use crate::pyvalue::PyValue;
use crate::verify_fixture::{
    chmod, detail_json, dumps_pretty, exists, get, head, load_json, object,
    repo_root_unknown_message, status_and_stderr_tail, tail, text, unlink_if_present, which, words,
    write_str, write_text, BroadSearchDriver, Checks, IsolatedCheckout, TextRun, BENCHMARK_SCRIPTS,
    CAMPAIGN, CELL, CONFIG, NOW_UTC,
};
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_broad_search_cell_args";

/// The deployed files the checkout needs, at their repository-relative paths. `broad_search.sh` and what `init` and
/// `cell` call after it: the cell classifier and the parser/verdict chain, the repeat axis and the measurement
/// preset (init takes the repeat count default from the preset), and the campaign owner.
pub(crate) const COPIES: [&str; 37] = [
    ".claude/skills/adversarial-benchmark/scripts/classify_cell.sh",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_native.py",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_role_query.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/classify_cell",
    ".claude/skills/adversarial-benchmark/rust/target/release/classify_cell_api",
    ".claude/skills/adversarial-benchmark/scripts/parse_guidellm.sh",
    ".claude/skills/adversarial-benchmark/scripts/parse_guidellm_native.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/parse_guidellm",
    ".claude/skills/adversarial-benchmark/rust/target/release/roofline",
    ".claude/skills/adversarial-benchmark/rust/target/release/verdict_rule",
    ".claude/skills/adversarial-benchmark/scripts/broad_search.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_shell.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_shell",
    ".claude/skills/adversarial-benchmark/scripts/sweep_stop.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/sweep_stop",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_native.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_owner_query.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis_api",
    // Measurement preset (plan_26101011): init takes the default repeat count from the preset.
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.py",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset_api.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset_api",
    ".claude/skills/adversarial-benchmark/config/bench_bindings.yaml",
    ".claude/skills/adversarial-benchmark/config/guide-llm/default_preset.yaml",
    ".claude/skills/adversarial-benchmark/scripts/verdict_rule.sh",
    ".claude/skills/adversarial-benchmark/scripts/roofline.sh",
    ".claude/skills/terraforming_node/scripts/campaign_init.py",
    ".claude/skills/terraforming_node/scripts/campaign_init.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/campaign_read",
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
];

/// The load scripts and the environment variable that sets each stub's exit status.
const LOAD_STUBS: [(&str, &str); 2] = [
    ("sweep_bench", "STUB_SWEEP_RC"),
    ("judge_bench", "STUB_JUDGE_RC"),
];

/// What the checks learn about one sweep state file.
struct Sweep {
    state: PathBuf,
}

impl Sweep {
    fn load(&self) -> Result<PyValue, Raised> {
        load_json(&self.state)
    }
}

/// The marker files the stubs and the shim leave: what the checks ask "was it called?" of.
struct Markers {
    sweep_argv: PathBuf,
    judge_argv: PathBuf,
    curl_marker: PathBuf,
}

impl Markers {
    fn clear(&self) -> Result<(), Raised> {
        for marker in [&self.sweep_argv, &self.judge_argv, &self.curl_marker] {
            unlink_if_present(marker)?;
        }
        Ok(())
    }
}

/// A stub that appends its argv, one line per call, beside itself and ends with the status the environment names.
fn load_stub_script(name: &str, status_variable: &str) -> String {
    format!(
        "#!/bin/sh\nprintf \"%s\\n\" \"$*\" >> \"$(dirname \"$0\")/{name}.argv\"\nexit \"${{{status_variable}:-0}}\"\n"
    )
}

fn write_load_stubs(checkout: &IsolatedCheckout) -> Result<(), Raised> {
    for (name, status_variable) in LOAD_STUBS {
        let stub = checkout
            .root()
            .join(BENCHMARK_SCRIPTS)
            .join(format!("{name}.sh"));
        write_str(&stub, &load_stub_script(name, status_variable))?;
        chmod(&stub, 0o755)?;
    }
    Ok(())
}

/// The checkout: the copies, the stubs, git, the campaign, the serving configuration and a finished sweep to
/// reassemble, the `curl` shim.
fn build_checkout(checkout: &IsolatedCheckout, repo_root: &Path) -> Result<(), Raised> {
    checkout.copy_from(repo_root, &COPIES)?;
    write_load_stubs(checkout)?;
    checkout.git_init()?;
    checkout.write_campaign()?;
    write_text(
        &checkout
            .root()
            .join("campaigns")
            .join(CAMPAIGN)
            .join("cells")
            .join(CELL)
            .join("lockset.json"),
        &crate::verify_fixture::dumps(&object(vec![
            ("id", text(CELL)),
            ("provenance", text("hand-authored")),
        ]))?,
    )?;
    checkout.write_serving_env()?;
    let sweep = checkout.sweep_directory();
    write_str(
        &sweep.join("sweep_index.json"),
        "{\"meta\": {}, \"levels\": []}",
    )?;
    write_str(
        &sweep.join("level_01").join("measured.json"),
        "{\"measurement_ok\": true}",
    )?;
    checkout.write_curl_shim("200")
}

/// `init` of a sweep named `name` with the declared rubric `authority`; a refusal ends the program.
fn init_sweep(driver: &BroadSearchDriver, name: &str, authority: &str) -> Result<Sweep, Raised> {
    let state = driver
        .root()
        .join("campaigns")
        .join(CAMPAIGN)
        .join("sweeps")
        .join(format!("{name}.json"));
    let mut arguments = words(&["init", "--sweep-id", name, "--state"]);
    arguments.push(state.clone().into_os_string());
    arguments.extend(words(&[
        "--cells",
        CELL,
        "--control-variable",
        "fixture",
        "--max-cells",
        "10",
        "--wall-clock-budget-s",
        "100000",
        "--consecutive-failure-limit",
        "5",
        "--declared-by",
        "selftest",
        "--basis",
        "fixture",
        "--authority",
        authority,
        "--now-utc",
        NOW_UTC,
        "--topology",
        "single",
    ]));
    let run = driver.run(&[], arguments)?;
    if run.status != 0 {
        return Err(Raised::new(
            "SystemExit",
            &format!("[{PROGRAM}] 픽스처 init 실패: {}", run.stderr),
        ));
    }
    Ok(Sweep { state })
}

/// `cell` of the fixture cell with the options every call shares, then `extra`.
fn run_cell(
    driver: &BroadSearchDriver,
    sweep: &Sweep,
    extra_environment: &[(&str, &str)],
    extra: &[&str],
) -> Result<TextRun, Raised> {
    let mut arguments = words(&["cell", "--state"]);
    arguments.push(sweep.state.clone().into_os_string());
    arguments.extend(words(&[
        "--cell-key",
        CELL,
        "--config",
        CONFIG,
        "--axis-citation",
        "fixture",
        "--next-intent",
        "fixture",
        "--bench-budget-mib",
        "1",
        "--now-utc",
        NOW_UTC,
        "--topology",
        "single",
        "--ack-uncalibrated-thermal",
    ]));
    arguments.extend(words(extra));
    driver.run(extra_environment, arguments)
}

/// `read_text` of an argv record, or the empty text when the stub was never called.
fn recorded_argv(path: &Path) -> Result<String, Raised> {
    if exists(path)? {
        crate::verify_fixture::read_text(path)
    } else {
        Ok(String::new())
    }
}

fn run_cases(
    checks: &mut Checks<'_>,
    driver: &BroadSearchDriver,
    markers: &Markers,
) -> Result<(), Raised> {
    // N1 — load path, no --backend
    markers.clear()?;
    let sweep = init_sweep(driver, "n1", "weak")?;
    let before = sweep.load()?;
    let run = run_cell(driver, &sweep, &[], &["--confirm-risk"])?;
    checks.check(
        "N1★ --backend 없음 → exit 2",
        run.status == 2,
        &status_and_stderr_tail(&run, 400),
    )?;
    checks.check(
        "N1★ 측정기·health 프로브 미호출(측정 진입 0)",
        !exists(&markers.sweep_argv)? && !exists(&markers.curl_marker)?,
        "",
    )?;
    checks.check(
        "N1★ 상태 파일 불변(셀 기록 0 · 예산 불변)",
        py_equals(&sweep.load()?, &before),
        "",
    )?;

    // N2 — the measurer answers rc 2
    markers.clear()?;
    let sweep = init_sweep(driver, "n2", "weak")?;
    let before = sweep.load()?;
    let run = run_cell(
        driver,
        &sweep,
        &[("STUB_SWEEP_RC", "2")],
        &["--confirm-risk", "--backend", "openai"],
    )?;
    checks.check(
        "N2★ 측정기 rc 2 → exit 2",
        run.status == 2,
        &status_and_stderr_tail(&run, 400),
    )?;
    checks.check(
        "N2 측정기는 실제로 불렸다(rc 2 를 받은 자리를 시험한다)",
        exists(&markers.sweep_argv)?,
        "",
    )?;
    let after = sweep.load()?;
    let unchanged = py_equals(&get(&after, "cells")?, &get(&before, "cells")?)
        && py_equals(
            &get(&after, "cells_remaining")?,
            &get(&before, "cells_remaining")?,
        );
    let detail = head(
        &detail_json(&object(vec![
            ("cells", get(&after, "cells")?),
            ("rem", get(&after, "cells_remaining")?),
        ]))?,
        300,
    );
    checks.check("N2★ 셀 기록 0 · cells_remaining 불변", unchanged, &detail)?;
    checks.check(
        "N2 판정기 미호출(측정 결과가 아니다)",
        !exists(&markers.judge_argv)?,
        "",
    )?;

    // N3 — authority inheritance and the judging knobs
    markers.clear()?;
    let sweep = init_sweep(driver, "n3", "weak")?;
    let run = run_cell(
        driver,
        &sweep,
        &[],
        &[
            "--confirm-risk",
            "--backend",
            "openai",
            "--reference-tps",
            "53.7",
            "--e-search",
            "hit",
        ],
    )?;
    let judge_argv = recorded_argv(&markers.judge_argv)?;
    let shown_argv = crate::pystr::PyStr::new(&judge_argv).repr();
    checks.check(
        "N3★ 판정기가 상태 파일 권한 weak 를 받는다",
        judge_argv.contains("--authority weak"),
        &format!("argv={shown_argv} rc={}", run.status),
    )?;
    checks.check(
        "N3★ 판정 노브가 그대로 도달한다",
        judge_argv.contains("--reference-tps 53.7") && judge_argv.contains("--e-search hit"),
        &format!("argv={shown_argv}"),
    )?;
    checks.check(
        "N3★ explore 폴백이 없다(판정기가 실제로 불린 위에서)",
        !judge_argv.is_empty() && !judge_argv.contains("--authority explore"),
        &format!("argv={shown_argv}"),
    )?;
    checks.check(
        "N3 stdout 이 권한 출처를 밝힌다",
        run.stdout.contains("루브릭 권한 = weak"),
        &tail(&run.stdout, 300),
    )?;

    // N4 — `--authority` given to cell
    markers.clear()?;
    let sweep = init_sweep(driver, "n4", "weak")?;
    let run = run_cell(
        driver,
        &sweep,
        &[],
        &[
            "--confirm-risk",
            "--backend",
            "openai",
            "--authority",
            "explore",
        ],
    )?;
    checks.check(
        "N4★ cell --authority → exit 2 · 측정기 미호출",
        run.status == 2 && !exists(&markers.sweep_argv)? && run.stderr.contains("유일한 원천"),
        &format!("rc={} {}", run.status, tail(&run.stderr, 300)),
    )?;

    // N5 — no authority in the state file
    markers.clear()?;
    let sweep = init_sweep(driver, "n5", "weak")?;
    let without_authority = crate::verify_fixture::without_key(sweep.load()?, "rubric_authority")?;
    write_text(&sweep.state, &dumps_pretty(&without_authority)?)?;
    let run = run_cell(
        driver,
        &sweep,
        &[],
        &["--confirm-risk", "--backend", "openai"],
    )?;
    checks.check(
        "N5★ rubric_authority 부재 → exit 2 · 측정기 미호출",
        run.status == 2 && !exists(&markers.sweep_argv)? && run.stderr.contains("rubric_authority"),
        &format!("rc={} {}", run.status, tail(&run.stderr, 300)),
    )?;

    // N6 — negative control: reassembly is not a load path
    markers.clear()?;
    let sweep = init_sweep(driver, "n6", "weak")?;
    let run = run_cell(driver, &sweep, &[], &["--reassemble-only"])?;
    checks.check(
        "N6 재조립 · --backend 없음 → 선검사 통과(exit 0)",
        run.status == 0 && !run.stderr.contains("--backend 가 필수"),
        &status_and_stderr_tail(&run, 400),
    )
}

/// `main()`: the exit status, 0 when every check passed.
pub fn main(repo_root: Option<&Path>, console: &mut dyn Console) -> Result<i32, Raised> {
    if which("git").is_none() {
        let mut checks = Checks::new(console);
        checks.print_error(&format!(
            "[{PROGRAM}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다"
        ));
        return Ok(1);
    }
    let mut checks = Checks::new(console);
    let Some(repo_root) = repo_root else {
        checks.print_error(&repo_root_unknown_message(PROGRAM));
        return Ok(2);
    };
    let checkout = IsolatedCheckout::create("bs-cellargs.")?;
    let outcome = build_checkout(&checkout, repo_root).and_then(|()| {
        let driver = BroadSearchDriver::new(&checkout);
        let scripts = checkout.root().join(BENCHMARK_SCRIPTS);
        let markers = Markers {
            sweep_argv: scripts.join("sweep_bench.argv"),
            judge_argv: scripts.join("judge_bench.argv"),
            curl_marker: checkout.curl_marker(),
        };
        run_cases(&mut checks, &driver, &markers)
    });
    // Leaving the `with` block removes the directory first; a cleanup failure replaces the original exception.
    checkout.close()?;
    outcome?;
    if !checks.failures().is_empty() {
        checks.print(&format!("[{PROGRAM}] FAIL {}건:", checks.failures().len()))?;
        for failure in checks.failures().to_vec() {
            checks.print(&format!("  - {failure}"))?;
        }
        return Ok(1);
    }
    checks.print(&format!(
        "[{PROGRAM}] PASS — N1~N6(★ 교정 전 거동이면 FAIL)"
    ))?;
    Ok(0)
}
