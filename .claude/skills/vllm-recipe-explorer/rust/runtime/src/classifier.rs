//! `sim_classify.py`: the deterministic classifier of a trial's result (Phase 2 of the recipe explorer). Given the output of a
//! trial (`load_ok`, `vllm_profile`, `functional`, `error_excerpt`, ...), the VRAM budget and the budget gate's margin, it names
//! the failure class (`none`, `vram_oom`, `vram_infeasible`, `functional`, `unknown`) and the one variable to adjust next; given a
//! representative request length it also re-checks the `max-num-seqs` formula against the KV tokens the engine reported.
//!
//! The inputs are JSON-shaped values of any type, so every `.get`, `or {}`, `float()`, `int()`, `//`, `*` and comparison is the
//! Python operation on a [`PyValue`], with the exception Python raises, in the order Python evaluates them: nothing is parsed
//! before the original parses it (a known OOM signature returns before the profile is read), and the values the original keeps
//! are returned as they came. The two regular-expression families the original applies to the error text are written out
//! against code points, with Python's own Unicode 15 notion of a word character and of white space (`\w`, `\s`, `\b`), `^` at
//! the start of a line after `\n` only, and a dot that excludes only `\n`.

use crate::model_dyn::{int_floor_div, Message};
use crate::pyexpr::{exception, get, Res};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{as_get_target, ensure_hashable, is_text};
use agent_runtime_support::pyops::{compare, multiply, percent_fixed, Compare};
use agent_runtime_support::pystr::{is_python_space, is_python_word, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::One;

/// The unit of the VRAM figures: GiB = 1024 ** 3.
pub const GIB: u64 = 1 << 30;

/// The known out-of-memory signatures (literal text), searched in the error excerpt of a failed load in this order.
pub const OOM_SIGNATURES: [&str; 3] = [
    "CUDA out of memory",
    "No available memory for the cache blocks",
    "ValueError: To serve at least one request",
];

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

/// `int(value)`. The decoder of the inputs may hand a number inside the sharing wrapper (the one `NaN` object); the conversions
/// look at the value inside.
fn int_of(value: &PyValue) -> Res<BigInt> {
    crate::pyexpr::int_of(value.peel())
}

/// `float(value)` (see [`int_of`]).
fn float_of(value: &PyValue) -> Res<f64> {
    crate::pyexpr::float_of(value.peel())
}

/// `value or {}`.
fn or_empty_dict(value: PyValue) -> PyValue {
    if value.is_truthy() {
        value
    } else {
        PyValue::Dict(PyDict::new())
    }
}

/// `_profile_gib(profile, key)`: a GiB figure of the engine's profile as a float; `None` when the profile is empty or does not
/// have the key. A profile that is not a dict has no `get`; a key that cannot be hashed is refused.
pub fn profile_gib(profile: &PyValue, key: &PyValue) -> Res<Option<f64>> {
    if !profile.is_truthy() {
        return Ok(None);
    }
    let members = as_get_target(profile)?;
    ensure_hashable(key)?;
    let found = match key.peel() {
        PyValue::Str(name) => members
            .keyed_entries()
            .find(|(other, held, _)| other.is_none() && *held == name)
            .map(|(_, _, value)| value),
        _ => None,
    };
    match found {
        Some(value) if !matches!(value.peel(), PyValue::None) => float_of(value).map(Some),
        _ => Ok(None),
    }
}

fn profile_key(profile: &PyValue, key: &str) -> Res<Option<f64>> {
    profile_gib(profile, &text(key))
}

fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// `kv_fit_check(trial_result, typical_request_tokens)`: the KV-fit of the batch against the engine's reported KV tokens, as the
/// original's dict. A check that cannot be made says so in `reason` (and leaves `checked` false).
pub fn kv_fit_check(trial: &PyValue, typical: &PyValue) -> Res<PyDict> {
    let candidate = or_empty_dict(get(trial, "candidate")?);
    let profile = or_empty_dict(get(trial, "vllm_profile")?);
    let mut out = PyDict::new();
    out.set("checked", PyValue::Bool(false));
    out.set("exceeds", PyValue::Bool(false));
    out.set("kv_fit", PyValue::None);
    out.set("batch", get(&candidate, "batch")?);
    out.set("kv_fit_length", typical.clone());
    out.set("kv_cache_tokens", get(&profile, "kv_cache_tokens")?);
    out.set("reason", text(""));
    if !typical.is_truthy() {
        out.set(
            "reason",
            text("대표 길이 L 미지정(손레버 batch 이거나 batch 미정) — 재검증 대상 아님"),
        );
        return Ok(out);
    }
    if is_none(&get(&candidate, "kv_cache_memory_bytes")?) {
        out.set(
            "reason",
            text("클램프 미설정(측정 트라이얼) — 엔진 토큰이 배포 KV 가 아니다"),
        );
        return Ok(out);
    }
    if is_none(&get(&candidate, "batch")?) {
        out.set("reason", text("batch 미정 — 재검증할 값이 없다"));
        return Ok(out);
    }
    let tokens = get(&profile, "kv_cache_tokens")?;
    if !tokens.is_truthy() {
        out.set(
            "reason",
            text("엔진 보고 kv_cache_tokens 미관측 — 재검증 불가(일치로 접지 않는다)"),
        );
        return Ok(out);
    }
    let fit = int_floor_div(&int_of(&tokens)?, &int_of(typical)?)?;
    out.set("kv_fit", PyValue::Int(fit.clone()));
    if fit < BigInt::one() {
        out.set(
            "reason",
            PyValue::Str(
                Message::new()
                    .lit("KV-fit 0 — 엔진 KV 토큰 ")
                    .int(&int_of(&tokens)?)
                    .lit(" 가 대표 길이 ")
                    .int(&int_of(typical)?)
                    .lit(" 보다 작다(판정 불가 · 선언을 확인하라)")
                    .done(),
            ),
        );
        return Ok(out);
    }
    out.set("checked", PyValue::Bool(true));
    let batch = get(&candidate, "batch")?;
    let exceeds = int_of(&batch)? > fit;
    out.set("exceeds", PyValue::Bool(exceeds));
    out.set(
        "reason",
        PyValue::Str(
            Message::new()
                .lit("batch ")
                .int(&int_of(&batch)?)
                .lit(" ")
                .lit(if exceeds { ">" } else { "≤" })
                .lit(" KV-fit ")
                .int(&fit)
                .lit(" (= 엔진 KV 토큰 ")
                .int(&int_of(&tokens)?)
                .lit(" ÷ L ")
                .int(&int_of(typical)?)
                .lit(")")
                .done(),
        ),
    );
    Ok(out)
}

// ── the error text: the two families of patterns, against code points ────────────────────────────

fn points_of(literal: &str) -> Vec<u32> {
    literal.chars().map(u32::from).collect()
}

fn find_literal(haystack: &[u32], needle: &[u32]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn at(points: &[u32], index: usize, literal: &[u32]) -> bool {
    points
        .get(index..)
        .is_some_and(|rest| rest.starts_with(literal))
}

/// `re.search(r"\bERROR\b.*(Error|Exception)", text)`: the word ERROR, then on the same line (a dot is anything but `\n`) the
/// text `Error` or `Exception`.
fn error_marker_on_a_line(points: &[u32]) -> bool {
    let (marker, error, exception) = (
        points_of("ERROR"),
        points_of("Error"),
        points_of("Exception"),
    );
    (0..points.len()).any(|start| {
        if !at(points, start, &marker) {
            return false;
        }
        let end = start + marker.len();
        let boundary_before = start == 0 || !is_python_word(points[start - 1]);
        let boundary_after = end == points.len() || !is_python_word(points[end]);
        if !(boundary_before && boundary_after) {
            return false;
        }
        let mut at_index = end;
        loop {
            if at(points, at_index, &error) || at(points, at_index, &exception) {
                return true;
            }
            if at_index >= points.len() || points[at_index] == u32::from('\n') {
                return false;
            }
            at_index += 1;
        }
    })
}

/// `re.search(r"^\s*\w*(Error|Exception):", text, re.MULTILINE)`: at the start of the text or of a line (after `\n` only),
/// white space (newlines included), word characters, then `Error:` or `Exception:`.
fn error_name_at_a_line_start(points: &[u32]) -> bool {
    let (error, exception) = (points_of("Error:"), points_of("Exception:"));
    let n = points.len();
    let mut start = 0;
    loop {
        let mut spaces_end = start;
        while spaces_end < n && is_python_space(points[spaces_end]) {
            spaces_end += 1;
        }
        for after_spaces in start..=spaces_end {
            let mut word_end = after_spaces;
            while word_end < n && is_python_word(points[word_end]) {
                word_end += 1;
            }
            for name_at in after_spaces..=word_end {
                if at(points, name_at, &error) || at(points, name_at, &exception) {
                    return true;
                }
            }
        }
        match points[start..]
            .iter()
            .position(|&point| point == u32::from('\n'))
        {
            Some(offset) => start += offset + 1,
            None => return false,
        }
    }
}

/// Whether the error excerpt shows an exception: a traceback header, an `ERROR ... Error|Exception` line, an exception name with
/// a colon at the start of a line, `CompilationError`, or the engine's initialization failure message.
pub fn has_exception(excerpt: &PyStr) -> bool {
    let points = excerpt.code_points();
    find_literal(points, &points_of("Traceback (most recent call last)"))
        || error_marker_on_a_line(points)
        || error_name_at_a_line_start(points)
        || find_literal(points, &points_of("CompilationError"))
        || find_literal(points, &points_of("Engine core initialization failed"))
}

/// The text `re.search` is given: only a string can be searched.
fn searchable(value: &PyValue) -> Res<&PyStr> {
    match value.peel() {
        PyValue::Str(text) => Ok(text),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                other.type_name()
            ),
        )),
    }
}

// ── arithmetic on the caller's budget and margin ─────────────────────────────────────────────────

/// A string or a list: what `*` repeats.
fn is_repeatable(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Str(_) | PyValue::List(_))
}

/// `value.__index__()` of an integer or a boolean as a signed index-sized integer (`PyNumber_AsSsize_t`): `None` for any other
/// type, the `OverflowError` of CPython for an integer outside the signed 64-bit range (whatever its sign).
fn index_of(count: &PyValue) -> Res<Option<i64>> {
    let number = match count.peel() {
        PyValue::Int(number) => number.clone(),
        PyValue::Bool(flag) => BigInt::from(u8::from(*flag)),
        _ => return Ok(None),
    };
    i64::try_from(number).map(Some).map_err(|_| {
        Failure::new(
            "OverflowError",
            "cannot fit 'int' into an index-sized integer",
        )
    })
}

/// The largest length CPython lets a sequence have (`PY_SSIZE_T_MAX`).
const MAX_LENGTH: usize = isize::MAX as usize;

/// `unit` repeated `times` times, built only after its whole length was reserved: `None` when that length cannot be addressed or
/// the allocation is refused (nothing is pushed in that case).
fn repeated<T: Clone>(unit: &[T], times: usize) -> Option<Vec<T>> {
    let mut out = Vec::new();
    out.try_reserve_exact(unit.len().checked_mul(times)?).ok()?;
    for _ in 0..times {
        out.extend_from_slice(unit);
    }
    Some(out)
}

/// `sequence * count` once `count` is a valid index. A negative count is zero repetitions and an empty sequence stays empty for any
/// count (nothing is built). Otherwise the result length is checked first, as CPython does: a string whose repetition would pass
/// the signed index limit is the `OverflowError` "repeated string is too long"; a list whose repetition would is the empty-message
/// `MemoryError`, which is also what a refused allocation of either is.
fn repeat_sequence(sequence: &PyValue, count: i64) -> Res<PyValue> {
    let memory_error = || Failure::new("MemoryError", "");
    let times = usize::try_from(count).unwrap_or(0);
    match sequence.peel() {
        PyValue::Str(own) => {
            let unit = own.code_points();
            if times == 0 || unit.is_empty() {
                return Ok(PyValue::Str(PyStr::default()));
            }
            if times > MAX_LENGTH / unit.len() {
                return Err(Failure::new("OverflowError", "repeated string is too long"));
            }
            let points = repeated(unit, times).ok_or_else(memory_error)?;
            Ok(PyValue::Str(PyStr::from_code_points(points)))
        }
        PyValue::List(items) => {
            if times == 0 || items.is_empty() {
                return Ok(PyValue::List(Vec::new()));
            }
            if items.len() > MAX_LENGTH / times {
                return Err(memory_error());
            }
            repeated(items, times)
                .map(PyValue::List)
                .ok_or_else(memory_error)
        }
        other => Ok(other.clone()),
    }
}

/// `left * right` as CPython evaluates it. Two numbers multiply as numbers, whatever their size (no index conversion is involved).
/// Otherwise a string or list on the left (and then on the right) is repeated by the other operand: that operand must be an
/// integer or boolean that fits a signed index-sized integer (`OverflowError` if it does not, checked before a negative count
/// becomes zero), or else the sequence's own `TypeError` names the other operand's type. Other operands are the common operator's
/// `TypeError`.
pub fn product(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    let (left, right) = (left.peel(), right.peel());
    for (sequence, other) in [(left, right), (right, left)] {
        if is_repeatable(sequence) {
            return match index_of(other)? {
                Some(count) => repeat_sequence(sequence, count),
                None => Err(Failure::new(
                    "TypeError",
                    &format!(
                        "can't multiply sequence by non-int of type '{}'",
                        other.type_name()
                    ),
                )),
            };
        }
    }
    multiply(left, right).map_err(exception)
}

fn fixed(value: &PyValue, places: u32) -> Res<String> {
    percent_fixed(value.peel(), places).map_err(exception)
}

fn fixed_float(number: f64, places: u32) -> String {
    agent_runtime_support::pynumber::format_fixed(number, places)
}

// ── classify ─────────────────────────────────────────────────────────────────────────────────────

fn set(result: &mut PyDict, class: &str, target: Option<&str>, note: PyStr) {
    result.set("failure_class", text(class));
    result.set("adjust_target", target.map_or(PyValue::None, text));
    result.set("note", PyValue::Str(note));
}

/// `classify(trial_result, budget_gib, safety_margin, typical_request_tokens=None)`: the failure class of one trial's result and
/// the variable to adjust. The result is `{failure_class, adjust_target, note}`, with `kv_fit_check` (and, when the batch is to be
/// lowered, `kv_fit`) when a representative request length is given and the trial passed.
pub fn classify(
    trial: &PyValue,
    budget_gib: &PyValue,
    safety_margin: &PyValue,
    typical: &PyValue,
) -> Res<PyDict> {
    let mut result = PyDict::new();
    set(&mut result, "unknown", None, PyStr::default());

    let load_ok = get(trial, "load_ok")?.is_truthy();
    let profile = or_empty_dict(get(trial, "vllm_profile")?);
    let functional = or_empty_dict(get(trial, "functional")?);
    let error_excerpt = {
        let given = get(trial, "error_excerpt")?;
        if given.is_truthy() {
            given
        } else {
            text("")
        }
    };
    let health_wait = or_empty_dict(get(trial, "health_wait")?);
    let wait_outcome = get(&health_wait, "outcome")?;
    let mut wait_note = PyStr::default();
    if is_text(&wait_outcome, "container_died") {
        let state = get(&health_wait, "container_state")?;
        let waited = get(&health_wait, "waited_s")?;
        let waited = if waited.is_truthy() {
            waited
        } else {
            PyValue::Float(0.0)
        };
        wait_note = Message::new()
            .lit(" [준비대기 종단=container_died(")
            .val(&state)
            .lit(", ")
            .lit(&fixed(&waited, 0)?)
            .lit("s 경과 — 타임아웃 소진 아님)]")
            .done();
    } else if is_text(&wait_outcome, "timeout") {
        let waited = get(&health_wait, "waited_s")?;
        let waited = if waited.is_truthy() {
            waited
        } else {
            PyValue::Float(0.0)
        };
        wait_note = Message::new()
            .lit(" [준비대기 종단=timeout(")
            .lit(&fixed(&waited, 0)?)
            .lit("s 전량 소진 — 컨테이너는 살아 있었다)]")
            .done();
    }

    // load failed
    if !load_ok {
        for signature in OOM_SIGNATURES {
            let excerpt = searchable(&error_excerpt)?;
            if find_literal(excerpt.code_points(), &points_of(signature)) {
                let note = Message::new()
                    .lit("OOM 시그니처 매칭('")
                    .lit(signature)
                    .lit("') → kv_cache_memory_bytes 축소 후 재시도.")
                    .text(&wait_note)
                    .done();
                set(&mut result, "vram_oom", Some("kv_cache_memory_bytes"), note);
                return Ok(result);
            }
        }
        let excerpt = searchable(&error_excerpt)?;
        let exception_shown = has_exception(excerpt);
        let kv_seen = profile_key(&profile, "kv_cache_gib")?;
        if let (Some(kv), false) = (kv_seen, exception_shown) {
            let weights_seen = profile_key(&profile, "weights_gib")?;
            let mut over = PyStr::default();
            if budget_gib.is_truthy() {
                let margin_or_one = if safety_margin.is_truthy() {
                    safety_margin.clone()
                } else {
                    PyValue::Float(1.0)
                };
                let safe = float_of(budget_gib)? * float_of(&margin_or_one)?;
                let weights = match weights_seen {
                    Some(value) if value != 0.0 => value,
                    _ => 0.0,
                };
                let resident = kv + weights;
                let basis = if weights_seen.is_some() {
                    "KV+가중치"
                } else {
                    "KV(가중치 실측 부재)"
                };
                let shown_resident = fixed_float(resident, 2);
                let shown_safe = fixed_float(safe, 1);
                if safe == 0.0 {
                    return Err(Failure::new("ZeroDivisionError", "float division by zero"));
                }
                let shown_percent = fixed_float(100.0 * resident / safe, 0);
                over = Message::new()
                    .lit(" 상주 ")
                    .lit(basis)
                    .lit(" ")
                    .lit(&shown_resident)
                    .lit(" GiB / 안전예산 ")
                    .lit(&shown_safe)
                    .lit(" GiB (")
                    .lit(&shown_percent)
                    .lit("%).")
                    .done();
            }
            let note = Message::new()
                .lit("load 실패 + OOM 예외 없음 + vllm_profile.kv_cache_gib=")
                .lit(&fixed_float(kv, 2))
                .lit(" GiB 존재 → 엔진이 KV 를 잡은 뒤 **외부에서 종료**됨(호스트 워치독 SIGKILL 계열).")
                .text(&over)
                .lit(" CUDA OOM 은 예외를 남기므로 이 조합은 CUDA OOM 이 아니다. kv_cache_memory_bytes 축소 후 재시도.")
                .text(&wait_note)
                .done();
            set(&mut result, "vram_oom", Some("kv_cache_memory_bytes"), note);
            return Ok(result);
        }
        let log_path = get(trial, "log_path")?;
        let mut why = PyStr::default();
        if exception_shown {
            why.push_str("엔진이 **예외를 남기고** 실패했다(외부 종료 아님) — 메모리 축이 아닐 가능성이 높다. 로그의 traceback 을 직접 읽어라. ");
            if let Some(kv) = kv_seen {
                why.push_str(&format!(
                    "참고: KV {} GiB 는 **정상 할당된 뒤** 실패했다 — KV 축소는 이 실패와 무관할 수 있다. ",
                    fixed_float(kv, 2)
                ));
            }
        }
        let mut note = Message::new()
            .lit("load 실패이나 알려진 OOM 시그니처 미매칭 → Model-C(HITL). ")
            .text(&why)
            .lit("시그니처 미매칭 = 미지 실패이지 불가 아님")
            .done();
        if log_path.is_truthy() {
            note.push_py(
                &Message::new()
                    .lit(" (raw log: ")
                    .val(&log_path)
                    .lit(")")
                    .done(),
            );
        }
        note.push_py(&wait_note);
        set(&mut result, "unknown", None, note);
        return Ok(result);
    }

    // load OK: the VRAM ceiling
    let weights_gib = profile_key(&profile, "weights_gib")?;
    let overhead_gib = profile_key(&profile, "non_kv_overhead_gib")?;
    let ceiling = product(budget_gib, safety_margin)?;
    if let (Some(weights), Some(overhead)) = (weights_gib, overhead_gib) {
        let total = weights + overhead;
        if compare(Compare::Greater, &PyValue::Float(total), ceiling.peel()).map_err(exception)? {
            let note = Message::new()
                .lit("weights+overhead=")
                .lit(&fixed_float(total, 2))
                .lit("GiB > ceiling=")
                .lit(&fixed(&ceiling, 2)?)
                .lit("GiB(budget*margin) → KV 조정 불가 구조적 초과(HITL).")
                .done();
            set(&mut result, "vram_infeasible", None, note);
            return Ok(result);
        }
    }

    // load OK, VRAM OK: the functional check
    if functional.is_truthy() && matches!(get(&functional, "passed")?, PyValue::Bool(false)) {
        result.set("failure_class", text("functional"));
        let (target, note) = if matches!(get(&functional, "completion_ok")?, PyValue::Bool(false)) {
            (
                "attention_backend",
                "completion 실패 → attention_backend 폴백 후보로 조정.",
            )
        } else if matches!(get(&functional, "tool_call_ok")?, PyValue::Bool(false)) {
            (
                "tool_call_parser",
                "tool_call 실패 → tool_call_parser 폴백 후보로 조정.",
            )
        } else if matches!(get(&functional, "reasoning_ok")?, PyValue::Bool(false)) {
            (
                "reasoning_parser",
                "reasoning 실패 → reasoning_parser 폴백 후보로 조정.",
            )
        } else {
            (
                "attention_backend",
                "functional 실패(세부 미상) → attention_backend 폴백 후보로 조정.",
            )
        };
        result.set("adjust_target", text(target));
        result.set("note", text(note));
        return Ok(result);
    }

    // everything is fine
    if functional.is_truthy() && matches!(get(&functional, "passed")?, PyValue::Bool(true)) {
        set(
            &mut result,
            "none",
            None,
            PyStr::new("load OK·VRAM OK·functional 통과 → 수렴."),
        );
        if typical.is_truthy() {
            let check = kv_fit_check(trial, typical)?;
            let reason = check.get("reason").cloned().unwrap_or(PyValue::None);
            let checked = check.get("checked").is_some_and(PyValue::is_truthy);
            let exceeds = check.get("exceeds").is_some_and(PyValue::is_truthy);
            let kv_fit = check.get("kv_fit").cloned().unwrap_or(PyValue::None);
            result.set("kv_fit_check", PyValue::Dict(check));
            if checked && exceeds {
                result.set("adjust_target", text("batch"));
                result.set("kv_fit", kv_fit);
                result.set(
                    "note",
                    PyValue::Str(
                        Message::new()
                            .lit("load OK·VRAM OK·functional 통과 — 단 KV-fit 재검증: ")
                            .val(&reason)
                            .lit(" → batch 를 KV-fit 으로 낮춰 재검증(실패 아님 · 산식 재조정).")
                            .done(),
                    ),
                );
            } else {
                let mut note = PyStr::new("load OK·VRAM OK·functional 통과 → 수렴.");
                note.push_py(
                    &Message::new()
                        .lit(" KV-fit 재검증: ")
                        .val(&reason)
                        .lit(".")
                        .done(),
                );
                result.set("note", PyValue::Str(note));
            }
        }
        return Ok(result);
    }

    set(
        &mut result,
        "unknown",
        None,
        PyStr::new("load OK·VRAM OK 이나 functional 판정 불가 → Model-C(HITL)."),
    );
    Ok(result)
}
