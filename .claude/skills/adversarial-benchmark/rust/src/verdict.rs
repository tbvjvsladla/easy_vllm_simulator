//! The deterministic PASS/REFUTE gate: a Rust port of `.claude/skills/adversarial-benchmark/scripts/verdict_rule.py`.
//!
//! Every function takes and returns [`PyValue`]s where the original takes untyped JSON: the measured and roofline
//! documents come from files written by other tools, and the API is called with whatever those contain. Python's
//! behaviour on odd types is part of the contract (a `bool` is a number to `isinstance`, a string multiplied by a
//! float raises, an `int` beyond the double range raises `OverflowError`), so those cases are reproduced rather
//! than rejected; the arithmetic they need lives in [`crate::pyops`].
//!
//! The ladder is the heart of the rule (see the original's header): the rubric candidates E (external reference),
//! c (user target) and expected (roofline × MBU) are admitted only through [`rubric_candidate`], the single
//! predicate that keeps a zero, negative, non-finite or non-numeric value from becoming a vacuous floor.

use crate::pynumber::{float_call, float_from_str, round_to_places, PyException};
use crate::pyops::{
    compare, divide, is_positive_number, multiply, num_of, percent_fixed, subtract, trunc_to_int,
    Compare,
};
use crate::pystr::{lit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;

/// Ladder slot labels (written once, as in the original).
pub const SRC_E: &str = "E(external_reference)";
pub const SRC_C: &str = "c(user_target)";
pub const SRC_EXPECTED: &str = "expected_achievable(roofline×MBU)";

pub const AUTHORITIES: [&str; 3] = ["weak", "explicit", "explore"];
pub const SPEC_DECLARED: [&str; 3] = ["on", "off", "unknown"];
/// Machine-readable reason, owned by this module.
pub const REASON_SPEC_ACCEPT_LEN_MISSING: &str = "SPEC_ACCEPT_LEN_MISSING";

/// The `accept_len` provenance vocabulary, owned by the roofline module and imported here.
pub use crate::roofline::{ACCEPT_LEN_SOURCES, ACCEPT_LEN_VALUE_SOURCES};

fn authority_note(authority: &str) -> &'static str {
    match authority {
        "weak" => "weak(기본): 외부 레퍼런스(E)가 정본",
        "explicit" => "explicit: 사용자 HITL 목표(c)가 정본 — 판정은 성능 게이트(loop-until-done)이고 발행은 막지 않는다",
        _ => "explore: 목표 미설정(광범위 탐색) — 문턱을 세우지 않는다(성능·밸런스 축 **모두 서술**). loop-until-done 해제. 승격 자격은 성능 판정이 아니라 **서빙 성립(기능) + 유효 측정**이다(SKILL.md §2).",
    }
}

// ── small helpers over untyped values ────────────────────────────────────────────

static NONE: PyValue = PyValue::None;

/// `document.get(key)`; anything that is not a dict has no `get`.
pub(crate) fn get<'a>(document: &'a PyValue, key: &str) -> Result<&'a PyValue, PyException> {
    match document.peel() {
        PyValue::Dict(map) => Ok(map.get(key).map_or(&NONE, PyValue::peel)),
        other => Err(PyException::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// `document[key]` with a string key.
fn subscript<'a>(document: &'a PyValue, key: &str) -> Result<&'a PyValue, PyException> {
    match document {
        PyValue::Dict(map) => map
            .get(key)
            .ok_or_else(|| PyException::new("KeyError", PyStr::new(key).repr())),
        PyValue::List(_) => Err(PyException::new(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        PyValue::Str(_) => Err(PyException::new(
            "TypeError",
            "string indices must be integers, not 'str'",
        )),
        other => Err(PyException::new(
            "TypeError",
            format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

fn equals_text(value: &PyValue, text: &str) -> bool {
    matches!(value, PyValue::Str(own) if own.equals(text))
}

fn is_one_of(value: &PyValue, names: &[&str]) -> bool {
    names.iter().any(|name| equals_text(value, name))
}

/// `str(value)` as the `%s` conversion renders it.
fn shown(value: &PyValue) -> PyStr {
    value.display_text()
}

/// Concatenates message pieces (text may carry lone surrogates, so pieces are code-point strings).
fn message(parts: &[&PyStr]) -> PyStr {
    let mut points = Vec::with_capacity(parts.iter().map(|part| part.len()).sum());
    for part in parts {
        points.extend_from_slice(part.code_points());
    }
    PyStr::from_code_points(points)
}

fn t(text: &str) -> PyStr {
    PyStr::new(text)
}

fn float_text(number: f64) -> PyStr {
    PyStr::new(&crate::pyvalue::float_repr(number))
}

fn text_value(text: &str) -> PyValue {
    PyValue::str(text)
}

/// The text of one of the ladder's source labels, shared like a literal.
fn source_text(label: &str) -> PyStr {
    match label {
        SRC_E => lit!(SRC_E),
        SRC_C => lit!(SRC_C),
        SRC_EXPECTED => lit!(SRC_EXPECTED),
        other => PyStr::new(other),
    }
}

/// `PASS` / `REFUTE` and the two strategy labels as shared text.
fn verdict_text(label: &str) -> PyStr {
    match label {
        "PASS" => lit!("PASS"),
        "REFUTE" => lit!("REFUTE"),
        "strategy" => lit!("strategy"),
        "structural?" => lit!("structural?"),
        other => PyStr::new(other),
    }
}

fn string_value(text: PyStr) -> PyValue {
    PyValue::Str(text)
}

/// A dict display with written-out, distinct keys (the keys are literals of this file).
fn dict_of<const N: usize>(items: [(PyStr, PyValue); N]) -> PyDict {
    PyDict::from_distinct_entries(items)
}

fn optional_float(number: Option<f64>) -> PyValue {
    number.map_or(PyValue::None, PyValue::Float)
}

// ── D1: ladder candidate validity ───────────────────────────────────────────────

/// A ladder slot's state: `unset`, `invalid` (with the offending value) or `valid` (a finite positive float).
#[derive(Clone, Debug, PartialEq)]
pub enum Candidate {
    Unset,
    Invalid(PyValue),
    Valid(f64),
}

impl Candidate {
    pub fn state(&self) -> &'static str {
        match self {
            Candidate::Unset => "unset",
            Candidate::Invalid(_) => "invalid",
            Candidate::Valid(_) => "valid",
        }
    }

    pub fn state_text(&self) -> PyStr {
        match self {
            Candidate::Unset => lit!("unset"),
            Candidate::Invalid(_) => lit!("invalid"),
            Candidate::Valid(_) => lit!("valid"),
        }
    }

    /// The `(state, value)` pair the original returns.
    pub fn to_value(&self) -> PyValue {
        let value = match self {
            Candidate::Unset => PyValue::None,
            Candidate::Invalid(raw) => raw.clone(),
            Candidate::Valid(number) => PyValue::Float(*number),
        };
        PyValue::List(vec![PyValue::Str(self.state_text()), value])
    }
}

/// `rubric_candidate(label, raw)`: the single predicate for a ladder slot. `OverflowError` (an `int` beyond the
/// double range) is not a rejection: it escapes, as in the original.
pub fn rubric_candidate(raw: &PyValue) -> Result<Candidate, PyException> {
    if matches!(raw, PyValue::None) {
        return Ok(Candidate::Unset);
    }
    if matches!(raw, PyValue::Bool(_)) {
        return Ok(Candidate::Invalid(raw.clone()));
    }
    let number = match float_call(raw) {
        Ok(number) => number,
        Err(error) if error.is_type_or_value_error() => return Ok(Candidate::Invalid(raw.clone())),
        Err(error) => return Err(error),
    };
    if number.is_nan() || number.is_infinite() {
        return Ok(Candidate::Invalid(raw.clone()));
    }
    if number <= 0.0 {
        return Ok(Candidate::Invalid(PyValue::Float(number)));
    }
    Ok(Candidate::Valid(number))
}

/// `_finite_measure(raw)`: `(ok, value)` of a measurement.
pub fn finite_measure(raw: &PyValue) -> Result<(bool, Option<f64>), PyException> {
    if matches!(raw, PyValue::None | PyValue::Bool(_)) {
        return Ok((false, None));
    }
    let number = match float_call(raw) {
        Ok(number) => number,
        Err(error) if error.is_type_or_value_error() => return Ok((false, None)),
        Err(error) => return Err(error),
    };
    if number.is_nan() || number.is_infinite() {
        return Ok((false, None));
    }
    Ok((true, Some(number)))
}

// ── D3: CLI input contract ──────────────────────────────────────────────────────

/// The arguments `_validate_cli` takes, untyped as the API passes them.
#[derive(Clone, Debug)]
pub struct CliArguments {
    pub authority: PyValue,
    pub reference_tps: PyValue,
    pub target_tps: PyValue,
    pub tolerance: PyValue,
    pub node_vram_gib: PyValue,
    pub balance_tol: PyValue,
    pub spec_declared: PyValue,
}

impl Default for CliArguments {
    /// The defaults of the self-test helper `_st_opts` (which are also the CLI defaults).
    fn default() -> Self {
        CliArguments {
            authority: PyValue::Str(lit!("weak")),
            reference_tps: PyValue::None,
            target_tps: PyValue::None,
            tolerance: PyValue::Float(0.15),
            node_vram_gib: PyValue::None,
            balance_tol: PyValue::Float(0.10),
            spec_declared: PyValue::Str(lit!("unknown")),
        }
    }
}

/// `_validate_cli`: the options and the (possibly empty) list of input errors.
pub fn validate_cli(arguments: &CliArguments) -> Result<(PyDict, Vec<PyStr>), PyException> {
    let mut errors: Vec<PyStr> = Vec::new();
    let authority = &arguments.authority;
    let r = |value: &PyValue| PyStr::new(&value.repr());

    if !is_one_of(authority, &AUTHORITIES) {
        errors.push(message(&[
            &lit!("--authority 는 "),
            &t(&AUTHORITIES.join("|")),
            &lit!(" 중 하나여야 한다(받은 값: "),
            &r(authority),
            &lit!(")"),
        ]));
    }
    if equals_text(authority, "explore") && !matches!(arguments.target_tps, PyValue::None) {
        errors.push(lit!("--authority explore 인데 --target-tps 가 주어졌다 — explore 는 정의상 **목표를 세우지 않는 상태**다(모순). 목표를 세우려면 --authority explicit 을 쓰라."));
    }
    if equals_text(authority, "explicit") && matches!(arguments.target_tps, PyValue::None) {
        errors.push(lit!("--authority explicit 인데 --target-tps 부재 — 명시적 권한은 사용자 목표(c)를 정본으로 요구한다(fail-closed)."));
    }
    for (flag, raw) in [
        ("--target-tps", &arguments.target_tps),
        ("--reference-tps", &arguments.reference_tps),
    ] {
        if matches!(raw, PyValue::None) {
            continue;
        }
        if !matches!(rubric_candidate(raw)?, Candidate::Valid(_)) {
            errors.push(message(&[
                &t(flag),
                &lit!("="),
                &r(raw),
                &lit!(" 은 루브릭이 될 수 없다(유한 양수 아님) — 이 값이 정본으로 낙찰되면 floor=0 인 **공허 PASS**(어떤 측정치도 통과)가 만들어진다. 목표 미설정(광범위 탐색)을 원하면 `--authority explore` 를 쓰고 --target-tps 는 넘기지 마십시오."),
            ]));
        }
    }

    let (tolerance_ok, tolerance_value) = finite_measure(&arguments.tolerance)?;
    let tolerance = match tolerance_value {
        Some(value) if tolerance_ok && (0.0..1.0).contains(&value) => Some(value),
        _ => {
            errors.push(message(&[
                &lit!("--tolerance="),
                &r(&arguments.tolerance),
                &lit!(" 는 0 ≤ tol < 1 이어야 한다 — tol=1 이면 floor=primary×0=0 이라 루브릭 값과 무관하게 **공허 PASS** 가 된다(같은 결함의 두 번째 입구)."),
            ]));
            None
        }
    };

    if !is_one_of(&arguments.spec_declared, &SPEC_DECLARED) {
        errors.push(message(&[
            &lit!("--spec-declared 는 "),
            &t(&SPEC_DECLARED.join("|")),
            &lit!(" 중 하나여야 한다(받은 값: "),
            &r(&arguments.spec_declared),
            &lit!(")"),
        ]));
    }

    let mut node_values: Option<Vec<f64>> = None;
    if arguments.node_vram_gib.is_truthy() {
        let text = shown(&arguments.node_vram_gib);
        let mut parsed = Vec::new();
        let mut failed = false;
        for piece in split_on(&text, ',') {
            if piece.strip().is_empty() {
                continue;
            }
            match float_from_str(&piece) {
                Ok(number) => parsed.push(number),
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            errors.push(message(&[
                &lit!("--node-vram-gib 파싱 실패: "),
                &r(&arguments.node_vram_gib),
            ]));
        } else if parsed.len() < 2 {
            errors.push(lit!(
                "--node-vram-gib 은 2개 이상 노드값 필요(멀티노드 전용)"
            ));
        } else {
            node_values = Some(parsed);
        }
    }

    let opts = dict_of([
        (lit!("authority"), authority.clone()),
        (lit!("reference_tps"), arguments.reference_tps.clone()),
        (lit!("target_tps"), arguments.target_tps.clone()),
        (lit!("tolerance"), optional_float(tolerance)),
        (
            lit!("node_vram"),
            node_values.map_or(PyValue::None, |values| {
                PyValue::List(values.into_iter().map(PyValue::Float).collect())
            }),
        ),
        (lit!("balance_tol"), arguments.balance_tol.clone()),
        (lit!("spec_declared"), arguments.spec_declared.clone()),
    ]);
    Ok((opts, errors))
}

fn split_on(text: &PyStr, separator: char) -> Vec<PyStr> {
    let mut pieces = vec![Vec::new()];
    for &point in text.code_points() {
        if point == u32::from(separator) {
            pieces.push(Vec::new());
        } else {
            pieces.last_mut().expect("never empty").push(point);
        }
    }
    pieces.into_iter().map(PyStr::from_code_points).collect()
}

// ── spec axis ───────────────────────────────────────────────────────────────────

/// `roofline.accept_len_state`, re-exported where the verdict uses it.
pub use crate::roofline::{accept_len_state, AcceptLenState};

/// `resolve_spec_axis`: the declaration × `accept_len` provenance → the physical limit and any gap.
pub fn resolve_spec_axis(
    spec_declared: &PyValue,
    roofline: &PyValue,
    measured: &PyValue,
) -> Result<PyDict, PyException> {
    let source = get(roofline, "accept_len_source")?;
    let raw = get(roofline, "accept_len")?;
    let state = accept_len_state(raw)?;
    let measured_spec_on = get(measured, "spec_on")?.is_truthy();
    let source_has_value = is_one_of(source, &ACCEPT_LEN_VALUE_SOURCES);
    let value = match state {
        AcceptLenState::Valid(number) => Some(number),
        _ => None,
    };
    let has_value = source_has_value && value.is_some();
    let r = |v: &PyValue| PyStr::new(&v.repr());
    let mut axis = dict_of([
        (lit!("declared"), spec_declared.clone()),
        (lit!("accept_len"), raw.clone()),
        (lit!("accept_len_source"), source.clone()),
        (
            lit!("accept_len_evidence"),
            get(roofline, "accept_len_evidence")?.clone(),
        ),
        (lit!("measured_spec_on"), PyValue::Bool(measured_spec_on)),
        (lit!("limit"), PyValue::None),
        (lit!("basis"), PyValue::None),
        (lit!("missing"), PyValue::Bool(false)),
        (lit!("mismatch"), PyValue::None),
    ]);
    if equals_text(spec_declared, "on") {
        if has_value {
            axis.insert(lit!("limit"), PyValue::Str(lit!("R_token")));
            axis.insert(
                lit!("basis"),
                string_value(message(&[
                    &lit!("spec 선언 on ∧ accept_len 출처="),
                    &shown(source),
                ])),
            );
        } else {
            axis.insert(lit!("missing"), PyValue::Bool(true));
            let why = if !is_one_of(source, &ACCEPT_LEN_SOURCES) {
                lit!("roofline.accept_len_source 부재(구세대 산출물 — 값의 출처를 말하지 못한다)")
            } else if source_has_value {
                message(&[
                    &lit!("accept_len="),
                    &r(raw),
                    &lit!(" 무효(유한 ∧ ≥1 아님)"),
                ])
            } else {
                message(&[
                    &lit!("accept_len 출처="),
                    &shown(source),
                    &lit!("(실측·명시 없음)"),
                ])
            };
            axis.insert(
                lit!("basis"),
                string_value(message(&[&lit!("spec 선언 on 인데 "), &why])),
            );
        }
    } else if equals_text(spec_declared, "off") {
        if measured_spec_on {
            axis.insert(lit!("limit"), PyValue::Str(lit!("R_token")));
            axis.insert(
                lit!("basis"),
                PyValue::Str(lit!("선언 off ↔ 실측 spec_on — 측정 우선")),
            );
            let accept = r(get(measured, "accept_len")?);
            axis.insert(
                lit!("mismatch"),
                string_value(message(&[
                    &lit!("YES(declared=off · measured spec_on accept_len="),
                    &accept,
                    &lit!(")"),
                ])),
            );
        } else if has_value && value.is_some_and(|number| number > 1.0) {
            axis.insert(lit!("limit"), PyValue::Str(lit!("R_fp")));
            axis.insert(
                lit!("basis"),
                string_value(message(&[
                    &lit!("선언 off ∧ 실측 spec_off 인데 accept_len 출처="),
                    &shown(source),
                    &lit!(" 값="),
                    &r(raw),
                    &lit!(" 이 R_token·expected_achievable 에 곱해졌다 — 상한은 R_fp"),
                ])),
            );
            axis.insert(
                lit!("mismatch"),
                string_value(message(&[
                    &lit!("YES(declared=off · accept_len_source="),
                    &shown(source),
                    &lit!(" accept_len="),
                    &r(raw),
                    &lit!(")"),
                ])),
            );
        } else {
            axis.insert(lit!("limit"), PyValue::Str(lit!("R_fp")));
            axis.insert(
                lit!("basis"),
                PyValue::Str(lit!("spec 미선언(off) — accept_len 1.0 이 정상(결손 아님)")),
            );
        }
    } else {
        axis.insert(
            lit!("limit"),
            PyValue::Str(if measured_spec_on {
                lit!("R_token")
            } else {
                lit!("R_fp")
            }),
        );
        axis.insert(
            lit!("basis"),
            PyValue::Str(lit!(
                "spec 선언 지문 부재(unknown) — 종전 규율(measured.spec_on) 보존"
            )),
        );
    }
    Ok(axis)
}

// ── the ladder ──────────────────────────────────────────────────────────────────

struct Ladder {
    /// Admitted candidates in priority order.
    admitted: Vec<(&'static str, f64)>,
    /// Every slot with its state, for the output.
    candidates: Vec<PyValue>,
}

fn build_ladder(
    authority: &PyValue,
    reference_tps: &PyValue,
    target_tps: &PyValue,
    expected: &PyValue,
) -> Result<Ladder, PyException> {
    let e_state = rubric_candidate(reference_tps)?;
    let x_state = rubric_candidate(expected)?;
    let explore = equals_text(authority, "explore");
    let (slots, order): (Vec<(&'static str, Option<Candidate>)>, Vec<&'static str>) = if explore {
        (
            vec![
                (SRC_E, Some(e_state)),
                (SRC_C, None),
                (SRC_EXPECTED, Some(x_state)),
            ],
            vec![SRC_E, SRC_EXPECTED],
        )
    } else {
        let c_state = rubric_candidate(target_tps)?;
        if equals_text(authority, "explicit") {
            (
                vec![
                    (SRC_C, Some(c_state)),
                    (SRC_E, Some(e_state)),
                    (SRC_EXPECTED, Some(x_state)),
                ],
                vec![SRC_C, SRC_E, SRC_EXPECTED],
            )
        } else {
            (
                vec![
                    (SRC_E, Some(e_state)),
                    (SRC_C, Some(c_state)),
                    (SRC_EXPECTED, Some(x_state)),
                ],
                vec![SRC_E, SRC_C, SRC_EXPECTED],
            )
        }
    };
    let mut admitted = Vec::new();
    for label in order {
        if let Some((_, Some(Candidate::Valid(value)))) =
            slots.iter().find(|(name, _)| *name == label)
        {
            admitted.push((label, *value));
        }
    }
    let candidates = slots
        .iter()
        .map(|(label, state)| {
            let mut entry = dict_of([(lit!("source"), PyValue::Str(source_text(label)))]);
            match state {
                None => entry.insert(lit!("state"), PyValue::Str(lit!("absent-by-authority"))),
                Some(candidate) => {
                    entry.insert(lit!("state"), PyValue::Str(candidate.state_text()));
                    match candidate {
                        Candidate::Valid(value) => {
                            entry.insert(lit!("value"), PyValue::Float(*value))
                        }
                        Candidate::Invalid(value) => entry.insert(lit!("value"), value.clone()),
                        Candidate::Unset => {}
                    }
                }
            }
            PyValue::Dict(entry)
        })
        .collect();
    Ok(Ladder {
        admitted,
        candidates,
    })
}

// ── build_verdict ───────────────────────────────────────────────────────────────

/// `dict(base)` followed by `.update(extra)`: existing keys keep their position, new keys are appended.
fn updated<const N: usize>(mut map: PyDict, extra: [(PyStr, PyValue); N]) -> PyDict {
    map.reserve(N);
    for (key, value) in extra {
        map.insert(key, value);
    }
    map
}

fn rounded(number: f64, places: u32) -> PyValue {
    PyValue::Float(round_to_places(number, places))
}

/// The verdict object for validated options. Raises what the original raises on odd documents.
pub fn build_verdict(
    measured: &PyValue,
    roofline: &PyValue,
    opts: &PyValue,
) -> Result<PyValue, PyException> {
    let authority = subscript(opts, "authority")?;
    let tolerance = subscript(opts, "tolerance")?;
    let e_search = match opts {
        PyValue::Dict(map) => map
            .get("e_search")
            .cloned()
            .unwrap_or_else(|| PyValue::Str(lit!("no"))),
        _ => PyValue::Str(lit!("no")),
    };

    let (measure_ok, measure) = finite_measure(get(measured, "decode_tps")?)?;
    let spec_on = get(measured, "spec_on")?.is_truthy();
    let accept_measured = get(measured, "accept_len")?.clone();
    let r_fp = get(roofline, "R_fp")?.clone();
    let r_token = get(roofline, "R_token")?.clone();
    let expected = get(roofline, "expected_achievable")?.clone();

    if !get(measured, "measurement_ok")?.is_truthy() || !measure_ok {
        let reason = message(&[
            &lit!("측정 실패(completed="),
            &shown(get(measured, "completed")?),
            &lit!(" failed="),
            &shown(get(measured, "failed")?),
            &lit!(" decode_tps="),
            &PyStr::new(&get(measured, "decode_tps")?.repr()),
            &lit!(") — 재측정 필요"),
        ]);
        return Ok(PyValue::Dict(dict_of([
            (lit!("verdict"), PyValue::Str(lit!("INVALID"))),
            (lit!("failure_axis"), PyValue::None),
            (lit!("structural_or_strategy"), PyValue::None),
            (lit!("reason"), string_value(reason)),
            (
                lit!("measured_decode_tps"),
                get(measured, "decode_tps")?.clone(),
            ),
            (lit!("rubric"), PyValue::None),
            (lit!("reason_code"), PyValue::None),
            (lit!("e_search"), e_search),
        ])));
    }
    let measure = measure.expect("ok implies a value");

    let reference_tps = subscript(opts, "reference_tps")?;
    let target_tps = subscript(opts, "target_tps")?;
    let mut ladder = build_ladder(authority, reference_tps, target_tps, &expected)?;
    let explore = equals_text(authority, "explore");
    let loop_until_done = !explore;
    let target_c_out = if explore {
        PyValue::None
    } else {
        subscript(opts, "target_tps")?.clone()
    };

    let mut rubric_common = dict_of([
        (lit!("tolerance"), tolerance.clone()),
        (lit!("R_fp"), r_fp.clone()),
        (lit!("R_token"), r_token.clone()),
        (lit!("expected_achievable"), expected.clone()),
        (
            lit!("reference_E"),
            subscript(opts, "reference_tps")?.clone(),
        ),
        (lit!("target_c"), target_c_out),
        (lit!("authority"), authority.clone()),
    ]);
    if !is_one_of(authority, &AUTHORITIES) {
        return Err(match authority {
            PyValue::List(_) | PyValue::Dict(_) => PyException::new(
                "TypeError",
                format!("unhashable type: '{}'", authority.type_name()),
            ),
            other => PyException::new("KeyError", other.repr()),
        });
    }
    let authority_name = authority.display_text().to_utf8().unwrap_or_default();
    rubric_common.insert(
        lit!("authority_note"),
        text_value(authority_note(&authority_name)),
    );
    rubric_common.insert(lit!("loop_until_done"), PyValue::Bool(loop_until_done));
    rubric_common.insert(
        lit!("candidates"),
        PyValue::List(std::mem::take(&mut ladder.candidates)),
    );

    let spec_declared = match opts {
        PyValue::Dict(map) => map
            .get("spec_declared")
            .cloned()
            .unwrap_or_else(|| PyValue::Str(lit!("unknown"))),
        _ => PyValue::Str(lit!("unknown")),
    };
    let spec_axis = resolve_spec_axis(&spec_declared, roofline, measured)?;
    let axis_text = |key: &str| spec_axis.get(key).cloned().unwrap_or(PyValue::None);
    // read what the next steps need, then hand the axis itself to the rubric
    let axis_missing = axis_text("missing");
    let axis_basis = if axis_missing.is_truthy() {
        axis_text("basis")
    } else {
        PyValue::None
    };
    let axis_limit = axis_text("limit");
    rubric_common.insert(lit!("spec_axis"), PyValue::Dict(spec_axis));

    if axis_missing.is_truthy() {
        let rubric = updated(
            rubric_common,
            [
                (lit!("primary"), PyValue::None),
                (lit!("source"), PyValue::None),
                (lit!("floor"), PyValue::None),
                (lit!("ratio_M_over_primary"), PyValue::None),
            ],
        );
        let reason = message(&[
            &lit!("루브릭 못 세움("),
            &t(REASON_SPEC_ACCEPT_LEN_MISSING),
            &lit!("): "),
            &shown(&axis_basis),
            &lit!(". accept_len 은 R_token 과 expected_achievable(= realistic_fraction × accept_len × R_fp) 양쪽에 곱해지므로 1.0 으로 메우면 물리 상한과 합격선이 함께 내려간다 — 자동 대체하지 않는다."),
        ]);
        return Ok(PyValue::Dict(dict_of([
            (lit!("verdict"), PyValue::Str(lit!("NEEDS_RUBRIC"))),
            (lit!("failure_axis"), PyValue::Str(lit!("establish"))),
            (lit!("reason_code"), text_value(REASON_SPEC_ACCEPT_LEN_MISSING)),
            (lit!("structural_or_strategy"), PyValue::None),
            (lit!("reason"), string_value(reason)),
            (lit!("measured_decode_tps"), PyValue::Float(measure)),
            (lit!("measured_spec_on"), PyValue::Bool(spec_on)),
            (lit!("measured_accept_len"), accept_measured),
            (lit!("rubric"), PyValue::Dict(rubric)),
            (lit!("authority"), authority.clone()),
            (lit!("e_search"), e_search),
            (lit!("ask_user"), PyValue::Str(lit!("spec(speculative) 이 선언된 서빙인데 accept_len 실측이 없습니다. 판정 레벨의 측정이 수용길이를 싣도록 재측정하거나(lite 레그 `vllm bench serve` 의 spec_decode_acceptance_length), 값을 알고 있으면 `judge_bench.sh --accept-len L` 로 명시하세요(출처는 declared 로 기록됩니다)."))),
        ])));
    }

    if ladder.admitted.is_empty() {
        let rubric = updated(
            rubric_common,
            [
                (lit!("primary"), PyValue::None),
                (lit!("source"), PyValue::None),
                (lit!("floor"), PyValue::None),
                (lit!("ratio_M_over_primary"), PyValue::None),
            ],
        );
        let ask = message(&[
            &lit!("동일 HW("),
            &shown(get(roofline, "gpu_model")?),
            &lit!(", tp="),
            &shown(get(roofline, "tp")?),
            &lit!(")에서 이 모델의 정상 디코드 t/s 레퍼런스를 제공해 주세요."),
        ]);
        return Ok(PyValue::Dict(dict_of([
            (lit!("verdict"), PyValue::Str(lit!("NEEDS_RUBRIC"))),
            (lit!("failure_axis"), PyValue::Str(lit!("establish"))),
            (lit!("structural_or_strategy"), PyValue::None),
            (lit!("reason"), PyValue::Str(lit!("루브릭 못 세움: 사다리의 유효 후보가 없다(부재 또는 invalid) → (c) 사용자 백스톱 필요"))),
            (lit!("reason_code"), PyValue::None),
            (lit!("measured_decode_tps"), PyValue::Float(measure)),
            (lit!("rubric"), PyValue::Dict(rubric)),
            (lit!("authority"), authority.clone()),
            (lit!("e_search"), e_search),
            (lit!("ask_user"), string_value(ask)),
        ])));
    }

    let (primary_source, primary) = ladder.admitted[0];
    let limit_is_token = equals_text(&axis_limit, "R_token");
    let physical_limit = if limit_is_token {
        r_token.clone()
    } else {
        r_fp.clone()
    };
    let limit_source = if limit_is_token {
        lit!("R_token(spec on)")
    } else {
        lit!("R_fp(spec off)")
    };
    let mut physical = dict_of([
        (lit!("limit_source"), PyValue::Str(limit_source.clone())),
        (lit!("limit_tps"), physical_limit.clone()),
        (lit!("exceeds"), PyValue::Bool(false)),
        (lit!("required_bandwidth_gbps"), PyValue::None),
    ]);
    let primary_value = PyValue::Float(primary);
    if is_positive_number(&physical_limit)?
        && compare(Compare::Greater, &primary_value, &physical_limit)?
    {
        physical.insert(lit!("exceeds"), PyValue::Bool(true));
        let bandwidth = get(roofline, "bandwidth_gbps")?;
        if is_positive_number(bandwidth)? {
            let product = multiply(bandwidth, &primary_value)?;
            let required = divide(&product, &physical_limit)?;
            physical.insert(lit!("required_bandwidth_gbps"), rounded(required, 1));
        }
        let rubric = updated(
            rubric_common,
            [
                (lit!("primary"), primary_value.clone()),
                (lit!("source"), PyValue::Str(source_text(primary_source))),
                (lit!("floor"), PyValue::None),
                (lit!("ratio_M_over_primary"), PyValue::None),
                (lit!("physical"), PyValue::Dict(physical.clone())),
            ],
        );
        let required = physical
            .get("required_bandwidth_gbps")
            .cloned()
            .unwrap_or(PyValue::None);
        let required_text = if required.is_truthy() {
            message(&[
                &lit!(" — 달성하려면 "),
                &shown(&required),
                &lit!(" GB/s 가 필요하나 이 HW 는 "),
                &shown(get(roofline, "bandwidth_gbps")?),
                &lit!(" GB/s 다"),
            ])
        } else {
            PyStr::default()
        };
        let reason = message(&[
            &lit!("루브릭 못 세움(물리 초과): 정본 문턱 "),
            &float_text(primary),
            &lit!(" t/s ["),
            &source_text(primary_source),
            &lit!("] 가 이 구성의 물리 상한 "),
            &shown(&physical_limit),
            &lit!(" t/s ["),
            &limit_source,
            &lit!("] 를 넘는다"),
            &required_text,
            &lit!(". 도달 불가능한 문턱으로 낸 REFUTE 는 측정이 아니라 루브릭의 결함이다."),
        ]);
        let ask = message(&[
            &lit!("문턱 "),
            &float_text(primary),
            &lit!(" t/s 의 출처가 **이 구성과 like-with-like** 인지 확인해 주세요 (같은 모델·HW 라도 양자화 범위·커널 백엔드가 다르면 active 바이트가 달라져 루프라인이 달라집니다). like-with-like 수치가 없으면 --e-search empty 로 재판정하세요."),
        ]);
        return Ok(PyValue::Dict(dict_of([
            (lit!("verdict"), PyValue::Str(lit!("NEEDS_RUBRIC"))),
            (lit!("failure_axis"), PyValue::Str(lit!("establish"))),
            (lit!("reason_code"), PyValue::None),
            (lit!("structural_or_strategy"), PyValue::None),
            (lit!("reason"), string_value(reason)),
            (lit!("measured_decode_tps"), PyValue::Float(measure)),
            (lit!("rubric"), PyValue::Dict(rubric)),
            (lit!("authority"), authority.clone()),
            (lit!("e_search"), e_search),
            (lit!("ask_user"), string_value(ask)),
        ])));
    }

    let one_minus_tolerance = subtract(&PyValue::Float(1.0), tolerance)?;
    let floor_value = multiply(&primary_value, &one_minus_tolerance)?;
    let floor = num_of(&floor_value).expect("float times number").to_f64()?;
    let passed = measure >= floor;
    let ratio = round_to_places(measure / primary, 3);
    rubric_common.insert(lit!("physical"), PyValue::Dict(physical));

    let mut notes: Vec<PyValue> = Vec::new();
    let mut reasons: Vec<PyValue> = Vec::new();
    let mut refuted_claims: Vec<PyValue> = Vec::new();

    let mut spec_hint: Option<PyStr> = None;
    let spec_supported = match opts {
        PyValue::Dict(map) => map.get("spec_supported").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    if spec_supported.is_truthy()
        && !spec_on
        && r_token.is_truthy()
        && r_fp.is_truthy()
        && compare(
            Compare::Greater,
            &r_token,
            &multiply(&r_fp, &PyValue::Float(1.1))?,
        )?
    {
        let hint = message(&[
            &lit!("speculative(MTP) OFF 감지(accept_len="),
            &shown(&accept_measured),
            &lit!("). 지원 모델인데 미사용 → R_token("),
            &shown(&r_token),
            &lit!(") 대비 단일패스 R_fp("),
            &shown(&r_fp),
            &lit!(")에 묶임. enable 권장."),
        ]);
        notes.push(string_value(hint.clone()));
        spec_hint = Some(hint);
    }

    let tolerance_text = percent_fixed(tolerance, 2)?;
    let floor_text = crate::pynumber::format_fixed(floor, 2);
    let mut verdict = "PASS";
    let mut structural: Option<&str> = None;
    if passed {
        reasons.push(string_value(message(&[
            &lit!("M("),
            &float_text(measure),
            &lit!(") ≥ "),
            &float_text(primary),
            &lit!("×(1−"),
            &t(&tolerance_text),
            &lit!(")="),
            &t(&floor_text),
            &lit!(" ["),
            &source_text(primary_source),
            &lit!("] → 충족"),
        ])));
    } else {
        verdict = "REFUTE";
        reasons.push(string_value(message(&[
            &lit!("M("),
            &float_text(measure),
            &lit!(") < "),
            &float_text(primary),
            &lit!("×(1−"),
            &t(&tolerance_text),
            &lit!(")="),
            &t(&floor_text),
            &lit!(" ["),
            &source_text(primary_source),
            &lit!("] (ratio "),
            &float_text(ratio),
            &lit!(") → 기대 이하"),
        ])));
        let claim_reason = message(&[
            &lit!("단일스트림 디코드 "),
            &float_text(measure),
            &lit!(" t/s 가 루브릭 "),
            &float_text(primary),
            &lit!(" t/s("),
            &source_text(primary_source),
            &lit!(")의 "),
            &float_text(ratio),
            &lit!("배 — floor "),
            &t(&floor_text),
            &lit!(" 미달"),
        ]);
        let evidence = dict_of([
            (lit!("measured"), get(measured, "decode_tps")?.clone()),
            (
                lit!("engine_cross"),
                get(measured, "engine_gen_throughput_max")?.clone(),
            ),
            (lit!("R_fp"), r_fp.clone()),
            (lit!("R_token"), r_token.clone()),
            (lit!("expected"), expected.clone()),
        ]);
        refuted_claims.push(PyValue::Dict(dict_of([
            (
                lit!("claim"),
                PyValue::Str(lit!("서빙이 충분히 빠르다(production/agent-ready)")),
            ),
            (lit!("reason"), string_value(claim_reason)),
            (lit!("evidence"), PyValue::Dict(evidence)),
        ])));
        if spec_hint.is_some() {
            structural = Some("strategy");
            notes.push(text_value(
                "처방 후보: --speculative-config(MTP) 활성(recipe 재탐색).",
            ));
        } else if expected.is_truthy()
            && compare(
                Compare::Less,
                &PyValue::Float(measure),
                &multiply(&expected, &one_minus_tolerance)?,
            )?
            && spec_on
        {
            structural = Some("structural?");
            notes.push(string_value(message(&[
                &lit!("spec on 인데도 expected("),
                &shown(&expected),
                &lit!(
                    ") 한참 아래 → 커널/이미지/백엔드 구조 의심(escalation 후보 — 외부 확증 필요)."
                ),
            ])));
        } else {
            structural = Some("strategy");
            notes.push(text_value(
                "처방 후보: recipe serve-config 재탐색(백엔드·플래그·KV).",
            ));
        }
        if !loop_until_done {
            notes.push(PyValue::Str(lit!("explore: loop-until-done 해제 상태 — 이 REFUTE 는 재탐색 강제가 아니라 **벽 지도 데이터**다(다음 항목으로 진행 가능). reconciliation_cap 규율은 불변.")));
        }
    }

    let mut warning: Option<&str> = None;
    if primary_source == SRC_EXPECTED {
        if equals_text(&e_search, "no") {
            warning = Some("E-not-attempted: 판정이 roofline-only로 강등됨 — 메인 인스턴스는 외부검색(E) 수행 후 판정(egress-restricted 서브만 예외·--e-search empty 로 기록)");
        } else if equals_text(&e_search, "empty") {
            warning =
                Some("E-attempted-empty: 외부 레퍼런스 부재 기록됨 — roofline-only 판정(음성정직)");
        }
    }

    let rubric = updated(
        rubric_common,
        [
            (lit!("primary"), primary_value),
            (lit!("source"), PyValue::Str(source_text(primary_source))),
            (lit!("floor"), rounded(floor, 2)),
            (lit!("ratio_M_over_primary"), PyValue::Float(ratio)),
        ],
    );
    let mut out = dict_of([
        (lit!("verdict"), PyValue::Str(verdict_text(verdict))),
        (lit!("failure_axis"), PyValue::Str(lit!("meet"))),
        (lit!("reason_code"), PyValue::None),
        (
            lit!("structural_or_strategy"),
            if verdict == "REFUTE" { structural.map_or(PyValue::None, |label| PyValue::Str(verdict_text(label))) } else { PyValue::None },
        ),
        (lit!("measured_decode_tps"), PyValue::Float(measure)),
        (lit!("measured_spec_on"), PyValue::Bool(spec_on)),
        (lit!("measured_accept_len"), accept_measured),
        (lit!("rubric"), PyValue::Dict(rubric)),
        (lit!("e_search"), e_search),
        (lit!("reasons"), PyValue::List(reasons)),
        (lit!("refuted_claims"), PyValue::List(refuted_claims)),
        (lit!("diagnosis_hint"), PyValue::List(notes)),
        (lit!("note"), PyValue::Str(lit!("게이트=결정론(이 규칙). LLM Devil's Advocate 는 E 외부검색·정성 진단·재탐색힌트만 보탠다(최종 structural/strategy = LLM+HITL)."))),
    ]);
    if verdict == "PASS" {
        out.insert(lit!("failure_axis"), PyValue::Str(lit!("meet")));
    }
    if let Some(text) = warning {
        out.insert(lit!("warning"), text_value(text));
    }

    let node_values = match opts {
        PyValue::Dict(map) => map.get("node_vram").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    if node_values.is_truthy() {
        apply_balance(&mut out, &node_values, opts, explore)?;
    }
    Ok(PyValue::Dict(out))
}

/// `list(iterable)` of the values the balance axis may be given (a list; a string or dict iterate too).
fn elements(value: &PyValue) -> Result<Vec<PyValue>, PyException> {
    match value {
        PyValue::List(items) => Ok(items.clone()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
            .collect()),
        PyValue::Dict(map) => Ok(map
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        other => Err(PyException::new(
            "TypeError",
            format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

/// The node-to-node VRAM balance axis (orthogonal to the decode axis; a description, not a gate, under explore).
fn apply_balance(
    out: &mut PyDict,
    node_values: &PyValue,
    opts: &PyValue,
    explore: bool,
) -> Result<(), PyException> {
    let gates = !explore;
    let items = elements(node_values)?;
    // Python's max()/min(): the first element wins unless a later one compares strictly greater/less.
    let mut maximum = items[0].clone();
    for item in &items[1..] {
        if compare(Compare::Greater, item, &maximum)? {
            maximum = item.clone();
        }
    }
    let mut minimum = items[0].clone();
    for item in &items[1..] {
        if compare(Compare::Less, item, &minimum)? {
            minimum = item.clone();
        }
    }
    let spread = subtract(&maximum, &minimum)?;
    let deviation = if maximum.is_truthy() {
        divide(&spread, &maximum)?
    } else {
        0.0
    };
    let balance_tol = subscript(opts, "balance_tol")?;
    let pass = compare(Compare::LessEqual, &PyValue::Float(deviation), balance_tol)?;
    let balance = dict_of([
        (lit!("node_vram_gib"), node_values.clone()),
        (lit!("max_gib"), maximum.clone()),
        (lit!("min_gib"), minimum.clone()),
        (lit!("balance_dev"), rounded(deviation, 4)),
        (lit!("tolerance"), balance_tol.clone()),
        (lit!("pass"), PyValue::Bool(pass)),
        (lit!("gates_verdict"), PyValue::Bool(gates)),
    ]);
    out.insert(lit!("balance"), PyValue::Dict(balance.clone()));
    if !pass && gates {
        out.insert(lit!("verdict"), PyValue::Str(lit!("REFUTE")));
        out.insert(lit!("failure_axis"), PyValue::Str(lit!("balance")));
        let percent = trunc_to_int(&multiply(balance_tol, &PyValue::int(100))?)?;
        let dev4 = crate::pynumber::format_fixed(deviation, 4);
        let tolerance2 = percent_fixed(balance_tol, 2)?;
        let (max2, min2) = (percent_fixed(&maximum, 2)?, percent_fixed(&minimum, 2)?);
        let claim = PyStr::new(&format!(
            "멀티노드 VRAM 배분이 균형적이다(≤{percent}% 편차)"
        ));
        let reason = PyStr::new(&format!(
            "balance_dev={dev4} > tolerance={tolerance2} (max={max2}GiB min={min2}GiB)"
        ));
        push_list(
            out,
            "refuted_claims",
            PyValue::Dict(dict_of([
                (lit!("claim"), string_value(claim)),
                (lit!("reason"), string_value(reason)),
                (lit!("evidence"), PyValue::Dict(balance)),
            ])),
        );
        push_list(out, "diagnosis_hint", PyValue::Str(lit!("노드간 VRAM 편차 초과 → recipe-explorer 재탐색(per-GPU 클램프 재산정 — plan_26070809_47_07 §4.8).")));
    } else if !pass {
        let dev4 = crate::pynumber::format_fixed(deviation, 4);
        let tolerance2 = percent_fixed(balance_tol, 2)?;
        let (max2, min2) = (percent_fixed(&maximum, 2)?, percent_fixed(&minimum, 2)?);
        push_list(out, "diagnosis_hint", string_value(PyStr::new(&format!(
            "explore: 노드간 VRAM 편차 초과(balance_dev={dev4} > tol={tolerance2} · max={max2}GiB min={min2}GiB) — **서술**이다(게이트 ✗). verdict 를 뒤집지 않으며 벽 지도 데이터로만 기록된다(SKILL.md §2.1 — explore 의 승격 자격은 서빙 성립(기능) ∧ 유효 측정). 균형이 필요하면 recipe-explorer per-GPU 클램프 재산정."
        ))));
    }
    Ok(())
}

fn push_list(out: &mut PyDict, key: &str, value: PyValue) {
    if let Some(PyValue::List(items)) = out.get(key).cloned() {
        let mut items = items;
        items.push(value);
        out.set(key, PyValue::List(items));
    }
}

/// `BigInt` is re-exported for the API layer's convenience.
pub type Integer = BigInt;
