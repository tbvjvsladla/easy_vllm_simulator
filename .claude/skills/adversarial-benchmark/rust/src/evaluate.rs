//! The stop decision: `stop <= (no cells remaining) or (cell budget used) or (wall clock used) or (too many
//! consecutive failures)`.
//!
//! Port of `evaluate` from `sweep_stop.py`. Every limit comes from the state's `declared_budget`; nothing here
//! supplies a default budget. Validation order matters because only the first problem is reported, so the
//! sequence of checks follows the original.

use crate::error::SweepError;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::utc::{parse_utc, UtcInstant};
use crate::FULL_REPEATS_MIN;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive};

pub const SCHEMA_VERSION: i64 = 1;

const BUDGET_LIMIT_KEYS: [&str; 3] = [
    "max_cells",
    "wall_clock_budget_s",
    "consecutive_failure_limit",
];
const BUDGET_PROVENANCE_KEYS: [&str; 2] = ["declared_by", "basis"];
const ALLOWED_OUTCOMES: &str = "measured|serve_failed|measurement_void|not_measured";
const REPEATS_ABSENT_SOURCE: &str = "absent(declared_budget 에 반복 수가 없다 — 반복 축 신설 전 상태 파일 · 읽기 경로는 판정하고 측정 진입(broad_search cell)은 거부된다)";

/// How a cell ended. `NotMeasured` is neutral: it neither extends nor resets the consecutive-failure count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CellOutcome {
    Measured,
    ServeFailed,
    MeasurementVoid,
    NotMeasured,
}

impl CellOutcome {
    fn from_value(value: Option<&PyValue>) -> Option<Self> {
        let PyValue::Str(text) = value? else {
            return None;
        };
        [
            ("measured", CellOutcome::Measured),
            ("serve_failed", CellOutcome::ServeFailed),
            ("measurement_void", CellOutcome::MeasurementVoid),
            ("not_measured", CellOutcome::NotMeasured),
        ]
        .into_iter()
        .find_map(|(name, outcome)| text.equals(name).then_some(outcome))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    CellsExhausted,
    CellBudgetExhausted,
    WallClockExhausted,
    ConsecutiveFailures,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::CellsExhausted => "cells_exhausted",
            StopReason::CellBudgetExhausted => "cell_budget_exhausted",
            StopReason::WallClockExhausted => "wall_clock_exhausted",
            StopReason::ConsecutiveFailures => "consecutive_failures",
        }
    }
}

/// A cell whose recorded repetition request differs from the declared repeat count.
#[derive(Clone, Debug, PartialEq)]
pub struct RepeatMismatch {
    pub cell_key: PyStr,
    pub requested: PyValue,
    pub requested_source: PyValue,
    pub declared_budget_repeats: BigInt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredLimits {
    pub max_cells: BigInt,
    pub wall_clock_budget_s: BigInt,
    pub consecutive_failure_limit: BigInt,
    pub repeats: Option<BigInt>,
}

/// The verdict. Field order is the key order of the JSON the original prints.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub stopped_by: Vec<StopReason>,
    pub cells_planned: BigInt,
    pub cells_attempted: usize,
    pub cells_remaining: Vec<PyStr>,
    pub consecutive_failures: usize,
    pub elapsed_s: i64,
    pub declared_budget: DeclaredLimits,
    pub budget_declared_by: PyStr,
    pub budget_basis: PyStr,
    pub budget_repeats_source: PyValue,
    pub runs_attempted: BigInt,
    /// `None` when no repeat count was declared, so there is nothing to compare against.
    pub repeats_mismatch: Option<Vec<RepeatMismatch>>,
    pub repeats_unrecorded: Vec<PyStr>,
    pub evaluated_utc: String,
}

impl Evaluation {
    pub fn stop(&self) -> bool {
        !self.stopped_by.is_empty()
    }

    /// Complete is decided by the remaining cells alone, not by the stop reasons.
    pub fn is_complete(&self) -> bool {
        self.cells_remaining.is_empty()
    }

    /// CLI exit status: 3 when stopping, otherwise 0.
    pub fn exit_code(&self) -> i32 {
        if self.stop() {
            3
        } else {
            0
        }
    }

    /// The JSON document the original prints, as an ordered dict.
    pub fn to_value(&self) -> PyValue {
        let mut doc = PyDict::new();
        doc.set("schema_version", PyValue::int(SCHEMA_VERSION));
        doc.set("stop", PyValue::Bool(self.stop()));
        doc.set(
            "stopped_by",
            PyValue::List(
                self.stopped_by
                    .iter()
                    .map(|reason| PyValue::str(reason.as_str()))
                    .collect(),
            ),
        );
        doc.set(
            "sweep_status",
            PyValue::str(if self.is_complete() {
                "complete"
            } else {
                "incomplete"
            }),
        );
        doc.set("cells_planned", PyValue::Int(self.cells_planned.clone()));
        doc.set("cells_attempted", PyValue::int(self.cells_attempted));
        doc.set("cells_remaining", string_list(&self.cells_remaining));
        doc.set(
            "consecutive_failures",
            PyValue::int(self.consecutive_failures),
        );
        doc.set("elapsed_s", PyValue::int(self.elapsed_s));
        let limits = &self.declared_budget;
        let mut budget = PyDict::new();
        budget.set("max_cells", PyValue::Int(limits.max_cells.clone()));
        budget.set(
            "wall_clock_budget_s",
            PyValue::Int(limits.wall_clock_budget_s.clone()),
        );
        budget.set(
            "consecutive_failure_limit",
            PyValue::Int(limits.consecutive_failure_limit.clone()),
        );
        budget.set(
            "repeats",
            limits.repeats.clone().map_or(PyValue::None, PyValue::Int),
        );
        doc.set("declared_budget", PyValue::Dict(budget));
        doc.set(
            "budget_declared_by",
            PyValue::Str(self.budget_declared_by.clone()),
        );
        doc.set("budget_basis", PyValue::Str(self.budget_basis.clone()));
        doc.set("budget_repeats_source", self.budget_repeats_source.clone());
        doc.set("runs_attempted", PyValue::Int(self.runs_attempted.clone()));
        doc.set("repeats_mismatch", self.mismatch_value());
        doc.set("repeats_unrecorded", string_list(&self.repeats_unrecorded));
        doc.set("evaluated_utc", PyValue::str(&self.evaluated_utc));
        PyValue::Dict(doc)
    }

    fn mismatch_value(&self) -> PyValue {
        let Some(mismatches) = &self.repeats_mismatch else {
            return PyValue::None;
        };
        PyValue::List(
            mismatches
                .iter()
                .map(|entry| {
                    let mut item = PyDict::new();
                    item.set("cell_key", PyValue::Str(entry.cell_key.clone()));
                    item.set("requested", entry.requested.clone());
                    item.set("requested_source", entry.requested_source.clone());
                    item.set(
                        "declared_budget_repeats",
                        PyValue::Int(entry.declared_budget_repeats.clone()),
                    );
                    PyValue::Dict(item)
                })
                .collect(),
        )
    }
}

fn string_list(items: &[PyStr]) -> PyValue {
    PyValue::List(items.iter().cloned().map(PyValue::Str).collect())
}

/// `_positive_int`: the field must exist and be a real `int` greater than zero.
fn positive_int(container: &PyDict, key: &str, owner: &str) -> Result<BigInt, SweepError> {
    let Some(value) = container.get(key) else {
        return Err(SweepError::stop_eval(format!(
            "{owner}.{key} 미선언 — 예산은 대화 층(깊이 HITL)이 낳는다(§4.8). 스크립트가 기본값을 발명하면 승인 없이 스윕이 돈다."
        )));
    };
    match value.as_plain_int() {
        Some(number) if number.is_positive() => Ok(number.clone()),
        _ => Err(SweepError::stop_eval(format!(
            "{owner}.{key} 는 양의 정수여야 한다(받은 값: {})",
            value.repr()
        ))),
    }
}

/// Reads the three limits, the optional repeat count with its recorded source, and the approval provenance.
fn read_budget(state: &PyDict) -> Result<(DeclaredLimits, PyValue, PyStr, PyStr), SweepError> {
    let Some(PyValue::Dict(budget)) = state.get("declared_budget") else {
        return Err(SweepError::stop_eval(
            "declared_budget 부재 — 셀 수·벽시계·연속 실패 한도를 선언 없이 판정하지 않는다(§4.8)",
        ));
    };
    let [max_cells, wall_clock_budget_s, consecutive_failure_limit] =
        BUDGET_LIMIT_KEYS.map(|key| positive_int(budget, key, "declared_budget"));
    let (max_cells, wall_clock_budget_s, consecutive_failure_limit) =
        (max_cells?, wall_clock_budget_s?, consecutive_failure_limit?);

    let (repeats, repeats_source) = if budget.contains_key("repeats") {
        let repeats = positive_int(budget, "repeats", "declared_budget")?;
        if repeats < BigInt::from(FULL_REPEATS_MIN) {
            return Err(SweepError::stop_eval(format!(
                "declared_budget.repeats={repeats} 는 full 정의(반복 ≥{FULL_REPEATS_MIN}) 위반이다 — 반복 수를 낮춰 예산을 맞추지 않는다(셀 수·벽시계로 맞춘다)"
            )));
        }
        (
            Some(repeats),
            budget
                .get("repeats_source")
                .cloned()
                .unwrap_or(PyValue::None),
        )
    } else {
        (None, PyValue::str(REPEATS_ABSENT_SOURCE))
    };

    let [declared_by, basis] = BUDGET_PROVENANCE_KEYS.map(|key| match budget.get(key) {
        Some(PyValue::Str(text)) if !text.is_blank() => Ok(text.clone()),
        _ => Err(SweepError::stop_eval(format!(
            "declared_budget.{key} 가 비었다 — 누가 언제 왜 이 숫자를 승인했는지가 남지 않으면 '사람이 숫자를 발명하지 않았다'가 검증 불가가 된다(§4.8)"
        ))),
    });
    let limits = DeclaredLimits {
        max_cells,
        wall_clock_budget_s,
        consecutive_failure_limit,
        repeats,
    };
    Ok((limits, repeats_source, declared_by?, basis?))
}

/// Tally of the cells already attempted.
#[derive(Default)]
struct CellTally {
    attempted: usize,
    consecutive_failures: usize,
    runs_attempted: BigInt,
    repeats_mismatch: Vec<RepeatMismatch>,
    repeats_unrecorded: Vec<PyStr>,
}

fn tally_cells(
    cells: &[PyValue],
    declared_repeats: Option<&BigInt>,
) -> Result<CellTally, SweepError> {
    let mut tally = CellTally::default();
    for (position, cell) in cells.iter().enumerate() {
        let PyValue::Dict(cell) = cell else {
            return Err(SweepError::stop_eval(format!(
                "cells[{position}] 는 객체여야 한다"
            )));
        };
        let key = match cell.get("cell_key") {
            Some(PyValue::Str(key)) if !key.is_blank() => key,
            _ => {
                return Err(SweepError::stop_eval(format!(
                    "cells[{position}].cell_key 가 비었다 — 셀 좌표 없이는 지도가 성립하지 않는다"
                )))
            }
        };
        let outcome_value = cell.get("cell_outcome");
        let Some(outcome) = CellOutcome::from_value(outcome_value) else {
            return Err(SweepError::stop_eval(format!(
                "cells[{position}].cell_outcome 이 3분류 밖이다(받은 값: {} · 허용: {ALLOWED_OUTCOMES}). 모르는 분류를 실패로 접지 않는다 — 부재와 결측은 다르다.",
                outcome_value.unwrap_or(&PyValue::None).repr()
            )));
        };
        tally.attempted += 1;

        match cell.get("repetition") {
            Some(PyValue::Dict(repetition)) => {
                if let Some(runs) = repetition
                    .get("runs_attempted")
                    .and_then(PyValue::as_plain_int)
                {
                    tally.runs_attempted += runs;
                }
                let requested = repetition.get("requested").unwrap_or(&PyValue::None);
                if let Some(declared) =
                    declared_repeats.filter(|declared| !requested.equals_int(declared))
                {
                    tally.repeats_mismatch.push(RepeatMismatch {
                        cell_key: key.clone(),
                        requested: requested.clone(),
                        requested_source: repetition
                            .get("requested_source")
                            .cloned()
                            .unwrap_or(PyValue::None),
                        declared_budget_repeats: declared.clone(),
                    });
                }
            }
            _ if outcome == CellOutcome::Measured => tally.repeats_unrecorded.push(key.clone()),
            _ => {}
        }

        match outcome {
            CellOutcome::NotMeasured => {}
            CellOutcome::Measured => tally.consecutive_failures = 0,
            CellOutcome::ServeFailed | CellOutcome::MeasurementVoid => {
                tally.consecutive_failures += 1
            }
        }
    }
    Ok(tally)
}

/// The keys still to run. Python takes `state.get("cells_remaining") or []`, iterates it, drops non-strings and
/// only afterwards checks that the field is a list; duplicates and blanks are kept.
fn remaining_cells(state: &PyDict) -> Result<Vec<PyStr>, SweepError> {
    let not_a_list = || SweepError::stop_eval("cells_remaining 은 리스트여야 한다");
    match state.get("cells_remaining") {
        None => Ok(Vec::new()),
        Some(PyValue::List(items)) => Ok(items
            .iter()
            .filter_map(|item| match item {
                PyValue::Str(key) => Some(key.clone()),
                _ => None,
            })
            .collect()),
        Some(other) if !other.is_truthy() => Err(not_a_list()),
        Some(truthy @ (PyValue::Int(_) | PyValue::Float(_) | PyValue::Bool(_))) => {
            Err(SweepError::NotIterable {
                type_name: truthy.type_name(),
            })
        }
        Some(_) => Err(not_a_list()),
    }
}

fn count_reached(count: usize, limit: &BigInt) -> bool {
    limit.to_u64().is_some_and(|limit| count as u64 >= limit)
}

/// Judges the state at `now`. Pure: no files, clock or environment are read.
pub fn evaluate(state: &PyValue, now: &UtcInstant) -> Result<Evaluation, SweepError> {
    let PyValue::Dict(state) = state else {
        return Err(SweepError::stop_eval("상태는 JSON 객체여야 한다"));
    };
    let (limits, repeats_source, declared_by, basis) = read_budget(state)?;

    let cells_planned = match state.get("cells_planned") {
        Some(PyValue::Int(planned)) if !planned.is_negative() => planned.clone(),
        other => {
            return Err(SweepError::stop_eval(format!(
                "cells_planned 는 0 이상 정수여야 한다(받은 값: {})",
                other.unwrap_or(&PyValue::None).repr()
            )))
        }
    };
    let cells = match state.get("cells") {
        Some(PyValue::List(cells)) => cells,
        other => {
            return Err(SweepError::stop_eval(format!(
                "cells 는 리스트여야 한다(받은 값: {})",
                PyStr::new(other.unwrap_or(&PyValue::None).type_name()).repr()
            )))
        }
    };
    let tally = tally_cells(cells, limits.repeats.as_ref())?;
    let remaining = remaining_cells(state)?;

    let started = parse_utc(
        state.get("started_utc").unwrap_or(&PyValue::None),
        "started_utc",
    )?;
    let elapsed_s = now.whole_seconds_since(&started);
    if elapsed_s < 0 {
        return Err(SweepError::stop_eval(format!(
            "started_utc 가 --now-utc 보다 미래다(경과 {elapsed_s}s) — 시각 주입이 뒤엉켰다"
        )));
    }

    let mut stopped_by = Vec::new();
    if remaining.is_empty() {
        stopped_by.push(StopReason::CellsExhausted);
    }
    if count_reached(tally.attempted, &limits.max_cells) {
        stopped_by.push(StopReason::CellBudgetExhausted);
    }
    if count_reached(elapsed_s as usize, &limits.wall_clock_budget_s) {
        stopped_by.push(StopReason::WallClockExhausted);
    }
    if count_reached(
        tally.consecutive_failures,
        &limits.consecutive_failure_limit,
    ) {
        stopped_by.push(StopReason::ConsecutiveFailures);
    }

    let repeats_mismatch = limits.repeats.is_some().then_some(tally.repeats_mismatch);
    Ok(Evaluation {
        stopped_by,
        cells_planned,
        cells_attempted: tally.attempted,
        cells_remaining: remaining,
        consecutive_failures: tally.consecutive_failures,
        elapsed_s,
        declared_budget: limits,
        budget_declared_by: declared_by,
        budget_basis: basis,
        budget_repeats_source: repeats_source,
        runs_attempted: tally.runs_attempted,
        repeats_mismatch,
        repeats_unrecorded: tally.repeats_unrecorded,
        evaluated_utc: now.evaluated_text(),
    })
}
