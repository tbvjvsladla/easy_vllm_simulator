//! `selftest_broad_search_precheck`: the isolated negative control of the cell-provenance precheck of
//! `broad_search.sh cell`. A Rust port of `selftest_broad_search_precheck.py`.
//!
//! Why it exists (plan_26091407 §4.0, §7 O1): 11 of 11 campaign cells were measured from a hand-written yaml with no
//! lockset and nothing marked that fact (F1). The correction is one precheck at the measurement entry: a missing or
//! invalid provenance mark is exit 2, `hand-authored` passes, and a mismatch between the declaration and the served
//! reality is *recorded* in `cell.status`. The precheck lives inside the shell script, so a unit test of the
//! campaign owner (`campaign_init --selftest`) cannot show that the call stands in the real entry path. The deployed
//! script itself is therefore run here.
//!
//! Isolation:
//!
//! * a temporary git checkout holds the deployed files, byte for byte, at their relative paths (nothing is
//!   re-implemented); `broad_search.sh` derives the repository root from git, so the copy sees only its own checkout
//!   and never reaches the live `campaigns/ACTIVE` or `output/**`;
//! * the load path is closed by construction: `sweep_bench.sh` and `judge_bench.sh` are not copied, and the health
//!   probe's `curl` is a shim in front of `PATH` that answers `000` (an external dependency blocked on the test
//!   plane; the cell is recorded as `serve_failed`, so nothing looks like a measurement).
//!
//! Cases (a positive control = must be blocked, a negative control = must pass):
//!
//! * A load path, no lockset: exit 2 before the health probe (shim not called).
//! * B reassembly path, no lockset: exit 2.
//! * C provenance outside the list: exit 2.
//! * D skeleton copy (`provenance=<<FILL>>`): exit 2.
//! * E `hand-authored`: exit 0, the sweep record and `cell.status` carry the provenance, a gmu mismatch is recorded.
//! * F load path with `hand-authored`: past the precheck to the health probe (shim called).
//! * G `explorer-phase2`: exit 0.
//! * H outside a campaign (`ACTIVE` absent = `_bootstrap`): exit 0, `not_applicable` named on stderr and in the record.
//! * I `--serve-failed` record: exit 0, `not_evaluated` (a failure record with no triplet is exempt).
//! * J the judge (`campaign_init.py`) is absent: exit 2 before the health probe, never read as a pass.
//! * K the validator is absent (the judge dies with rc 1): exit 2 "cannot judge", with no "mark it hand-authored"
//!   advice (that would be a misclassification).
//! * L invalid pointer and a state file inside a campaign: judged by the state file's campaign.
//! * M the judging reason code (`SPEC_ACCEPT_LEN_MISSING`) of `verdict.json` reaches the cell record's
//!   `verdict_narrative`; negative control: with `reason_code=null` the narrative keeps its former shape.

use crate::conformance::{py_equals, Raised};
use crate::dataset_prep::Console;
use crate::pyvalue::PyValue;
use crate::verify_fixture::{
    contains, detail_json_ascii, dumps, dumps_unicode, exists, get, get_object, is_file, is_text,
    items_or_empty, list, load_json, object, or_default, read_bytes, repo_root_unknown_message,
    status_and_stderr_tail, tail, text, unlink, unlink_if_present, which, with_id, words,
    write_bytes, write_str, write_text, BroadSearchDriver, Checks, IsolatedCheckout, TextRun,
    CAMPAIGN, CELL, CONFIG, NOW_UTC,
};
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_broad_search_precheck";

/// The deployed files the checkout needs, at their repository-relative paths.
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
    ".claude/skills/terraforming_node/scripts/campaign_init.py",
    ".claude/skills/terraforming_node/scripts/campaign_init.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/campaign_init",
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
    // 반복 축 소유자(2026-09-14 · plan_26091407 §4.4) — init 이 declared_budget.repeats 를 여기서 파생하고
    // 정지 평가기·분류기가 full 정의 하한을 여기서 읽는다. 사본에 없으면 init 이 스윕을 열지 않는다(fail-closed).
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_native.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_owner_query.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis_api",
    // 측정 형상 프리셋(plan_26101011) — init 이 반복 수 기본값을 프리셋에서 받는다(bench_preset.py default).
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.py",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset_api.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset_api",
    ".claude/skills/adversarial-benchmark/config/bench_bindings.yaml",
    ".claude/skills/adversarial-benchmark/config/guide-llm/default_preset.yaml",
    // 루브릭 권한 값역의 단일 소유자(2026-10-01 · plan_26100113 D1) — cell 은 상태 파일 권한을 이 목록으로 검사한다.
    ".claude/skills/adversarial-benchmark/scripts/verdict_rule.sh",
    ".claude/skills/adversarial-benchmark/scripts/roofline.sh",
];

/// The load scripts are never copied: if a path were opened by mistake there would be nothing to run.
const NEVER_COPIED: [&str; 2] = [
    ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh",
    ".claude/skills/adversarial-benchmark/scripts/judge_bench.sh",
];

const CAMPAIGN_INIT: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";
const TEMPLATE_VALIDATOR: &str =
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py";
const LOCKSET_TEMPLATE: &str = "campaigns/_template/cells/_cell/lockset.json";

/// One sweep state file of the checkout.
struct Sweep {
    state: PathBuf,
}

fn python_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// The checkout: the copies, git, the campaign, a cell with a declared target, the serving configuration, a finished
/// sweep to reassemble and the `curl` shim that answers `000`.
fn build_checkout(checkout: &IsolatedCheckout, repo_root: &Path) -> Result<(), Raised> {
    checkout.copy_from(repo_root, &COPIES)?;
    checkout.git_init()?;
    checkout.write_campaign()?;
    write_str(
        &checkout
            .root()
            .join("campaigns")
            .join(CAMPAIGN)
            .join("cells")
            .join(CELL)
            .join("config.yaml"),
        "target_gpu:\n  gpu_model: fixture\n  target_gmu: 0.85\n",
    )?;
    checkout.write_serving_env()?;
    // The reassembly carries the producer's configuration and time binding too: these are fixture facts, not live
    // measurement evidence, and omitting them makes the writer refuse.
    let sweep_index = object(vec![
        ("generated_utc", text(NOW_UTC)),
        (
            "meta",
            object(vec![
                ("config_name", text(CONFIG)),
                ("topology", text("single")),
                ("gpu_memory_utilization", text("0.80")),
                ("image_digest", text("sha256:fixture")),
            ]),
        ),
        ("levels", list(Vec::new())),
    ]);
    let sweep = checkout.sweep_directory();
    write_text(&sweep.join("sweep_index.json"), &dumps(&sweep_index)?)?;
    write_str(
        &sweep.join("level_01").join("measured.json"),
        "{\"measurement_ok\": true}",
    )?;
    // The shim answers `000`: a health status other than 200 is the "serving is not up" path.
    checkout.write_curl_shim("000")
}

/// Writes (`Some`) or removes (`None`) the cell's lockset.
fn set_lockset(checkout: &IsolatedCheckout, document: Option<&PyValue>) -> Result<(), Raised> {
    let path = lockset_path(checkout);
    match document {
        None => unlink_if_present(&path),
        Some(document) => write_text(&path, &dumps_unicode(document)?),
    }
}

fn lockset_path(checkout: &IsolatedCheckout) -> PathBuf {
    checkout
        .root()
        .join("campaigns")
        .join(CAMPAIGN)
        .join("cells")
        .join(CELL)
        .join("lockset.json")
}

fn lockset_with(provenance: &str) -> PyValue {
    object(vec![("id", text(CELL)), ("provenance", text(provenance))])
}

/// `init` of a sweep named `name` under `campaign`'s sweeps; a refusal ends the program.
fn init_state(driver: &BroadSearchDriver, campaign: &str, name: &str) -> Result<Sweep, Raised> {
    let state = driver
        .root()
        .join("campaigns")
        .join(campaign)
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
        "explore",
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
fn run_cell(driver: &BroadSearchDriver, sweep: &Sweep, extra: &[&str]) -> Result<TextRun, Raised> {
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
        "--backend",
        "openai",
    ]));
    arguments.extend(words(extra));
    driver.run(&[], arguments)
}

/// The last record of the fixture cell in the sweep file, or an empty object when there is none.
fn cell_record(sweep: &Sweep) -> Result<PyValue, Raised> {
    let document = load_json(&sweep.state)?;
    let mut last = None;
    for candidate in items_or_empty(get(&document, "cells")?)? {
        if is_text(&get(&candidate, "cell_key")?, CELL) {
            last = Some(candidate);
        }
    }
    Ok(last.unwrap_or_else(|| object(Vec::new())))
}

/// `(record.get("cell_provenance") or {})`.
fn provenance_verdict(sweep: &Sweep) -> Result<PyValue, Raised> {
    get_object(&cell_record(sweep)?, "cell_provenance")
}

/// Runs `body` with the file at `path` removed, then puts its bytes back (also when `body` fails). The marker of the
/// health probe is cleared first so the body's "was curl called" starts from nothing.
fn while_removed(
    path: &Path,
    marker: &Path,
    body: impl FnOnce() -> Result<(), Raised>,
) -> Result<(), Raised> {
    let bytes = read_bytes(path)?;
    unlink_if_present(marker)?;
    unlink(path)?;
    let outcome = body();
    write_bytes(path, &bytes)?;
    outcome
}

fn run_cases(
    checks: &mut Checks<'_>,
    checkout: &IsolatedCheckout,
    driver: &BroadSearchDriver,
    repo_root: &Path,
) -> Result<(), Raised> {
    let marker = checkout.curl_marker();
    let mut never_copied = false;
    for relative in NEVER_COPIED {
        if exists(&checkout.root().join(relative))? {
            never_copied = true;
            break;
        }
    }
    checks.check(
        "사본에는 부하 스크립트가 없다(경로가 잘못 열려도 실행할 대상이 없다)",
        !never_copied,
        "",
    )?;

    // A — load path: the precheck stands *before* the health probe.
    set_lockset(checkout, None)?;
    let sweep = init_state(driver, CAMPAIGN, "a")?;
    let run = run_cell(driver, &sweep, &["--confirm-risk"])?;
    checks.check(
        "A 부하 경로 · lockset 부재 → exit 2",
        run.status == 2,
        &status_and_stderr_tail(&run, 400),
    )?;
    checks.check(
        "A 거부 메시지가 '셀 materialize 는 explorer 소관' 을 안내한다",
        run.stderr.contains("explorer 소관"),
        "",
    )?;
    checks.check(
        "A 표시 부재는 **거부**로 분류된다('판정 불가' 로 새지 않는다 — 두 분류는 처방이 다르다)",
        run.stderr.contains("precheck 거부") && !run.stderr.contains("판정 불가(rc="),
        &tail(&run.stderr, 400),
    )?;
    checks.check(
        "A precheck 가 health 프로브 전에 멈췄다(curl 미호출)",
        !exists(&marker)?,
        "",
    )?;
    checks.check(
        "A 거부된 셀은 스윕 기록에 들어가지 않는다",
        py_equals(&cell_record(&sweep)?, &object(Vec::new())),
        "",
    )?;

    // B, C, D — the same precheck stands on the reassembly path (no load).
    let sweep = init_state(driver, CAMPAIGN, "b")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "B 재조립 경로 · lockset 부재 → exit 2",
        run.status == 2,
        &status_and_stderr_tail(&run, 400),
    )?;
    set_lockset(checkout, Some(&lockset_with("explorer")))?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "C provenance 목록 밖 값 → exit 2 · 거부로 분류",
        run.status == 2
            && run.stderr.contains("precheck 거부")
            && !run.stderr.contains("판정 불가(rc="),
        &status_and_stderr_tail(&run, 400),
    )?;
    let template = load_json(&repo_root.join(LOCKSET_TEMPLATE))?;
    set_lockset(checkout, Some(&with_id(&template, CELL)?))?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "D 뼈대 복사본(provenance=<<FILL>>) → exit 2",
        run.status == 2,
        &status_and_stderr_tail(&run, 400),
    )?;

    // E — hand-authored passes; its provenance and the mismatch are recorded (nothing is blocked).
    let hand_authored_with_source = object(vec![
        ("id", text(CELL)),
        ("provenance", text("hand-authored")),
        ("gmu_source", text("hand")),
    ]);
    set_lockset(checkout, Some(&hand_authored_with_source))?;
    let sweep = init_state(driver, CAMPAIGN, "e")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "E hand-authored → exit 0(손작성은 금지가 아니라 표시 대상)",
        run.status == 0,
        &status_and_stderr_tail(&run, 600),
    )?;
    let record = cell_record(&sweep)?;
    let verdict = get_object(&record, "cell_provenance")?;
    checks.check(
        "E 스윕 셀 기록에 precheck 판정이 남는다",
        is_text(&get(&verdict, "status")?, "passed")
            && is_text(&get(&verdict, "provenance")?, "hand-authored"),
        &detail_json_ascii(&verdict)?,
    )?;
    let status_path = checkout
        .root()
        .join("campaigns")
        .join(CAMPAIGN)
        .join("cells")
        .join(CELL)
        .join("cell.status.json");
    let cell_status = if is_file(&status_path)? {
        load_json(&status_path)?
    } else {
        object(Vec::new())
    };
    checks.check(
        "E cell.status 에 셀 출처가 옮겨진다(writer 가 lockset 을 직접 읽는다)",
        is_text(
            &get(&get_object(&cell_status, "provenance")?, "lockset")?,
            "hand-authored",
        ),
        &detail_json_ascii(&get(&cell_status, "provenance")?)?,
    )?;
    let has_yaml = driver.interpreter_has_yaml()?;
    let mismatch = get(&cell_status, "provenance_mismatch")?;
    if has_yaml {
        checks.check(
            "E 서빙 gmu 0.80 ≠ 선언 target_gmu 0.85 가 provenance_mismatch[] 에 기재된다",
            recorded_gmu_mismatch(&mismatch)?,
            &detail_json_ascii(&mismatch)?,
        )?;
    } else {
        checks.check(
            "E (PyYAML 부재 노드) 불일치 대신 '대조 불가' 가 기재된다 — 일치로 접지 않는다",
            py_equals(&mismatch, &list(Vec::new())) && names_missing_pyyaml(&cell_status)?,
            &detail_json_ascii(&get(&cell_status, "provenance")?)?,
        )?;
    }
    checks.check(
        "E 불일치는 셀 결과를 바꾸지 않는다(기재일 뿐)",
        py_equals(
            &get(&cell_status, "cell_outcome")?,
            &get(&record, "cell_outcome")?,
        ),
        "",
    )?;

    // F — on the load path hand-authored passes the precheck and reaches the health probe.
    let sweep = init_state(driver, CAMPAIGN, "f")?;
    let run = run_cell(driver, &sweep, &["--confirm-risk"])?;
    let serve_failed = run.status == 0
        && exists(&marker)?
        && is_text(&get(&cell_record(&sweep)?, "cell_outcome")?, "serve_failed");
    checks.check(
        "F 부하 경로 · hand-authored → precheck 통과(health 프로브까지 진행 · shim 이 000)",
        serve_failed,
        &format!(
            "rc={} marker={} {}",
            run.status,
            python_bool(exists(&marker)?),
            tail(&run.stderr, 400)
        ),
    )?;

    // G — the second allowed value passes as well.
    set_lockset(checkout, Some(&lockset_with("explorer-phase2")))?;
    let sweep = init_state(driver, CAMPAIGN, "g")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "G explorer-phase2 → exit 0",
        run.status == 0,
        &status_and_stderr_tail(&run, 400),
    )?;

    // H — a call outside a campaign is not blocked, but it is *named* (no silent bypass).
    set_lockset(checkout, None)?;
    let active = checkout.root().join("campaigns").join("ACTIVE");
    unlink(&active)?;
    let sweep = init_state(driver, "_bootstrap", "h")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    let verdict = provenance_verdict(&sweep)?;
    checks.check(
        "H ACTIVE 부재(_bootstrap) → exit 0 · stderr 에 '대상 아님'",
        run.status == 0 && run.stderr.contains("대상 아님"),
        &status_and_stderr_tail(&run, 400),
    )?;
    checks.check(
        "H 셀 기록에 not_applicable 이 남는다",
        is_text(&get(&verdict, "status")?, "not_applicable"),
        &detail_json_ascii(&verdict)?,
    )?;
    write_str(&active, &format!("{CAMPAIGN}\n"))?;

    // I — a serve_failed record has no triplet, so it does not take the precheck (no over-blocking).
    let sweep = init_state(driver, CAMPAIGN, "i")?;
    let run = run_cell(
        driver,
        &sweep,
        &[
            "--serve-failed",
            "fixture: materialize 실패",
            "--serve-started-utc",
            NOW_UTC,
        ],
    )?;
    let verdict = provenance_verdict(&sweep)?;
    checks.check(
        "I --serve-failed · lockset 부재 → exit 0(실패 기록은 지도가 실어야 할 정보)",
        run.status == 0,
        &status_and_stderr_tail(&run, 400),
    )?;
    checks.check(
        "I 셀 기록에 not_evaluated 가 남는다",
        is_text(&get(&verdict, "status")?, "not_evaluated"),
        &detail_json_ascii(&verdict)?,
    )?;

    // J — the judge is absent. The cell is hand-authored so that, were the precheck gone, the call would reach curl
    // (the negative control only catches "absent judge → pass" when the passing conditions are in place).
    set_lockset(checkout, Some(&lockset_with("hand-authored")))?;
    while_removed(&checkout.root().join(CAMPAIGN_INIT), &marker, || {
        let sweep = init_state(driver, CAMPAIGN, "j")?;
        let run = run_cell(driver, &sweep, &["--confirm-risk"])?;
        checks.check(
            "J 판정자(campaign_init.py) 부재 → exit 2 · health 프로브 전(부재를 통과로 접지 않는다)",
            run.status == 2
                && run.stderr.contains("판정자 부재")
                && !exists(&marker)?
                // the absence branch itself stopped it, not the later "cannot judge" branch
                && !run.stderr.contains("판정 불가(rc="),
            &format!("rc={} marker={} {}", run.status, python_bool(exists(&marker)?), tail(&run.stderr, 400)),
        )
    })?;

    // K — the validator is absent: the judge runs but cannot judge (rc 1, empty stdout). Calling it a provenance
    // problem would make an operator overwrite the lockset with a false label, so the classification must differ.
    while_removed(&checkout.root().join(TEMPLATE_VALIDATOR), &marker, || {
        let sweep = init_state(driver, CAMPAIGN, "k")?;
        let run = run_cell(driver, &sweep, &["--confirm-risk"])?;
        checks.check(
            "K 검증기 부재 → exit 2 '판정 불가' · health 프로브 전",
            run.status == 2 && run.stderr.contains("판정 불가") && !exists(&marker)?,
            &format!(
                "rc={} marker={} {}",
                run.status,
                python_bool(exists(&marker)?),
                tail(&run.stderr, 400)
            ),
        )?;
        checks.check(
            "K 판정 불가를 '출처 표시 없음' 으로 오분류하지 않는다(hand-authored 표시 안내 ✗)",
            !run.stderr.contains("hand-authored 로 표시하라")
                && !run.stderr.contains("precheck 거부"),
            &tail(&run.stderr, 400),
        )
    })?;

    // L — a pointer gone invalid (typo, purge leftover) while the state file lives inside a campaign: losing the
    // pointer must not open the campaign cell's gate, so the state file's campaign decides.
    set_lockset(checkout, None)?;
    let sweep = init_state(driver, CAMPAIGN, "l")?;
    write_str(&active, "camp-typo\n")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    checks.check(
        "L 포인터 무효 ∧ 상태 파일 campaigns/<캠페인>/sweeps · lockset 부재 → exit 2(not_applicable 로 새지 않는다)",
        run.status == 2 && run.stderr.contains("precheck 거부") && run.stderr.contains("활성 캠페인은"),
        &status_and_stderr_tail(&run, 600),
    )?;
    set_lockset(checkout, Some(&lockset_with("hand-authored")))?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    let verdict = provenance_verdict(&sweep)?;
    checks.check(
        "L 음성대조: 같은 조건에서 표시가 있으면 통과하고 판정 캠페인은 상태 파일의 캠페인이다",
        run.status == 0
            && is_text(&get(&verdict, "status")?, "passed")
            && is_text(&get(&verdict, "campaign_id")?, CAMPAIGN),
        &format!(
            "rc={} {} {}",
            run.status,
            detail_json_ascii(&verdict)?,
            tail(&run.stderr, 400)
        ),
    )?;
    write_str(&active, &format!("{CAMPAIGN}\n"))?;

    // M — does the judging reason code reach the map layer (the cell record)? The reassembly reads the sweep's
    // verdict.json.
    let verdict_path = checkout.sweep_directory().join("verdict.json");
    set_lockset(checkout, Some(&lockset_with("hand-authored")))?;
    let needs_rubric = object(vec![
        ("verdict", text("NEEDS_RUBRIC")),
        ("failure_axis", text("establish")),
        ("reason_code", text("SPEC_ACCEPT_LEN_MISSING")),
        (
            "rubric",
            object(vec![
                ("authority", text("explore")),
                ("source", PyValue::None),
                ("floor", PyValue::None),
            ]),
        ),
    ]);
    write_text(&verdict_path, &dumps(&needs_rubric)?)?;
    let sweep = init_state(driver, CAMPAIGN, "m")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    let narrative = verdict_narrative(&sweep)?;
    let carries_reason = run.status == 0
        && string_method(&narrative, "endswith")?
            .ends_with(" · reason_code=SPEC_ACCEPT_LEN_MISSING")
        && string_method(&narrative, "startswith")?.starts_with("NEEDS_RUBRIC · authority=explore");
    checks.check(
        "M verdict.reason_code 가 셀 기록 verdict_narrative 에 실린다(마지막 홉에서 삼키지 않는다)",
        carries_reason,
        &format!(
            "rc={} {} {}",
            run.status,
            narrative.repr(),
            tail(&run.stderr, 300)
        ),
    )?;
    let passing = object(vec![
        ("verdict", text("PASS")),
        ("reason_code", PyValue::None),
        (
            "rubric",
            object(vec![
                ("authority", text("explore")),
                ("source", text("E")),
                ("floor", PyValue::Float(1.0)),
            ]),
        ),
    ]);
    write_text(&verdict_path, &dumps(&passing)?)?;
    let sweep = init_state(driver, CAMPAIGN, "m2")?;
    let run = run_cell(driver, &sweep, &["--reassemble-only"])?;
    let narrative = verdict_narrative(&sweep)?;
    checks.check(
        "M 음성대조: reason_code=null 이면 서술은 종전 모양 그대로(사유 꼬리 없음)",
        run.status == 0
            && py_equals(
                &narrative,
                &text("PASS · authority=explore · source=E · floor=1.0"),
            ),
        &format!("rc={} {}", run.status, narrative.repr()),
    )?;
    unlink(&verdict_path)
}

/// `(record.get("verdict_narrative") or "")`.
fn verdict_narrative(sweep: &Sweep) -> Result<PyValue, Raised> {
    Ok(or_default(
        get(&cell_record(sweep)?, "verdict_narrative")?,
        text(""),
    ))
}

/// The text of a value on which the program calls a `str` method (`endswith`, `startswith`): any other kind has no
/// such method.
fn string_method(value: &PyValue, method: &str) -> Result<String, Raised> {
    match value.peel() {
        PyValue::Str(own) => Ok(own.to_utf8().unwrap_or_default()),
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute '{method}'", other.type_name()),
        )),
    }
}

/// The record that serving gmu 0.80 differs from the declared 0.85: exactly one entry of the knob
/// `gpu_memory_utilization`.
fn recorded_gmu_mismatch(mismatch: &PyValue) -> Result<bool, Raised> {
    let PyValue::List(entries) = mismatch.peel() else {
        return Ok(false);
    };
    if entries.len() != 1 {
        return Ok(false);
    }
    Ok(
        is_text(&get(&entries[0], "knob")?, "gpu_memory_utilization")
            && py_equals(&get(&entries[0], "declared")?, &PyValue::Float(0.85))
            && py_equals(&get(&entries[0], "observed")?, &PyValue::Float(0.8)),
    )
}

/// Whether the provenance block names PyYAML among its `compare_gaps` (the node cannot compare without it).
fn names_missing_pyyaml(cell_status: &PyValue) -> Result<bool, Raised> {
    let gaps = get(&get_object(cell_status, "provenance")?, "compare_gaps")?;
    for gap in items_or_empty(gaps)? {
        if contains(&gap, "PyYAML")? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `main()`: the exit status, 0 when every check passed.
pub fn main(repo_root: Option<&Path>, console: &mut dyn Console) -> Result<i32, Raised> {
    if which("git").is_none() {
        let mut checks = Checks::new(console);
        checks.print_error(&format!(
            "[{PROGRAM}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)"
        ));
        return Ok(1);
    }
    let mut checks = Checks::new(console);
    let Some(repo_root) = repo_root else {
        checks.print_error(&repo_root_unknown_message(PROGRAM));
        return Ok(2);
    };
    let checkout = IsolatedCheckout::create("bs-precheck.")?;
    let outcome = build_checkout(&checkout, repo_root).and_then(|()| {
        let driver = BroadSearchDriver::new(&checkout);
        run_cases(&mut checks, &checkout, &driver, repo_root)
    });
    // Leaving the `with` block removes the directory first; a cleanup failure replaces the original exception.
    checkout.close()?;
    outcome?;
    if !checks.failures().is_empty() {
        checks.print_error(&format!("[{PROGRAM}] FAIL {}건:", checks.failures().len()));
        for failure in checks.failures().to_vec() {
            checks.print_error(&format!("  - {failure}"));
        }
        return Ok(1);
    }
    checks.print(&format!("[{PROGRAM}] PASS"))?;
    Ok(0)
}
