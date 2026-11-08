//! `--self-test`: the S1..S20 scenarios of the original `_self_test`, run against the Rust evaluator.
//!
//! Names and output lines match the original so the two programs' transcripts can be compared line by line.

use crate::evaluate::{evaluate, DeclaredLimits, Evaluation, StopReason};
use crate::pyvalue::{PyDict, PyValue};
use crate::utc::{parse_utc, UtcInstant};
use num_bigint::BigInt;

const MEASURED: &str = "measured";
const SERVE_FAILED: &str = "serve_failed";
const MEASUREMENT_VOID: &str = "measurement_void";
const NOT_MEASURED: &str = "not_measured";
const DEFAULT_ERROR_NOW: &str = "2026-09-04T10:00:00Z";

/// Transcript and verdict of one self-test run.
pub struct SelfTestReport {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

fn instant(text: &str) -> UtcInstant {
    parse_utc(&PyValue::str(text), "now").expect("fixture timestamps are valid")
}

fn base_budget() -> PyDict {
    let mut budget = PyDict::new();
    budget.set("max_cells", PyValue::int(6));
    budget.set("wall_clock_budget_s", PyValue::int(3600));
    budget.set("consecutive_failure_limit", PyValue::int(3));
    budget.set("repeats", PyValue::int(3));
    budget.set(
        "repeats_source",
        PyValue::str("declared(fixture campaign.yaml budgets.repeats)"),
    );
    budget.set("declared_by", PyValue::str("hitl:2026-09-04 깊이 승인"));
    budget.set("basis", PyValue::str("직전 로드 실측 12분 × 6셀"));
    budget
}

fn budget_with(changes: &[(&str, PyValue)]) -> PyDict {
    let mut budget = base_budget();
    for (key, value) in changes {
        budget.set(key, value.clone());
    }
    budget
}

fn budget_without(key: &str) -> PyDict {
    let mut budget = PyDict::new();
    for (name, value) in base_budget().iter() {
        if !name.equals(key) {
            budget.insert(name.clone(), value.clone());
        }
    }
    budget
}

fn state_with(cells: Vec<PyValue>, remaining: &[&str], started: &str, budget: PyDict) -> PyDict {
    let mut state = PyDict::new();
    state.set("declared_budget", PyValue::Dict(budget));
    state.set("cells_planned", PyValue::int(6));
    state.set("started_utc", PyValue::str(started));
    state.set("cells", PyValue::List(cells));
    state.set(
        "cells_remaining",
        PyValue::List(remaining.iter().map(|key| PyValue::str(key)).collect()),
    );
    state
}

fn state(cells: Vec<PyValue>, remaining: &[&str]) -> PyDict {
    state_with(cells, remaining, "2026-09-04T09:00:00Z", base_budget())
}

fn cell(key: &str, outcome: &str) -> PyValue {
    let mut cell = PyDict::new();
    cell.set("cell_key", PyValue::str(key));
    cell.set("cell_outcome", PyValue::str(outcome));
    PyValue::Dict(cell)
}

fn repeated_cell(key: &str, requested: i64, runs: i64) -> PyValue {
    let PyValue::Dict(mut cell) = cell(key, MEASURED) else {
        unreachable!("cell() builds a dict")
    };
    let mut repetition = PyDict::new();
    repetition.set("requested", PyValue::int(requested));
    repetition.set("requested_source", PyValue::str("declared(x)"));
    repetition.set("runs_attempted", PyValue::int(runs));
    cell.set("repetition", PyValue::Dict(repetition));
    PyValue::Dict(cell)
}

fn cells(count: usize, outcome: &str) -> Vec<PyValue> {
    (0..count)
        .map(|index| cell(&format!("c{index}"), outcome))
        .collect()
}

fn run(state: PyDict, now: &UtcInstant) -> Evaluation {
    evaluate(&PyValue::Dict(state), now).expect("scenario state is valid")
}

struct Recorder {
    stdout: String,
    failures: Vec<&'static str>,
}

impl Recorder {
    fn check(&mut self, name: &'static str, condition: bool) {
        let verdict = if condition { "PASS" } else { "FAIL" };
        self.stdout.push_str(&format!("  [{verdict}] {name}\n"));
        if !condition {
            self.failures.push(name);
        }
    }

    fn expect_error(&mut self, name: &'static str, state: PyDict, fragment: &str, now: &str) {
        let outcome = evaluate(&PyValue::Dict(state), &instant(now));
        self.check(
            name,
            matches!(&outcome, Err(error) if error.message().contains(fragment)),
        );
    }
}

fn limits_are(
    limits: &DeclaredLimits,
    max_cells: i64,
    wall: i64,
    failures: i64,
    repeats: Option<i64>,
) -> bool {
    limits.max_cells == BigInt::from(max_cells)
        && limits.wall_clock_budget_s == BigInt::from(wall)
        && limits.consecutive_failure_limit == BigInt::from(failures)
        && limits.repeats == repeats.map(BigInt::from)
}

/// Runs every scenario and returns the transcript the CLI prints.
pub fn run_self_test() -> SelfTestReport {
    let mut rec = Recorder {
        stdout: String::new(),
        failures: Vec::new(),
    };
    let now = instant("2026-09-04T09:30:00Z");

    let out = run(state(vec![cell("c1", MEASURED)], &["c2", "c3"]), &now);
    rec.check(
        "S1 진행 중이면 stop=False",
        !out.stop() && out.stopped_by.is_empty() && !out.is_complete() && out.cells_attempted == 1,
    );

    let out = run(state(vec![cell("c1", MEASURED)], &[]), &now);
    rec.check(
        "S2 남은 셀 0 → stop·complete",
        out.stop() && out.stopped_by == [StopReason::CellsExhausted] && out.is_complete(),
    );

    let mixed_failures = vec![
        cell("c1", SERVE_FAILED),
        cell("c2", MEASUREMENT_VOID),
        cell("c3", SERVE_FAILED),
    ];
    let out = run(state(mixed_failures, &["c4"]), &now);
    rec.check(
        "S3 연속 실패 3(분류 혼합) → stop",
        out.stop()
            && out.stopped_by.contains(&StopReason::ConsecutiveFailures)
            && out.consecutive_failures == 3
            && !out.is_complete(),
    );

    let broken_streak = vec![
        cell("c1", SERVE_FAILED),
        cell("c2", MEASURED),
        cell("c3", SERVE_FAILED),
    ];
    let out = run(state(broken_streak, &["c4"]), &now);
    rec.check(
        "S4 성공이 끼면 연속 카운터 리셋",
        !out.stop() && out.consecutive_failures == 1,
    );

    let out = run(state(cells(6, MEASURED), &["c9"]), &now);
    rec.check(
        "S5 셀 수 예산 소진 → stop",
        out.stop() && out.stopped_by.contains(&StopReason::CellBudgetExhausted),
    );

    let out = run(
        state(vec![cell("c1", MEASURED)], &["c2"]),
        &instant("2026-09-04T10:00:00Z"),
    );
    rec.check(
        "S6 벽시계 경계값 포함 → stop",
        out.stop()
            && out.stopped_by.contains(&StopReason::WallClockExhausted)
            && out.elapsed_s == 3600,
    );

    let out = run(
        state(cells(6, SERVE_FAILED), &[]),
        &instant("2026-09-04T11:00:00Z"),
    );
    let mut reasons: Vec<&str> = out
        .stopped_by
        .iter()
        .map(|reason| reason.as_str())
        .collect();
    reasons.sort_unstable();
    rec.check(
        "S7 동시 성립 4가지를 모두 싣는다",
        reasons
            == [
                "cell_budget_exhausted",
                "cells_exhausted",
                "consecutive_failures",
                "wall_clock_exhausted",
            ],
    );

    let mut without_budget = state(vec![], &["c1"]);
    without_budget.set("declared_budget", PyValue::None);
    without_budget.set("cells_planned", PyValue::int(1));
    rec.expect_error(
        "S8 declared_budget 부재 → 거부",
        without_budget,
        "declared_budget 부재",
        DEFAULT_ERROR_NOW,
    );
    for (key, name) in [
        ("max_cells", "S9 max_cells 미선언 → 거부"),
        (
            "wall_clock_budget_s",
            "S9 wall_clock_budget_s 미선언 → 거부",
        ),
        (
            "consecutive_failure_limit",
            "S9 consecutive_failure_limit 미선언 → 거부",
        ),
    ] {
        let mut broken = state(vec![], &["c1"]);
        broken.set("declared_budget", PyValue::Dict(budget_without(key)));
        rec.expect_error(name, broken, &format!("{key} 미선언"), DEFAULT_ERROR_NOW);
    }
    rec.expect_error(
        "S10 한도 0 → 거부",
        state_with(
            vec![],
            &["c1"],
            "2026-09-04T09:00:00Z",
            budget_with(&[("max_cells", PyValue::int(0))]),
        ),
        "양의 정수",
        DEFAULT_ERROR_NOW,
    );
    rec.expect_error(
        "S11 declared_by 공백 → 거부",
        state_with(
            vec![],
            &["c1"],
            "2026-09-04T09:00:00Z",
            budget_with(&[("declared_by", PyValue::str("  "))]),
        ),
        "declared_budget.declared_by",
        DEFAULT_ERROR_NOW,
    );
    rec.expect_error(
        "S12 basis 부재 → 거부",
        state_with(
            vec![],
            &["c1"],
            "2026-09-04T09:00:00Z",
            budget_with(&[("basis", PyValue::str(""))]),
        ),
        "declared_budget.basis",
        DEFAULT_ERROR_NOW,
    );

    let neutral = vec![
        cell("c1", SERVE_FAILED),
        cell("c2", NOT_MEASURED),
        cell("c3", SERVE_FAILED),
    ];
    let out = run(state(neutral, &["c4"]), &now);
    rec.check(
        "S12b not_measured 는 연속 실패를 끊지도 늘리지도 않는다(2연속 유지)",
        out.consecutive_failures == 2,
    );

    rec.expect_error(
        "S13 미지의 cell_outcome → 거부(실패로 접지 않는다)",
        state(vec![cell("c1", "refuted")], &["c2"]),
        "3분류 밖",
        DEFAULT_ERROR_NOW,
    );

    let three_measured = (1..=3)
        .map(|index| cell(&format!("c{index}"), MEASURED))
        .collect();
    let out = run(state(three_measured, &["c4"]), &now);
    rec.check(
        "S14 measured 는 verdict 와 무관하게 실패가 아니다",
        !out.stop() && out.consecutive_failures == 0,
    );

    rec.expect_error(
        "S15 시작이 미래 → 거부",
        state_with(vec![], &["c1"], "2026-09-04T23:00:00Z", base_budget()),
        "미래다",
        DEFAULT_ERROR_NOW,
    );

    let out = run(state(vec![cell("c1", MEASURED)], &["c2"]), &now);
    rec.check(
        "S16 산출물이 예산과 승인 출처를 싣는다(반복 수·그 출처 포함)",
        limits_are(&out.declared_budget, 6, 3600, 3, Some(3))
            && out.budget_declared_by.equals("hitl:2026-09-04 깊이 승인")
            && out.budget_basis.equals("직전 로드 실측 12분 × 6셀")
            && out.budget_repeats_source
                == PyValue::str("declared(fixture campaign.yaml budgets.repeats)"),
    );

    let legacy = state_with(
        vec![cell("c1", MEASURED)],
        &["c2"],
        "2026-09-04T09:00:00Z",
        budget_without("repeats"),
    );
    let out = run(legacy, &now);
    rec.check(
        "S17 ★declared_budget.repeats 미선언(신설 전 상태) → 판정은 한다(읽기 경로) · 3 으로 메우지 않고 None · 출처 absent · 대조 안 함(None) · 정지 술어 불변",
        out.declared_budget.repeats.is_none()
            && matches!(&out.budget_repeats_source, PyValue::Str(text) if text.to_utf8().is_some_and(|t| t.starts_with("absent(")))
            && out.repeats_mismatch.is_none()
            && !out.stop(),
    );
    rec.expect_error(
        "S18 ★repeats=2(full 정의 위반) → 거부",
        state_with(
            vec![],
            &["c1"],
            "2026-09-04T09:00:00Z",
            budget_with(&[("repeats", PyValue::int(2))]),
        ),
        "full 정의",
        DEFAULT_ERROR_NOW,
    );
    rec.expect_error(
        "S18b repeats 비정수 → 거부",
        state_with(
            vec![],
            &["c1"],
            "2026-09-04T09:00:00Z",
            budget_with(&[("repeats", PyValue::str("3"))]),
        ),
        "양의 정수",
        DEFAULT_ERROR_NOW,
    );

    let mixed = vec![
        repeated_cell("c1", 3, 6),
        repeated_cell("c2", 5, 10),
        cell("c3", MEASURED),
        cell("c4", SERVE_FAILED),
    ];
    let out = run(state(mixed, &["c5"]), &now);
    let mismatched: Vec<bool> = out
        .repeats_mismatch
        .iter()
        .flatten()
        .map(|entry| {
            entry.cell_key.equals("c2") && entry.declared_budget_repeats == BigInt::from(3)
        })
        .collect();
    rec.check(
        "S19 셀이 요청한 반복 ≠ 선언 → repeats_mismatch 기재 · 정지 조건은 불변(기재를 게이트로 격상 ✗)",
        !out.stop() && mismatched == [true],
    );
    rec.check(
        "S20 runs_attempted 는 반복 기록의 합 · 측정 셀인데 기록이 없으면 이름으로 남긴다(부재 ≠ 0회)",
        out.runs_attempted == BigInt::from(16) && out.repeats_unrecorded.len() == 1 && out.repeats_unrecorded[0].equals("c3"),
    );

    let Recorder {
        mut stdout,
        failures,
    } = rec;
    if failures.is_empty() {
        stdout.push_str("[sweep_stop --self-test] OK — S1~S20 전부 통과\n");
        return SelfTestReport {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        };
    }
    let names: Vec<String> = failures
        .iter()
        .map(|name| crate::pystr::PyStr::new(name).repr())
        .collect();
    SelfTestReport {
        stdout,
        stderr: format!(
            "[sweep_stop --self-test] FAIL {} 건: [{}]\n",
            failures.len(),
            names.join(", ")
        ),
        exit_code: 1,
    }
}
