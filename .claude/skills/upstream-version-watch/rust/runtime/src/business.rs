//! `resolve_build_track.py`: the build-track proposal, the torch CUDA arch mapping and the module's small helpers, as the original computes them
//! (Python truthiness, `str()`, tuple comparison, `str.isdigit`/`isalpha`/`strip`, `int()` of digit strings).

use crate::pytext::{int_of_digits, is_alpha, is_digit, py_str, text_of};
use crate::thrown::{Done, Thrown};
use agent_runtime_support::pydyn::as_get_target;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use recipe_runtime::escalation::Raised;

/// `SOURCE_BUILD_MIN`: torch from this (major, minor) on is proposed as `source-build`.
pub const SOURCE_BUILD_MIN: (i64, i64) = (2, 11);

/// `_MANIFEST_SM_KEYS`: the manifest keys that may hold the compute capability, in priority order.
pub const MANIFEST_SM_KEYS: [&str; 4] = [
    "compute_capability",
    "sm_arch",
    "gpu_compute_capability",
    "cuda_arch",
];

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

/// `SOURCE_BUILD_MIN` as the value Python holds.
pub fn source_build_min() -> PyValue {
    PyValue::Tuple(vec![
        PyValue::int(SOURCE_BUILD_MIN.0),
        PyValue::int(SOURCE_BUILD_MIN.1),
    ])
}

/// `_MANIFEST_SM_KEYS` as the value Python holds.
pub fn manifest_sm_keys() -> PyValue {
    PyValue::Tuple(MANIFEST_SM_KEYS.iter().map(|key| text(key)).collect())
}

/// `norm_release(v)`: `(major, minor)` of a torch pin, `(major, 0)` for one number, `None` when nothing parses or `v` is falsy. Only the first two
/// numeric parts are read; a character that is a digit but not a decimal digit makes `int()` raise where it is read.
pub fn norm_release(v: &PyValue) -> Done<PyValue> {
    if !v.is_truthy() {
        return Ok(PyValue::None);
    }
    let head = py_str(v).strip();
    let mut digits: Vec<BigInt> = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    for &ch in head.code_points() {
        if is_digit(ch) {
            current.push(ch);
        } else if ch == u32::from('.') && !current.is_empty() {
            digits.push(int_of_digits(&current)?);
            current.clear();
            if digits.len() == 2 {
                break;
            }
        } else {
            break;
        }
    }
    if !current.is_empty() && digits.len() < 2 {
        digits.push(int_of_digits(&current)?);
    }
    Ok(match digits.as_slice() {
        [major, minor, ..] => PyValue::Tuple(vec![
            PyValue::Int(major.clone()),
            PyValue::Int(minor.clone()),
        ]),
        [major] => PyValue::Tuple(vec![PyValue::Int(major.clone()), PyValue::int(0)]),
        [] => PyValue::None,
    })
}

fn release_pair(release: &PyValue) -> Option<(BigInt, BigInt)> {
    match release {
        PyValue::Tuple(items) => match items.as_slice() {
            [PyValue::Int(major), PyValue::Int(minor)] => Some((major.clone(), minor.clone())),
            _ => None,
        },
        _ => None,
    }
}

fn dict(entries: Vec<(&str, PyValue)>) -> PyDict {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value);
    }
    map
}

/// `propose_track(torch_pin)`: `{"decision", "rationale"}` (a proposal; the smoke arbitrates).
pub fn propose_track(torch_pin: &PyValue) -> Done<PyValue> {
    let release = norm_release(torch_pin)?;
    let Some((major, minor)) = release_pair(&release) else {
        let mut rationale = PyStr::new("torch 핀 '");
        rationale.push_py(&py_str(torch_pin));
        rationale.push_str("' 파싱 실패 — 보수적으로 source-build 제안. 스모크가 최종 중재.");
        return Ok(PyValue::Dict(dict(vec![
            ("decision", text("source-build")),
            ("rationale", PyValue::Str(rationale)),
        ])));
    };
    let (min_major, min_minor) = (
        BigInt::from(SOURCE_BUILD_MIN.0),
        BigInt::from(SOURCE_BUILD_MIN.1),
    );
    let source_build = (major.clone(), minor.clone()) >= (min_major, min_minor);
    let (decision, rationale) = if source_build {
        (
            "source-build",
            format!(
                "torch {major}.{minor} >= {}.{} → 하드 ABI 벽(_C 를 NGC torch 에 링크) → source-build 제안. 최종 변별자=(NGC 베이스/실-링크 torch)×vLLM source version, 스모크가 중재.",
                SOURCE_BUILD_MIN.0, SOURCE_BUILD_MIN.1
            ),
        )
    } else {
        (
            "wheel",
            format!(
                "torch {major}.{minor} < {}.{} → pre-built wheel 트랙 제안(--no-deps 설치). 스모크가 중재.",
                SOURCE_BUILD_MIN.0, SOURCE_BUILD_MIN.1
            ),
        )
    };
    Ok(PyValue::Dict(dict(vec![
        ("decision", text(decision)),
        ("rationale", text(&rationale)),
    ])))
}

/// `map_torch_cuda_arch(sm)`: `sm_121a` → `12.1a`, `sm_90a` → `9.0a`, `89` → `8.9`; a dotted value passes; the last character of a digit string is the
/// minor version as written; anything unrecognised is returned as `str(sm).strip()`.
pub fn map_torch_cuda_arch(sm: &PyValue) -> Done<PyValue> {
    if !sm.is_truthy() {
        return Ok(PyValue::None);
    }
    let original = py_str(sm).strip();
    let mut s: Vec<u32> = original.code_points().to_vec();
    // `s.lower().startswith("sm_")`: only ASCII `S`/`M` lower to `s`/`m`
    if s.len() >= 3
        && matches!(s[0], 0x53 | 0x73)
        && matches!(s[1], 0x4D | 0x6D)
        && s[2] == u32::from('_')
    {
        s.drain(..3);
    }
    let mut suffix: Vec<u32> = Vec::new();
    if let Some(&last) = s.last() {
        if is_alpha(last) {
            suffix.push(last);
            s.pop();
        }
    }
    let join = |parts: &[&[u32]]| text_of(&parts.concat());
    let dot = [u32::from('.')];
    if s.contains(&dot[0]) {
        return Ok(PyValue::Str(join(&[&s, &suffix])));
    }
    if !s.is_empty() && s.iter().all(|&c| is_digit(c)) {
        if s.len() >= 2 {
            let (head, last) = (&s[..s.len() - 1], &s[s.len() - 1..]);
            let major = int_of_digits(head)?;
            let mut out = PyStr::new(&format!("{major}."));
            out.push_py(&join(&[last, &suffix]));
            return Ok(PyValue::Str(out));
        }
        let mut out = join(&[&s]);
        out.push_str(".0");
        out.push_py(&text_of(&suffix));
        return Ok(PyValue::Str(out));
    }
    Ok(PyValue::Str(original))
}

/// `sm_from_manifest(manifest)`: `(str(value), key)` of the first truthy alias in priority order, else `(None, None)`; a manifest that is not a mapping has
/// no `.get`.
pub fn sm_from_manifest(manifest: &PyValue) -> Done<PyValue> {
    for key in MANIFEST_SM_KEYS {
        let map =
            as_get_target(manifest).map_err(|failure| Thrown::plain(Raised::from(failure)))?;
        if let Some(value) = map.get(key) {
            if value.is_truthy() {
                return Ok(PyValue::Tuple(vec![PyValue::Str(py_str(value)), text(key)]));
            }
        }
    }
    Ok(PyValue::Tuple(vec![PyValue::None, PyValue::None]))
}

/// `resolve(torch_pin, sm_arch)`: the fragment merged into `resolved.json`.
pub fn resolve(torch_pin: &PyValue, sm_arch: &PyValue) -> Done<PyValue> {
    let PyValue::Dict(mut track) = propose_track(torch_pin)? else {
        unreachable!("propose_track returns a dict")
    };
    track.set("arbiter", text("smoke"));
    let arch = map_torch_cuda_arch(sm_arch)?;
    Ok(PyValue::Dict(dict(vec![
        ("build_track", PyValue::Dict(track)),
        (
            "source_build",
            PyValue::Dict(dict(vec![("torch_cuda_arch", arch)])),
        ),
    ])))
}

/// `_require(condition, message)`: `AssertionError(message)` unless the condition is truthy.
pub fn require(condition: &PyValue, message: &PyValue) -> Done<PyValue> {
    if condition.is_truthy() {
        return Ok(PyValue::None);
    }
    let mut raised = Raised::new("AssertionError", "builtins", py_str(message));
    raised.module = "builtins".to_string();
    Err(Thrown::plain(raised))
}

/// `_strip_inline_comment(val)`: `val` without a `#` that is outside quotes and follows a space (not at index 0). The quote state toggles on
/// the same quote character and opens on either; there is no escape.
pub fn strip_inline_comment(val: &PyValue) -> Done<PyValue> {
    match val.peel() {
        PyValue::Str(text) => {
            let points = text.code_points();
            let mut quote: Option<u32> = None;
            for (index, &ch) in points.iter().enumerate() {
                if ch == u32::from('\'') || ch == u32::from('"') {
                    quote = if quote == Some(ch) {
                        None
                    } else {
                        quote.or(Some(ch))
                    };
                } else if ch == u32::from('#')
                    && quote.is_none()
                    && index > 0
                    && points[index - 1] == u32::from(' ')
                {
                    return Ok(PyValue::Str(text_of(&points[..index])));
                }
            }
            Ok(val.clone())
        }
        PyValue::List(items) | PyValue::Tuple(items) => {
            let is_text = |item: &PyValue, wanted: &str| matches!(item.peel(), PyValue::Str(text) if text.equals(wanted));
            let mut quote: Option<PyValue> = None;
            for (index, item) in items.iter().enumerate() {
                if is_text(item, "'") || is_text(item, "\"") {
                    quote = match &quote {
                        Some(open) if open == item => None,
                        Some(open) => Some(open.clone()),
                        None => Some(item.clone()),
                    };
                } else if is_text(item, "#")
                    && quote.is_none()
                    && index > 0
                    && is_text(&items[index - 1], " ")
                {
                    let head = items[..index].to_vec();
                    return Ok(if matches!(val.peel(), PyValue::Tuple(_)) {
                        PyValue::Tuple(head)
                    } else {
                        PyValue::List(head)
                    });
                }
            }
            Ok(val.clone())
        }
        PyValue::Dict(map) => {
            // iterating a dict is its keys; `val[i - 1]` is a lookup by an integer, which a JSON object never holds
            let keys: Vec<PyValue> = map
                .iter()
                .map(|(key, _)| PyValue::Str(key.clone()))
                .collect();
            let mut quote: Option<PyValue> = None;
            for (index, key) in keys.iter().enumerate() {
                let is = |wanted: &str| matches!(key, PyValue::Str(text) if text.equals(wanted));
                if is("'") || is("\"") {
                    quote = match &quote {
                        Some(open) if open == key => None,
                        Some(open) => Some(open.clone()),
                        None => Some(key.clone()),
                    };
                } else if is("#") && quote.is_none() && index > 0 {
                    return Err(Thrown::builtin("KeyError", &(index - 1).to_string()));
                }
            }
            Ok(val.clone())
        }
        other => Err(Thrown::builtin(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}
