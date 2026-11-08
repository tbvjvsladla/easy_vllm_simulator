//! The run-side arithmetic of `run_bench.sh`: the warm-up fraction, the prefix-cache counters read from the server's
//! metrics text and the record of their difference over a run.

use super::document::write_pretty;
use super::{Console, Context, Done, Stop};
use crate::guidellm::Failure;
use crate::pydyn::dict_of;
use crate::pynumber::{float_from_str, int_from_float, int_from_str, round_to_places};
use crate::pystr::{is_python_space, PyStr};
use crate::pyvalue::{float_repr, PyDict, PyValue};
use crate::repeat_text::cat;
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};

fn usage_error(console: &mut Console, what: &str) -> Stop {
    console.warn(&format!("[bench_shell] ERROR {what}\n"));
    Stop::Exit(2)
}

/// The diagnostic of every refused `warm-fraction` input (after `[bench_shell] ERROR `).
const WARM_FRACTION_GRAMMAR: &str = "warm-fraction 은 10진 정수 둘(WARMUPS TOTAL_REQ)만 계산한다";

/// A count of the declared grammar `[+-]?[0-9]+`: ASCII digits only, one optional sign, any number of digits (leading
/// zeros are decimal, so `01` is 1). Anything else is nothing: no expression, name, hex, float, exponent, underscore,
/// whitespace or non-ASCII digit is ever evaluated or guessed at.
fn count(text: &PyStr) -> Option<BigInt> {
    let shown = text.to_utf8()?;
    let digits = shown.strip_prefix(['+', '-']).unwrap_or(&shown);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let magnitude: BigInt = digits.parse().ok()?;
    Some(if shown.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}

/// `round(WARMUPS / max(TOTAL_REQ, 1), 4)` for exactly two literal counts.
///
/// The original pasted both values into Python source, so a leading-zero count was a `SyntaxError` and any non-empty
/// text was evaluated. Under the recorded value contract the two arguments are plain decimal integers (see [`count`]):
/// the quotient is the exact integer true division (`OverflowError` when it does not fit a float), `max(total, 1)`,
/// Python's rounding of the binary double to four places and its float `repr`. Any other input (another argument count
/// included) prints nothing, says so on stderr and ends with status 2; nothing is evaluated and no Python is started.
pub(super) fn warm_fraction(context: &Context<'_>, console: &mut Console) -> Done {
    let counts = if context.args.len() == 2 {
        count(context.argument(0)?).zip(count(context.argument(1)?))
    } else {
        None
    };
    let Some((warmups, total)) = counts else {
        return Err(usage_error(console, WARM_FRACTION_GRAMMAR));
    };
    let fraction = true_divide(&warmups, &total.max(BigInt::from(1)))?;
    console.print(PyStr::new(&float_repr(round_to_places(fraction, 4))));
    Ok(())
}

/// `int / int` as Python's `long_true_divide`: the correctly rounded quotient, `OverflowError` when it does not fit.
pub(crate) fn true_divide(numerator: &BigInt, denominator: &BigInt) -> Result<f64, Failure> {
    if denominator.is_zero() {
        return Err(Failure::new("ZeroDivisionError", "division by zero"));
    }
    let negative = numerator.is_negative() != denominator.is_negative();
    let (top, bottom) = (numerator.abs(), denominator.abs());
    if top.is_zero() {
        return Ok(if negative { -0.0 } else { 0.0 });
    }
    // Scale so that the integer quotient has at least 66 significant bits, then round once: the remainder is folded
    // into the lowest bit (a sticky bit far below the 53 that survive), so the conversion rounds the true quotient.
    let shift = 66 + bottom.bits() as i64 - top.bits() as i64;
    let (scaled_top, scaled_bottom) = if shift >= 0 {
        (top << shift as usize, bottom)
    } else {
        (top, bottom << (-shift) as usize)
    };
    let quotient = &scaled_top / &scaled_bottom;
    let mut sticky = quotient.clone() | BigInt::from(1);
    if (&scaled_top % &scaled_bottom).is_zero() {
        sticky = quotient;
    }
    let value = sticky.to_f64().unwrap_or(f64::INFINITY);
    let exponent = -shift;
    if value.is_infinite() || !(-2000..=1100).contains(&exponent) {
        return Err(Failure::new(
            "OverflowError",
            "integer division result too large for a float",
        ));
    }
    let scaled = scale_by_power_of_two(value, exponent);
    if scaled.is_infinite() {
        return Err(Failure::new(
            "OverflowError",
            "integer division result too large for a float",
        ));
    }
    Ok(if negative { -scaled } else { scaled })
}

/// `value * 2**exponent` without an intermediate overflow or underflow for the exponents that matter here.
fn scale_by_power_of_two(value: f64, exponent: i64) -> f64 {
    let mut result = value;
    let mut left = exponent;
    while left != 0 {
        let step = left.clamp(-1000, 1000);
        result *= 2f64.powi(step as i32);
        left -= step;
    }
    result
}

/// What `re.findall(r"^vllm:NAME\{[^}]*\} ([0-9.eE+-]+)$", text, re.M)` returns: the value of every line of the
/// metric (a label block may span lines, as `[^}]` accepts a newline).
fn metric_values(text: &[u32], name: &str) -> Vec<PyStr> {
    let literal: Vec<u32> = format!("vllm:{name}{{").chars().map(u32::from).collect();
    let (newline, close) = (u32::from('\n'), u32::from('}'));
    let is_value_char = |point: u32| {
        char::from_u32(point).is_some_and(|c| c.is_ascii_digit() || ".eE+-".contains(c))
    };
    let mut found = Vec::new();
    let mut at = 0;
    while at <= text.len() {
        let line_start = at == 0 || text[at - 1] == newline;
        if line_start && text[at..].starts_with(&literal) {
            let after_open = at + literal.len();
            if let Some(offset) = text[after_open..].iter().position(|&point| point == close) {
                let mut cursor = after_open + offset + 1;
                if text.get(cursor) == Some(&u32::from(' ')) {
                    cursor += 1;
                    let start = cursor;
                    while text.get(cursor).is_some_and(|&point| is_value_char(point)) {
                        cursor += 1;
                    }
                    if cursor > start && (cursor == text.len() || text[cursor] == newline) {
                        found.push(PyStr::from_code_points(text[start..cursor].to_vec()));
                        at = cursor;
                        continue;
                    }
                }
            }
        }
        at += 1;
    }
    found
}

/// `sum(float(x) for x in values) if values else None`, with the compensated summation of CPython 3.12.
fn float_sum(values: &[PyStr]) -> Result<Option<f64>, Failure> {
    let mut numbers = Vec::new();
    for value in values {
        numbers.push(float_from_str(value).map_err(Failure::from)?);
    }
    let Some((&first, rest)) = numbers.split_first() else {
        return Ok(None);
    };
    let (mut total, mut compensation) = (0.0 + first, 0.0_f64);
    for &item in rest {
        let next = total + item;
        if total.abs() >= item.abs() {
            compensation += (total - next) + item;
        } else {
            compensation += (item - next) + total;
        }
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    Ok(Some(total))
}

/// `prefix-snapshot`: `"queries hits requests"` from the metrics text on standard input, or nothing when any of the
/// three counters is absent.
pub(super) fn prefix_snapshot(context: &Context<'_>, console: &mut Console) -> Done {
    let text = context.stdin.code_points();
    let queries = float_sum(&metric_values(text, "prefix_cache_queries_total"))?;
    let hits = float_sum(&metric_values(text, "prefix_cache_hits_total"))?;
    let requests = float_sum(&metric_values(text, "request_success_total"))?;
    let (Some(queries), Some(hits), Some(requests)) = (queries, hits, requests) else {
        console.print(PyStr::default());
        return Ok(());
    };
    let whole = |value: f64| int_from_float(value).map_err(Failure::from);
    console.print(PyStr::new(&format!(
        "{} {} {}",
        whole(queries)?,
        whole(hits)?,
        whole(requests)?
    )));
    Ok(())
}

/// `a, b, c = map(int, text.split())`.
fn three_counts(text: &PyStr) -> Result<[BigInt; 3], Failure> {
    let mut words: Vec<PyStr> = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    for &point in text.code_points() {
        if is_python_space(point) {
            if !current.is_empty() {
                words.push(PyStr::from_code_points(std::mem::take(&mut current)));
            }
        } else {
            current.push(point);
        }
    }
    if !current.is_empty() {
        words.push(PyStr::from_code_points(current));
    }
    let mut numbers = Vec::new();
    for word in words.iter().take(4) {
        numbers.push(int_from_str(word).map_err(Failure::from)?);
    }
    match numbers.len() {
        3 => {
            let mut taken = numbers.into_iter();
            Ok([
                taken.next().unwrap(),
                taken.next().unwrap(),
                taken.next().unwrap(),
            ])
        }
        n if n > 3 => Err(Failure::new(
            "ValueError",
            "too many values to unpack (expected 3)",
        )),
        n => Err(Failure::new(
            "ValueError",
            &format!("not enough values to unpack (expected 3, got {n})"),
        )),
    }
}

/// `prefix-record OUT CONFIG BEFORE AFTER SINCE`: the run's prefix-cache difference as a record file and one line.
pub(super) fn prefix_record(context: &Context<'_>, console: &mut Console) -> Done {
    if context.args.len() != 5 {
        let count = context.args.len();
        return Err(Failure::new(
            "ValueError",
            &if count > 5 {
                "too many values to unpack (expected 5)".to_string()
            } else {
                format!("not enough values to unpack (expected 5, got {count})")
            },
        )
        .into());
    }
    let [output, config, before, after, since] = [0, 1, 2, 3, 4].map(|i| context.args[i].clone());
    let mut record = PyDict::new();
    record.set("schema_version", PyValue::int(1));
    record.set("config", PyValue::Str(config));
    record.set("window_start_utc", PyValue::Str(since));
    record.set(
        "source",
        PyValue::str(
            "vllm /metrics prefix_cache_queries_total·prefix_cache_hits_total·request_success_total 실행 전후 차분(서버 전체 카운터)",
        ),
    );
    if !before.is_empty() && !after.is_empty() {
        let [queries_before, hits_before, requests_before] = three_counts(&before)?;
        let [queries_after, hits_after, requests_after] = three_counts(&after)?;
        let (queries, hits, requests) = (
            queries_after - queries_before,
            hits_after - hits_before,
            requests_after - requests_before,
        );
        record.set("provenance", PyValue::str("measured"));
        record.set("queries_delta", PyValue::int(queries.clone()));
        record.set("hits_delta", PyValue::int(hits.clone()));
        record.set("requests_delta", PyValue::int(requests));
        let rate = if queries.sign() == Sign::Plus {
            PyValue::Float(round_to_places(true_divide(&hits, &queries)?, 4))
        } else {
            PyValue::None
        };
        record.set("hit_rate_run", rate);
    } else {
        record.set("provenance", PyValue::str("unobserved"));
        record.set(
            "reason",
            PyValue::str("/metrics 수집 실패(전 또는 후) — 값을 추정하지 않는다"),
        );
    }
    let record = PyValue::Dict(record);
    write_pretty(&output, &record, false)?;
    let members = dict_of(&record).expect("the record is an object");
    let mut shown = PyDict::new();
    for key in ["provenance", "hit_rate_run", "requests_delta"] {
        shown.set(key, members.get(key).cloned().unwrap_or(PyValue::None));
    }
    console.print(cat!(
        "[run_bench] prefix cache(실행 차분): ",
        PyValue::Dict(shown).display_text()
    ));
    Ok(())
}
