//! The first scan-node migration phase (P1): the pure and file-reading business of `scan_node.py`, over Python-compatible values
//! ([`PyValue`]) exactly as the original's callers hand them in. P1 is sixteen public names and their reachable helpers: `valid_host`,
//! `_ver_tuple`, `_yaml_str`, `parse_gpu_memory_line`, `check_model_env`, `derive_platform_preset`, `assert_homogeneity`,
//! `default_env_interconnect_path`, `emit_gate`, `evaluate_gate`, `emission_blockers` (this file), `build_sub_manifest` and
//! `emit_manifest_block` ([`crate::scan_node_manifest`]), `cross_validate`, `read_manifest_field` and `read_manifest_topology`
//! ([`crate::scan_node_files`]).
//!
//! **Not the whole scanner.** The hardware probes, the SSH and network collection, the argument parser and `main` of the original are
//! other phases; this crate neither runs a process nor opens a socket. The two boundaries a P1 function touches are explicit arguments:
//! the files it reads ([`crate::scan_node_files::Files`]) and the local clock hour ([`crate::scan_node_manifest::Clock`]); the pure
//! functions take neither.
//!
//! **Python behaviour reproduced, not fixed.** Truthiness chains (`a or b or c` returns the operand), `is False` identity, `in` over
//! closed tuples by equality, `isinstance` with `bool` as an `int`, dict insertion order, f-string `str()` of any value, and the
//! exceptions CPython raises for a shape the original did not guard (`AttributeError` on `.get`, `KeyError`, `TypeError`).

use agent_runtime_support::failure::Failure as TextFailure;
use agent_runtime_support::pydyn::Text;
use agent_runtime_support::pynumber::{int_from_str, PyException};
use agent_runtime_support::pyops::{compare, num_of, Compare};
use agent_runtime_support::pysort::{py_eq, sorted_by_key};
use agent_runtime_support::pystr::{decimal_digit_value, is_decimal_digit, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// `TOKEN_RE`, `PRESET_RE`, `CUDA_RE`, `HOST_LABEL_RE` are character-class patterns used with `fullmatch`; they are written as predicates.
pub const NCCL_KEYS: [&str; 3] = ["NCCL_IB_HCA", "NCCL_IB_GID_INDEX", "NCCL_SOCKET_IFNAME"];
pub const PLATFORM_PRESET_BY_GPU: [(&str, &str); 1] = [("GB10", "dgx-spark-gb10")];
pub const PLATFORM_PRESET_DEFAULT: &str = "generic";
const NVSMI_ABSENT: [&str; 5] = ["[n/a]", "n/a", "not supported", "[not supported]", ""];

/// An exception the original raises, as data the importer rebuilds: the class, its constructor arguments, and the `__cause__` of a
/// `raise ... from`. `context_is_cause` records that the exception was raised inside the handler of its cause (so `__context__` is the
/// same object). Frames are not carried (the importer's traceback is the adapter's, not the original's).
#[derive(Clone, Debug, PartialEq)]
pub struct Raised {
    pub class: &'static str,
    pub message: PyStr,
    pub ctor: Vec<PyValue>,
    /// Keyword arguments of the constructor (`AttributeError(msg, name=..., obj=...)`); `obj` is the actual object whose attribute
    /// access failed, carried as a value (a caller's object travels as the shared-identity plan of the bridge, never as a copy).
    pub ctor_kwargs: Vec<(String, PyValue)>,
    pub cause: Option<Box<Raised>>,
}

impl Raised {
    /// A built-in exception constructed from one message (`AttributeError`, `TypeError`, `ValueError`, `OverflowError`).
    pub fn with_message(class: &'static str, message: PyStr) -> Raised {
        Raised {
            class,
            ctor: vec![PyValue::Str(message.clone())],
            ctor_kwargs: Vec::new(),
            message,
            cause: None,
        }
    }

    /// `AttributeError("'T' object has no attribute 'name'", name=name, obj=obj)` as CPython 3.12 raises it for a failed attribute read.
    pub fn attribute(obj: &PyValue, name: &str) -> Raised {
        let message = PyStr::new(&format!(
            "'{}' object has no attribute '{name}'",
            obj.type_name()
        ));
        Raised {
            class: "AttributeError",
            ctor: vec![PyValue::Str(message.clone())],
            ctor_kwargs: vec![
                ("name".to_string(), PyValue::str(name)),
                ("obj".to_string(), obj.clone()),
            ],
            message,
            cause: None,
        }
    }
}

/// How a P1 call ends other than by returning: the original raises, or the shape is not modelled (reported before the answer is built;
/// nothing is guessed).
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    Raised(Raised),
    Unsupported(String),
}

pub type Res<T> = Result<T, Failure>;

impl From<TextFailure> for Failure {
    fn from(error: TextFailure) -> Self {
        Failure::Raised(Raised::with_message(error.class, error.message))
    }
}

impl From<PyException> for Failure {
    fn from(error: PyException) -> Self {
        Failure::Raised(Raised::with_message(
            error.class,
            PyStr::new(&error.message),
        ))
    }
}

pub fn raise(class: &'static str, message: &str) -> Failure {
    Failure::Raised(Raised::with_message(class, PyStr::new(message)))
}

/// The `AttributeError` of `obj.name` where `obj` has no such attribute.
pub fn attribute_error(obj: &PyValue, name: &str) -> Failure {
    Failure::Raised(Raised::attribute(obj, name))
}

pub fn unsupported(reason: &str) -> Failure {
    Failure::Unsupported(reason.to_string())
}

// ── small Python operations over values ─────────────────────────────────────────────────────────────────────────────────────────

pub(crate) fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

pub(crate) fn none_if_absent(value: Option<&PyValue>) -> PyValue {
    value.cloned().unwrap_or(PyValue::None)
}

/// `value.get(key)` (`AttributeError` when the value is not a dict).
pub(crate) fn dict_get(value: &PyValue, key: &str) -> Res<PyValue> {
    match value.peel() {
        PyValue::Dict(map) => Ok(none_if_absent(map.get(key))),
        _ => Err(attribute_error(value, "get")),
    }
}

/// `value.get(key, default)`.
pub(crate) fn dict_get_or(value: &PyValue, key: &str, default: PyValue) -> Res<PyValue> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map.get(key).cloned().unwrap_or(default)),
        _ => Err(attribute_error(value, "get")),
    }
}

/// `value[key]` for a string key (`KeyError(key)` when absent; the subscript errors of the other types).
pub(crate) fn subscript(value: &PyValue, key: &str) -> Res<PyValue> {
    match value.peel() {
        PyValue::Dict(map) => match map.get(key) {
            Some(found) => Ok(found.clone()),
            None => Err(Failure::Raised(Raised {
                class: "KeyError",
                message: PyStr::new(&PyStr::new(key).repr()),
                ctor: vec![text(key)],
                ctor_kwargs: Vec::new(),
                cause: None,
            })),
        },
        other => Err(agent_runtime_support::pydyn::subscript(other, key)
            .err()
            .map(Failure::from)
            .unwrap_or_else(|| raise("TypeError", "unsubscriptable"))),
    }
}

/// `a or b`: the first operand when it is truthy, otherwise the second.
pub(crate) fn or(left: PyValue, right: impl FnOnce() -> Res<PyValue>) -> Res<PyValue> {
    if left.is_truthy() {
        Ok(left)
    } else {
        right()
    }
}

/// `str(value)` of any value (an f-string field without a format spec).
pub(crate) fn shown(value: &PyValue) -> PyStr {
    value.display_text()
}

pub(crate) fn as_str(value: &PyValue) -> Option<&PyStr> {
    match value.peel() {
        PyValue::Str(text) => Some(text),
        _ => None,
    }
}

/// `value == "literal"` for a value of any type.
pub(crate) fn equals_text(value: &PyValue, literal: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(literal))
}

/// `value in ("a", "b", ...)`.
pub(crate) fn in_texts(value: &PyValue, literals: &[&str]) -> bool {
    literals.iter().any(|literal| equals_text(value, literal))
}

/// `value is False`.
pub(crate) fn is_false(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Bool(false))
}

pub(crate) fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// `isinstance(value, int) and not isinstance(value, bool)`.
pub(crate) fn plain_int(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Int(_))
}

/// `value >= 0` / `value < 1` etc. for an `int` value (cannot raise).
fn int_at_least(value: &PyValue, bound: i64) -> bool {
    compare(Compare::GreaterEqual, value.peel(), &PyValue::int(bound)).unwrap_or(false)
}

pub(crate) fn dict_of(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

pub(crate) fn list_of_texts(items: &[PyStr]) -> PyValue {
    PyValue::List(items.iter().cloned().map(PyValue::Str).collect())
}

fn is_ascii_alnum(point: u32) -> bool {
    char::from_u32(point).is_some_and(|c| c.is_ascii_alphanumeric())
}

/// `TOKEN_RE.fullmatch(text)`: `[A-Za-z0-9_.:-]+`.
pub(crate) fn token_matches(text: &PyStr) -> bool {
    !text.is_empty()
        && text.code_points().iter().all(|&point| {
            is_ascii_alnum(point) || matches!(char::from_u32(point), Some('_' | '.' | ':' | '-'))
        })
}

/// `PRESET_RE.fullmatch(text)`: `[A-Za-z0-9][A-Za-z0-9._-]*`.
pub(crate) fn preset_matches(text: &PyStr) -> bool {
    let points = text.code_points();
    !points.is_empty()
        && is_ascii_alnum(points[0])
        && points[1..].iter().all(|&point| {
            is_ascii_alnum(point) || matches!(char::from_u32(point), Some('.' | '_' | '-'))
        })
}

/// `CUDA_RE.fullmatch(text)`: `[0-9]{2,4}`.
pub(crate) fn cuda_matches(text: &PyStr) -> bool {
    let points = text.code_points();
    (2..=4).contains(&points.len()) && points.iter().all(|&point| (0x30..=0x39).contains(&point))
}

/// `HOST_LABEL_RE.fullmatch(label)`: `[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?`.
fn host_label_matches(label: &[u32]) -> bool {
    let alnum = |point: u32| is_ascii_alnum(point);
    match label.len() {
        0 => false,
        1 => alnum(label[0]),
        length => {
            length <= 63
                && alnum(label[0])
                && alnum(label[length - 1])
                && label[1..length - 1]
                    .iter()
                    .all(|&point| alnum(point) || point == u32::from(b'-'))
        }
    }
}

/// `ipaddress.ip_address(text)` succeeds (IPv4 dotted quad, or IPv6 with the optional `%scope` of Python 3.9+).
fn is_ip_address(text: &PyStr) -> bool {
    let Some(utf8) = text.to_utf8() else {
        return false;
    };
    if utf8.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    let (address, scope) = match utf8.split_once('%') {
        Some((address, scope)) => (address, Some(scope)),
        None => (utf8.as_str(), None),
    };
    if let Some(scope) = scope {
        // `_split_scope_id`: the scope id is non-empty and holds no second `%`; Python also refuses `/` in it.
        if scope.is_empty() || scope.contains('%') || scope.contains('/') {
            return false;
        }
    }
    address.parse::<std::net::Ipv6Addr>().is_ok()
}

// ── pure P1 functions ───────────────────────────────────────────────────────────────────────────────────────────────────────────

/// `valid_host(value)`: an IP address, or dot-separated RFC 1123 labels (one optional trailing dot), at most 253 characters.
pub fn valid_host(value: &PyValue) -> bool {
    let Some(host) = as_str(value) else {
        return false;
    };
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if is_ip_address(host) {
        return true;
    }
    let points = host.code_points();
    let body = if points.last() == Some(&u32::from(b'.')) {
        &points[..points.len() - 1]
    } else {
        points
    };
    body.split(|&point| point == u32::from(b'.'))
        .all(host_label_matches)
}

/// `_ver_tuple(s)`: the first two `\d+` runs of `str(s)` as an int tuple; `None` for a falsy value or text without digits.
pub fn ver_tuple(value: &PyValue) -> Res<PyValue> {
    if !value.is_truthy() {
        return Ok(PyValue::None);
    }
    let shown_text = shown(value);
    let points = shown_text.code_points();
    let mut runs: Vec<PyStr> = Vec::new();
    let mut index = 0;
    while index < points.len() && runs.len() < 2 {
        if is_decimal_digit(points[index]) {
            let start = index;
            while index < points.len() && is_decimal_digit(points[index]) {
                index += 1;
            }
            runs.push(PyStr::from_code_points(points[start..index].to_vec()));
        } else {
            index += 1;
        }
    }
    if runs.is_empty() {
        return Ok(PyValue::None);
    }
    let mut numbers = Vec::new();
    for run in &runs {
        numbers.push(PyValue::Int(int_from_str(run)?));
    }
    Ok(PyValue::Tuple(numbers))
}

/// `_yaml_str(value)`: `json.dumps(value, ensure_ascii=False)` of a non-blank string, otherwise `null`.
pub fn yaml_str(value: &PyValue) -> PyStr {
    match as_str(value) {
        Some(own) if !own.strip().is_empty() => {
            let mut out = PyStr::default();
            own.write_json(&mut out, false);
            out
        }
        _ => PyStr::new("null"),
    }
}

/// `str.isdigit()` of the characters `int()` accepts: only decimal digits are modelled.
fn digits_only(value: &PyStr) -> Res<bool> {
    let mut all_decimal = true;
    for &point in value.code_points() {
        if decimal_digit_value(point).is_none() {
            all_decimal = false;
            if char::from_u32(point).is_some_and(char::is_numeric) {
                return Err(unsupported(
                    "str.isdigit() of a non-decimal numeric character is not modelled",
                ));
            }
        }
    }
    Ok(!value.is_empty() && all_decimal)
}

/// `re.fullmatch(r"\d+\.\d+", text)` (Unicode decimal digits).
fn is_dotted_digits(text: &PyStr) -> bool {
    let points = text.code_points();
    let Some(dot) = points.iter().position(|&point| point == u32::from(b'.')) else {
        return false;
    };
    let (head, tail) = (&points[..dot], &points[dot + 1..]);
    !head.is_empty()
        && !tail.is_empty()
        && head.iter().all(|&point| is_decimal_digit(point))
        && tail.iter().all(|&point| is_decimal_digit(point))
}

fn lower_text(value: &PyStr) -> Res<String> {
    value
        .to_utf8()
        .map(|own| agent_runtime_support::pylower::python_lower(&own))
        .ok_or_else(|| unsupported("str.lower() of a lone surrogate is not modelled"))
}

/// `parse_gpu_memory_line(line)`: `memory.total,compute_cap` of one `nvidia-smi` csv line as the three facts (`None` when unreadable).
pub fn parse_gpu_memory_line(line: &PyValue) -> Res<PyValue> {
    let source = if line.is_truthy() {
        line.clone()
    } else {
        text("")
    };
    let Some(whole) = as_str(&source) else {
        return Err(attribute_error(&source, "split"));
    };
    let parts: Vec<PyStr> = whole
        .code_points()
        .split(|&point| point == u32::from(b','))
        .map(|piece| PyStr::from_code_points(piece.to_vec()).strip())
        .collect();
    let empty = || {
        dict_of(vec![
            ("gpu_memory_model", PyValue::None),
            ("gpu_vram_mib", PyValue::None),
            ("compute_capability", PyValue::None),
        ])
    };
    if parts.len() < 2 {
        return Ok(empty());
    }
    let (mem, cc) = (&parts[0], &parts[1]);
    let lowered = lower_text(mem)?;
    let (model, vram) = if NVSMI_ABSENT.contains(&lowered.as_str()) {
        (text("unified"), PyValue::None)
    } else if digits_only(mem)? {
        (text("discrete"), PyValue::Int(int_from_str(mem)?))
    } else {
        (PyValue::None, PyValue::None)
    };
    let compute = if is_dotted_digits(cc) {
        PyValue::Str(cc.clone())
    } else {
        PyValue::None
    };
    Ok(dict_of(vec![
        ("gpu_memory_model", model),
        ("gpu_vram_mib", vram),
        ("compute_capability", compute),
    ]))
}

/// `check_model_env(model_source, egress, nas_reachable, hf_token_present)`: the blocking model-environment verdict.
pub fn check_model_env(
    model_source: &PyValue,
    egress: &PyValue,
    nas_reachable: &PyValue,
    hf_token_present: &PyValue,
) -> PyValue {
    let ok = || dict_of(vec![("ok", PyValue::Bool(true)), ("reason", PyValue::None)]);
    if equals_text(model_source, "managed") {
        if is_false(nas_reachable) {
            return dict_of(vec![
                ("ok", PyValue::Bool(false)),
                (
                    "reason",
                    text("managed: 서브 모델 NAS 경로 도달 불가(fail-closed)"),
                ),
            ]);
        }
        return ok();
    }
    if in_texts(model_source, &["ephemeral", "custom"]) {
        if equals_text(egress, "restricted") && is_false(hf_token_present) {
            let reason = Text::new()
                .val(model_source)
                .lit(": HF_TOKEN 부재 ∧ egress-restricted → 다운로드 물리 불가(fail-closed)")
                .done();
            return dict_of(vec![
                ("ok", PyValue::Bool(false)),
                ("reason", PyValue::Str(reason)),
            ]);
        }
        return ok();
    }
    ok()
}

/// `derive_platform_preset(gpu_model)`: `(preset, source)` from the closed GPU table (`generic` is the default; there is no failure).
pub fn derive_platform_preset(gpu_model: &PyValue) -> Res<PyValue> {
    if let Some(model) = as_str(gpu_model) {
        let lowered = lower_text(model)?;
        for (needle, preset) in PLATFORM_PRESET_BY_GPU {
            if lowered.contains(&agent_runtime_support::pylower::python_lower(needle)) {
                return Ok(PyValue::Tuple(vec![
                    text(preset),
                    text(&format!("derived:gpu_model~{needle}")),
                ]));
            }
        }
    }
    Ok(PyValue::Tuple(vec![
        text(PLATFORM_PRESET_DEFAULT),
        text("default:no-gpu-model-match"),
    ]))
}

/// `assert_homogeneity(local, peer)`: the five-fact main/sub comparison (three exact, two by version) as `{verified, blocks, warns, detail}`.
pub fn assert_homogeneity(local: &PyValue, peer: &PyValue) -> Res<PyValue> {
    if !peer.is_truthy() {
        return Ok(dict_of(vec![
            ("verified", PyValue::Bool(false)),
            ("warns", PyValue::List(vec![])),
            ("detail", PyValue::Dict(PyDict::new())),
            (
                "blocks",
                PyValue::List(vec![text(
                    "서브 HW 수집 실패(SSH/nvidia-smi 미도달) — 동질성 미검증(fail-closed)",
                )]),
            ),
        ]));
    }
    let mut blocks: Vec<PyStr> = Vec::new();
    let mut warns: Vec<PyStr> = Vec::new();
    let mut detail = PyDict::new();
    for field in ["cpu_arch", "gpu_model", "gpus_per_node"] {
        let (lv, pv) = (dict_get(local, field)?, dict_get(peer, field)?);
        let matched = py_eq(&lv, &pv) && !is_none(&lv);
        detail.set(
            field,
            dict_of(vec![
                ("main", lv.clone()),
                ("sub", pv.clone()),
                ("match", PyValue::Bool(matched)),
            ]),
        );
        if is_none(&lv) || is_none(&pv) {
            blocks.push(
                Text::new()
                    .lit(field)
                    .lit(" 미상(하드): main=")
                    .val(&lv)
                    .lit(" sub=")
                    .val(&pv)
                    .lit(" — 수집 실패(None==None 은 동질 아님)")
                    .done(),
            );
        } else if !py_eq(&lv, &pv) {
            blocks.push(
                Text::new()
                    .lit(field)
                    .lit(" 불일치(하드): main=")
                    .val(&lv)
                    .lit(" ≠ sub=")
                    .val(&pv)
                    .done(),
            );
        }
    }
    for field in ["cuda", "driver"] {
        let (lv, pv) = (dict_get(local, field)?, dict_get(peer, field)?);
        let (lt, pt) = (ver_tuple(&lv)?, ver_tuple(&pv)?);
        let (lhead, phead) = (tuple_head(&lt), tuple_head(&pt));
        let major_match = match (&lhead, &phead) {
            (Some(left), Some(right)) => py_eq(left, right),
            _ => false,
        };
        detail.set(
            field,
            dict_of(vec![
                ("main", lv.clone()),
                ("sub", pv.clone()),
                ("major_match", PyValue::Bool(major_match)),
            ]),
        );
        if !lt.is_truthy() || !pt.is_truthy() {
            blocks.push(
                Text::new()
                    .lit(field)
                    .lit(" 버전 미상(main=")
                    .val(&lv)
                    .lit(" sub=")
                    .val(&pv)
                    .lit(") — 검증 불가(하드)")
                    .done(),
            );
        } else if !heads_equal(&lhead, &phead) {
            blocks.push(
                Text::new()
                    .lit(field)
                    .lit(" major 불일치(하드): main=")
                    .val(&lv)
                    .lit(" ≠ sub=")
                    .val(&pv)
                    .done(),
            );
        } else if !py_eq(&lt, &pt) {
            warns.push(
                Text::new()
                    .lit(field)
                    .lit(" minor/patch drift(허용·경고): main=")
                    .val(&lv)
                    .lit(" sub=")
                    .val(&pv)
                    .done(),
            );
        }
    }
    Ok(dict_of(vec![
        ("verified", PyValue::Bool(blocks.is_empty())),
        ("blocks", list_of_texts(&blocks)),
        ("warns", list_of_texts(&warns)),
        ("detail", PyValue::Dict(detail)),
    ]))
}

fn tuple_head(value: &PyValue) -> Option<PyValue> {
    match value.peel() {
        PyValue::Tuple(items) => items.first().cloned(),
        _ => None,
    }
}

fn heads_equal(left: &Option<PyValue>, right: &Option<PyValue>) -> bool {
    match (left, right) {
        (Some(l), Some(r)) => py_eq(l, r),
        (None, None) => true,
        _ => false,
    }
}

/// `os.path.join(os.path.dirname(compose_path), "envs", ".env.interconnect")` for a `str` (POSIX rules).
pub fn default_env_interconnect_path(compose_path: &PyValue) -> Res<PyValue> {
    let Some(path) = as_str(compose_path) else {
        return Err(raise(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                compose_path.type_name()
            ),
        ));
    };
    let points = path.code_points();
    let slash = u32::from(b'/');
    let cut = points
        .iter()
        .rposition(|&p| p == slash)
        .map_or(0, |i| i + 1);
    let mut head: Vec<u32> = points[..cut].to_vec();
    if !head.is_empty() && !head.iter().all(|&p| p == slash) {
        while head.last() == Some(&slash) {
            head.pop();
        }
    }
    let mut out = head;
    for part in ["envs", ".env.interconnect"] {
        if !out.is_empty() && out.last() != Some(&slash) {
            out.push(slash);
        }
        out.extend(part.chars().map(u32::from));
    }
    Ok(PyValue::Str(PyStr::from_code_points(out)))
}

/// `emit_gate(emit_manifest, topology)`: `3` when `--emit-manifest` is asked while the topology is still `auto`, otherwise `0`.
pub fn emit_gate(emit_manifest: &PyValue, topology: &PyValue) -> i64 {
    if emit_manifest.is_truthy() && equals_text(topology, "auto") {
        3
    } else {
        0
    }
}

/// `not isinstance(x, (int, float)) or isinstance(x, bool) or not math.isfinite(x) or x < 0` for a number that must be finite and
/// non-negative. `OverflowError` for an `int` too large for `math.isfinite`.
pub(crate) fn invalid_number(value: &PyValue) -> Res<bool> {
    let value = value.peel();
    if matches!(value, PyValue::Bool(_)) {
        return Ok(true);
    }
    let Some(number) = num_of(value) else {
        return Ok(true);
    };
    let as_float = number.to_f64().map_err(Failure::from)?;
    if !as_float.is_finite() {
        return Ok(true);
    }
    Ok(compare(Compare::Less, value, &PyValue::int(0))?)
}

/// The arguments of `evaluate_gate` (keyword-only in the original), each of any type.
#[derive(Clone, Debug)]
pub struct GateArgs {
    pub declared: PyValue,
    pub ic_present: PyValue,
    pub peer_given: PyValue,
    pub peer_reachable: PyValue,
    pub bandwidth: PyValue,
    pub bw_floor: PyValue,
    pub per_port_gbps: PyValue,
    pub per_port_floor: PyValue,
    pub branch: PyValue,
    pub branch_topo: PyValue,
    pub mani_topo: PyValue,
    pub mani_path: PyValue,
    pub peer_ip: PyValue,
    pub egress_self: PyValue,
    pub egress_peer: PyValue,
    pub model_env_ok: PyValue,
    pub model_env_reason: PyValue,
    pub manifest_read_error: PyValue,
}

fn gate_note_perf_pending(per_port_floor: &PyValue, bw_floor: &PyValue) -> PyStr {
    Text::new()
        .lit("구조 충족·성능 미측정(fail-closed). ib_write_bw 로 포트당 ≥")
        .val(per_port_floor)
        .lit("Gb/s 및 합산 ≥")
        .val(bw_floor)
        .lit("Gb/s 를 측정해 --bandwidth-gbps(및 --per-port-gbps)로 주입하라.")
        .done()
}

/// `evaluate_gate(...)`: the deterministic gate verdict `(assertion, gate, exit_code)`, one pure function.
pub fn evaluate_gate(args: &GateArgs) -> Res<PyValue> {
    let mut mism: Vec<PyStr> = Vec::new();
    let mut warns: Vec<PyStr> = Vec::new();
    let declared = &args.declared;
    if in_texts(declared, &["single", "multi"]) {
        if args.branch_topo.is_truthy() && !py_eq(&args.branch_topo, declared) {
            mism.push(
                Text::new()
                    .lit("declared=")
                    .val(declared)
                    .lit(" ≠ git branch(")
                    .val(&args.branch)
                    .lit(")⇒")
                    .val(&args.branch_topo)
                    .done(),
            );
        } else if !args.branch_topo.is_truthy() {
            mism.push(
                Text::new()
                    .lit("git branch(")
                    .val(&args.branch)
                    .lit(") 로 토폴로지를 해소하지 못했다 — 3자-일치(branch↔manifest↔scan)를 판정할 수 없다(fail-closed)")
                    .done(),
            );
        }
        if equals_text(declared, "multi") && !args.ic_present.is_truthy() {
            mism.push(PyStr::new("declared=multi 인데 스캔: RoCE v2 미탐지"));
        }
        if equals_text(declared, "single") && args.ic_present.is_truthy() {
            warns.push(PyStr::new(
                "single 선언 + RoCE 하드웨어 존재 — 멀티 가능 머신의 단일노드 운용(정상·정보)",
            ));
        }
        if args.mani_topo.is_truthy() && !py_eq(&args.mani_topo, declared) {
            mism.push(
                Text::new()
                    .lit("manifest(")
                    .val(&args.mani_path)
                    .lit(") topology=")
                    .val(&args.mani_topo)
                    .lit(" ≠ declared=")
                    .val(declared)
                    .done(),
            );
        }
        if args.mani_topo.is_truthy()
            && args.branch_topo.is_truthy()
            && !py_eq(&args.mani_topo, &args.branch_topo)
        {
            mism.push(
                Text::new()
                    .lit("manifest(")
                    .val(&args.mani_path)
                    .lit(") topology=")
                    .val(&args.mani_topo)
                    .lit(" ≠ branch⇒")
                    .val(&args.branch_topo)
                    .done(),
            );
        }
    }
    if args.egress_self.is_truthy()
        && args.egress_peer.is_truthy()
        && !py_eq(&args.egress_self, &args.egress_peer)
    {
        warns.push(
            Text::new()
                .lit("egress 능력차(정보·fail-open): main=")
                .val(&args.egress_self)
                .lit(" sub=")
                .val(&args.egress_peer)
                .lit(" — 서브는 자동 격하(루프라인-only)")
                .done(),
        );
    }
    let model_block: Option<PyValue> = if is_false(&args.model_env_ok) {
        Some(or(args.model_env_reason.clone(), || {
            Ok(text("모델 제반환경 불충족(fail-closed)"))
        })?)
    } else {
        None
    };
    if args.manifest_read_error.is_truthy() {
        mism.push(
            Text::new()
                .lit("manifest 판독 불가(부재 아님): ")
                .val(&args.manifest_read_error)
                .done(),
        );
    }
    let assertion = dict_of(vec![
        ("git_branch", args.branch.clone()),
        ("branch_implies", args.branch_topo.clone()),
        ("manifest_path", args.mani_path.clone()),
        ("manifest_topology", args.mani_topo.clone()),
        ("declared", declared.clone()),
        ("scan_interconnect_present", args.ic_present.clone()),
        ("egress_self", args.egress_self.clone()),
        ("egress_peer", args.egress_peer.clone()),
        ("consistent", PyValue::Bool(mism.is_empty())),
        ("mismatches", list_of_texts(&mism)),
        ("warnings", list_of_texts(&warns)),
        (
            "per_port_evaluated",
            PyValue::Bool(!is_none(&args.per_port_gbps) && !is_none(&args.per_port_floor)),
        ),
        ("per_port_gbps", args.per_port_gbps.clone()),
        ("per_port_floor", args.per_port_floor.clone()),
        (
            "bandwidth_source",
            text(if is_none(&args.bandwidth) {
                "unmeasured"
            } else {
                "injected:--bandwidth-gbps"
            }),
        ),
    ]);
    let mut exit_code: i64 = 0;
    let gate;
    if equals_text(declared, "single") {
        let mut blockers: Vec<PyValue> = mism.iter().cloned().map(PyValue::Str).collect();
        if let Some(block) = &model_block {
            blockers.push(block.clone());
        }
        let blocked = !blockers.is_empty();
        gate = dict_of(vec![
            ("branch", text("alpha")),
            ("status", text(if blocked { "blocked" } else { "ok" })),
            ("mismatches", PyValue::List(blockers)),
            (
                "note",
                text(if blocked {
                    "일관성 단언 실패 → blocked"
                } else {
                    "single-node: interconnect 스캔 skip(실패 아님)"
                }),
            ),
        ]);
        if blocked {
            exit_code = 2;
        }
    } else if equals_text(declared, "multi") {
        let mut struct_block: Vec<PyValue> = Vec::new();
        if !args.ic_present.is_truthy() {
            struct_block.push(text("RoCE v2 미탐지"));
        }
        if !args.peer_given.is_truthy() {
            struct_block.push(text("peer-ip 미지정(도달성 미검증)"));
        } else if !args.peer_reachable.is_truthy() {
            struct_block.push(PyValue::Str(
                Text::new()
                    .lit("peer ")
                    .val(&args.peer_ip)
                    .lit(" 미도달")
                    .done(),
            ));
        }
        if let Some(block) = &model_block {
            struct_block.push(block.clone());
        }
        if invalid_number(&args.bw_floor)? {
            struct_block.push(text("대역폭 합격선이 finite non-negative number가 아님"));
        }
        if !is_none(&args.bandwidth) && invalid_number(&args.bandwidth)? {
            struct_block.push(text("대역폭 측정값이 finite non-negative number가 아님"));
        }
        struct_block.extend(mism.iter().cloned().map(PyValue::Str));
        if !struct_block.is_empty() {
            gate = dict_of(vec![
                ("branch", text("gamma")),
                ("status", text("blocked")),
                ("reasons", PyValue::List(struct_block)),
                (
                    "note",
                    text("멀티-ready manifest 미생성. 구조/일관성 확보 후 재실행."),
                ),
            ]);
            exit_code = 2;
        } else if is_none(&args.bandwidth) {
            gate = dict_of(vec![
                ("branch", text("multi-ready-candidate")),
                ("status", text("pending-perf")),
                ("pending", PyValue::List(vec![text("bandwidth_gbps")])),
                (
                    "note",
                    PyValue::Str(gate_note_perf_pending(&args.per_port_floor, &args.bw_floor)),
                ),
            ]);
            exit_code = 2;
        } else {
            let per_port_present = !is_none(&args.per_port_gbps) && !is_none(&args.per_port_floor);
            let per_port_less = |args: &GateArgs| {
                compare(
                    Compare::Less,
                    args.per_port_gbps.peel(),
                    args.per_port_floor.peel(),
                )
            };
            // `bandwidth < bw_floor or (per_port_gbps is not None and per_port_floor is not None and per_port_gbps < per_port_floor)`
            let short = compare(Compare::Less, args.bandwidth.peel(), args.bw_floor.peel())?
                || (per_port_present && per_port_less(args)?);
            if short {
                let mut reasons: Vec<PyValue> = Vec::new();
                if compare(Compare::Less, args.bandwidth.peel(), args.bw_floor.peel())? {
                    reasons.push(PyValue::Str(
                        Text::new()
                            .lit("합산 대역폭 ")
                            .val(&args.bandwidth)
                            .lit("Gb/s < 합격선 ")
                            .val(&args.bw_floor)
                            .lit("Gb/s")
                            .done(),
                    ));
                }
                if per_port_present && per_port_less(args)? {
                    reasons.push(PyValue::Str(
                        Text::new()
                            .lit("포트당 대역폭 ")
                            .val(&args.per_port_gbps)
                            .lit("Gb/s < 합격선 ")
                            .val(&args.per_port_floor)
                            .lit("Gb/s")
                            .done(),
                    ));
                }
                let note = Text::new()
                    .lit("성능 미달 → 멀티-ready 거부(fail-closed). 오설정 점검(케이블 수/GID/MTU/GDR). 합격선 ")
                    .val(&args.bw_floor)
                    .lit("은 200Gbps 플랫폼 파생 기본값 — 저속-그러나-정상 링크(예 100GbE)면 불가가 아니라 `--bw-floor <합산 line-rate×0.9>` 재설정 대상(HITL·시도-우선, SKILL §1.4).")
                    .done();
                gate = dict_of(vec![
                    ("branch", text("gamma")),
                    ("status", text("blocked")),
                    ("reasons", PyValue::List(reasons)),
                    ("note", PyValue::Str(note)),
                ]);
                exit_code = 2;
            } else {
                let note = Text::new()
                    .lit("구조+성능 통과(대역폭 ")
                    .val(&args.bandwidth)
                    .lit(" ≥ ")
                    .val(&args.bw_floor)
                    .lit("Gb/s). 멀티-ready.")
                    .done();
                gate = dict_of(vec![
                    ("branch", text("multi-ready")),
                    ("status", text("ok")),
                    ("bandwidth_gbps", args.bandwidth.clone()),
                    ("note", PyValue::Str(note)),
                ]);
            }
        }
    } else {
        gate = dict_of(vec![
            ("branch", text("auto")),
            ("status", text("report-only")),
            (
                "note",
                text("토폴로지 미선언 — single/multi 명시 시 게이트 판정. 스캔 사실만 보고."),
            ),
        ]);
    }
    Ok(PyValue::Tuple(vec![
        assertion,
        gate,
        PyValue::int(exit_code),
    ]))
}

fn sorted_texts(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items
}

fn count_equal(roles: &[PyValue], literal: &str) -> usize {
    roles
        .iter()
        .filter(|role| equals_text(role, literal))
        .count()
}

/// `emission_blockers(result)`: the canonical scan facts that forbid a `complete: true` manifest emission, sorted.
pub fn emission_blockers(result: &PyValue) -> Res<PyValue> {
    let mut blockers: Vec<String> = Vec::new();
    for field in ["cpu_arch", "cuda_version", "gpu_model"] {
        let got = dict_get(result, field)?;
        match as_str(&got) {
            Some(own) if !own.strip().is_empty() => {}
            _ => blockers.push(field.to_string()),
        }
    }
    if !in_texts(
        &dict_get(result, "cpu_arch")?,
        &["aarch64", "x86_64", "amd64"],
    ) {
        blockers.push("cpu_arch".into());
    }
    let cuda = dict_get(result, "cuda_version")?;
    if let Some(own) = as_str(&cuda) {
        if !cuda_matches(own) {
            blockers.push("cuda_version".into());
        }
    }
    let count = dict_get(result, "gpus_per_node")?;
    let count_bad = !plain_int(&count) || !int_at_least(&count, 1);
    if count_bad {
        blockers.push("gpus_per_node".into());
    }
    if !in_texts(
        &dict_get(result, "topology_declared")?,
        &["single", "multi"],
    ) {
        blockers.push("topology_declared".into());
    }
    let interconnect = dict_get(result, "interconnect")?;
    let PyValue::Dict(ic) = interconnect.peel() else {
        blockers.push("interconnect".into());
        return Ok(strings_value(sorted_texts(blockers)));
    };
    let bandwidth = none_if_absent(ic.get("bandwidth_gbps"));
    if !ic.contains_key("bandwidth_gbps") || (!is_none(&bandwidth) && invalid_number(&bandwidth)?) {
        blockers.push("interconnect.bandwidth_gbps".into());
    }
    let ic_type = none_if_absent(ic.get("type"));
    if !in_texts(&ic_type, &["generic-ethernet", "RoCE v2"]) {
        blockers.push("interconnect.type".into());
    }
    let hcas = none_if_absent(ic.get("hca_devices"));
    let hca_items: Option<&Vec<PyValue>> = match hcas.peel() {
        PyValue::List(items) => Some(items),
        _ => None,
    };
    if hca_items.is_none_or(|items| {
        items
            .iter()
            .any(|item| as_str(item).is_none_or(|own| !token_matches(own)))
    }) {
        blockers.push("interconnect.hca_devices".into());
    }
    let gid_index = none_if_absent(ic.get("gid_index"));
    let gid_scalar_valid =
        is_none(&gid_index) || (plain_int(&gid_index) && int_at_least(&gid_index, 0));
    let gid_list_valid = match gid_index.peel() {
        PyValue::List(items) if items.len() >= 2 => {
            items
                .iter()
                .all(|item| plain_int(item) && int_at_least(item, 0))
                && items.windows(2).all(|pair| {
                    compare(Compare::Less, pair[0].peel(), pair[1].peel()).unwrap_or(false)
                })
        }
        _ => false,
    };
    if !ic.contains_key("gid_index") || !(gid_scalar_valid || gid_list_valid) {
        blockers.push("interconnect.gid_index".into());
    }
    let socket_iface = none_if_absent(ic.get("socket_iface"));
    if !ic.contains_key("socket_iface")
        || (!is_none(&socket_iface) && as_str(&socket_iface).is_none_or(|own| !token_matches(own)))
    {
        blockers.push("interconnect.socket_iface".into());
    }
    let preset = none_if_absent(ic.get("platform_preset"));
    if !ic.contains_key("platform_preset")
        || (!is_none(&preset) && as_str(&preset).is_none_or(|own| !preset_matches(own)))
    {
        blockers.push("interconnect.platform_preset".into());
    }
    if equals_text(&ic_type, "RoCE v2") {
        if hca_items.is_none_or(|items| items.is_empty()) {
            blockers.push("interconnect.hca_devices".into());
        }
        if is_none(&gid_index) {
            blockers.push("interconnect.gid_index".into());
        }
        if is_none(&socket_iface) {
            blockers.push("interconnect.socket_iface".into());
        }
    }
    if equals_text(&dict_get(result, "topology_declared")?, "multi") {
        let nodes = dict_get(result, "nodes")?;
        let node_list: Option<&Vec<PyValue>> = match nodes.peel() {
            PyValue::List(items) => Some(items),
            _ => None,
        };
        let roles: Vec<PyValue> = match node_list {
            Some(items) => items
                .iter()
                .filter(|node| matches!(node.peel(), PyValue::Dict(_)))
                .map(|node| dict_get(node, "role"))
                .collect::<Res<Vec<_>>>()?,
            None => Vec::new(),
        };
        if count_equal(&roles, "main") != 1 || count_equal(&roles, "sub") < 1 {
            blockers.push("nodes.roster".into());
        }
        if let Some(items) = node_list {
            for (index, node) in items.iter().enumerate() {
                if !matches!(node.peel(), PyValue::Dict(_)) {
                    blockers.push(format!("nodes[{index}]"));
                    continue;
                }
                if !in_texts(&dict_get(node, "role")?, &["main", "sub"]) {
                    blockers.push(format!("nodes[{index}].role"));
                }
                if !valid_host(&dict_get(node, "host")?) {
                    blockers.push(format!("nodes[{index}].host"));
                }
            }
        }
    }
    let unique: std::collections::BTreeSet<String> = blockers.into_iter().collect();
    Ok(strings_value(unique.into_iter().collect()))
}

fn strings_value(items: Vec<String>) -> PyValue {
    PyValue::List(items.iter().map(|item| text(item)).collect())
}

/// `sorted(items)` of arbitrary values with CPython's comparison schedule (`TypeError` on an unorderable pair).
pub(crate) fn sorted_values(items: Vec<PyValue>) -> Res<Vec<PyValue>> {
    let keys = items.clone();
    Ok(sorted_by_key(items, &keys)?)
}
