//! `rope_scaling_translate.py`: a context-extension declaration translated into the `--hf-overrides` a serve needs.
//!
//! `translate(cfg, target_len, factor=None)` finds the model's native `max_position_embeddings`, decides whether the target needs an
//! extension, finds the rope dictionary (the first container that is a dictionary, even an empty one), refuses a rope that already
//! declares an extension, derives or checks the YaRN factor, and returns the **merged** override: every existing rope key is carried,
//! with `rope_type`, `factor` and `original_max_position_embeddings` set. It states what is unverified (`unknowns`) instead of hiding it.
//!
//! The order of the original is kept, and so are its types and failures: the native size (an int or float, `bool` included, converted
//! with `int()`, which may fail before the target is looked at), then `int(target_len)`, then (no native size) the unknown verdict,
//! (target not above native) the no-extension verdict, the container search, the rope type test against `EXTENDABLE_ROPE_TYPES`
//! (tuple membership: `==`), the factor (`float(factor)` when it is truthy, otherwise the **integer** `ceil(int(target) / native)`),
//! the under-factor test (`native * f < int(target)`), the merge, the reasons, then the unknowns about mrope and partial rotary
//! factor. Nothing is bounded, defaulted or guarded that the original does not guard: a zero native size without a factor is its
//! `ZeroDivisionError`, a NaN is its `ValueError`, an infinity its `OverflowError`.
//!
//! The two module globals the declared profile may change are values given to each call ([`Profile`]); an alias or identity of a
//! caller's tuple is not modelled.

use crate::pyexpr::{compare, exception, float_of, int_of, Res};
use agent_runtime_support::pyformat::percent_float;
use agent_runtime_support::pynumber::{float_from_int, int_from_float, int_true_divide};
use agent_runtime_support::pyops::{multiply, Compare};
use agent_runtime_support::pysort::same_or_eq;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;

pub const MODULE: &str = "rope_scaling_translate";

/// `ROPE_CONTAINERS`: where the rope settings live (two layouts of transformers coexist).
pub const ROPE_CONTAINERS: [&[&str]; 4] = [
    &["text_config", "rope_parameters"],
    &["text_config", "rope_scaling"],
    &["rope_parameters"],
    &["rope_scaling"],
];

/// The two globals of the module the profile declares, as values for one call: ordered paths of string keys (an empty path is the
/// config itself) and the rope types an extension may start from (JSON scalars).
#[derive(Clone, Debug)]
pub struct Profile {
    pub containers: Vec<Vec<PyStr>>,
    pub extendable: Vec<PyValue>,
}

impl Profile {
    /// The globals as the module defines them: `EXTENDABLE_ROPE_TYPES = ("default", "linear", None)`.
    pub fn canonical() -> Profile {
        Profile {
            containers: ROPE_CONTAINERS
                .iter()
                .map(|path| path.iter().map(|key| PyStr::new(key)).collect())
                .collect(),
            extendable: vec![
                PyValue::str("default"),
                PyValue::str("linear"),
                PyValue::None,
            ],
        }
    }

    /// The profile as JSON arrays (`{"ROPE_CONTAINERS": [[...], ...], "EXTENDABLE_ROPE_TYPES": [...]}`).
    pub fn to_value(&self) -> PyValue {
        let mut all = PyDict::new();
        all.set(
            "ROPE_CONTAINERS",
            PyValue::List(
                self.containers
                    .iter()
                    .map(|path| PyValue::List(path.iter().cloned().map(PyValue::Str).collect()))
                    .collect(),
            ),
        );
        all.set(
            "EXTENDABLE_ROPE_TYPES",
            PyValue::List(self.extendable.clone()),
        );
        PyValue::Dict(all)
    }

    /// Replaces what the object names: `ROPE_CONTAINERS` an array of arrays of strings, `EXTENDABLE_ROPE_TYPES` an array of JSON
    /// scalars (null, bool, number, string). Anything else, or another member, is the error text.
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        for (name, value) in members.iter() {
            match (name.to_utf8().as_deref(), value) {
                (Some("ROPE_CONTAINERS"), PyValue::List(paths)) => {
                    let mut parsed = Vec::new();
                    for path in paths {
                        let PyValue::List(keys) = path else {
                            return Err("ROPE_CONTAINERS holds arrays of strings".to_string());
                        };
                        let mut one = Vec::new();
                        for key in keys {
                            match key {
                                PyValue::Str(text) => one.push(text.clone()),
                                _ => {
                                    return Err(
                                        "ROPE_CONTAINERS holds arrays of strings".to_string()
                                    )
                                }
                            }
                        }
                        parsed.push(one);
                    }
                    self.containers = parsed;
                }
                (Some("EXTENDABLE_ROPE_TYPES"), PyValue::List(kinds)) => {
                    if kinds.iter().any(|kind| {
                        // The scalar's own kind: a literal `NaN` of a request arrives as the shared object carrier.
                        !matches!(
                            kind.peel(),
                            PyValue::None
                                | PyValue::Bool(_)
                                | PyValue::Int(_)
                                | PyValue::Float(_)
                                | PyValue::Str(_)
                        )
                    }) {
                        return Err("EXTENDABLE_ROPE_TYPES holds JSON scalars".to_string());
                    }
                    self.extendable = kinds.clone();
                }
                _ => return Err(format!("globals member {} is not usable", name.repr())),
            }
        }
        Ok(())
    }
}

fn dotted(path: &[PyStr]) -> PyStr {
    let mut text = PyStr::default();
    for (position, key) in path.iter().enumerate() {
        if position > 0 {
            text.push_str(".");
        }
        text.push_py(key);
    }
    text
}

/// `cur[key]` when `cur` is a dictionary holding `key`.
fn step<'a>(current: &'a PyValue, key: &PyStr) -> Option<&'a PyValue> {
    match current.peel() {
        PyValue::Dict(map) => map
            .iter()
            .find(|(name, _)| name.code_points() == key.code_points())
            .map(|(_, value)| value),
        _ => None,
    }
}

/// `_get(cfg, path)`: the dictionary at the path, `None` for anything else.
fn container_at<'a>(cfg: &'a PyValue, path: &[PyStr]) -> Option<&'a PyDict> {
    let mut current = cfg;
    for key in path {
        current = step(current, key)?;
    }
    match current.peel() {
        PyValue::Dict(map) => Some(map),
        _ => None,
    }
}

/// `_max_pos(cfg)`: the first of `text_config.max_position_embeddings` and `max_position_embeddings` that is an `int` or `float`
/// (a `bool` is an `int`), converted with `int()`, and the dotted path it came from.
fn native_size(cfg: &PyValue) -> Res<Option<(BigInt, &'static str)>> {
    for path in [
        &["text_config", "max_position_embeddings"][..],
        &["max_position_embeddings"][..],
    ] {
        let mut current = cfg;
        let mut found = true;
        for key in path {
            match step(current, &PyStr::new(key)) {
                Some(next) => current = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found
            && matches!(
                current.peel(),
                PyValue::Bool(_) | PyValue::Int(_) | PyValue::Float(_)
            )
        {
            let name = if path.len() == 2 {
                "text_config.max_position_embeddings"
            } else {
                "max_position_embeddings"
            };
            return Ok(Some((int_of(current.peel())?, name)));
        }
    }
    Ok(None)
}

fn get(container: &PyDict, key: &str) -> PyValue {
    container.get(key).cloned().unwrap_or(PyValue::None)
}

/// `"%g" % f` of the factor (a float, or the derived int).
fn percent_g(value: &PyValue) -> Res<String> {
    let number = match value {
        PyValue::Float(number) => *number,
        PyValue::Int(number) => float_from_int(number).map_err(exception)?,
        _ => unreachable!("the factor is a float or an int"),
    };
    let formatted = percent_float(&"%g".chars().map(u32::from).collect::<Vec<_>>(), number)?;
    Ok(PyStr::from_code_points(formatted).to_utf8().expect("ASCII"))
}

fn message(parts: &[&PyStr]) -> PyValue {
    let mut text = PyStr::default();
    for part in parts {
        text.push_py(part);
    }
    PyValue::Str(text)
}

fn plain(text: &str) -> PyStr {
    PyStr::new(text)
}

/// `translate(cfg, target_len, factor=None)`: the translation, with its reasons and unknowns.
pub fn translate(
    profile: &Profile,
    cfg: &PyValue,
    target_len: &PyValue,
    factor: &PyValue,
) -> Res<PyDict> {
    let native = native_size(cfg)?;
    let target = int_of(target_len.peel())?;
    let mut out = PyDict::new();
    out.set("target_len", PyValue::Int(target.clone()));
    out.set(
        "native_max_position_embeddings",
        native
            .as_ref()
            .map_or(PyValue::None, |(size, _)| PyValue::Int(size.clone())),
    );
    out.set(
        "native_source",
        native
            .as_ref()
            .map_or(PyValue::None, |(_, name)| PyValue::str(name)),
    );
    for key in ["needed", "hf_overrides", "container_path", "factor"] {
        out.set(key, PyValue::None);
    }
    out.set("unknowns", PyValue::List(Vec::new()));
    out.set("reasons", PyValue::List(Vec::new()));
    let reason = |out: &mut PyDict, text: PyValue| {
        if let Some(PyValue::List(items)) = out.get_mut("reasons") {
            items.push(text);
        }
    };
    let Some((native, _)) = native else {
        reason(
            &mut out,
            PyValue::str(
                "모델 config 에서 max_position_embeddings 를 찾지 못했다 — 확장 필요 여부를 판정할 수 없다(추측하지 않는다).",
            ),
        );
        return Ok(out);
    };
    if target <= native {
        out.set("needed", PyValue::Bool(false));
        reason(
            &mut out,
            message(&[
                &plain("목표 "),
                &plain(&target.to_string()),
                &plain(" ≤ 네이티브 "),
                &plain(&native.to_string()),
                &plain(" — 확장이 필요 없다. rope 인자를 넣지 않는 것이 옳다."),
            ]),
        );
        return Ok(out);
    }
    out.set("needed", PyValue::Bool(true));

    let found = profile
        .containers
        .iter()
        .find_map(|path| container_at(cfg, path).map(|container| (container, path)));
    let Some((container, path)) = found else {
        let mut text = PyStr::new("rope 설정 딕셔너리를 찾지 못했다(");
        let mut joined = PyStr::default();
        for (position, path) in profile.containers.iter().enumerate() {
            if position > 0 {
                joined.push_str(" · ");
            }
            joined.push_py(&dotted(path));
        }
        text.push_py(&joined);
        text.push_str(
            " 어디에도 없다) — 어느 키에 얹어야 하는지 모른 채 override 를 만들면 조용히 무시된다.",
        );
        reason(&mut out, PyValue::Str(text));
        return Ok(out);
    };
    out.set("container_path", PyValue::Str(dotted(path)));

    let rope_type = {
        let first = get(container, "rope_type");
        if first.is_truthy() {
            first
        } else {
            get(container, "type")
        }
    };
    if !profile
        .extendable
        .iter()
        .any(|kind| same_or_eq(kind, &rope_type))
    {
        reason(
            &mut out,
            message(&[
                &plain("이미 rope_type="),
                &plain(&rope_type.repr()),
                &plain(
                    " 로 선언돼 있다 — 그 위에 YaRN 을 얹으면 **이중 적용**이다. 모델이 이미 확장된 것인지 먼저 확인하라.",
                ),
            ]),
        );
        return Ok(out);
    }

    let f = if factor.is_truthy() {
        PyValue::Float(float_of(factor.peel())?)
    } else {
        let quotient = int_true_divide(&target, &native).map_err(exception)?;
        PyValue::Int(int_from_float(quotient.ceil()).map_err(exception)?)
    };
    let reach = multiply(&PyValue::Int(native.clone()), &f).map_err(exception)?;
    if compare(Compare::Less, &reach, &PyValue::Int(target.clone()))? {
        let reached = int_of(&reach)?;
        let shown = percent_g(&f)?;
        reason(
            &mut out,
            message(&[
                &plain("factor "),
                &plain(&shown),
                &plain(" 로는 "),
                &plain(&target.to_string()),
                &plain(" 에 닿지 않는다("),
                &plain(&native.to_string()),
                &plain(" × "),
                &plain(&shown),
                &plain(" = "),
                &plain(&reached.to_string()),
                &plain("). 선언한 factor 가 목표와 어긋난다 — 둘 중 하나가 틀렸다."),
            ]),
        );
        return Ok(out);
    }
    out.set("factor", f.clone());

    // The merge: every existing key is carried; a replaced key keeps its place, a new one is appended.
    let mut merged = container.clone();
    merged.set("rope_type", PyValue::str("yarn"));
    merged.set("factor", f.clone());
    merged.set(
        "original_max_position_embeddings",
        PyValue::Int(native.clone()),
    );
    let mut node = PyValue::Dict(merged);
    for key in path.iter().rev() {
        let mut wrapper = PyDict::new();
        wrapper.insert(key.clone(), node);
        node = PyValue::Dict(wrapper);
    }
    out.set("hf_overrides", node);
    let mut names: Vec<PyStr> = container.iter().map(|(name, _)| name.clone()).collect();
    names.sort_by(|a, b| a.code_points().cmp(b.code_points()));
    let mut listed = PyStr::default();
    for (position, name) in names.iter().enumerate() {
        if position > 0 {
            listed.push_str(", ");
        }
        listed.push_py(name);
    }
    if listed.is_empty() {
        listed = plain("없음");
    }
    reason(
        &mut out,
        message(&[
            &plain("네이티브 "),
            &plain(&native.to_string()),
            &plain(" → 목표 "),
            &plain(&target.to_string()),
            &plain(" 를 YaRN factor "),
            &plain(&percent_g(&f)?),
            &plain(" 로 연다. 기존 rope 키("),
            &listed,
            &plain(")를 **전부 실어** 보낸다 — 딕셔너리를 갈아끼우면 그 키들이 조용히 사라지고 위치 인코딩이 바뀐다."),
        ]),
    );

    let unknown = |out: &mut PyDict, text: PyValue| {
        if let Some(PyValue::List(items)) = out.get_mut("unknowns") {
            items.push(text);
        }
    };
    let (interleaved, section) = (
        get(container, "mrope_interleaved"),
        get(container, "mrope_section"),
    );
    if interleaved.is_truthy() || section.is_truthy() {
        unknown(
            &mut out,
            message(&[
                &plain("mrope(interleaved="),
                &plain(&interleaved.repr()),
                &plain(" · section="),
                &plain(&section.repr()),
                &plain(") 위에서 YaRN 이 어떻게 합성되는지 이 하드웨어·이 vLLM 에서 검증된 바 없다. 스모크가 유일한 중재자다."),
            ]),
        );
    }
    let prf = get(container, "partial_rotary_factor");
    if !matches!(prf.peel(), PyValue::None) && float_of(prf.peel())? != 1.0 {
        unknown(
            &mut out,
            message(&[
                &plain("partial_rotary_factor="),
                &prf.display_text(),
                &plain(" — 회전 차원이 head_dim 의 일부다. YaRN 의 주파수 보간이 부분 회전 구간에만 적용되는지 전 차원에 걸리는지는 구현 의존이며 미검증이다."),
            ]),
        );
    }
    Ok(out)
}
