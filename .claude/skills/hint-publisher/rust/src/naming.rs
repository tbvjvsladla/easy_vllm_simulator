//! Port of `hintlib/naming.py` — hint 태그 이름: 도구 전량 파생(D5~D8) · 정규 어휘표 · 신/구 문법 파서(구 문법 읽기 전용).
//!
//! ```text
//! hint/<vllm>/<model>/<arch>/<recipe>
//!   <vllm>   빌드 입력(X18)        릴리스 태그 → `0.29.0rc6` · 커밋 핀 → `<직전 릴리스>-g<sha12>`
//!   <model>  체크포인트 슬러그      HF repo 이름 또는 체크포인트 basename 소문자(= 인증서 model 키)
//!   <arch>   `<hw>-<G>g<N>n-<main|sub|cluster>-<target>[-<plane>]`
//!   <recipe> v7 `q<quant>-len<n>-kv<dtype>[-<tail>][-t<YYMMDDHHMM>]` (v6 6축은 읽기 전용)
//! ```
//!
//! The design notes (dated invariants, why each axis exists) live in the original's module docstring; this port keeps
//! every rule byte-identical. Pure module: no load-time side effect (no git, no file, no path computation).
//!
//! Python dataclasses `TagName` / `Axis` / `DerivedName` are pub structs; `DerivedName.segments`/`.axes` are ordered
//! `Vec<(String, Axis)>` (dict insertion order).
use std::path::Path;

use agent_runtime_support::pylower::python_lower;
use agent_runtime_support::pystr::{decimal_digit_value, is_python_space};

use crate::core::{self, fail, HintError, HR};
use crate::pyre::named;
use crate::re_static;
use crate::value::{dumps_unicode, loads, py_repr, Map, J};

// ── 문법 세대 ────────────────────────────────────────────────────────────────────────────────
pub const GRAMMAR_V7: &str = "v7";
pub const GRAMMAR_V6: &str = "v6";
pub const GRAMMAR_LEGACY_ARCH_NODE: &str = "legacy-5seg-node";
pub const GRAMMAR_LEGACY_ARCH: &str = "legacy-5seg";
pub const GRAMMAR_LEGACY_4SEG: &str = "legacy-4seg";
pub const GRAMMAR_UNKNOWN: &str = "unknown";

pub const NODE_AXIS: [&str; 3] = ["main", "sub", "cluster"];
pub const VOCAB_AXES: [&str; 5] = ["hw", "quant", "kv", "ple", "graph"];
pub const ARCH_AXES: [&str; 5] = ["hw", "gpus_per_node", "nodes", "role", "target"];
pub const PLANE_AXIS: &str = "plane";
pub const PLANE_NAMES: [&str; 2] = ["docker", "native"];
const _PLANE_NO_TOKEN: &str = "docker";
pub const RECIPE_AXES: [&str; 3] = ["q", "len", "kv"];
pub const RECIPE_AXES_V6: [&str; 6] = ["q", "len", "kv", "ple", "spec", "graph"];
pub const V6_TAIL_AXES: [&str; 3] = ["ple", "spec", "graph"];
const _GRAPH_TOKENS: [&str; 2] = ["eager", "graph"]; // frozenset; sorted order for messages
pub const TAIL_TOKEN_MAX: usize = 24;
pub const TAIL_MAX: usize = 64;
re_static!(_TS_TOKEN_RE, r"^t\d{10}$");

// ── 형태 ─────────────────────────────────────────────────────────────────────────────────────
re_static!(_TOKEN_RE, r"^[a-z0-9]+$");
macro_rules! rel {
    () => {
        r"\d+\.\d+\.\d+(?:\.\d+)?(?:[a-z]+\d+)?(?:\.post\d+)?"
    };
}
re_static!(_RELEASE_RE, concat!(r"^v?(?P<rel>", rel!(), r")$"));
re_static!(_DESCRIBE_RE, concat!(r"^v?(?P<rel>", rel!(), r")(?:-\d+-g(?P<g>[0-9a-f]{4,40}))?$"));
re_static!(_SHA40_RE, r"^[0-9a-f]{40}$");
re_static!(_VLLM_SEGMENT_RE, concat!(r"^", rel!(), r"(?:-g[0-9a-f]{12})?$"));
pub const UPSTREAM_VLLM_REPOS: [&str; 1] = ["github.com/vllm-project/vllm"];
re_static!(_REPO_URL_RE, r"^(?:[A-Za-z][A-Za-z0-9+.-]*://)?(?:[^@/\s]+@)?(?P<host>[^/:\s]+)[:/](?P<path>[^\s]+?)(?:\.git)?/*$");
re_static!(_MODEL_SLUG_RE, r"^[a-z0-9][a-z0-9._-]*$");
re_static!(_ARCH_V6_LIKE_RE, r"^[^-]+-\d+g\d+n(?:-|$)");
re_static!(
    _ARCH_V6_RE,
    r"^(?P<hw>[a-z0-9]+)-(?P<g>[1-9]\d*)g(?P<n>[1-9]\d*)n-(?P<role>main|sub|cluster)-(?P<target>native|sim-[a-z0-9]+)(?:-(?P<plane>[a-z0-9]+))?$"
);
re_static!(_ARCH_LEGACY_NODE_RE, r"^(?P<hw>[a-z0-9]+)-(?P<role>main|sub|cluster)-(?P<target>[a-z0-9][a-z0-9-]*)$");
re_static!(_ARCH_LEGACY_RE, r"^(?P<hw>[a-z0-9]+)-(?!main-|sub-|cluster-)(?P<target>[a-z0-9][a-z0-9-]*)$");
re_static!(
    _RECIPE_V6_RE,
    r"^q(?P<q>[a-z0-9]+)-len(?P<len>[1-9]\d*)-kv(?P<kv>[a-z0-9]+)-ple(?P<ple>[a-z0-9]+)-spec(?P<spec>[1-9]\d*|off)-(?P<graph>graph|eager)$"
);
const _UNOBSERVED: [&str; 6] = ["", "n/a", "na", "null", "unknown", "?"];
re_static!(_TEMPLATE_BLANK_RE, r"^(?:<<[^<>]*>>|__[A-Z_]+__)$");
re_static!(_POS_INT_RE, r"[1-9]\d*");
re_static!(_Q_TOK, r"q[a-z0-9]+");
re_static!(_LEN_TOK, r"len[1-9]\d*");
re_static!(_KV_TOK, r"kv[a-z0-9]+");
re_static!(_SHORT_SHA_RE, r"[0-9a-f]{7,39}");
re_static!(_WHEEL_FILE_RE, r"^vllm-([^-]+)-");
re_static!(
    _VERSION_KEY_RE,
    r"^v?(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?(?:\.?(dev|a|b|rc|post)(\d+))?(?:\.post(\d+))?(?:-g([0-9a-f]{12})|\+g([0-9a-f]+))?$"
);

// ── Python str helpers ───────────────────────────────────────────────────────────────────────
fn is_ws(c: char) -> bool {
    is_python_space(c as u32)
}
/// `str.strip()`.
fn pstrip(s: &str) -> String {
    s.trim_matches(is_ws).to_string()
}
/// `str.split()` (whitespace).
fn psplit(s: &str) -> Vec<&str> {
    s.split(is_ws).filter(|p| !p.is_empty()).collect()
}
/// `str.casefold()` (full case folding of the few characters where it differs from `lower()`).
fn casefold(s: &str) -> String {
    let low = python_lower(s);
    if low.is_ascii() {
        return low;
    }
    let mut out = String::with_capacity(low.len());
    for c in low.chars() {
        match c {
            'ß' => out.push_str("ss"),
            'ς' => out.push('σ'),
            'ſ' => out.push('s'),
            'ﬀ' => out.push_str("ff"),
            'ﬁ' => out.push_str("fi"),
            'ﬂ' => out.push_str("fl"),
            'ﬃ' => out.push_str("ffi"),
            'ﬄ' => out.push_str("ffl"),
            'ﬅ' | 'ﬆ' => out.push_str("st"),
            'ẞ' => out.push_str("ss"),
            c => out.push(c),
        }
    }
    out
}
fn clen(s: &str) -> usize {
    s.chars().count()
}
fn orepr(o: Option<&str>) -> String {
    match o {
        Some(s) => py_repr(s),
        None => "None".into(),
    }
}
fn tuple_repr(v: &[&str]) -> String {
    let parts: Vec<String> = v.iter().map(|s| py_repr(s)).collect();
    format!("({})", parts.join(", "))
}
fn list_repr<S: AsRef<str>>(v: &[S]) -> String {
    let parts: Vec<String> = v.iter().map(|s| py_repr(s.as_ref())).collect();
    format!("[{}]", parts.join(", "))
}
/// `str(x or "").strip() or None`.
fn str_or_none(v: Option<&J>) -> Option<String> {
    let v = v.unwrap_or(&J::Null);
    if !v.truthy() {
        return None;
    }
    let s = pstrip(&v.display());
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
/// `int(text)` of a `\d+` match (Unicode decimal digits), saturating.
fn digits(text: &str) -> i128 {
    let mut n: i128 = 0;
    for c in text.chars() {
        let d = decimal_digit_value(c as u32).unwrap_or(0) as i128;
        n = n.saturating_mul(10).saturating_add(d);
    }
    n
}
/// `str(int(text))` of a `[1-9]\d*` match.
fn digits_str(text: &str) -> String {
    if text.is_ascii() {
        text.to_string()
    } else {
        digits(text).to_string()
    }
}
fn head(s: &str, n: usize) -> String {
    core::head_chars(s, n)
}
/// `(vocab or {}).get(k)` — a truthy non-dict raises AttributeError like Python.
fn or_get<'a>(v: &'a J, k: &str) -> HR<Option<&'a J>> {
    if !v.truthy() {
        return Ok(None);
    }
    match v {
        J::Dict(m) => Ok(m.get(k)),
        other => Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'get'", other.type_name()))),
    }
}
/// `v[k]` with Python's KeyError/TypeError.
fn py_sub<'a>(v: &'a J, k: &str) -> HR<&'a J> {
    match v {
        J::Dict(m) => m.get(k).ok_or_else(|| HintError::py("KeyError", py_repr(k))),
        J::List(_) => Err(HintError::py("TypeError", "list indices must be integers or slices, not str")),
        J::Str(_) => Err(HintError::py("TypeError", "string indices must be integers, not 'str'")),
        other => Err(HintError::py("TypeError", format!("'{}' object is not subscriptable", other.type_name()))),
    }
}
/// `repr(e)` of a KeyError/TypeError made by [`py_sub`].
fn exc_repr(e: &HintError) -> String {
    let ty = e.code.trim_start_matches("py:");
    if ty == "KeyError" {
        format!("KeyError({})", e.message)
    } else {
        format!("{ty}({})", py_repr(&e.message))
    }
}
fn is_key_or_type(e: &HintError) -> bool {
    e.code == "py:KeyError" || e.code == "py:TypeError"
}
fn tok_match(s: &str) -> bool {
    _TOKEN_RE().is_match(s)
}
fn j_in(v: &J, set: &[&str]) -> bool {
    v.as_str().map(|s| set.contains(&s)).unwrap_or(false)
}

fn _is_unobserved(text: &J) -> bool {
    match text {
        J::Str(s) => _UNOBSERVED.contains(&_vnorm(text).as_str()) || _TEMPLATE_BLANK_RE().is_match(&pstrip(s)),
        _ => false,
    }
}

fn _axis_evidence(axis: &str) -> String {
    match axis {
        "vllm" => "셀 env VLLM_REF(소스빌드)/VLLM_VERSION(wheel) · native wheel URL · 소스빌드 릴리스 모양 ref 는 VLLM_REPO(업스트림 \
                   여부 · D-h) · 직전 릴리스 = 빌드 원장 vllm.describe 또는 이미지 안 `git describe --tags --match 'v*' --abbrev=0`"
            .into(),
        "model" => "서빙 yaml `model:`(체크포인트 경로) 또는 HF 카드 repo id".into(),
        "arch" => "output/<topology>/manifest.yaml gpu_model·gpus_per_node·nodes · 측정 노드(sweep meta measured_node) · \
                   셀 config target_gpu(시뮬레이션 타겟 선언 여부)"
            .into(),
        "plane" => "artifacts.plane_of(셀 env IMAGE_TAG·BUILD_DOCKERFILE = docker · native serve-proof plane=native)".into(),
        "q" => "체크포인트 config.json quantization_config / hf_quant_config.json(부재를 관측했으면 'none')".into(),
        "len" => "서빙 yaml max-model-len".into(),
        "kv" => "서빙 yaml kv-cache-dtype(부재를 관측했으면 엔진 기본값 'auto')".into(),
        "ple" => "셀 config.yaml declared_axes.ple_mode · 모델 config(PLE 부재 판정이면 'none')".into(),
        "spec" => "서빙 yaml·러너 speculative-config(부재를 관측했으면 raw=None → specoff)".into(),
        "graph" => "서빙 yaml enforce-eager(eager|graph 로 변환해 넘긴다)".into(),
        other => other.to_string(),
    }
}

/// `_SLUG_PRODUCER`: the producer file and the literals the selftest requires in it.
pub const _SLUG_PRODUCER_PATH: &str = ".claude/skills/adversarial-benchmark/rust/src/assemble_sweep_meta.rs";
pub const _SLUG_PRODUCER_LITERALS: [&str; 3] = [
    "let model_base = crate::ospath::basename(&PyStr::new(model_path.trim_end_matches('/')))\n        .strip()",
    "let (model_key, model_source) = if !model_base.is_empty() {",
    "pystr_of(&crate::pylower::python_lower(&model_base)),\n            \"derived(model_path basename · lowercased)\"",
];

// ── 자료형 ───────────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug, PartialEq)]
pub struct TagName {
    pub vllm: String,
    pub model: String,
    pub arch: String,
    pub recipe: String,
    pub grammar: String,
    pub base_recipe: Option<String>,
    pub tail: Vec<String>,
    pub timestamp: Option<String>,
}

impl TagName {
    /// property `tag`.
    pub fn tag(&self) -> String {
        format!("{}{}/{}/{}/{}", core::HINT_TAG_PREFIX, self.vllm, self.model, self.arch, self.recipe)
    }
    /// property `base_tag` (꼬리·timestamp 를 뗀 기본 이름 · v6·v7 이 아니면 None).
    pub fn base_tag(&self) -> Option<String> {
        self.base_recipe
            .as_ref()
            .map(|b| format!("{}{}/{}/{}/{}", core::HINT_TAG_PREFIX, self.vllm, self.model, self.arch, b))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub value: String,
    pub source: String,
    pub token: Option<String>,
}

impl Axis {
    pub fn new(value: impl Into<String>, source: impl Into<String>) -> Axis {
        Axis { value: value.into(), source: source.into(), token: None }
    }
    pub fn as_dict(&self) -> J {
        let mut m = Map::new();
        m.insert("value", self.value.clone());
        m.insert("source", self.source.clone());
        if let Some(t) = &self.token {
            m.insert("token", t.clone());
        }
        J::Dict(m)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedName {
    pub tag: String,
    pub segments: Vec<(String, Axis)>,
    pub axes: Vec<(String, Axis)>,
    pub vllm_build_input: J,
}

impl DerivedName {
    pub fn segment(&self, k: &str) -> Option<&Axis> {
        self.segments.iter().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    pub fn axis(&self, k: &str) -> Option<&Axis> {
        self.axes.iter().find(|(a, _)| a == k).map(|(_, v)| v)
    }
    /// PAYLOAD.json `naming` 의 도구 파생분.
    pub fn to_payload(&self) -> J {
        let mut seg = Map::new();
        for (k, v) in &self.segments {
            seg.insert(k.clone(), v.as_dict());
        }
        let mut ax = Map::new();
        for (k, v) in &self.axes {
            ax.insert(k.clone(), v.as_dict());
        }
        crate::jd! {"grammar" => GRAMMAR_V7, "segments" => seg, "axes" => ax, "vllm_build_input" => self.vllm_build_input.clone()}
    }
}

// ── 이름 해체 (단일 소유자) ─────────────────────────────────────────────────────────────────────
fn _split5(name: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 5 || parts[0] != "hint" || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(parts)
}

fn _grammar(vllm: &str, model: &str, arch: &str, recipe: &str) -> &'static str {
    if _VLLM_SEGMENT_RE().is_match(vllm)
        && _MODEL_SLUG_RE().is_match(model)
        && arch_why(arch).is_none()
        && _split_recipe_raw(&J::from(recipe)).1.is_none()
    {
        return if _RECIPE_V6_RE().is_match(recipe) { GRAMMAR_V6 } else { GRAMMAR_V7 };
    }
    if _ARCH_V6_LIKE_RE().is_match(arch) {
        return GRAMMAR_UNKNOWN;
    }
    if _ARCH_LEGACY_NODE_RE().is_match(arch) {
        return GRAMMAR_LEGACY_ARCH_NODE;
    }
    if _ARCH_LEGACY_RE().is_match(arch) {
        return GRAMMAR_LEGACY_ARCH;
    }
    GRAMMAR_UNKNOWN
}

pub fn parse_tag(name: &str) -> HR<TagName> {
    let parts = match _split5(name) {
        Some(p) => p,
        None => {
            return fail(
                "HINT_NAME_SHAPE",
                format!("이름 형태 위반(hint/<vllm>/<model>/<arch>/<recipe> 5세그먼트): {}", py_repr(name)),
                Some("이름은 손으로 짓지 않는다 — `hint.py name --campaign <id> --cell <cell>` 이 파생 이름을 보여 준다."),
            )
        }
    };
    let (vllm, model, arch, recipe) = (parts[1], parts[2], parts[3], parts[4]);
    let grammar = _grammar(vllm, model, arch, recipe);
    let mut t = TagName {
        vllm: vllm.into(),
        model: model.into(),
        arch: arch.into(),
        recipe: recipe.into(),
        grammar: grammar.into(),
        base_recipe: None,
        tail: Vec::new(),
        timestamp: None,
    };
    if grammar == GRAMMAR_V6 || grammar == GRAMMAR_V7 {
        if let (Some(sp), _) = _split_recipe_raw(&J::from(recipe)) {
            t.base_recipe = sp.get_str("base").map(str::to_string);
            t.tail = sp["tail"].items().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            t.timestamp = sp.get_str("timestamp").map(str::to_string);
        }
    }
    Ok(t)
}

pub fn base_name(name: &str) -> HR<String> {
    let t = parse_tag(name)?;
    match t.base_tag() {
        Some(b) => Ok(b),
        None => fail(
            "HINT_NAME_GRAMMAR_OLD",
            format!("v6/v7 이름이 아니다(문법 {}) — 기본 이름을 뗄 수 없다: {}", t.grammar, py_repr(name)),
            Some("옛 세대 태그는 읽기 전용이다(P1) — 재파생 대조 대상이 아니다."),
        ),
    }
}

pub fn grammar_of(name: &str) -> HR<String> {
    if let Some(p) = _split5(name) {
        return Ok(_grammar(p[1], p[2], p[3], p[4]).into());
    }
    let p4: Vec<&str> = name.split('/').collect();
    if p4.len() == 4 && p4[0] == "hint" && p4.iter().all(|p| !p.is_empty()) {
        return Ok(GRAMMAR_LEGACY_4SEG.into());
    }
    Ok(GRAMMAR_UNKNOWN.into())
}

pub fn plane_of_token(token: &str, vocab: Option<&J>) -> HR<Option<String>> {
    if token.is_empty() {
        return Ok(Some(_PLANE_NO_TOKEN.into()));
    }
    let table = match vocab {
        Some(J::Dict(m)) => m.get(PLANE_AXIS),
        _ => None,
    };
    let table = match table {
        Some(J::Dict(t)) => t,
        _ => return Ok(None),
    };
    let hits: Vec<&str> = table.iter().filter(|(p, t)| !p.starts_with('_') && t.as_str() == Some(token)).map(|(p, _)| p).collect();
    Ok(if hits.len() == 1 { Some(hits[0].to_string()) } else { None })
}

pub fn parse_arch(arch: &str, vocab: Option<&J>) -> HR<Option<J>> {
    if arch_why(arch).is_none() {
        let m = _ARCH_V6_RE().match_at_start(arch).expect("v6 arch");
        let tok = named(&m, "plane").unwrap_or("").to_string();
        let plane = plane_of_token(&tok, vocab)?;
        return Ok(Some(crate::jd! {
            "grammar" => GRAMMAR_V6, "hw" => named(&m, "hw").unwrap_or(""),
            "gpus_per_node" => digits(named(&m, "g").unwrap_or("")) as i64,
            "nodes" => digits(named(&m, "n").unwrap_or("")) as i64,
            "role" => named(&m, "role").unwrap_or(""), "target" => named(&m, "target").unwrap_or(""),
            "plane_token" => tok, "plane" => plane,
        }));
    }
    if _ARCH_V6_LIKE_RE().is_match(arch) {
        return Ok(None);
    }
    if let Some(m) = _ARCH_LEGACY_NODE_RE().match_at_start(arch) {
        return Ok(Some(crate::jd! {"grammar" => GRAMMAR_LEGACY_ARCH_NODE, "hw" => named(&m, "hw").unwrap_or(""),
            "role" => named(&m, "role").unwrap_or(""), "target" => named(&m, "target").unwrap_or("")}));
    }
    if let Some(m) = _ARCH_LEGACY_RE().match_at_start(arch) {
        return Ok(Some(crate::jd! {"grammar" => GRAMMAR_LEGACY_ARCH, "hw" => named(&m, "hw").unwrap_or(""),
            "target" => named(&m, "target").unwrap_or("")}));
    }
    Ok(None)
}

pub fn parse_recipe(recipe: &J) -> HR<Option<J>> {
    let s = match recipe.as_str() {
        Some(s) => s,
        None => return Ok(None),
    };
    Ok(_RECIPE_V6_RE().match_at_start(s).map(|m| {
        let mut out = Map::new();
        for k in RECIPE_AXES_V6 {
            out.insert(k, named(&m, k).unwrap_or(""));
        }
        J::Dict(out)
    }))
}

/// 레시피 → ({q, len, kv, base, tail, timestamp}, None) 또는 (None, 사유코드).
pub fn _split_recipe_raw(recipe: &J) -> (Option<J>, Option<&'static str>) {
    let recipe = match recipe {
        J::Str(s) if !s.is_empty() => s.as_str(),
        _ => return (None, Some("HINT_RECIPE_ABSENT")),
    };
    let toks: Vec<&str> = recipe.split('-').collect();
    if toks.len() < 3 || !_Q_TOK().is_full(toks[0]) || !_LEN_TOK().is_full(toks[1]) || !_KV_TOK().is_full(toks[2]) {
        return (None, Some("HINT_RECIPE_SHAPE_VIOLATION"));
    }
    let mut rest: Vec<&str> = toks[3..].to_vec();
    let ts = if rest.last().map(|t| _TS_TOKEN_RE().is_match(t)).unwrap_or(false) { rest.pop() } else { None };
    if rest.iter().any(|t| !tok_match(t)) {
        return (None, Some("HINT_RECIPE_SHAPE_VIOLATION"));
    }
    let rest_j: Vec<J> = rest.iter().map(|t| J::from(*t)).collect();
    if !_tail_token_problems(&rest_j).is_empty() {
        return (None, Some("HINT_TAIL_FORMAT"));
    }
    let q: String = toks[0].chars().skip(1).collect();
    let l: String = toks[1].chars().skip(3).collect();
    let kv: String = toks[2].chars().skip(2).collect();
    (
        Some(crate::jd! {"q" => q, "len" => l, "kv" => kv, "base" => toks[..3].join("-"),
            "tail" => J::List(rest_j), "timestamp" => ts}),
        None,
    )
}

pub fn split_recipe(recipe: &str) -> HR<Option<J>> {
    Ok(_split_recipe_raw(&J::from(recipe)).0)
}

// ── 판정 (순수) ─────────────────────────────────────────────────────────────────────────────────
fn arch_why(arch: &str) -> Option<&'static str> {
    if arch.is_empty() {
        return Some("HINT_ARCH_ABSENT");
    }
    if _ARCH_V6_LIKE_RE().is_match(arch) {
        let m = match _ARCH_V6_RE().match_at_start(arch) {
            Some(m) => m,
            None => return Some("HINT_ARCH_SHAPE_VIOLATION"),
        };
        let cluster = named(&m, "role") == Some("cluster");
        if cluster != (digits(named(&m, "n").unwrap_or("")) >= 2) {
            return Some("HINT_ARCH_ROLE_NODES_MISMATCH");
        }
        return None;
    }
    if _ARCH_LEGACY_NODE_RE().is_match(arch) {
        return Some("HINT_ARCH_LEGACY_GRAMMAR");
    }
    if _ARCH_LEGACY_RE().is_match(arch) {
        return Some("HINT_ARCH_NODE_AXIS_ABSENT");
    }
    Some("HINT_ARCH_SHAPE_VIOLATION")
}

/// arch 세그먼트 위반 사유코드(정상이면 None). **arch 문법의 단일 소유자.**
pub fn arch_violation(arch: &str) -> HR<Option<String>> {
    Ok(arch_why(arch).map(str::to_string))
}

/// v7 레시피(v6 포함) 위반 사유코드(정상이면 None).
pub fn recipe_violation(recipe: &str) -> HR<Option<String>> {
    Ok(_split_recipe_raw(&J::from(recipe)).1.map(str::to_string))
}

fn _arch_message(why: &str) -> String {
    match why {
        "HINT_ARCH_ABSENT" => "arch 세그먼트가 비었다.".into(),
        "HINT_ARCH_SHAPE_VIOLATION" => "arch 형태 위반(소문자 <hw>-<G>g<N>n-<main|sub|cluster>-<native|sim-<hw>>[-<plane>]).".into(),
        "HINT_ARCH_ROLE_NODES_MISMATCH" => "노드 축과 노드 수가 모순된다 — cluster 는 N≥2, main·sub 는 N=1 이다.".into(),
        "HINT_ARCH_LEGACY_GRAMMAR" => "옛 arch 문법(<hw>-<main|sub|cluster>-<target> · 2026-09-06 세대)이다 — 신규 이름은 GPU 수·노드 수를 분리한 \
             <hw>-<G>g<N>n-<role>-<target> 을 쓴다(D7). 옛 `x2` 는 gb10x2(2노드)와 rtxpro6000x2(2 GPU)를 구분하지 못했다(F13)."
            .into(),
        "HINT_ARCH_NODE_AXIS_ABSENT" => "arch 에 **노드 축이 없다**(2026-09-04 세대 <hw>-<target>). hint 태그는 '이 조합이 된다'가 아니라 \
             '어느 노드 형상이 수행한 기록'이다 — 축이 없으면 같은 하드웨어의 메인·서브가 같은 이름을 원하고, 서브의 \
             기록은 완주했어도 발행되지 못한다(2026-09-05 실측)."
            .into(),
        other => other.into(),
    }
}

pub fn validate_new_name(tag: &str) -> HR<()> {
    let t = parse_tag(tag)?;
    let remedy = Some("이름은 손으로 짓지 않는다 — `hint.py name --campaign <id> --cell <cell>` 로 파생 이름과 축별 출처를 본다.");
    if !_VLLM_SEGMENT_RE().is_match(&t.vllm) {
        return fail(
            "HINT_VLLM_SEGMENT_SHAPE",
            format!("vLLM 세그먼트 형태 위반({}) — 릴리스(`0.29.0rc6`, v 없음) 또는 `<직전 릴리스>-g<sha12>` 만 쓴다(X18).", py_repr(&t.vllm)),
            remedy,
        );
    }
    if !_MODEL_SLUG_RE().is_match(&t.model) {
        return fail("HINT_MODEL_SLUG_SHAPE", format!("모델 슬러그 형태 위반({}) — 소문자 [a-z0-9._-] 파생값만 쓴다.", py_repr(&t.model)), remedy);
    }
    if let Some(why) = arch_why(&t.arch) {
        return fail(why, format!("{} arch={}", _arch_message(why), py_repr(&t.arch)), remedy);
    }
    if let (_, Some(why)) = _split_recipe_raw(&J::from(t.recipe.as_str())) {
        return fail(
            why,
            format!(
                "레시피 세그먼트 형태 위반({}) — q<quant>-len<n>-kv<dtype>[-<꼬리 토큰>…][-t<YYMMDDHHMM>] \
                 (3축 순서 고정 · 꼬리 토큰 [a-z0-9]+ ≤{TAIL_TOKEN_MAX}자 · 꼬리 ≤{TAIL_MAX}자 · 중복 ✗ · timestamp 는 맨 끝만 · \
                 plan_26092908 §4.1).",
                py_repr(&t.recipe)
            ),
            remedy,
        );
    }
    Ok(())
}

// ── 조립 ─────────────────────────────────────────────────────────────────────────────────────
/// `_pos_int(value, what)` → the decimal text of the positive integer.
fn _pos_int(value: &J, what: &str) -> HR<String> {
    let text = match value {
        J::Int(i) => i.to_string(),
        J::Str(s) => pstrip(s),
        _ => return fail("HINT_AXIS_VALUE_INVALID", format!("{what} 는 양의 정수여야 한다: {}", value.repr()), None),
    };
    if !_POS_INT_RE().is_full(&text) {
        return fail("HINT_AXIS_VALUE_INVALID", format!("{what} 는 양의 정수여야 한다: {}", value.repr()), None);
    }
    Ok(digits_str(&text))
}

fn build_arch_j(hw: &J, gpus_per_node: &J, nodes: &J, role: &J, target: &J, plane_token: &J) -> HR<String> {
    if !hw.as_str().map(tok_match).unwrap_or(false) {
        return fail("HINT_ARCH_SHAPE_VIOLATION", format!("hw 토큰은 [a-z0-9]+ 이어야 한다(vocab 정규화 결과): {}", hw.repr()), None);
    }
    if !j_in(role, &NODE_AXIS) {
        return fail("HINT_ARCH_SHAPE_VIOLATION", format!("노드 축은 {} 중 하나여야 한다: {}", tuple_repr(&NODE_AXIS), role.repr()), None);
    }
    let ptok = match plane_token.as_str() {
        Some(p) if p.is_empty() || tok_match(p) => p,
        _ => {
            return fail(
                "HINT_ARCH_SHAPE_VIOLATION",
                format!("평면 토큰은 빈 문자열 또는 [a-z0-9]+ 이어야 한다(vocab plane 값): {}", plane_token.repr()),
                None,
            )
        }
    };
    let g = _pos_int(gpus_per_node, "gpus_per_node(G)")?;
    let n = _pos_int(nodes, "nodes(N)")?;
    let mut arch = format!("{}-{g}g{n}n-{}-{}", hw.display(), role.display(), target.display());
    if !ptok.is_empty() {
        arch.push('-');
        arch.push_str(ptok);
    }
    if let Some(why) = arch_why(&arch) {
        return fail(why, format!("조립한 arch 가 문법을 위반한다: {} — {}", py_repr(&arch), _arch_message(why)), None);
    }
    Ok(arch)
}

/// 축 5개(+평면 토큰) → arch 세그먼트.
pub fn build_arch(hw: &str, gpus_per_node: i64, nodes: i64, role: &str, target: &str, plane_token: &str) -> HR<String> {
    build_arch_j(&J::from(hw), &J::Int(gpus_per_node), &J::Int(nodes), &J::from(role), &J::from(target), &J::from(plane_token))
}

fn build_recipe_j(q: &J, length: &J, kv: &J, ple: &J, spec: &J, graph: &J) -> HR<String> {
    for (what, tok) in [("q", q), ("kv", kv), ("ple", ple)] {
        if !tok.as_str().map(tok_match).unwrap_or(false) {
            return fail("HINT_RECIPE_SHAPE_VIOLATION", format!("{what} 토큰은 [a-z0-9]+ 이어야 한다(vocab 정규화 결과): {}", tok.repr()), None);
        }
    }
    if !j_in(graph, &_GRAPH_TOKENS) {
        return fail("HINT_RECIPE_SHAPE_VIOLATION", format!("graph 축은 graph|eager 둘 중 하나다: {}", graph.repr()), None);
    }
    let n = _pos_int(length, "max-model-len(len)")?;
    let k = if spec.is_null() || spec.as_str() == Some("off") { "off".to_string() } else { _pos_int(spec, "speculative k(spec)")? };
    let recipe = format!("q{}-len{n}-kv{}-ple{}-spec{k}-{}", q.display(), kv.display(), ple.display(), graph.display());
    if !_RECIPE_V6_RE().is_match(&recipe) {
        return fail("HINT_RECIPE_SHAPE_VIOLATION", format!("조립한 v6 레시피가 문법을 위반한다: {}", py_repr(&recipe)), None);
    }
    Ok(recipe)
}

/// **v6** 축 6개(토큰) → v6 레시피 세그먼트. spec = None/"off" → specoff.
pub fn build_recipe(q: &str, length: &J, kv: &str, ple: &str, spec: &J, graph: &str) -> HR<String> {
    build_recipe_j(&J::from(q), length, &J::from(kv), &J::from(ple), spec, &J::from(graph))
}

fn build_recipe_base_j(q: &J, length: &J, kv: &J) -> HR<String> {
    for (what, tok) in [("q", q), ("kv", kv)] {
        if !tok.as_str().map(tok_match).unwrap_or(false) {
            return fail("HINT_RECIPE_SHAPE_VIOLATION", format!("{what} 토큰은 [a-z0-9]+ 이어야 한다(vocab 정규화 결과): {}", tok.repr()), None);
        }
    }
    let recipe = format!("q{}-len{}-kv{}", q.display(), _pos_int(length, "max-model-len(len)")?, kv.display());
    if let (_, Some(why)) = _split_recipe_raw(&J::from(recipe.as_str())) {
        return fail(why, format!("조립한 기본 레시피가 문법을 위반한다: {}", py_repr(&recipe)), None);
    }
    Ok(recipe)
}

/// v7 결정론 3축(토큰) → **기본** 레시피 `q<quant>-len<n>-kv<dtype>`.
pub fn build_recipe_base(q: &str, length: &J, kv: &str) -> HR<String> {
    build_recipe_base_j(&J::from(q), length, &J::from(kv))
}

fn _payload_tail_tokens(naming: &J) -> HR<Vec<J>> {
    let tail = match naming.get("tail") {
        None => return Ok(Vec::new()),
        Some(t) => t,
    };
    let rows = match tail {
        J::List(l) => l,
        other => return fail("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming.tail 은 목록이어야 한다: {}", other.repr()), None),
    };
    let mut toks = Vec::new();
    for row in rows {
        match row.get("token") {
            Some(t @ J::Str(_)) => toks.push(t.clone()),
            _ => return fail("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming.tail 행에 token 문자열이 없다: {}", row.repr()), None),
        }
    }
    Ok(toks)
}

fn j_strs(v: &[J]) -> Vec<String> {
    v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
}

pub fn compose_from_payload(naming: &J) -> HR<String> {
    let caught = |e: HintError| -> HintError {
        if is_key_or_type(&e) {
            HintError::new("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming 에 segments/axes 칸이 없다: {}", exc_repr(&e)), None)
        } else {
            e
        }
    };
    let mut seg = Map::new();
    for k in ["vllm", "model", "arch", "recipe"] {
        let v = (|| -> HR<J> { Ok(py_sub(py_sub(py_sub(naming, "segments")?, k)?, "value")?.clone()) })().map_err(caught)?;
        seg.insert(k, v);
    }
    let mut ax = Map::new();
    for k in ARCH_AXES.iter().chain(RECIPE_AXES.iter()) {
        let v = (|| -> HR<J> { Ok(py_sub(py_sub(py_sub(naming, "axes")?, k)?, "value")?.clone()) })().map_err(caught)?;
        ax.insert(*k, v);
    }
    let axes = &naming["axes"];
    let mut ptok = String::new();
    if let Some(pl) = axes.get(PLANE_AXIS) {
        let ok = pl.is_dict() && j_in(&pl["value"], &PLANE_NAMES) && pl["token"].is_str();
        if !ok {
            return fail(
                "HINT_NAMING_INCONSISTENT",
                format!("PAYLOAD.naming.axes.plane 모양 위반(value∈{} · token 문자열): {}", tuple_repr(&PLANE_NAMES), pl.repr()),
                None,
            );
        }
        ptok = pl.get_str("token").unwrap_or("").to_string();
        if (pl["value"].as_str() == Some(_PLANE_NO_TOKEN)) != ptok.is_empty() {
            return fail(
                "HINT_NAMING_INCONSISTENT",
                format!("평면 {} 와 토큰 {} 가 모순된다(docker ⇔ 토큰 없음 · 문법 불변식)", pl["value"].repr(), py_repr(&ptok)),
                None,
            );
        }
    }
    let g = |k: &str| ax.get(k).cloned().unwrap_or(J::Null);
    let arch = build_arch_j(&g("hw"), &g("gpus_per_node"), &g("nodes"), &g("role"), &g("target"), &J::from(ptok.as_str()))?;
    let base = build_recipe_base_j(&g("q"), &g("len"), &g("kv"))?;
    let grammar = naming.get("grammar").cloned().unwrap_or(J::Null);
    let recipe;
    if grammar.as_str() == Some(GRAMMAR_V7) {
        let toks = _payload_tail_tokens(naming)?;
        let probs = _tail_token_problems(&toks);
        if !probs.is_empty() {
            return fail("HINT_TAIL_FORMAT", format!("PAYLOAD.naming.tail 형식 위반: {}", probs.join(" · ")), None);
        }
        let ts = naming.get("timestamp").cloned().unwrap_or(J::Null);
        if !ts.is_null() && !ts.as_str().map(|s| _TS_TOKEN_RE().is_match(s)).unwrap_or(false) {
            return fail("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming.timestamp 는 't<YYMMDDHHMM>' 토큰이어야 한다: {}", ts.repr()), None);
        }
        let mut parts = vec![base.clone()];
        parts.extend(j_strs(&toks));
        if ts.truthy() {
            parts.push(ts.display());
        }
        recipe = parts.join("-");
    } else if grammar.is_null() || grammar.as_str() == Some(GRAMMAR_V6) {
        let have: Vec<&str> = V6_TAIL_AXES.iter().copied().filter(|k| axes.as_dict().map(|m| m.contains_key(k)).unwrap_or(false)).collect();
        if have.len() == V6_TAIL_AXES.len() {
            let mut v6 = Map::new();
            for k in V6_TAIL_AXES {
                let v = (|| -> HR<J> { Ok(py_sub(py_sub(axes, k)?, "value")?.clone()) })().map_err(|e| {
                    if is_key_or_type(&e) {
                        HintError::new("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming.axes 의 v6 뒤 3축 모양 위반: {}", exc_repr(&e)), None)
                    } else {
                        e
                    }
                })?;
                v6.insert(k, v);
            }
            let gv = |k: &str| v6.get(k).cloned().unwrap_or(J::Null);
            recipe = build_recipe_j(&g("q"), &g("len"), &g("kv"), &gv("ple"), &gv("spec"), &gv("graph"))?;
        } else if have.is_empty() {
            let segr = seg.get("recipe").cloned().unwrap_or(J::Null);
            let sp = _split_recipe_raw(&segr).0;
            let ok = match &sp {
                Some(sp) => sp["timestamp"].is_null() && _RECIPE_V6_RE().is_match(&segr.display()),
                None => false,
            };
            if !ok {
                return fail("HINT_NAMING_INCONSISTENT", format!("v6 페이로드의 레시피 세그먼트가 v6 레시피 모양이 아니다: {}", segr.repr()), None);
            }
            let mut parts = vec![base.clone()];
            parts.extend(j_strs(sp.unwrap()["tail"].items()));
            recipe = parts.join("-");
        } else {
            return fail(
                "HINT_NAMING_INCONSISTENT",
                format!("v6 뒤 3축이 일부만 있다({}) — 전부 있거나(6축 재조립) 전부 없어야 한다(3축 + 꼬리)", list_repr(&have)),
                None,
            );
        }
    } else {
        return fail("HINT_NAMING_INCONSISTENT", format!("PAYLOAD.naming.grammar 가 v6·v7 이 아니다: {}", grammar.repr()), None);
    }
    let sarch = seg.get("arch").cloned().unwrap_or(J::Null);
    let srec = seg.get("recipe").cloned().unwrap_or(J::Null);
    if sarch.as_str() != Some(arch.as_str()) || srec.as_str() != Some(recipe.as_str()) {
        return fail(
            "HINT_NAMING_INCONSISTENT",
            format!(
                "PAYLOAD.naming 의 축과 세그먼트가 어긋난다: arch {}≠{} 또는 recipe {}≠{}",
                sarch.repr(),
                py_repr(&arch),
                srec.repr(),
                py_repr(&recipe)
            ),
            None,
        );
    }
    let sd = |k: &str| seg.get(k).map(J::display).unwrap_or_default();
    let tag = format!("{}{}/{}/{arch}/{recipe}", core::HINT_TAG_PREFIX, sd("vllm"), sd("model"));
    validate_new_name(&tag)?;
    Ok(tag)
}

// ── 꼬리 · timestamp ────────────────────────────────────────────────────────────────────────
/// 꼬리 토큰 목록의 **형식** 문제(빈 목록 = 정상).
pub fn _tail_token_problems(tokens: &[J]) -> Vec<String> {
    let mut bad = Vec::new();
    for t in tokens {
        let s = match t.as_str() {
            Some(s) if tok_match(s) => s,
            _ => {
                bad.push(format!("토큰 {} 가 [a-z0-9]+ 가 아니다", t.repr()));
                continue;
            }
        };
        if clen(s) > TAIL_TOKEN_MAX {
            bad.push(format!("토큰 {} 가 {TAIL_TOKEN_MAX}자를 넘는다({}자)", t.repr(), clen(s)));
        }
        if _TS_TOKEN_RE().is_match(s) {
            bad.push(format!("토큰 {} 는 timestamp 예약 모양(t<10자리>)이다", t.repr()));
        }
    }
    let strs = j_strs(tokens);
    let mut dup: Vec<String> = strs.iter().filter(|t| strs.iter().filter(|x| x == t).count() > 1).cloned().collect();
    dup.sort();
    dup.dedup();
    if !dup.is_empty() {
        bad.push(format!("중복 토큰 {}", list_repr(&dup)));
    }
    let total = clen(&strs.join("-"));
    if total > TAIL_MAX {
        bad.push(format!("꼬리 전체가 {TAIL_MAX}자를 넘는다({total}자)"));
    }
    bad
}

/// 근거 값 대조 정규화 — 공백 압축 · 바깥 따옴표 한 겹 · 대소문자.
pub fn _norm_value(v: &J) -> String {
    let text = v.display();
    let mut s = psplit(&text).join(" ");
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 && chars[0] == chars[chars.len() - 1] && (chars[0] == '\'' || chars[0] == '"') {
        let inner: String = chars[1..chars.len() - 1].iter().collect();
        s = psplit(&inner).join(" ");
    }
    casefold(&s)
}

pub fn validate_tail(tail: &J, sources: &J) -> HR<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    let rows = match tail {
        J::List(l) => l,
        other => return Ok(vec![("HINT_TAIL_FORMAT".into(), format!("꼬리는 행 목록이어야 한다: {}", other.type_name()))]),
    };
    let empty = Map::new();
    let srcs = sources.as_dict().unwrap_or(&empty);
    let mut toks: Vec<J> = Vec::new();
    let mut push = |c: &str, m: String| out.push((c.to_string(), m));
    for (i, row) in rows.iter().enumerate() {
        let r = match row.as_dict() {
            Some(r) => r,
            None => {
                push("HINT_TAIL_FORMAT", format!("꼬리 {i}번 행이 객체가 아니다: {}", row.repr()));
                continue;
            }
        };
        let tok = r.get("token").cloned().unwrap_or(J::Null);
        let where_ = format!("꼬리 토큰 {}", tok.repr());
        toks.push(tok);
        let meaning = r.get("meaning");
        if !meaning.and_then(J::as_str).map(|m| !pstrip(m).is_empty()).unwrap_or(false) {
            push("HINT_TAIL_MEANING_ABSENT", format!("{where_} 의 meaning 이 비었다 — 이 토큰이 무엇을 가르는지 한 줄로 적는다"));
        }
        let ev = r.get("evidence").cloned().unwrap_or(J::Null);
        let ev_ok = match &ev {
            J::Dict(e) => {
                ["file", "key"].iter().all(|k| e.get(k).and_then(J::as_str).map(|s| !pstrip(s).is_empty()).unwrap_or(false))
                    && e.get("value").map(|v| !v.is_null()).unwrap_or(false)
            }
            _ => false,
        };
        if !ev_ok {
            push("HINT_TAIL_UNGROUNDED", format!("{where_} 의 evidence 에 file·key·value 가 다 있지 않다: {}", ev.repr()));
            continue;
        }
        let f = pstrip(ev.get_str("file").unwrap_or(""));
        let k = pstrip(ev.get_str("key").unwrap_or(""));
        let table = match srcs.get(&f) {
            Some(J::Dict(t)) => t,
            _ => {
                let keys: Vec<String> = crate::value::sorted_strs(srcs.keys());
                push("HINT_TAIL_UNGROUNDED", format!("{where_} 의 근거 파일 {} 이 이 셀의 서빙 설정 목록에 없다({})", py_repr(&f), list_repr(&keys)));
                continue;
            }
        };
        let actual = match table.get(&k) {
            Some(a) => a,
            None => {
                push("HINT_TAIL_UNGROUNDED", format!("{where_} 의 근거 키 {} 가 {f} 에 없다", py_repr(&k)));
                continue;
            }
        };
        if _norm_value(actual) != _norm_value(&ev["value"]) {
            push(
                "HINT_TAIL_UNGROUNDED",
                format!("{where_} 의 근거 값 {} ≠ {f}:{k} 의 실제 값 {}", ev["value"].repr(), actual.repr()),
            );
        }
    }
    for p in _tail_token_problems(&toks) {
        push("HINT_TAIL_FORMAT", p);
    }
    if toks.len() >= 3 && toks.iter().all(J::is_str) {
        let joined = j_strs(&toks).join("-");
        if _RECIPE_V6_RE().is_match(&format!("qx-len1-kvx-{joined}")) {
            push(
                "HINT_TAIL_FORMAT",
                format!(
                    "꼬리 {} 가 v6 뒤 3축 모양 그대로다 — 이름이 v6 로 읽힌다. 토큰을 줄이거나 \
                     순서·철자를 바꿔 이 셀을 가르는 노브만 적는다(예: `mmap-spec3-eager`)",
                    py_repr(&joined)
                ),
            );
        }
    }
    Ok(out)
}

pub fn compose_name(base_tag: &str, tail: &J) -> HR<String> {
    let t = parse_tag(base_tag)?;
    if t.base_tag().as_deref() != Some(base_tag) || !t.tail.is_empty() || t.timestamp.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
        return fail(
            "HINT_TAIL_FORMAT",
            format!("기본 이름이 아니다(꼬리·timestamp 가 이미 있다): {}", py_repr(base_tag)),
            Some("derive_name(...).tag 를 넘긴다."),
        );
    }
    let rows = match tail {
        J::List(l) => l,
        other => return fail("HINT_TAIL_FORMAT", format!("꼬리는 목록이어야 한다: {}", other.repr()), None),
    };
    let toks: Vec<J> = rows.iter().map(|r| if r.is_dict() { r.get("token").cloned().unwrap_or(J::Null) } else { r.clone() }).collect();
    let probs = _tail_token_problems(&toks);
    if !probs.is_empty() {
        return fail("HINT_TAIL_FORMAT", format!("꼬리 형식 위반: {}", probs.join(" · ")), None);
    }
    let mut parts = vec![base_tag.to_string()];
    parts.extend(j_strs(&toks));
    let name = parts.join("-");
    validate_new_name(&name)?;
    Ok(name)
}

/// generated_utc(주입 UTC) → `t<YYMMDDHHMM>`(KST).
pub fn timestamp_token(generated_utc: &str) -> HR<String> {
    let k = core::Civil::from_epoch(core::parse_utc(generated_utc)?.epoch() + 9 * 3600);
    Ok(format!("t{:02}{:02}{:02}{:02}{:02}", k.year.rem_euclid(100), k.month, k.day, k.hour, k.minute))
}

pub fn with_timestamp(name: &str, generated_utc: &str) -> HR<String> {
    let t = parse_tag(name)?;
    if t.base_tag().is_none() {
        return fail("HINT_NAME_GRAMMAR_OLD", format!("v6/v7 이름이 아니다(문법 {}): {}", t.grammar, py_repr(name)), None);
    }
    if let Some(ts) = t.timestamp.as_deref().filter(|s| !s.is_empty()) {
        return fail(
            "HINT_TAIL_FORMAT",
            format!("이미 timestamp({ts})가 있다 — 같은 분에 같은 이름 2건은 차단이다: {}", py_repr(name)),
            Some("publish 를 다시 실행해 새 generated_utc 로 이름을 만든다."),
        );
    }
    let out = format!("{name}-{}", timestamp_token(generated_utc)?);
    validate_new_name(&out)?;
    Ok(out)
}

/// `for r in (tail or [])` items.
fn iter_items(tail: &J) -> HR<Vec<J>> {
    if !tail.truthy() {
        return Ok(Vec::new());
    }
    match tail {
        J::List(l) => Ok(l.clone()),
        J::Dict(m) => Ok(m.keys().map(J::from).collect()),
        J::Str(s) => Ok(s.chars().map(|c| J::Str(c.to_string())).collect()),
        other => Err(HintError::py("TypeError", format!("'{}' object is not iterable", other.type_name()))),
    }
}

pub fn apply_tail(naming: &J, tail: &J, timestamp: Option<&str>) -> HR<J> {
    let grammar = or_get(naming, "grammar")?.cloned().unwrap_or(J::Null);
    if !naming.is_dict() || grammar.as_str() != Some(GRAMMAR_V7) {
        return fail("HINT_NAMING_INCONSISTENT", format!("v7 PAYLOAD.naming 이 아니다(grammar={})", grammar.repr()), None);
    }
    let mut doc = naming.clone();
    let rows: Vec<J> = iter_items(tail)?
        .into_iter()
        .map(|r| if r.is_dict() { r } else { crate::jd! {"token" => r} })
        .collect();
    let toks: Vec<J> = rows.iter().map(|r| r.get("token").cloned().unwrap_or(J::Null)).collect();
    let probs = _tail_token_problems(&toks);
    if !probs.is_empty() {
        return fail("HINT_TAIL_FORMAT", format!("꼬리 형식 위반: {}", probs.join(" · ")), None);
    }
    if let Some(ts) = timestamp {
        if !_TS_TOKEN_RE().is_match(ts) {
            return fail("HINT_TAIL_FORMAT", format!("timestamp 는 timestamp_token() 의 't<YYMMDDHHMM>' 이어야 한다: {}", py_repr(ts)), None);
        }
    }
    let ax = py_sub(&doc, "axes")?;
    let base = build_recipe_base_j(py_sub(py_sub(ax, "q")?, "value")?, py_sub(py_sub(ax, "len")?, "value")?, py_sub(py_sub(ax, "kv")?, "value")?)?;
    let mut parts = vec![base];
    parts.extend(j_strs(&toks));
    if let Some(ts) = timestamp.filter(|s| !s.is_empty()) {
        parts.push(ts.to_string());
    }
    let recipe = parts.join("-");
    let mut src = "derived(q·len·kv)".to_string();
    if !toks.is_empty() {
        src.push_str(" + tail(agent · naming.tail[] 근거)");
    }
    if timestamp.map(|s| !s.is_empty()).unwrap_or(false) {
        src.push_str(" + timestamp(이름 중복 · generated_utc KST)");
    }
    doc.set("tail", J::List(rows));
    doc.set("timestamp", J::from(timestamp));
    {
        let segs = doc.get_mut("segments").ok_or_else(|| HintError::py("KeyError", py_repr("segments")))?;
        match segs {
            J::Dict(m) => m.insert("recipe", crate::jd! {"value" => recipe, "source" => src}),
            J::List(_) => return Err(HintError::py("TypeError", "list indices must be integers or slices, not str")),
            other => return Err(HintError::py("TypeError", format!("'{}' object does not support item assignment", other.type_name()))),
        }
    }
    compose_from_payload(&doc)?;
    Ok(doc)
}

pub fn tail_candidates(facts: &J, vocab: &J) -> HR<Vec<J>> {
    let mut out: Vec<J> = Vec::new();
    for axis in V6_TAIL_AXES {
        let d = facts.get(axis);
        let r: HR<(String, String, String)> = (|| {
            if axis == "spec" {
                let (sd, ssrc) = _facts_axis(facts, "spec")?;
                let raw = _raw("spec", sd, &ssrc, true)?;
                let off = raw.is_null() || raw.as_str().map(|s| python_lower(&pstrip(s)) == "off").unwrap_or(false);
                let k = if off { "off".to_string() } else { _pos_int(&raw, "speculative k(spec)")? };
                let val = if k == "off" { "끔".to_string() } else { format!("k={k}") };
                Ok((format!("spec{k}"), val, ssrc))
            } else {
                let ax = _vocab_axis(axis, axis, facts, vocab)?;
                let tok = if axis == "ple" { format!("ple{}", ax.value) } else { ax.value.clone() };
                Ok((tok, ax.value, ax.source))
            }
        })();
        let (tok, val, src) = match r {
            Ok(x) => x,
            Err(e) if !e.is_py() => {
                out.push(crate::jd! {"axis" => axis, "token" => J::Null, "error" => e.code, "message" => e.message});
                continue;
            }
            Err(e) => return Err(e),
        };
        let ev = d.and_then(|d| d.get("evidence"));
        let ev = match ev {
            Some(J::Dict(e)) if ["file", "key", "value"].iter().all(|k| e.get(k).map(|v| !v.is_null()).unwrap_or(false)) => {
                let mut m = Map::new();
                for k in ["file", "key", "value"] {
                    m.insert(k, e.get(k).unwrap().display());
                }
                J::Dict(m)
            }
            _ => J::Null,
        };
        let meaning = match axis {
            "ple" => format!("PLE(per-layer embedding) 적재 모드 {val}"),
            "spec" => format!("speculative decoding {val}"),
            _ => format!("실행 모드 {val}(cudagraph|eager)"),
        };
        out.push(crate::jd! {"axis" => axis, "token" => tok, "meaning" => meaning, "evidence" => ev, "source" => src});
    }
    Ok(out)
}

// ── 슬러그 ───────────────────────────────────────────────────────────────────────────────────
/// `os.path.basename(p)`.
fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub fn derive_slug(hf_repo: Option<&str>, model_path: Option<&str>) -> HR<String> {
    let (slug, src);
    if let Some(h0) = hf_repo.filter(|s| !s.is_empty()) {
        let h = pstrip(h0).trim_end_matches('/').to_string();
        if !h.contains('/') {
            return fail("HINT_MODEL_HF_REPO_SHAPE", format!("hf_repo 는 '<org>/<name>' 형태여야 한다: {}", py_repr(h0)), None);
        }
        slug = python_lower(h.rsplit('/').next().unwrap_or(""));
        src = format!("hf_repo({h})");
    } else if let Some(mp) = model_path.filter(|s| !s.is_empty()) {
        let base = pstrip(basename(mp.trim_end_matches('/')));
        if base.is_empty() {
            return fail(
                "HINT_AXIS_UNDERIVABLE",
                format!("model_path 에서 이름을 뽑지 못했다: {}", py_repr(mp)),
                Some(&format!("필요한 증거: {}", _axis_evidence("model"))),
            );
        }
        slug = python_lower(&base);
        src = format!("model_path basename({base})");
    } else {
        return fail(
            "HINT_AXIS_UNDERIVABLE",
            "hf_repo(정본) 또는 model_path(체크포인트 경로) 중 하나가 필요하다 — 슬러그는 발행자가 짓지 않는다(plan_26082008 R1).",
            Some(&format!("필요한 증거: {}", _axis_evidence("model"))),
        );
    }
    if !_MODEL_SLUG_RE().is_match(&slug) {
        return fail(
            "HINT_MODEL_SLUG_UNSAFE",
            format!(
                "{src} → {} 는 태그 슬러그 문자([a-z0-9._-], 첫 글자 영숫자) 밖이다. 치환하면 인증서 model 키(체크포인트 basename 소문자)와 갈라진다.",
                py_repr(&slug)
            ),
            Some("체크포인트 디렉터리 이름을 [A-Za-z0-9._-] 로 바꿔 다시 서빙·측정한다(인증서 키와 태그 슬러그가 같은 원문에서 나와야 한다)."),
        );
    }
    Ok(slug)
}

pub fn base_slug(slug: &str, vocab: &J) -> HR<String> {
    let sfx = match vocab.get("quant_suffixes") {
        Some(J::List(l)) => l,
        _ => {
            return fail(
                "HINT_VOCAB_ABSENT",
                "어휘표에 quant_suffixes 목록이 없다 — 기반 슬러그를 파생할 수 없다",
                Some("load_vocab 으로 적재한 어휘표를 넘긴다."),
            )
        }
    };
    let suffixes: Vec<String> = sfx.iter().map(|s| python_lower(&s.display())).collect();
    let mut s = slug.to_string();
    loop {
        let cut = match (s.rfind('-'), s.rfind('_')) {
            (None, None) => return Ok(s),
            (a, b) => a.max(b).unwrap(),
        };
        if cut == 0 || !suffixes.contains(&python_lower(&s[cut + 1..])) {
            return Ok(s);
        }
        s.truncate(cut);
    }
}

/// 슬러그 비교용 정규화(None 은 호출부에서 빈 키 — Python `norm_slug(None) == ""`).
pub fn norm_slug(s: &str) -> HR<String> {
    Ok(python_lower(s).chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect())
}

/// `version_key` tuple: (M, m, p, q, stage rank, stage n, post, after-release, text).
pub type VersionKey = (i128, i128, i128, i128, i128, i128, i128, i128, String);

pub fn version_key(v: &str) -> HR<VersionKey> {
    let m = match _VERSION_KEY_RE().match_at_start(v) {
        Some(m) => m,
        None => return Ok((-1, -1, -1, -1, -1, -1, -1, 0, v.to_string())),
    };
    let gi = |i: usize| crate::pyre::group(&m, i);
    let rank = match gi(5) {
        Some("dev") => 0,
        Some("a") => 1,
        Some("b") => 2,
        Some("rc") => 3,
        Some("post") => 5,
        _ => 4,
    };
    Ok((
        digits(gi(1).unwrap_or("")),
        digits(gi(2).unwrap_or("")),
        digits(gi(3).unwrap_or("")),
        gi(4).map(digits).unwrap_or(0),
        rank,
        gi(6).map(digits).unwrap_or(0),
        gi(7).map(digits).unwrap_or(-1),
        if gi(8).map(|s| !s.is_empty()).unwrap_or(false) || gi(9).map(|s| !s.is_empty()).unwrap_or(false) { 1 } else { 0 },
        v.to_string(),
    ))
}

// ── 어휘 ─────────────────────────────────────────────────────────────────────────────────────
/// 어휘 매칭 키: 대소문자·공백만 정규화.
pub fn _vnorm(text: &J) -> String {
    casefold(&psplit(&text.display()).join(" "))
}
fn vnorm_s(s: &str) -> String {
    casefold(&psplit(s).join(" "))
}

fn eq_one(v: Option<&J>) -> bool {
    matches!(v, Some(J::Int(1)) | Some(J::Bool(true))) || matches!(v, Some(J::Float(f)) if *f == 1.0)
}

pub fn validate_vocab(vocab: &J) -> HR<Vec<String>> {
    let mut bad: Vec<String> = Vec::new();
    let v = match vocab.as_dict() {
        Some(v) => v,
        None => return Ok(vec!["최상위가 객체가 아니다".into()]),
    };
    if !eq_one(v.get("schema_version")) {
        bad.push(format!("schema_version 이 1 이 아니다: {}", v.get("schema_version").cloned().unwrap_or(J::Null).repr()));
    }
    let known: Vec<&str> = VOCAB_AXES.iter().copied().chain(["schema_version", "quant_suffixes", "hw_ambiguous", PLANE_AXIS]).collect();
    for k in v.keys() {
        if !k.starts_with('_') && !known.contains(&k) {
            bad.push(format!("알 수 없는 키(오타?): {}", py_repr(k)));
        }
    }
    for axis in VOCAB_AXES {
        let table = match v.get(axis) {
            Some(J::Dict(t)) if !t.is_empty() => t,
            _ => {
                bad.push(format!("{axis}: 토큰 사전이 없다"));
                continue;
            }
        };
        let mut seen: Vec<(String, String)> = Vec::new();
        for (tok, aliases) in table.iter() {
            if !tok_match(tok) {
                bad.push(format!("{axis}: 토큰 {} 가 [a-z0-9]+ 가 아니다", py_repr(tok)));
            }
            let al = match aliases {
                J::List(l) if !l.is_empty() => l,
                _ => {
                    bad.push(format!("{axis}.{tok}: 원문 목록이 비었다"));
                    continue;
                }
            };
            for a in al {
                let s = match a.as_str() {
                    Some(s) if !pstrip(s).is_empty() => s,
                    _ => {
                        bad.push(format!("{axis}.{tok}: 빈 원문"));
                        continue;
                    }
                };
                let key = vnorm_s(s);
                if _is_unobserved(a) {
                    bad.push(format!("{axis}.{tok}: {} 는 '읽지 못함' 표기(또는 템플릿 빈칸)라 값으로 등재할 수 없다", py_repr(s)));
                }
                match seen.iter_mut().find(|(k, _)| *k == key) {
                    Some(slot) => {
                        if slot.1 != tok {
                            bad.push(format!("{axis}: 원문 {} 가 두 토큰({}·{tok})에 걸린다", py_repr(s), slot.1));
                        }
                        slot.1 = tok.to_string();
                    }
                    None => seen.push((key, tok.to_string())),
                }
            }
        }
    }
    if let Some(J::Dict(g)) = v.get("graph") {
        let mut keys: Vec<&str> = g.keys().collect();
        keys.sort();
        keys.dedup();
        if keys != _GRAPH_TOKENS {
            bad.push(format!("graph: 토큰은 문법이 고정한 {} 여야 한다", list_repr(&_GRAPH_TOKENS)));
        }
    }
    if let Some(J::Dict(p)) = v.get("ple") {
        if !p.contains_key("none") {
            bad.push("ple: PLE 없는 모델의 명시 토큰 'none'(plenone)이 없다".into());
        }
    }
    bad.extend(_plane_vocab_problems(v.get(PLANE_AXIS))?);
    let sfx_ok = match v.get("quant_suffixes") {
        Some(J::List(l)) => l.iter().all(|s| s.as_str().map(tok_match).unwrap_or(false)),
        _ => false,
    };
    if !sfx_ok {
        bad.push("quant_suffixes: [a-z0-9]+ 문자열 목록이어야 한다".into());
    }
    match v.get("hw_ambiguous") {
        None | Some(J::Dict(_)) => {
            let empty = Map::new();
            let amb = v.get("hw_ambiguous").and_then(J::as_dict).unwrap_or(&empty);
            if let Some(J::Dict(hw)) = v.get("hw") {
                let hw_keys: Vec<String> =
                    hw.values().filter_map(J::as_list).flat_map(|al| al.iter().filter_map(J::as_str).map(vnorm_s)).collect();
                for a in amb.keys() {
                    if hw_keys.contains(&vnorm_s(a)) {
                        bad.push(format!("hw_ambiguous: {} 가 hw 토큰에도 등재돼 있다(에디션 구분이 무너진다)", py_repr(a)));
                    }
                }
            }
        }
        Some(_) => bad.push("hw_ambiguous: 객체여야 한다".into()),
    }
    Ok(bad)
}

pub fn _plane_vocab_problems(table: Option<&J>) -> HR<Vec<String>> {
    let t = match table {
        Some(J::Dict(t)) => t,
        _ => return Ok(vec![format!("{PLANE_AXIS}: 평면→토큰 사전이 없다(O-N1 · docker=\"\" · native=<토큰>)")]),
    };
    let mut bad = Vec::new();
    let entries: Vec<(&str, &J)> = t.iter().filter(|(k, _)| !k.starts_with('_')).collect();
    let mut ek: Vec<&str> = entries.iter().map(|(k, _)| *k).collect();
    ek.sort();
    if ek != PLANE_NAMES {
        bad.push(format!(
            "{PLANE_AXIS}: 키는 판정 소유자(artifacts._plane)의 닫힌 어휘 {} 여야 한다: {}",
            list_repr(&PLANE_NAMES),
            list_repr(&ek)
        ));
    }
    for (k, v) in &entries {
        let ok = match v.as_str() {
            Some(s) => s.is_empty() || tok_match(s),
            None => false,
        };
        if !ok {
            bad.push(format!("{PLANE_AXIS}.{k}: 토큰은 빈 문자열 또는 [a-z0-9]+ 이어야 한다: {}", v.repr()));
        }
    }
    let docker = entries.iter().find(|(k, _)| *k == _PLANE_NO_TOKEN).map(|(_, v)| *v);
    if !matches!(docker, None | Some(J::Str(_))) || docker.and_then(J::as_str).map(|s| !s.is_empty()).unwrap_or(false) {
        bad.push(format!(
            "{PLANE_AXIS}.{_PLANE_NO_TOKEN}: 빈 문자열이어야 한다(Docker 이름 불변 — 옛·신 태그가 바이트 그대로 파생돼야 한다)"
        ));
    }
    let toks: Vec<&J> = entries.iter().filter(|(k, _)| *k != _PLANE_NO_TOKEN).map(|(_, v)| *v).collect();
    if toks.iter().any(|t| !t.truthy()) {
        bad.push(format!("{PLANE_AXIS}: docker 밖 평면의 토큰이 비었다 — 축이 같은 Docker 셀과 이름이 충돌한다(2026-09-23 F10)"));
    }
    let mut uniq: Vec<&J> = Vec::new();
    for t in &toks {
        if !uniq.contains(t) {
            uniq.push(t);
        }
    }
    if uniq.len() != toks.len() {
        bad.push(format!("{PLANE_AXIS}: 두 평면이 같은 토큰을 쓴다"));
    }
    Ok(bad)
}

pub fn plane_token(plane: &J, vocab: &J) -> HR<String> {
    let table = if vocab.is_dict() { vocab.get(PLANE_AXIS) } else { None };
    let t = match table {
        Some(J::Dict(t)) => t,
        _ => {
            return fail(
                "HINT_VOCAB_ABSENT",
                format!("어휘표에 {PLANE_AXIS} 축이 없다(O-N1 · 2026-09-23)"),
                Some(&format!("`{}` 에 \"{PLANE_AXIS}\": {{\"docker\": \"\", \"native\": \"<토큰>\"}} 를 둔다(사람 편집).", core::REL_VOCAB)),
            )
        }
    };
    let p = match plane.as_str() {
        Some(p) if !p.starts_with('_') && t.contains_key(p) => p,
        _ => {
            return fail(
                "HINT_VOCAB_UNKNOWN",
                format!("평면 {} 가 어휘표 {PLANE_AXIS} 밖이다(tripwire 닫힌 목록)", plane.repr()),
                Some(&format!(
                    "평면은 artifacts.plane_of 가 내는 {} 중 하나다 — 새 평면이면 `{}` 의 \"{PLANE_AXIS}\" 에 토큰을 추가한다(사람 편집).",
                    tuple_repr(&PLANE_NAMES),
                    core::REL_VOCAB
                )),
            )
        }
    };
    let tok = t.get(p).unwrap();
    let ok = match tok.as_str() {
        Some(s) => (s.is_empty() || tok_match(s)) && ((p == _PLANE_NO_TOKEN) == s.is_empty()),
        None => false,
    };
    if !ok {
        return fail("HINT_VOCAB_ABSENT", format!("어휘표 {PLANE_AXIS}.{p} 토큰이 깨졌다: {}", tok.repr()), Some("load_vocab 으로 적재한 어휘표를 넘긴다."));
    }
    Ok(tok.as_str().unwrap().to_string())
}

pub fn load_vocab(repo: &Path) -> HR<J> {
    let path = repo.join(core::REL_VOCAB);
    let remedy = format!("`{}` 을 복구한다(추적 파일 · `git checkout -- {}`).", core::REL_VOCAB, core::REL_VOCAB);
    if !path.is_file() {
        return fail("HINT_VOCAB_ABSENT", format!("어휘표가 없다: {}", core::REL_VOCAB), Some(&remedy));
    }
    let doc = match core::read_text(&path).and_then(|t| loads(&t)) {
        Ok(d) => d,
        Err(e) => return fail("HINT_VOCAB_ABSENT", format!("어휘표를 읽을 수 없다({}): {e}", core::REL_VOCAB), Some(&remedy)),
    };
    let problems = validate_vocab(&doc)?;
    if !problems.is_empty() {
        let shown: Vec<&str> = problems.iter().take(6).map(String::as_str).collect();
        return fail(
            "HINT_VOCAB_ABSENT",
            format!("어휘표가 깨졌다({}): {}", core::REL_VOCAB, shown.join(" · ")),
            Some("tripwire 목록을 고친다(원문 하나는 토큰 하나에만 · 토큰은 [a-z0-9]+)."),
        );
    }
    Ok(doc)
}

pub fn normalize(axis: &str, raw: &J, vocab: &J) -> HR<String> {
    if !VOCAB_AXES.contains(&axis) {
        return fail("HINT_VOCAB_UNKNOWN", format!("어휘 축이 아니다: {} (허용 {})", py_repr(axis), tuple_repr(&VOCAB_AXES)), None);
    }
    let where_ = format!("`{}` 의 \"{axis}\"", core::REL_VOCAB);
    let rs = match raw.as_str() {
        Some(s) if !pstrip(s).is_empty() => s,
        _ => {
            return fail(
                "HINT_VOCAB_UNKNOWN",
                format!("{axis} 원문이 문자열이 아니다: {}", raw.repr()),
                Some(&format!("evidence 가 {axis} 축을 문자열로 넘긴다(예 graph 는 enforce-eager 관측을 eager|graph 로 변환).")),
            )
        }
    };
    if _is_unobserved(raw) {
        return fail(
            "HINT_VOCAB_UNKNOWN",
            format!("{axis} 원문 {} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다 — 값이 아니다", raw.repr()),
            Some(&format!("어휘표에 추가하지 않는다. {axis} 값을 실제로 관측한 증거를 넘긴다(derive_name 은 이 경우 HINT_AXIS_UNDERIVABLE).")),
        );
    }
    let table = match or_get(vocab, axis)? {
        Some(J::Dict(t)) => t,
        _ => return fail("HINT_VOCAB_ABSENT", format!("어휘표에 {axis} 축이 없다"), Some("load_vocab 으로 적재한 어휘표를 넘긴다.")),
    };
    let key = vnorm_s(rs);
    let mut hits: Vec<String> = Vec::new();
    for (tok, aliases) in table.iter() {
        if let J::List(al) = aliases {
            if al.iter().any(|a| a.as_str().map(|s| vnorm_s(s) == key).unwrap_or(false)) && !hits.iter().any(|h| h == tok) {
                hits.push(tok.to_string());
            }
        }
    }
    if hits.len() == 1 {
        return Ok(hits.remove(0));
    }
    if hits.len() > 1 {
        hits.sort();
        return fail(
            "HINT_VOCAB_ABSENT",
            format!("{axis} 원문 {} 가 여러 토큰 {} 에 걸린다(어휘표 깨짐)", raw.repr(), list_repr(&hits)),
            Some(&format!("{where_} 에서 원문 하나는 토큰 하나에만 둔다.")),
        );
    }
    if axis == "hw" {
        let amb_src = or_get(vocab, "hw_ambiguous")?.cloned().unwrap_or(J::Null);
        let amb: Vec<(String, String)> = if !amb_src.truthy() {
            Vec::new()
        } else {
            match &amb_src {
                J::Dict(m) => {
                    let mut v: Vec<(String, String)> = Vec::new();
                    for (a, why) in m.iter() {
                        let k = vnorm_s(a);
                        match v.iter_mut().find(|(x, _)| *x == k) {
                            Some(slot) => slot.1 = why.display(),
                            None => v.push((k, why.display())),
                        }
                    }
                    v
                }
                other => return Err(HintError::py("AttributeError", format!("'{}' object has no attribute 'items'", other.type_name()))),
            }
        };
        if let Some((_, why)) = amb.iter().find(|(k, _)| *k == key) {
            return fail(
                "HINT_VOCAB_UNKNOWN",
                format!("hw 원문 {} 는 에디션이 드러나지 않는다: {why}", raw.repr()),
                Some("에디션이 드러나는 원문을 증거로 넘긴다 — 이 원문을 어느 토큰에 합치지 않는다(합치면 에디션이 섞인다)."),
            );
        }
    }
    fail(
        "HINT_VOCAB_UNKNOWN",
        format!("{axis} 원문 {} 가 어휘표 밖이다(tripwire 닫힌 목록)", raw.repr()),
        Some(&format!(
            "{where_} 에서 이 값이 뜻하는 토큰의 목록에 \"{rs}\" 를 추가한다(사람 편집 · 새 토큰이면 [a-z0-9]+ 키를 만든다{}).",
            if axis == "hw" { " · hw 는 에디션이 구분되는 토큰에만" } else { "" }
        )),
    )
}

// ── 파생 ─────────────────────────────────────────────────────────────────────────────────────
fn _underivable<T>(axis: &str, why: &str, source: Option<&str>) -> HR<T> {
    fail(
        "HINT_AXIS_UNDERIVABLE",
        format!("{axis} 축을 파생하지 못했다: {why} — 이름을 지어내지 않는다."),
        Some(&format!(
            "필요한 증거: {} (관측 기록: {}). `hint.py name --campaign <id> --cell <cell>` 로 축별 출처를 확인한다.",
            _axis_evidence(axis),
            source.filter(|s| !s.is_empty()).unwrap_or("없음")
        )),
    )
}

fn _facts_axis<'a>(facts: &'a J, axis: &str) -> HR<(&'a J, String)> {
    let d = match facts.get(axis) {
        Some(d @ J::Dict(_)) => d,
        _ => return _underivable(axis, "naming facts 에 이 축이 없다", None),
    };
    let src = str_or_none(d.get("source")).unwrap_or_default();
    if src.is_empty() {
        return _underivable(axis, "출처(source)가 비었다 — 출처 없는 값은 이름이 되지 않는다", None);
    }
    Ok((d, src))
}

fn _raw(axis: &str, d: &J, src: &str, none_ok: bool) -> HR<J> {
    let raw = match d.get("raw") {
        None => return _underivable(axis, "raw 키가 없다(읽지 못함 ≠ 없음)", Some(src)),
        Some(r) => r.clone(),
    };
    if raw.is_null() {
        if none_ok {
            return Ok(J::Null);
        }
        return _underivable(axis, "raw 가 None 이다(관측되지 않은 값)", Some(src));
    }
    if _is_unobserved(&raw) {
        return _underivable(
            axis,
            &format!(
                "raw={} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다(인증서·스윕 meta 의 N/A 는 '없음'이 \
                 아니다 — 태그2 PAYLOAD.identity.quant='N/A' 였으나 체크포인트는 NVFP4 혼합이었다)",
                raw.repr()
            ),
            Some(src),
        );
    }
    Ok(raw)
}

fn _vocab_axis(axis_name: &str, vocab_axis: &str, facts: &J, vocab: &J) -> HR<Axis> {
    let (d, src) = _facts_axis(facts, axis_name)?;
    let raw = _raw(axis_name, d, &src, false)?;
    let tok = normalize(vocab_axis, &raw, vocab)?;
    Ok(Axis::new(tok.clone(), format!("{src} · vocab {vocab_axis}[{tok}]←{}", raw.repr())))
}

pub fn _describe_release(text: &J) -> HR<Option<String>> {
    if !text.truthy() {
        return Ok(None);
    }
    let s = pstrip(&text.display());
    Ok(_DESCRIBE_RE().match_at_start(&s).and_then(|m| named(&m, "rel").map(str::to_string)))
}

pub fn release_of(text: &J) -> HR<Option<String>> {
    let s = if text.truthy() { pstrip(&text.display()) } else { String::new() };
    Ok(_RELEASE_RE().match_at_start(&s).and_then(|m| named(&m, "rel").map(str::to_string)))
}

pub fn vllm_repo_kind(url: &J) -> HR<Option<String>> {
    if url.is_null() {
        return Ok(None);
    }
    let s = pstrip(&url.display());
    if s.is_empty() || _is_unobserved(&J::Str(s.clone())) {
        return Ok(None);
    }
    let m = match _REPO_URL_RE().match_at_start(&s) {
        Some(m) => m,
        None => return Ok(Some("fork".into())),
    };
    let key = format!(
        "{}/{}",
        python_lower(named(&m, "host").unwrap_or("")),
        python_lower(named(&m, "path").unwrap_or("").trim_matches('/'))
    );
    Ok(Some(if UPSTREAM_VLLM_REPOS.contains(&key.as_str()) { "upstream" } else { "fork" }.into()))
}

// urllib.parse — `urlparse(url).path` and `unquote`.
const USES_PARAMS: [&str; 16] =
    ["", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu", "sip", "sips", "mms", "sftp", "tel"];

fn url_path(url: &str) -> String {
    let u: String = url.chars().filter(|c| !matches!(c, '\t' | '\r' | '\n')).collect();
    let u = u.trim_start_matches(|c: char| (c as u32) <= 0x20).to_string();
    let mut scheme = String::new();
    let mut rest: &str = &u;
    if let Some(i) = u.find(':') {
        let first_alpha = u.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false);
        if i > 0 && first_alpha && u[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
            scheme = u[..i].to_ascii_lowercase();
            rest = &u[i + 1..];
        }
    }
    if let Some(after) = rest.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        rest = &after[end..];
    }
    let rest = rest.split('#').next().unwrap_or("");
    let rest = rest.split('?').next().unwrap_or("");
    if USES_PARAMS.contains(&scheme.as_str()) && rest.contains(';') {
        let i = if let Some(slash) = rest.rfind('/') { rest[slash..].find(';').map(|j| j + slash) } else { rest.find(';') };
        if let Some(i) = i {
            return rest[..i].to_string();
        }
    }
    rest.to_string()
}

fn unquote(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let h = |c: u8| (c as char).to_digit(16).unwrap() as u8;
            out.push(h(b[i + 1]) * 16 + h(b[i + 2]));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn conflict<T>(a: &str, b: &str, what: &str) -> HR<T> {
    fail(
        "HINT_VLLM_INPUT_CONFLICT",
        format!("vLLM 빌드 입력이 서로 모순된다({what}: {a} ≠ {b})"),
        Some("빌드 원장·셀 env 중 어느 쪽이 실제로 빌드된 입력인지 증거를 바로잡는다(이름은 한쪽을 고르지 않는다)."),
    )
}

fn bi(kind: &str, r: &str, sha: Option<&str>, prev: Option<&str>) -> J {
    crate::jd! {"kind" => kind, "ref" => r, "sha" => sha, "prev_release" => prev}
}

fn _resolve_vllm(d: &J, src: &str) -> HR<(Axis, J)> {
    let track = d.get("track").cloned().unwrap_or(J::Null);
    if !j_in(&track, &["source-build", "wheel", "native"]) {
        return _underivable("vllm", &format!("빌드 트랙이 source-build|wheel|native 가 아니다: {}", track.repr()), Some(src));
    }
    let track = track.as_str().unwrap();
    let r#ref = str_or_none(d.get("vllm_ref"));
    let version = str_or_none(d.get("vllm_version"));
    let sha = str_or_none(d.get("vllm_sha")).map(|s| python_lower(&s)).filter(|s| !s.is_empty());
    if let Some(s) = &sha {
        if !_SHA40_RE().is_match(s) {
            return _underivable("vllm", &format!("vllm_sha 가 40자 SHA 가 아니다: {}", py_repr(s)), Some(src));
        }
    }
    let url = str_or_none(d.get("wheel_url"));
    let (mut url_sha, mut url_file): (Option<String>, Option<String>) = (None, None);
    if let Some(u) = &url {
        let path = url_path(u);
        let shas: Vec<&str> = path.split('/').filter(|p| _SHA40_RE().is_match(&python_lower(p))).collect();
        if shas.len() > 1 {
            let mut low: Vec<String> = shas.iter().map(|s| python_lower(s)).collect();
            low.sort();
            low.dedup();
            if low.len() > 1 {
                return _underivable("vllm", "wheel URL 에 40자 SHA 가 둘 이상이다(모호)", Some(src));
            }
        }
        url_sha = shas.first().map(|s| python_lower(s));
        let last = path.rsplit('/').next().unwrap_or("");
        url_file = Some(unquote(last)).filter(|s| !s.is_empty());
    }
    let prev_raw = d.get("prev_release").cloned().unwrap_or(J::Null);
    let commit = |pin_sha: &str, ref_text: &str, why: &str| -> HR<(Axis, J)> {
        let prev = match _describe_release(&prev_raw)? {
            Some(p) => p,
            None => {
                return _underivable(
                    "vllm",
                    &format!("커밋 핀({})인데 직전 릴리스를 해소하지 못했다(prev_release={})", head(pin_sha, 12), prev_raw.repr()),
                    Some(src),
                )
            }
        };
        let pr = pstrip(&prev_raw.display());
        if let Some(dm) = _DESCRIBE_RE().match_at_start(&pr) {
            if let Some(g) = named(&dm, "g").filter(|g| !g.is_empty()) {
                if !pin_sha.starts_with(g) {
                    return conflict(g, &head(pin_sha, 12), "prev_release describe 의 커밋 vs 핀 SHA");
                }
            }
        }
        let seg = format!("{prev}-g{}", head(pin_sha, 12));
        Ok((Axis::new(seg, format!("{src} · {why} → <직전 릴리스 {prev}>-g<sha12>")), bi("commit", ref_text, Some(pin_sha), Some(&prev))))
    };

    if track == "source-build" {
        let r = match &r#ref {
            Some(r) => r.as_str(),
            None => return _underivable("vllm", "소스빌드인데 VLLM_REF 가 없다", Some(src)),
        };
        if let Some(m) = _RELEASE_RE().match_at_start(r) {
            let relv = named(&m, "rel").unwrap_or("").to_string();
            let dm = d.as_dict().unwrap();
            if !dm.contains_key("vllm_repo") {
                return _underivable(
                    "vllm",
                    &format!(
                        "naming facts 에 vllm_repo 키가 없다 — 릴리스 모양 VLLM_REF={r} 가 업스트림 태그인지 \
                         판정할 수 없다(D-h · evidence.naming_facts 가 싣는다)"
                    ),
                    Some(src),
                );
            }
            let rk = vllm_repo_kind(dm.get("vllm_repo").unwrap())?;
            match rk.as_deref() {
                None => {
                    return _underivable(
                        "vllm",
                        &format!(
                            "VLLM_REPO 미관측 — 릴리스 모양 VLLM_REF={r} 가 업스트림 태그인지 판정할 수 없다\
                             (D-h · 이미지 history build-arg 또는 셀 env 의 VLLM_REPO)"
                        ),
                        Some(src),
                    )
                }
                Some("upstream") => {
                    return Ok((
                        Axis::new(relv, format!("{src} · VLLM_REF={r}(업스트림 릴리스 태그 · v 제거)")),
                        bi("release", r, sha.as_deref(), None),
                    ))
                }
                _ => {}
            }
            let s = match &sha {
                Some(s) => s.clone(),
                None => {
                    return _underivable(
                        "vllm",
                        &format!(
                            "포크 저장소(비-업스트림 VLLM_REPO)의 릴리스 모양 VLLM_REF={r} — 이름은 SHA 경로\
                             (<직전 릴리스>-g<sha12>)만 허용하는데 빌드된 vllm_sha 가 없다(D-h · 빌드 원장 vllm.git_sha)"
                        ),
                        Some(src),
                    )
                }
            };
            return commit(&s, r, &format!("포크 VLLM_REPO 의 릴리스 모양 VLLM_REF={r} → SHA 경로(D-h · 업스트림 릴리스 이름 ✗)"));
        }
        let rl = python_lower(r);
        if _SHA40_RE().is_match(&rl) {
            if let Some(s) = &sha {
                if *s != rl {
                    return conflict(&rl, s, "VLLM_REF 커밋 vs vllm_sha");
                }
            }
            return commit(&rl, r, &format!("VLLM_REF={}…(40자 커밋)", head(r, 12)));
        }
        if let Some(s) = &sha {
            if _SHORT_SHA_RE().is_full(&rl) && !s.starts_with(&rl) {
                return conflict(r, s, "짧은 VLLM_REF vs vllm_sha");
            }
            return commit(s, r, &format!("VLLM_REF={r}(릴리스·40자 SHA 아님) · 빌드된 vllm_sha"));
        }
        return _underivable("vllm", &format!("VLLM_REF={} 는 릴리스 태그도 40자 SHA 도 아니고 빌드된 vllm_sha 도 없다", py_repr(r)), Some(src));
    }

    if track == "wheel" {
        if let Some(v) = &version {
            if let Some(m) = _RELEASE_RE().match_at_start(v) {
                return Ok((
                    Axis::new(named(&m, "rel").unwrap_or(""), format!("{src} · VLLM_VERSION={v}(wheel 릴리스)")),
                    bi("wheel", v, sha.as_deref(), None),
                ));
            }
        }
        let pin = url_sha.clone().or_else(|| sha.clone());
        if let (Some(a), Some(b)) = (&url_sha, &sha) {
            if a != b {
                return conflict(a, b, "wheel URL SHA vs vllm_sha");
            }
        }
        if let Some(p) = &pin {
            let r = version.clone().or_else(|| url_file.clone()).unwrap_or_else(|| p.clone());
            return commit(p, &r, &format!("wheel VLLM_VERSION={}(릴리스 아님) · 커밋 핀", orepr(version.as_deref())));
        }
        return _underivable(
            "vllm",
            &format!("wheel 트랙인데 VLLM_VERSION={} 가 릴리스가 아니고 커밋 SHA 도 없다", orepr(version.as_deref())),
            Some(src),
        );
    }

    // native — wheel URL 의 SHA 도 같은 규칙(X18)
    if let (Some(a), Some(b)) = (&url_sha, &sha) {
        if a != b {
            return conflict(a, b, "native wheel URL SHA vs vllm_sha");
        }
    }
    if let Some(us) = &url_sha {
        let r = url_file.clone().unwrap_or_else(|| us.clone());
        return commit(us, &r, &format!("native wheel URL({}) 의 커밋 SHA", url_file.as_deref().unwrap_or("None")));
    }
    let mut file_ver: Option<String> = None;
    if let Some(uf) = &url_file {
        if let Some(fm) = _WHEEL_FILE_RE().match_at_start(uf) {
            file_ver = crate::pyre::group(&fm, 1).map(str::to_string);
        }
    }
    let cands = [
        (file_ver.clone(), format!("native wheel 파일명({})", url_file.as_deref().unwrap_or("None"))),
        (version.clone(), "native vllm_version".to_string()),
    ];
    for (cand, why) in cands.iter() {
        if let Some(c) = cand.as_deref().filter(|c| !c.is_empty()) {
            if let Some(m) = _RELEASE_RE().match_at_start(c) {
                let relv = named(&m, "rel").unwrap_or("");
                let r = url_file.clone().unwrap_or_else(|| c.to_string());
                return Ok((Axis::new(relv, format!("{src} · {why} 릴리스 {relv}")), bi("wheel", &r, sha.as_deref(), None)));
            }
        }
    }
    if let Some(s) = &sha {
        let r = url_file.clone().or_else(|| version.clone()).unwrap_or_else(|| s.clone());
        return commit(s, &r, "native 빌드된 vllm_sha");
    }
    _underivable("vllm", "native 인데 wheel URL SHA·릴리스 버전·vllm_sha 중 어느 것도 없다", Some(src))
}

/// X18 — naming facts 의 `vllm` 사전 → vLLM 세그먼트 Axis.
pub fn vllm_segment(build_input: &J) -> HR<Axis> {
    let facts = crate::jd! {"vllm" => build_input.clone()};
    let (d, src) = _facts_axis(&facts, "vllm")?;
    Ok(_resolve_vllm(d, &src)?.0)
}

pub fn derive_name(facts: &J, vocab: &J) -> HR<DerivedName> {
    if !facts.is_dict() {
        return _underivable("facts", "naming facts 가 사전이 아니다", None);
    }
    let (vd, vsrc) = _facts_axis(facts, "vllm")?;
    let (vllm_axis, build_input) = _resolve_vllm(vd, &vsrc)?;

    let (md, msrc) = _facts_axis(facts, "model")?;
    let hf_repo = str_or_none(md.get("hf_repo"));
    let model_path = str_or_none(md.get("model_path"));
    let slug = derive_slug(hf_repo.as_deref(), model_path.as_deref())?;
    if let (Some(_), Some(mp)) = (&hf_repo, &model_path) {
        let by_path = derive_slug(None, Some(mp))?;
        if by_path != slug {
            return fail(
                "HINT_MODEL_SLUG_CONFLICT",
                format!("hf_repo 슬러그 {} ≠ 체크포인트 basename 슬러그 {}", py_repr(&slug), py_repr(&by_path)),
                Some("hf_repo 는 **서빙한 체크포인트**의 repo id 여야 한다(base_model 이 아니다). 확인할 수 없으면 hf_repo 를 비운다."),
            );
        }
    }
    let model_src = match &hf_repo {
        Some(h) => format!("{msrc} · hf_repo({h}) 마지막 성분 소문자"),
        None => format!(
            "{msrc} · model_path basename({}) 소문자 = 인증서 model 키 규칙",
            basename(model_path.as_deref().unwrap_or("").trim_end_matches('/'))
        ),
    };
    let model_axis = Axis::new(slug.clone(), model_src);

    let (ad, asrc) = _facts_axis(facts, "arch")?;
    let gpu_model = ad.get("gpu_model").cloned().unwrap_or(J::Null);
    if !gpu_model.is_str() || _is_unobserved(&gpu_model) {
        return _underivable("arch", &format!("gpu_model 을 관측하지 못했다({})", gpu_model.repr()), Some(&asrc));
    }
    let hw = normalize("hw", &gpu_model, vocab)?;
    for k in ["gpus_per_node", "nodes", "role"] {
        if ad.get(k).map(J::is_null).unwrap_or(true) {
            return _underivable("arch", &format!("{k} 가 없다"), Some(&asrc));
        }
    }
    let g = _pos_int(&ad["gpus_per_node"], "gpus_per_node(G)")?;
    let n = _pos_int(&ad["nodes"], "nodes(N)")?;
    let role = ad["role"].clone();
    let tgt_raw = match ad.get("target_gpu") {
        None => {
            return _underivable(
                "arch",
                "target_gpu 키가 없다(시뮬레이션 타겟 선언 여부를 관측하지 못했다 — 읽지 못함 ≠ 없음)",
                Some(&asrc),
            )
        }
        Some(t) => t.clone(),
    };
    if _is_unobserved(&tgt_raw) {
        return _underivable(
            "arch",
            &format!("target_gpu={} 는 '읽지 못함' 표기(또는 템플릿 빈칸)다 — 타겟 없음은 None 으로만 넘긴다", tgt_raw.repr()),
            Some(&asrc),
        );
    }
    let (target, tsrc) = if tgt_raw.is_null() {
        ("native".to_string(), format!("{asrc} · no-simulation-target-declared → native"))
    } else {
        let ttok = normalize("hw", &tgt_raw, vocab)?;
        if ttok == hw {
            ("native".to_string(), format!("{asrc} · target_gpu {} = 호스트 hw → native", tgt_raw.repr()))
        } else {
            (format!("sim-{ttok}"), format!("{asrc} · vocab hw[{ttok}]←{} → sim-{ttok}", tgt_raw.repr()))
        }
    };
    let (pd, psrc) = _facts_axis(facts, PLANE_AXIS)?;
    let plane = _raw(PLANE_AXIS, pd, &psrc, false)?;
    let ptok = plane_token(&plane, vocab)?;
    let arch = build_arch_j(&J::from(hw.as_str()), &J::from(g.as_str()), &J::from(n.as_str()), &role, &J::from(target.as_str()), &J::from(ptok.as_str()))?;
    let mut axes: Vec<(String, Axis)> = vec![
        ("hw".into(), Axis::new(hw.clone(), format!("{asrc} · vocab hw[{hw}]←{}", gpu_model.repr()))),
        ("gpus_per_node".into(), Axis::new(g.clone(), asrc.clone())),
        ("nodes".into(), Axis::new(n.clone(), asrc.clone())),
        ("role".into(), Axis::new(role.display(), asrc.clone())),
        ("target".into(), Axis::new(target.clone(), tsrc)),
        (
            PLANE_AXIS.into(),
            Axis {
                value: plane.display(),
                source: format!("{psrc} · vocab {PLANE_AXIS}[{}]→{}", plane.display(), py_repr(&ptok)),
                token: Some(ptok.clone()),
            },
        ),
    ];
    let q = _vocab_axis("q", "quant", facts, vocab)?;
    axes.push(("q".into(), q.clone()));
    let (ld, lsrc) = _facts_axis(facts, "len")?;
    let lraw = _raw("len", ld, &lsrc, false)?;
    let len_axis = Axis::new(_pos_int(&lraw, "max-model-len(len)")?, lsrc.clone());
    axes.push(("len".into(), len_axis.clone()));
    let kv = _vocab_axis("kv", "kv", facts, vocab)?;
    axes.push(("kv".into(), kv.clone()));
    let recipe = build_recipe_base_j(&J::from(q.value.as_str()), &J::from(len_axis.value.as_str()), &J::from(kv.value.as_str()))?;

    let tag = format!("{}{}/{slug}/{arch}/{recipe}", core::HINT_TAG_PREFIX, vllm_axis.value);
    validate_new_name(&tag)?;
    let segments = vec![
        ("vllm".to_string(), vllm_axis),
        ("model".to_string(), model_axis),
        ("arch".to_string(), Axis::new(arch, "derived(hw·gpus_per_node·nodes·role·target·plane)")),
        ("recipe".to_string(), Axis::new(recipe, "derived(q·len·kv)")),
    ];
    Ok(DerivedName { tag, segments, axes, vllm_build_input: build_input })
}

// ── 자체검사 픽스처 ──────────────────────────────────────────────────────────────────────────
pub fn _fixture_vocab() -> J {
    loads(
        r#"{"schema_version": 1,
 "hw": {"gb10": ["NVIDIA GB10", "GB10"], "h100": ["NVIDIA H100 80GB HBM3", "NVIDIA H100", "H100"],
        "rtxpro6000maxq": ["NVIDIA RTX PRO 6000 Blackwell Max-Q Workstation Edition"],
        "rtxpro6000srv": ["NVIDIA RTX PRO 6000 Blackwell Server Edition"]},
 "hw_ambiguous": {"NVIDIA RTX PRO 6000": "에디션 미표기"},
 "quant": {"nvfp4": ["nvfp4", "modelopt:NVFP4", "modelopt-dominant:NVFP4"], "fp8": ["fp8"],
           "mxfp4": ["mxfp4"], "bf16": ["bf16", "bfloat16", "none"]},
 "kv": {"auto": ["auto"], "fp8": ["fp8", "fp8_e4m3", "fp8e4m3"], "fp8e5m2": ["fp8_e5m2"]},
 "ple": {"mmap": ["mmap"], "resident": ["resident"], "offload": ["offload"], "none": ["none"]},
 "graph": {"graph": ["graph", "cudagraph"], "eager": ["eager"]},
 "plane": {"docker": "", "native": "bare"},
 "quant_suffixes": ["nvfp4", "fp8", "mxfp4", "int4", "awq", "gptq", "w4a16", "bf16"]}"#,
    )
    .expect("fixture vocab")
}

pub const _TAG2_SHA: &str = "74c96922ecb9017f413318c76d1af83aa2ab45a5";
pub const _TAG2_V6: &str = "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-plemmap-spec3-eager";
pub fn _tag2_expected() -> String {
    _TAG2_V6.rsplit_once("-plemmap").map(|(a, _)| a.to_string()).unwrap_or_default()
}
pub const _V6_LIVE: [&str; 4] = [
    "hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plenone-spec7-graph",
    "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native-bare/qnvfp4-len262144-kvauto-plemmap-spec3-eager",
    _TAG2_V6,
    "hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8-plemmap-spec3-eager",
];

pub fn _tag2_facts() -> J {
    let text = r#"{"vllm": {"track": "source-build", "vllm_ref": "v0.29.0rc6", "vllm_version": null, "vllm_sha": "@SHA@",
                     "vllm_repo": "https://@REPO@.git",
                     "prev_release": null, "wheel_url": null, "source": "셀 env VLLM_REF(output/multi/envs/.env.<cell>)"},
            "model": {"hf_repo": null, "model_path": "/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4",
                      "source": "서빙 yaml model"},
            "arch": {"gpu_model": "NVIDIA GB10", "gpus_per_node": 1, "nodes": 2, "role": "cluster",
                     "target_gpu": null, "source": "output/multi/manifest.yaml · sweep meta measured_node=cluster"},
            "plane": {"raw": "docker", "source": "artifacts.plane_of → cell-env(IMAGE_TAG|BUILD_DOCKERFILE)"},
            "q": {"raw": "modelopt-dominant:NVFP4", "source": "hf_quant_config.json quantized_layers 우세 알고리즘"},
            "len": {"raw": 262144, "source": "서빙 yaml max-model-len"},
            "kv": {"raw": "auto", "source": "서빙 yaml kv-cache-dtype"},
            "ple": {"raw": "mmap", "source": "셀 env VLLM_PLE_MMAP=1"},
            "spec": {"raw": 3, "source": "서빙 yaml speculative-config num_speculative_tokens"},
            "graph": {"raw": "eager", "source": "서빙 yaml enforce-eager: true"}}"#;
    loads(&text.replace("@SHA@", _TAG2_SHA).replace("@REPO@", UPSTREAM_VLLM_REPOS[0])).expect("tag2 facts")
}

// ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
fn code<T>(r: HR<T>) -> Option<String> {
    r.err().map(|e| e.code)
}
fn with(j: &J, k: &str, v: J) -> J {
    let mut o = j.clone();
    o.set(k, v);
    o
}
fn del(j: &mut J, path: &[&str]) {
    let mut cur = j;
    for k in &path[..path.len() - 1] {
        cur = match cur.get_mut(k) {
            Some(c) => c,
            None => return,
        };
    }
    if let Some(m) = cur.as_dict_mut() {
        m.remove(path[path.len() - 1]);
    }
}
fn set_path(j: &mut J, path: &[&str], v: J) {
    let mut cur = j;
    for k in &path[..path.len() - 1] {
        cur = cur.get_mut(k).expect("path");
    }
    cur.set(path[path.len() - 1], v);
}
fn jl_strs(v: &[&str]) -> J {
    J::List(v.iter().map(|s| J::from(*s)).collect())
}

/// 실패 메시지 목록(빈 목록 = 통과).
///
/// The original's last two checks spawn a fresh Python interpreter with an audit hook (`_IMPORT_PROBE`) to prove that
/// `import hintlib.naming` starts no process and opens no non-`.py` file. A Rust module has no import-time execution at
/// all (no module initializer; every regex here is a lazy `OnceLock` compiled on first use), so the fact holds by
/// construction and there is nothing to probe — those two checks are dropped.
pub fn selftest() -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    macro_rules! ck {
        ($n:expr, $c:expr $(,)?) => {
            if !$c {
                bad.push(format!("naming: {}", $n));
            }
        };
    }
    let v = _fixture_vocab();
    let vv = &v;
    ck!("픽스처 어휘표가 구조 검사를 통과한다", validate_vocab(vv).map(|b| b.is_empty()).unwrap_or(false));
    let tag2_expected = _tag2_expected();
    let pt = |s: &str| parse_tag(s).ok();

    // ── 태그2 셀 기대값(AC4) ──
    let dn = match derive_name(&_tag2_facts(), vv) {
        Ok(d) => d,
        Err(e) => {
            bad.push(format!("naming: 태그2 derive_name 실패 {}", e.str()));
            return bad;
        }
    };
    ck!(&format!("태그2 facts → 정확히 기대 이름(실제 {})", dn.tag), dn.tag == tag2_expected);
    {
        let mut want: Vec<&str> = ARCH_AXES.iter().copied().chain([PLANE_AXIS]).chain(RECIPE_AXES).collect();
        want.sort();
        let mut got: Vec<&str> = dn.axes.iter().map(|(k, _)| k.as_str()).collect();
        got.sort();
        let mut segs: Vec<&str> = dn.segments.iter().map(|(k, _)| k.as_str()).collect();
        segs.sort();
        ck!(
            "축별 {값, 출처} 가 전부 채워진다(v7 = q·len·kv 까지 · ple·spec·graph 는 이름 축 ✗)",
            got == want
                && dn.axes.iter().all(|(_, a)| !a.value.is_empty() && !a.source.is_empty())
                && segs == ["arch", "model", "recipe", "vllm"]
                && RECIPE_AXES == ["q", "len", "kv"]
                && !V6_TAIL_AXES.iter().any(|a| dn.axis(a).is_some()),
        );
    }
    let ax = |d: &DerivedName, k: &str| d.axis(k).cloned().unwrap_or_else(|| Axis::new("", ""));
    ck!(
        "q 출처에 어휘 정규화 흔적(modelopt-dominant:NVFP4 → nvfp4)",
        ax(&dn, "q").value == "nvfp4" && ax(&dn, "q").source.contains("modelopt-dominant:NVFP4"),
    );
    ck!(
        "target None → native · 출처에 no-simulation-target-declared",
        ax(&dn, "target").value == "native" && ax(&dn, "target").source.contains("no-simulation-target-declared"),
    );
    ck!(
        "릴리스 빌드 입력 기록(kind=release · sha 보존)",
        dn.vllm_build_input == crate::jd! {"kind" => "release", "ref" => "v0.29.0rc6", "sha" => _TAG2_SHA, "prev_release" => J::Null},
    );
    ck!("★출처에 체크포인트 절대경로를 싣지 않는다(basename 만)", !dumps_unicode(&dn.to_payload()).contains("/app/quant_models"));
    {
        let t = pt(&dn.tag);
        ck!(
            "파생 기본 이름은 v7 문법으로 해체된다(꼬리 0 · timestamp 없음)",
            t.as_ref().map(|t| t.grammar == GRAMMAR_V7 && t.tail.is_empty() && t.timestamp.is_none() && t.base_tag().as_deref() == Some(dn.tag.as_str()))
                .unwrap_or(false),
        );
    }
    let pay = dn.to_payload();
    ck!(
        "to_payload grammar = v7 · 레시피 출처 = derived(q·len·kv)",
        pay["grammar"].as_str() == Some(GRAMMAR_V7) && pay["segments"]["recipe"]["source"].as_str() == Some("derived(q·len·kv)"),
    );
    ck!("PAYLOAD.naming 재조립 = 태그", compose_from_payload(&pay).ok().as_deref() == Some(dn.tag.as_str()));
    let mut tam = pay.clone();
    set_path(&mut tam, &["axes", "len", "value"], J::from("131072"));
    ck!("★음성대조 PAYLOAD 축 위조는 재조립 대조가 잡는다", code(compose_from_payload(&tam)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));

    // ── 평면 토큰(O-N1) ──
    let ntok = v["plane"]["native"].display();
    ck!(
        "docker 평면 = 토큰 없음(태그2 이름 바이트 불변) · 축 기록",
        ax(&dn, PLANE_AXIS).value == "docker"
            && ax(&dn, PLANE_AXIS).token.as_deref() == Some("")
            && dn.tag.contains("/gb10-1g2n-cluster-native/")
            && pay["axes"][PLANE_AXIS]["token"].as_str() == Some(""),
    );
    let fnat = with(
        &_tag2_facts(),
        "plane",
        crate::jd! {"raw" => "native", "source" => "artifacts.plane_of → evidence.plane(declared) · serve_proof plane=native"},
    );
    let dnn = derive_name(&fnat, vv);
    let dnn = match dnn {
        Ok(d) => d,
        Err(e) => {
            bad.push(format!("naming: native derive_name 실패 {}", e.str()));
            return bad;
        }
    };
    ck!(
        &format!("native 평면 → arch 끝 -{ntok}({})", dnn.tag),
        dnn.tag == tag2_expected.replace("/gb10-1g2n-cluster-native/", &format!("/gb10-1g2n-cluster-native-{ntok}/")),
    );
    ck!("native 는 축이 같은 Docker 이름과 충돌하지 않는다(F10)", dnn.tag != dn.tag);
    ck!(
        "평면 출처가 PAYLOAD.naming 에 실린다(serve_proof · vocab 대응)",
        ax(&dnn, PLANE_AXIS).source.contains("serve_proof plane=native") && ax(&dnn, PLANE_AXIS).source.contains(&format!("→{}", py_repr(&ntok))),
    );
    ck!("native PAYLOAD.naming 재조립 = 태그", compose_from_payload(&dnn.to_payload()).ok().as_deref() == Some(dnn.tag.as_str()));
    {
        let t = pt(&dnn.tag);
        let want = crate::jd! {"grammar" => GRAMMAR_V6, "hw" => "gb10", "gpus_per_node" => 1, "nodes" => 2, "role" => "cluster",
            "target" => "native", "plane_token" => ntok.clone(), "plane" => "native"};
        ck!(
            "파서 왕복: -<plane> 있음 → v7 · plane=native",
            t.as_ref().map(|t| t.grammar == GRAMMAR_V7 && parse_arch(&t.arch, Some(vv)).ok().flatten() == Some(want.clone())).unwrap_or(false),
        );
    }
    let pa = |a: &str, voc: Option<&J>| parse_arch(a, voc).ok().flatten().unwrap_or(J::Null);
    ck!(
        "파서 왕복: 토큰 없음 → plane=docker(vocab 없이도)",
        pa("gb10-1g2n-cluster-native", None)["plane"].as_str() == Some("docker")
            && pa("gb10-1g2n-cluster-native", None)["plane_token"].as_str() == Some(""),
    );
    let simarch = format!("gb10-1g1n-sub-sim-h100-{ntok}");
    ck!(
        "sim 타겟 뒤 평면 토큰도 해체된다",
        pa(&simarch, Some(vv))["target"].as_str() == Some("sim-h100") && pa(&simarch, Some(vv))["plane"].as_str() == Some("native"),
    );
    ck!(
        "vocab 없이 비어 있지 않은 토큰은 추측하지 않는다(plane=None)",
        pa(&format!("gb10-1g2n-cluster-native-{ntok}"), None).get("plane").map(J::is_null).unwrap_or(false),
    );
    let mut old_pay = pay.clone();
    del(&mut old_pay, &["axes", PLANE_AXIS]);
    ck!("O-N1 이전 페이로드(평면 칸 없음 · Docker)도 재조립된다", compose_from_payload(&old_pay).ok().as_deref() == Some(dn.tag.as_str()));
    let mut old_nat = dnn.to_payload();
    del(&mut old_nat, &["axes", PLANE_AXIS]);
    ck!("★음성대조 평면 칸 없이 -<plane> 이름 = 재조립 불일치", code(compose_from_payload(&old_nat)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    let mut lie = dnn.to_payload();
    set_path(&mut lie, &["axes", PLANE_AXIS, "value"], J::from("docker"));
    ck!("★음성대조 평면 값·토큰 모순 = 재조립 거부", code(compose_from_payload(&lie)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    let mut vn = v.clone();
    del(&mut vn, &[PLANE_AXIS]);
    ck!(
        "★vocab 에 plane 축 없음 = HINT_VOCAB_ABSENT(fail-loud)",
        code(derive_name(&fnat, &vn)).as_deref() == Some("HINT_VOCAB_ABSENT")
            && validate_vocab(&vn).map(|b| b.iter().any(|x| x.contains(PLANE_AXIS))).unwrap_or(false),
    );
    let fx = with(&_tag2_facts(), "plane", crate::jd! {"raw" => "podman", "source" => "fixture"});
    ck!("★어휘 밖 평면 = HINT_VOCAB_UNKNOWN", code(derive_name(&fx, vv)).as_deref() == Some("HINT_VOCAB_UNKNOWN"));
    let mut fx = _tag2_facts();
    del(&mut fx, &["plane"]);
    ck!("★평면 사실 부재 = HINT_AXIS_UNDERIVABLE(docker 로 추측 ✗)", code(derive_name(&fx, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    for (bp, why) in [
        (crate::jd! {"docker" => "dk", "native" => ntok.clone()}, "docker 토큰 금지(Docker 이름 불변)"),
        (crate::jd! {"docker" => "", "native" => ""}, "native 빈 토큰(충돌)"),
        (crate::jd! {"docker" => ""}, "키 집합 ≠ 판정 소유자 어휘"),
    ] {
        ck!(&format!("★vocab plane 구조 위반 거부 — {why}"), validate_vocab(&with(vv, PLANE_AXIS, bp)).map(|b| !b.is_empty()).unwrap_or(false));
    }
    ck!(
        "★평면 토큰 형태 위반 조립 거부",
        code(build_arch("gb10", 1, 2, "cluster", "native", "Bare")).as_deref() == Some("HINT_ARCH_SHAPE_VIOLATION"),
    );

    // ── vLLM 세그먼트(X18) ──
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let vseg = |j: J| vllm_segment(&j).map(|a| a.value);
    let vcode = |j: J| code(vllm_segment(&j));
    let f = with(
        &_tag2_facts(),
        "vllm",
        crate::jd! {"track" => "source-build", "vllm_ref" => sha, "vllm_sha" => J::Null, "prev_release" => "v0.29.0rc5", "source" => "셀 env VLLM_REF"},
    );
    let dc = derive_name(&f, vv);
    ck!(
        "커밋 핀 → <prev>-g<sha12>",
        dc.as_ref()
            .map(|dc| {
                dc.tag.starts_with("hint/0.29.0rc5-g0123456789ab/")
                    && dc.vllm_build_input["kind"].as_str() == Some("commit")
                    && dc.vllm_build_input["sha"].as_str() == Some(sha)
            })
            .unwrap_or(false),
    );
    ck!(
        "직전 릴리스는 describe 긴 형태에서도 해소된다",
        vseg(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "prev_release" => "v0.29.0rc5-37-g0123456", "source" => "빌드 원장 vllm.describe"})
            .ok()
            .as_deref()
            == Some("0.29.0rc5-g0123456789ab"),
    );
    ck!(
        "describe 약식 SHA 가 핀의 접두면 통과(긴 약식도)",
        vseg(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "prev_release" => "v0.29.0rc5-37-g0123456789ab", "source" => "빌드 원장 vllm.describe"})
            .ok()
            .as_deref()
            == Some("0.29.0rc5-g0123456789ab"),
    );
    ck!(
        "★음성대조 다른 커밋의 describe(약식 SHA ≠ 핀 접두) = 모순 차단",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "prev_release" => "v0.29.0rc5-37-g7777777", "source" => "빌드 원장 vllm.describe"})
            .as_deref()
            == Some("HINT_VLLM_INPUT_CONFLICT"),
    );
    ck!(
        "★음성대조 커밋 핀인데 직전 릴리스 없음 = 파생 불가",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "source" => "셀 env"}).as_deref() == Some("HINT_AXIS_UNDERIVABLE"),
    );
    ck!(
        "★음성대조 VLLM_REF 커밋과 vllm_sha 가 다르면 모순 차단",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "vllm_sha" => _TAG2_SHA, "prev_release" => "0.29.0rc5", "source" => "셀 env"})
            .as_deref()
            == Some("HINT_VLLM_INPUT_CONFLICT"),
    );
    ck!(
        "브랜치 ref 는 빌드된 SHA 로 이름 짓는다",
        vseg(crate::jd! {"track" => "source-build", "vllm_ref" => "feature-x", "vllm_sha" => sha, "prev_release" => "0.29.0", "source" => "빌드 원장"})
            .ok()
            .as_deref()
            == Some("0.29.0-g0123456789ab"),
    );
    ck!(
        "wheel 트랙 VLLM_VERSION 릴리스",
        vseg(crate::jd! {"track" => "wheel", "vllm_version" => "0.18.0", "source" => "셀 env VLLM_VERSION"}).ok().as_deref() == Some("0.18.0"),
    );
    ck!(
        "native wheel URL 의 SHA 도 같은 규칙",
        vseg(crate::jd! {"track" => "native", "prev_release" => "v0.1.0",
            "wheel_url" => format!("https://wheels.vllm.ai/{sha}/vllm-0.1.1.dev53%2Bg0123456-cp38-abi3-linux_x86_64.whl"),
            "source" => "native 설치 명령"})
        .ok()
        .as_deref()
            == Some("0.1.0-g0123456789ab"),
    );
    ck!(
        "★음성대조 nightly 버전 문자열만으로는 파생하지 않는다(짧은 SHA · 직전 릴리스 없음)",
        vcode(crate::jd! {"track" => "native", "vllm_version" => "0.1.1.dev53+g30118ba27", "source" => "엔진 자기보고"}).as_deref()
            == Some("HINT_AXIS_UNDERIVABLE"),
    );
    ck!(
        "★음성대조 출처 빈 vllm 입력 = 파생 불가",
        vcode(crate::jd! {"track" => "wheel", "vllm_version" => "0.18.0", "source" => ""}).as_deref() == Some("HINT_AXIS_UNDERIVABLE"),
    );

    // ── 명시 토큰(D6) ──
    let mut f = _tag2_facts();
    f.set("spec", crate::jd! {"raw" => J::Null, "source" => "서빙 yaml·러너에 speculative-config 없음(관측)"});
    f.set("ple", crate::jd! {"raw" => "none", "source" => "모델 config 에 PLE 키 없음"});
    f.set("graph", crate::jd! {"raw" => "cudagraph", "source" => "서빙 yaml enforce-eager 부재"});
    f.set("kv", crate::jd! {"raw" => "fp8_e4m3", "source" => "서빙 yaml kv-cache-dtype"});
    let ds_tag = derive_name(&f, vv).map(|d| d.tag).unwrap_or_default();
    ck!(&format!("kvfp8(fp8_e4m3 정규화) · 뒤 3축은 이름에 들어가지 않는다({ds_tag})"), ds_tag.ends_with("/qnvfp4-len262144-kvfp8"));
    let by_axis = |rows: &[J]| -> Map {
        let mut m = Map::new();
        for r in rows {
            m.insert(r["axis"].display(), r.clone());
        }
        m
    };
    let cands = by_axis(&tail_candidates(&f, vv).unwrap_or_default());
    let cj = J::Dict(cands);
    ck!(
        "tail_candidates: specoff·plenone·graph 는 후보 토큰으로만 나온다(강제 ✗)",
        V6_TAIL_AXES.iter().map(|a| cj[*a]["token"].display()).collect::<Vec<_>>() == ["plenone", "specoff", "graph"]
            && V6_TAIL_AXES.iter().all(|a| cj[*a]["meaning"].truthy() && cj[*a]["source"].truthy()),
    );

    // ── ★음성대조: 어휘 밖 · 파생 불가 ──
    let f = with(&_tag2_facts(), "arch", with(&_tag2_facts()["arch"], "gpu_model", J::from("NVIDIA B300")));
    ck!("★미지 hw = HINT_VOCAB_UNKNOWN", code(derive_name(&f, vv)).as_deref() == Some("HINT_VOCAB_UNKNOWN"));
    match normalize("hw", &J::from("NVIDIA RTX PRO 6000"), vv) {
        Ok(_) => ck!("★에디션 모호 hw 는 어느 토큰에도 합치지 않는다", false),
        Err(e) => ck!("★에디션 모호 hw = HINT_VOCAB_UNKNOWN(에디션 remedy)", e.code == "HINT_VOCAB_UNKNOWN" && e.message.contains("에디션")),
    }
    match normalize("kv", &J::from("fp4_weird"), vv) {
        Ok(_) => ck!("★어휘 밖 kv 는 통과하지 않는다", false),
        Err(e) => {
            let r = e.remedy.clone().unwrap_or_default();
            ck!("★어휘 밖 remedy 가 추가할 키를 가리킨다", e.code == "HINT_VOCAB_UNKNOWN" && r.contains("\"kv\"") && r.contains(core::REL_VOCAB))
        }
    }
    ck!("hw 는 대소문자·공백만 정규화한 정확 일치", normalize("hw", &J::from("  nvidia   gb10 "), vv).ok().as_deref() == Some("gb10"));
    ck!(
        "★부분 일치 ✗(에디션이 다른 원문의 접두사)",
        code(normalize("hw", &J::from("NVIDIA GB10 Superchip"), vv)).as_deref() == Some("HINT_VOCAB_UNKNOWN"),
    );
    let mut f = _tag2_facts();
    del(&mut f, &["spec", "raw"]);
    ck!("v7: spec raw 부재여도 이름은 파생된다(뒤 3축은 이름 축이 아니다)", derive_name(&f, vv).map(|d| d.tag).ok() == Some(tag2_expected.clone()));
    let sc = J::Dict(by_axis(&tail_candidates(&f, vv).unwrap_or_default()))["spec"].clone();
    ck!(
        "★raw 키 부재(읽지 못함) → 후보도 specoff 로 떨어지지 않고 error 행(HINT_AXIS_UNDERIVABLE)",
        sc["token"].is_null() && sc["error"].as_str() == Some("HINT_AXIS_UNDERIVABLE"),
    );
    let mut f = _tag2_facts();
    for a in V6_TAIL_AXES {
        del(&mut f, &[a]);
    }
    ck!("v7: ple·spec·graph 사실이 통째 없어도 기본 이름 파생(결정론 파생에서 뺐다)", derive_name(&f, vv).map(|d| d.tag).ok() == Some(tag2_expected.clone()));
    let f = with(&_tag2_facts(), "kv", crate::jd! {"raw" => "auto", "source" => ""});
    ck!("★출처 빈 축 = HINT_AXIS_UNDERIVABLE", code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    let f = with(&_tag2_facts(), "q", crate::jd! {"raw" => "N/A", "source" => "인증서 quantization"});
    ck!("★인증서 'N/A' 는 양자화 없음(bf16)이 아니라 파생 불가", code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    let mut f = _tag2_facts();
    del(&mut f, &["arch", "target_gpu"]);
    ck!("★target_gpu 키 부재 = 파생 불가(None 은 관측한 경우에만)", code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    for (what, k, val) in [
        ("target_gpu 'N/A'", "target_gpu", "N/A"),
        ("target_gpu 빈칸", "target_gpu", "<<FILL>>"),
        ("gpu_model 빈칸", "gpu_model", "<<FILL>>"),
        ("gpu_model __REQUIRED__", "gpu_model", "__REQUIRED__"),
    ] {
        let f = with(&_tag2_facts(), "arch", with(&_tag2_facts()["arch"], k, J::from(val)));
        ck!(&format!("★{what} = HINT_AXIS_UNDERIVABLE(어휘 추가 처방 ✗)"), code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    }
    let f = with(&_tag2_facts(), "q", crate::jd! {"raw" => "<<FILL>>", "source" => "셀 선언"});
    ck!("★레시피 축 템플릿 빈칸 = HINT_AXIS_UNDERIVABLE", code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    match normalize("hw", &J::from("N/A"), vv) {
        Ok(_) => ck!("★'읽지 못함' 표기는 정규화되지 않는다", false),
        Err(e) => ck!(
            "★'읽지 못함' 표기의 remedy 는 어휘 추가를 권하지 않는다",
            e.code == "HINT_VOCAB_UNKNOWN" && e.remedy.unwrap_or_default().contains("추가하지 않는다"),
        ),
    }
    let mut f = _tag2_facts();
    del(&mut f, &["len"]);
    ck!("★축 통째 부재 = HINT_AXIS_UNDERIVABLE", code(derive_name(&f, vv)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    let f = with(&_tag2_facts(), "arch", with(&_tag2_facts()["arch"], "role", J::from("main")));
    ck!("★main 인데 N=2 = HINT_ARCH_ROLE_NODES_MISMATCH", code(derive_name(&f, vv)).as_deref() == Some("HINT_ARCH_ROLE_NODES_MISMATCH"));
    let mut arch2 = _tag2_facts()["arch"].clone();
    arch2.set("nodes", 1);
    arch2.set("role", "sub");
    arch2.set("target_gpu", "NVIDIA H100");
    let mut f = with(&_tag2_facts(), "arch", arch2.clone());
    ck!("시뮬레이션 타겟 → sim-<hw> · 서브 단독 1g1n", derive_name(&f, vv).map(|d| d.tag.contains("/gb10-1g1n-sub-sim-h100/")).unwrap_or(false));
    f.set("arch", with(&arch2, "target_gpu", J::from("GB10")));
    ck!("타겟 = 호스트 hw 면 native", derive_name(&f, vv).map(|d| d.tag.contains("/gb10-1g1n-sub-native/")).unwrap_or(false));
    let f = with(
        &_tag2_facts(),
        "model",
        crate::jd! {"hf_repo" => "Qwen/Qwen3.8-Flash-Next", "model_path" => "/app/q/Qwen/Qwen3.8-Flash-Next-NVFP4", "source" => "HF 카드 base_model"},
    );
    ck!("★hf_repo(base_model 오기) ≠ 체크포인트 basename = HINT_MODEL_SLUG_CONFLICT", code(derive_name(&f, vv)).as_deref() == Some("HINT_MODEL_SLUG_CONFLICT"));

    // ── 옛 세대 이름 ──
    let legacy = [
        ("hint/0.18.0/gpt-oss-20b/gb10-sim-h100/qmxfp4-len131072-kvfp8", GRAMMAR_LEGACY_ARCH, "HINT_ARCH_NODE_AXIS_ABSENT"),
        ("hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvauto-plemmap", GRAMMAR_LEGACY_ARCH_NODE, "HINT_ARCH_LEGACY_GRAMMAR"),
        ("hint/0.18.0/gpt-oss-20b/gb10-main-sim-h100/qmxfp4-len131072-kvfp8", GRAMMAR_LEGACY_ARCH_NODE, "HINT_ARCH_LEGACY_GRAMMAR"),
    ];
    for (name, gram, c) in legacy {
        let arch = name.split('/').nth(3).unwrap_or("");
        ck!(&format!("옛 이름 읽기 전용 해체({arch} → {gram})"), pt(name).map(|t| t.grammar == gram).unwrap_or(false));
        ck!(&format!("★옛 arch {arch} 는 신규 이름에서 {c} 로 거부"), code(validate_new_name(name)).as_deref() == Some(c));
    }
    {
        let mut s: Vec<&str> = legacy.iter().map(|(_, _, c)| *c).chain(["HINT_ARCH_SHAPE_VIOLATION"]).collect();
        s.sort();
        s.dedup();
        ck!("★옛 세대 사유코드는 형태 위반과 다르고 서로도 다르다", s.len() == 3);
    }
    ck!(
        "타 발행처 옛 이름(nightly vllm 세그먼트)도 읽기는 한다",
        pt("hint/0.1.1.dev53+g30118ba27/qwen3.8-flash-next-fp8/rtxpro6000x2-main-native/qfp8-len262144-kvauto-pleresident")
            .map(|t| t.grammar == GRAMMAR_LEGACY_ARCH_NODE)
            .unwrap_or(false),
    );
    ck!("★음성대조 4세그먼트는 parse_tag 가 HINT_NAME_SHAPE", code(parse_tag("hint/0.23.0/deepseek-v4-flash/sm121")).as_deref() == Some("HINT_NAME_SHAPE"));
    let go = |s: &str| grammar_of(s).unwrap_or_default();
    ck!("grammar_of 는 4세그먼트에 라벨만 붙인다", go("hint/0.23.0/deepseek-v4-flash/sm121") == GRAMMAR_LEGACY_4SEG);
    ck!(
        "★음성대조 6세그먼트·빈 세그먼트 거부",
        code(parse_tag("hint/a/b/c/d/e")).as_deref() == Some("HINT_NAME_SHAPE") && code(parse_tag("hint/a//c/d")).as_deref() == Some("HINT_NAME_SHAPE"),
    );
    let av = |a: &str| arch_why(a);
    ck!(
        "★v6 처럼 생긴 위반은 옛 세대로 오분류하지 않는다",
        av("gb10-0g1n-main-native") == Some("HINT_ARCH_SHAPE_VIOLATION") && go("hint/0.29.0/m/gb10-0g1n-main-native/x") == GRAMMAR_UNKNOWN,
    );
    ck!("arch 부재 차단", av("") == Some("HINT_ARCH_ABSENT"));
    ck!(
        "★대문자는 형태 위반",
        av("GB10-main-x") == Some("HINT_ARCH_SHAPE_VIOLATION") && av("GB10-1g1n-main-native") == Some("HINT_ARCH_SHAPE_VIOLATION"),
    );
    ck!("★미지 노드 축(옛 세대 모양)은 노드 축 부재", av("gb10-node-x") == Some("HINT_ARCH_NODE_AXIS_ABSENT"));
    ck!(
        "v6 3형상 통과",
        ["gb10-1g1n-main-sim-h100", "gb10-1g1n-sub-native", "gb10-1g2n-cluster-native", "rtxpro6000maxq-2g1n-main-native"]
            .iter()
            .all(|a| av(a).is_none()),
    );
    ck!(
        "arch 해체가 축을 준다",
        pa("gb10-1g2n-cluster-native", None)
            == crate::jd! {"grammar" => GRAMMAR_V6, "hw" => "gb10", "gpus_per_node" => 1, "nodes" => 2, "role" => "cluster",
            "target" => "native", "plane_token" => "", "plane" => "docker"},
    );
    ck!(
        "옛 arch 도 축 해체(읽기 전용)",
        pa("gb10x2-cluster-sim-h100", None)
            == crate::jd! {"grammar" => GRAMMAR_LEGACY_ARCH_NODE, "hw" => "gb10x2", "role" => "cluster", "target" => "sim-h100"},
    );
    ck!("★음성대조 미지 노드 축 조립 거부", code(build_arch("gb10", 1, 1, "worker", "native", "")).as_deref() == Some("HINT_ARCH_SHAPE_VIOLATION"));
    ck!("★음성대조 G=0 조립 거부", code(build_arch("gb10", 0, 1, "main", "native", "")).as_deref() == Some("HINT_AXIS_VALUE_INVALID"));
    ck!(
        "★음성대조 bool 은 수가 아니다",
        code(build_arch_j(&J::from("gb10"), &J::Bool(true), &J::Int(1), &J::from("main"), &J::from("native"), &J::from(""))).as_deref()
            == Some("HINT_AXIS_VALUE_INVALID"),
    );
    let rv = |r: &str| _split_recipe_raw(&J::from(r)).1;
    ck!("v6 레시피 통과(v7 문법의 한 경우)", rv("qnvfp4-len262144-kvauto-plemmap-spec3-eager").is_none());
    ck!("v7 기본 레시피(3축) 통과", rv("qmxfp4-len131072-kvfp8").is_none());
    ck!("★축 순서가 다르면 위반", rv("len262144-qnvfp4-kvauto-plemmap-spec3-eager") == Some("HINT_RECIPE_SHAPE_VIOLATION"));
    ck!("★q 축 생략(옛 동작)은 위반", rv("len262144-kvauto-plemmap") == Some("HINT_RECIPE_SHAPE_VIOLATION"));
    ck!(
        "v7: v6 뒤 축 일부만 있는 꼬리는 형태상 통과(꼬리는 자율 · 뜻·근거는 validate_tail)",
        rv("qnvfp4-len262144-kvauto-spec3-eager").is_none() && rv("qnvfp4-len1-kvauto-plemmap-spec0-eager").is_none(),
    );
    ck!(
        "★손저작 형태(대문자·옛 접미·빈 토큰·len0)는 위반",
        [
            "QNVFP4-len1-kvauto-plemmap-spec3-eager",
            "qnvfp4-len1-kvauto-plemmap-spec3-eager_260904T0730Z",
            "qnvfp4-len1-kvauto--mmap",
            "qnvfp4-len0-kvauto",
            "qnvfp4-len1-kv",
            "q-len1-kvauto",
        ]
        .iter()
        .all(|r| rv(r) == Some("HINT_RECIPE_SHAPE_VIOLATION")),
    );
    ck!("레시피 비었음", rv("") == Some("HINT_RECIPE_ABSENT"));
    ck!(
        "★vLLM 세그먼트에 v 접두 ✗",
        code(validate_new_name(&tag2_expected.replace("/0.29.0rc6/", "/v0.29.0rc6/"))).as_deref() == Some("HINT_VLLM_SEGMENT_SHAPE"),
    );
    ck!("★모델 슬러그 대문자 ✗", code(validate_new_name(&tag2_expected.replace("qwen3.8", "Qwen3.8"))).as_deref() == Some("HINT_MODEL_SLUG_SHAPE"));
    ck!("모델 슬러그 인덱스는 그대로 [2](레시피를 맨 뒤에 붙인 이유 · 2026-09-04)", tag2_expected.split('/').nth(2) == Some("qwen3.8-flash-next-nvfp4"));

    // ── 슬러그 · sweep_bench 규칙 교차 대조 ──
    let skill = core::skill_dir();
    let repo = skill.ancestors().nth(3).map(Path::to_path_buf).unwrap_or(skill.clone());
    let ptext = std::fs::read_to_string(repo.join(_SLUG_PRODUCER_PATH)).unwrap_or_default();
    ck!(
        "★producer(assemble_sweep_meta.rs) 에 인증서 model 키 규칙 리터럴이 실재한다(거울 갈라짐 방지)",
        _SLUG_PRODUCER_LITERALS.iter().all(|l| ptext.contains(l)),
    );
    let sweep_rule = |p: &str| python_lower(&pstrip(basename(p.trim_end_matches('/'))));
    let old_rule = |p: &str| {
        let low = python_lower(basename(p.trim_end_matches('/')));
        let mut out = String::new();
        let mut in_bad = false;
        for c in low.chars() {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-' {
                out.push(c);
                in_bad = false;
            } else if !in_bad {
                out.push('-');
                in_bad = true;
            }
        }
        out.trim_matches('-').to_string()
    };
    let ds = |h: Option<&str>, m: Option<&str>| derive_slug(h, m);
    for p in [
        "/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4",
        "/app/models/OpenAI/gpt-oss-20b/",
        "/app/models/google/gemma-4-E2B-it",
        "/app/models/LiquidAI/LFM2.5-2.6B",
        "rel/Qwen3-4B",
    ] {
        ck!(&format!("derive_slug(model_path) = sweep_bench 인증서 model 키({p})"), ds(None, Some(p)).ok() == Some(sweep_rule(p)));
        ck!(&format!("derive_slug(model_path) = 옛 hint_tag 규칙(안전 문자 basename · {p})"), ds(None, Some(p)).ok() == Some(old_rule(p)));
    }
    ck!(
        "★두 옛 규칙이 갈라지는 basename(공백·+)은 어느 쪽으로도 맞추지 않고 차단한다",
        sweep_rule("/m/My Model+v2") != old_rule("/m/My Model+v2") && code(ds(None, Some("/m/My Model+v2"))).as_deref() == Some("HINT_MODEL_SLUG_UNSAFE"),
    );
    ck!("hf_repo 는 마지막 성분 소문자", ds(Some("Qwen/Qwen3.8-Flash-Next-NVFP4"), None).ok().as_deref() == Some("qwen3.8-flash-next-nvfp4"));
    ck!("hf_repo 가 model_path 보다 먼저(옛 우선순위)", ds(Some("org/Abc"), Some("/x/Def")).ok().as_deref() == Some("abc"));
    ck!("★음성대조 '/' 없는 hf_repo", code(ds(Some("Qwen3-4B"), None)).as_deref() == Some("HINT_MODEL_HF_REPO_SHAPE"));
    ck!("★음성대조 입력 없음 = 파생 불가", code(ds(None, None)).as_deref() == Some("HINT_AXIS_UNDERIVABLE"));
    ck!("★음성대조 치환이 필요한 basename 은 차단(인증서 키와 갈라짐)", code(ds(None, Some("/m/My Model+v2"))).as_deref() == Some("HINT_MODEL_SLUG_UNSAFE"));
    let bs = |s: &str, voc: &J| base_slug(s, voc).ok().unwrap_or_default();
    ck!(
        "base_slug 가 양자화 접미를 뗀다",
        bs("qwen3.8-flash-next-nvfp4", vv) == "qwen3.8-flash-next"
            && bs("hy3-nvfp4-w4a16", vv) == "hy3"
            && bs("gpt-oss-20b", vv) == "gpt-oss-20b"
            && bs("qwen3-4b", vv) == "qwen3-4b",
    );
    ck!("★접미뿐인 슬러그는 비우지 않는다", bs("fp8", vv) == "fp8");
    ck!(
        "★quant_suffixes 없는 어휘표 = 차단(조용히 아무것도 떼지 않는 폴백 ✗)",
        code(base_slug("qwen3-4b-fp8", &J::dict())).as_deref() == Some("HINT_VOCAB_ABSENT") && code(base_slug("qwen3-4b-fp8", &J::Null)).as_deref() == Some("HINT_VOCAB_ABSENT"),
    );
    ck!("정규화 슬러그 동치(2026-08-20 철자 분열)", norm_slug("gemma-4-E2B-it").ok() == norm_slug("gemma-4-e2b-it").ok());
    // norm_slug(None) == "" is the caller's (bridge's) None → "" mapping; the string "None" folds to "none".
    ck!("★None 은 빈 키('none' 슬러그로 접지 않는다)", norm_slug("None").ok().as_deref() == Some("none"));
    let sort_by_key = |v: &[&str]| -> Vec<String> {
        let mut x: Vec<String> = v.iter().rev().map(|s| s.to_string()).collect();
        x.sort_by_key(|s| version_key(s).unwrap());
        x
    };
    let order = ["0.29.0rc5", "0.29.0rc5-g0123456789ab", "0.29.0rc6", "0.29.0", "0.29.1"];
    ck!("version_key 가 릴리스·커밋 핀·rc 를 문법대로 줄 세운다", sort_by_key(&order) == order);
    let order4 = ["0.9.2", "0.9.2.post1", "0.9.2.1", "0.9.2.1.post1", "0.9.3rc1", "0.9.3"];
    ck!("version_key — 4마디 · .postN 도 문법대로(D-g)", sort_by_key(&order4) == order4);

    // ── D-g ──
    for (r, seg) in [("v0.9.2.1", "0.9.2.1"), ("v0.9.2.post1", "0.9.2.post1"), ("0.9.2.1rc2", "0.9.2.1rc2")] {
        let f = with(&_tag2_facts(), "vllm", with(&_tag2_facts()["vllm"], "vllm_ref", J::from(r)));
        let ok = derive_name(&f, vv)
            .map(|got| {
                got.tag.split('/').nth(1) == Some(seg)
                    && pt(&got.tag).map(|t| t.grammar == GRAMMAR_V7).unwrap_or(false)
                    && got.vllm_build_input["kind"].as_str() == Some("release")
            })
            .unwrap_or(false);
        ck!(&format!("D-g 릴리스 {r} → 세그먼트 {seg} · v7 문법 통과"), ok);
    }
    ck!(
        "D-g wheel 4마디 릴리스",
        vseg(crate::jd! {"track" => "wheel", "vllm_version" => "0.9.2.1", "source" => "셀 env VLLM_VERSION"}).ok().as_deref() == Some("0.9.2.1"),
    );
    ck!(
        "D-g describe 4마디 · 커밋 핀",
        vseg(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "prev_release" => "v0.9.2.1-3-g0123456", "source" => "빌드 원장"})
            .ok()
            .as_deref()
            == Some("0.9.2.1-g0123456789ab"),
    );
    ck!(
        "★D-g 다섯 마디·nightly 는 여전히 릴리스 아님",
        vcode(crate::jd! {"track" => "wheel", "vllm_version" => "0.9.2.1.1", "source" => "셀 env"}).as_deref() == Some("HINT_AXIS_UNDERIVABLE")
            && code(validate_new_name(&tag2_expected.replace("/0.29.0rc6/", "/0.1.1.dev53/"))).as_deref() == Some("HINT_VLLM_SEGMENT_SHAPE"),
    );

    // ── D-h ──
    let up = format!("https://{}.git", UPSTREAM_VLLM_REPOS[0]);
    let rk = |s: &J| vllm_repo_kind(s).ok().flatten();
    ck!(
        "vllm_repo_kind: https · scp형 · 끝 슬래시 = upstream",
        rk(&J::from(up.as_str())).as_deref() == Some("upstream")
            && rk(&J::from(format!("git@{}.git", UPSTREAM_VLLM_REPOS[0].replacen('/', ":", 1)))).as_deref() == Some("upstream")
            && rk(&J::from(format!("{}/", &up[..up.len() - 4]))).as_deref() == Some("upstream"),
    );
    ck!(
        "vllm_repo_kind: 포크 · 로컬 경로 = fork · 미관측 = None",
        rk(&J::from("https://example.invalid/me/vllm.git")).as_deref() == Some("fork")
            && rk(&J::from("./vllm-src")).as_deref() == Some("fork")
            && rk(&J::Null).is_none()
            && rk(&J::from("N/A")).is_none(),
    );
    let fork = "https://example.invalid/someone/vllm.git";
    ck!(
        "★D-h 포크 + 릴리스 모양 ref + SHA 없음 = 파생 불가(업스트림 이름 ✗)",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => "v0.29.0rc6", "vllm_repo" => fork, "source" => "셀 env"}).as_deref()
            == Some("HINT_AXIS_UNDERIVABLE"),
    );
    let mut fkv = _tag2_facts()["vllm"].clone();
    fkv.set("vllm_repo", fork);
    fkv.set("prev_release", "v0.29.0rc6");
    let fk = derive_name(&with(&_tag2_facts(), "vllm", fkv), vv);
    let fk_seg = fk.as_ref().map(|d| d.tag.split('/').nth(1).unwrap_or("").to_string()).unwrap_or_default();
    ck!(
        &format!("D-h 포크 + SHA + 직전 릴리스 → <prev>-g<sha12>({fk_seg})"),
        fk.as_ref()
            .map(|fk| {
                fk_seg == format!("0.29.0rc6-g{}", &_TAG2_SHA[..12])
                    && fk.vllm_build_input["kind"].as_str() == Some("commit")
                    && !dumps_unicode(&fk.to_payload()).contains(fork)
            })
            .unwrap_or(false),
    );
    ck!(
        "★D-h vllm_repo 키 부재 = 파생 불가",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => "v0.29.0rc6", "vllm_sha" => _TAG2_SHA, "source" => "셀 env"}).as_deref()
            == Some("HINT_AXIS_UNDERIVABLE"),
    );
    ck!(
        "★D-h VLLM_REPO 미관측(None) = 파생 불가",
        vcode(crate::jd! {"track" => "source-build", "vllm_ref" => "v0.29.0rc6", "vllm_repo" => J::Null, "vllm_sha" => _TAG2_SHA, "source" => "셀 env"})
            .as_deref()
            == Some("HINT_AXIS_UNDERIVABLE"),
    );
    ck!(
        "D-h 40자 커밋 핀은 저장소와 무관(SHA 가 곧 좌표)",
        vseg(crate::jd! {"track" => "source-build", "vllm_ref" => sha, "vllm_repo" => fork, "prev_release" => "v0.29.0rc5", "source" => "셀 env"})
            .ok()
            .as_deref()
            == Some("0.29.0rc5-g0123456789ab"),
    );

    // ── v7 이름 문법 ──
    for live in _V6_LIVE {
        let (pre, rec) = live.rsplit_once('/').unwrap();
        let base = format!("{pre}/{}", rec.split('-').take(3).collect::<Vec<_>>().join("-"));
        let parts: Vec<&str> = live.split('/').collect();
        let ok = pt(live)
            .map(|t| {
                t.grammar == GRAMMAR_V6
                    && t.base_tag().as_deref() == Some(base.as_str())
                    && base_name(live).ok().as_deref() == Some(base.as_str())
                    && t.tail.len() == 3
                    && t.tail[0].starts_with("ple")
                    && t.tail[1].starts_with("spec")
                    && _GRAPH_TOKENS.contains(&t.tail[2].as_str())
                    && t.timestamp.is_none()
                    && code(validate_new_name(live)).is_none()
            })
            .unwrap_or(false);
        ck!(&format!("v6 실태그 재해석({}·{}) → v6 · 기본 이름 + 꼬리 3토큰 · timestamp 없음", parts[2], parts[3]), ok);
    }
    let v6r = _TAG2_V6.split('/').nth(4).unwrap();
    ck!(
        "v6 실태그: 레시피 6축 해체도 그대로(parse_recipe)",
        parse_recipe(&J::from(v6r)).ok().flatten()
            == Some(crate::jd! {"q" => "nvfp4", "len" => "262144", "kv" => "auto", "ple" => "mmap", "spec" => "3", "graph" => "eager"}),
    );
    ck!("v6 실태그 기본 이름 = 같은 facts 의 v7 derive_name(tag.py 재파생 대조 입력)", base_name(_TAG2_V6).ok().as_deref() == Some(dn.tag.as_str()));
    let mut v6p = pay.clone();
    v6p.set("grammar", GRAMMAR_V6);
    set_path(&mut v6p, &["segments", "recipe", "value"], J::from(v6r));
    for (a, val) in [("ple", "mmap"), ("spec", "3"), ("graph", "eager")] {
        set_path(&mut v6p, &["axes", a], crate::jd! {"value" => val, "source" => "fixture"});
    }
    ck!("기발행 v6 페이로드(6축)는 옛 6축 재조립 그대로", compose_from_payload(&v6p).ok().as_deref() == Some(_TAG2_V6));
    let mut v6t = v6p.clone();
    set_path(&mut v6t, &["axes", "spec", "value"], J::from("2"));
    ck!("★음성대조 v6 페이로드 뒤 축 위조 = 재조립 불일치(엄격 경로 유지)", code(compose_from_payload(&v6t)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    let mut v6n = v6p.clone();
    for a in V6_TAIL_AXES {
        del(&mut v6n, &["axes", a]);
    }
    ck!("v6 페이로드에 뒤 3축 칸이 전부 없으면 3축 + 꼬리로 해석해 재조립", compose_from_payload(&v6n).ok().as_deref() == Some(_TAG2_V6));
    let mut v6h = v6p.clone();
    del(&mut v6h, &["axes", "graph"]);
    ck!("★음성대조 v6 뒤 3축 일부만 = 모순", code(compose_from_payload(&v6h)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    set_path(&mut v6n, &["axes", "len", "value"], J::from("131072"));
    ck!("★음성대조 3축 + 꼬리 해석 경로도 결정론 축 위조를 잡는다", code(compose_from_payload(&v6n)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    let bad_g = with(&pay, "grammar", J::from("v9"));
    ck!("★음성대조 알 수 없는 grammar = 재조립 거부", code(compose_from_payload(&bad_g)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));

    // ② v7 기본 이름 + 꼬리 + timestamp
    ck!("빈 꼬리 = 기본 이름 그대로", compose_name(&dn.tag, &J::list()).ok().as_deref() == Some(dn.tag.as_str()));
    let y_rel = "output/multi/configs/nv4-bf-262k-mmp.yaml";
    let e_rel = "output/multi/envs/.env.nv4-bf-262k-mmp";
    let mut srcs_m = Map::new();
    srcs_m.insert(
        y_rel,
        crate::jd! {"max-model-len" => "262144", "speculative-config.num_speculative_tokens" => "3",
            "enforce-eager" => "True", "served-model-name" => "'Qwen3.8'"},
    );
    srcs_m.insert(e_rel, crate::jd! {"VLLM_PLE_MMAP" => "1"});
    let srcs = J::Dict(srcs_m);
    let tail = crate::jl![
        crate::jd! {"token" => "mmap", "meaning" => "PLE 를 mmap 으로 적재(res·mmp 셀을 가른다)",
            "evidence" => crate::jd!{"file" => e_rel, "key" => "VLLM_PLE_MMAP", "value" => "1"}},
        crate::jd! {"token" => "spec3", "meaning" => "speculative k=3",
            "evidence" => crate::jd!{"file" => y_rel, "key" => "speculative-config.num_speculative_tokens", "value" => 3}},
        crate::jd! {"token" => "eager", "meaning" => "cudagraph 끔",
            "evidence" => crate::jd!{"file" => y_rel, "key" => "enforce-eager", "value" => " true "}},
    ];
    let vt = |t: &J, s: &J| validate_tail(t, s).unwrap_or_else(|e| vec![(e.code, e.message)]);
    ck!("validate_tail 양성: 형식·뜻·근거 전부 맞으면 문제 0(값 정규화 = 공백·대소문자 · 수는 문자열로)", vt(&tail, &srcs).is_empty());
    ck!("validate_tail: 빈 꼬리 = 문제 0", vt(&J::list(), &srcs).is_empty() && vt(&J::list(), &J::dict()).is_empty());
    ck!(
        "validate_tail: 바깥 따옴표 정규화",
        vt(
            &crate::jl![crate::jd! {"token" => "q38", "meaning" => "서빙 이름", "evidence" => crate::jd!{"file" => y_rel, "key" => "served-model-name", "value" => "qwen3.8"}}],
            &srcs
        )
        .is_empty(),
    );
    let full = compose_name(&dn.tag, &tail).unwrap_or_default();
    {
        let ft = pt(&full);
        ck!(
            &format!("compose_name → v7 이름({}) · 꼬리 해체 · 기본 이름 복원", full.split('/').nth(4).unwrap_or("")),
            full == format!("{}-mmap-spec3-eager", dn.tag)
                && ft
                    .map(|ft| {
                        ft.grammar == GRAMMAR_V7
                            && ft.tail == ["mmap", "spec3", "eager"]
                            && ft.base_tag().as_deref() == Some(dn.tag.as_str())
                            && ft.timestamp.is_none()
                    })
                    .unwrap_or(false)
                && base_name(&full).ok().as_deref() == Some(dn.tag.as_str()),
        );
    }
    ck!("compose_name 은 토큰 문자열 목록도 받는다", compose_name(&dn.tag, &jl_strs(&["mmap", "spec3", "eager"])).ok().as_deref() == Some(full.as_str()));
    ck!("★음성대조 compose_name 에 기본 이름 아닌 입력 = HINT_TAIL_FORMAT", code(compose_name(&full, &jl_strs(&["x"]))).as_deref() == Some("HINT_TAIL_FORMAT"));
    ck!(
        "★음성대조 compose_name 형식 위반 꼬리 = HINT_TAIL_FORMAT",
        code(compose_name(&dn.tag, &jl_strs(&["Mmap"]))).as_deref() == Some("HINT_TAIL_FORMAT")
            && code(compose_name(&dn.tag, &jl_strs(&["a", "a"]))).as_deref() == Some("HINT_TAIL_FORMAT"),
    );
    let codes = |t: &J| -> Vec<String> { vt(t, &srcs).into_iter().map(|(c, _)| c).collect() };
    let ev_default = crate::jd! {"file" => e_rel, "key" => "VLLM_PLE_MMAP", "value" => "1"};
    let row = |tok: &str| crate::jd! {"token" => tok, "meaning" => "뜻", "evidence" => ev_default.clone()};
    let row_kw = |tok: &str, k: &str, val: J| with(&row(tok), k, val);
    let a20 = "a".repeat(20);
    let b20 = "b".repeat(20);
    let c20 = "c".repeat(20);
    let cases: Vec<(&str, J, &str)> = vec![
        ("대문자 토큰", crate::jl![row("Mmap")], "HINT_TAIL_FORMAT"),
        ("비영숫자 토큰", crate::jl![row("mm_ap")], "HINT_TAIL_FORMAT"),
        ("25자 토큰", crate::jl![row(&"a".repeat(25))], "HINT_TAIL_FORMAT"),
        ("꼬리 65자", crate::jl![row(&a20), row(&b20), row(&c20), row("dd")], "HINT_TAIL_FORMAT"),
        ("timestamp 예약 모양", crate::jl![row("t2609290823")], "HINT_TAIL_FORMAT"),
        ("중복 토큰", crate::jl![row("mmap"), row("mmap")], "HINT_TAIL_FORMAT"),
        ("행이 객체 아님", jl_strs(&["mmap"]), "HINT_TAIL_FORMAT"),
        ("v6 뒤 3축 모양 그대로(파서가 v6 로 읽는다)", crate::jl![row("plemmap"), row("spec3"), row("eager")], "HINT_TAIL_FORMAT"),
        ("meaning 빈칸", crate::jl![row_kw("mmap", "meaning", J::from("  "))], "HINT_TAIL_MEANING_ABSENT"),
        ("meaning 부재", crate::jl![crate::jd! {"token" => "mmap", "evidence" => ev_default.clone()}], "HINT_TAIL_MEANING_ABSENT"),
        ("evidence 부재", crate::jl![row_kw("mmap", "evidence", J::Null)], "HINT_TAIL_UNGROUNDED"),
        ("evidence value 부재", crate::jl![row_kw("mmap", "evidence", crate::jd! {"file" => e_rel, "key" => "VLLM_PLE_MMAP"})], "HINT_TAIL_UNGROUNDED"),
        ("서빙 설정 밖 파일", crate::jl![row_kw("mmap", "evidence", crate::jd! {"file" => "README.md", "key" => "x", "value" => "1"})], "HINT_TAIL_UNGROUNDED"),
        ("파일에 없는 키", crate::jl![row_kw("mmap", "evidence", crate::jd! {"file" => e_rel, "key" => "VLLM_PLE_MAP", "value" => "1"})], "HINT_TAIL_UNGROUNDED"),
        ("값 불일치(거짓 꼬리)", crate::jl![row_kw("mmap", "evidence", crate::jd! {"file" => e_rel, "key" => "VLLM_PLE_MMAP", "value" => "0"})], "HINT_TAIL_UNGROUNDED"),
    ];
    for (what, t, want) in cases {
        ck!(&format!("★validate_tail 음성대조 {what} → {want}"), codes(&t).iter().any(|c| c == want));
    }
    ck!("★validate_tail 음성대조 목록 아님 = HINT_TAIL_FORMAT", codes(&J::from("mmap")) == ["HINT_TAIL_FORMAT"]);
    ck!(
        "v6 뒤 3축 중 일부·재배열은 형식상 허용(자율 꼬리)",
        codes(&crate::jl![row("mmap"), row("spec3"), row("eager")]).is_empty() && codes(&crate::jl![row("plemmap"), row("eager")]).is_empty(),
    );

    let utc = "2026-09-28T23:23:13Z";
    ck!(
        "timestamp_token = t + KST YYMMDDHHMM(자정 넘김 포함)",
        timestamp_token(utc).ok().as_deref() == Some("t2609290823") && timestamp_token("2026-12-31T15:00:00Z").ok().as_deref() == Some("t2701010000"),
    );
    ck!("★timestamp_token 주입 시각 형식 위반 = HINT_TIME_NOT_INJECTED", code(timestamp_token("2026-09-29 08:23")).as_deref() == Some("HINT_TIME_NOT_INJECTED"));
    let tsn = with_timestamp(&full, utc).unwrap_or_default();
    {
        let tt = pt(&tsn);
        ck!(
            &format!("with_timestamp → 맨 끝 -t<YYMMDDHHMM>({}) · v7 · 꼬리 보존", tsn.split('/').nth(4).unwrap_or("")),
            tsn == format!("{full}-t2609290823")
                && tt
                    .map(|tt| {
                        tt.grammar == GRAMMAR_V7
                            && tt.timestamp.as_deref() == Some("t2609290823")
                            && tt.tail == ["mmap", "spec3", "eager"]
                            && tt.base_tag().as_deref() == Some(dn.tag.as_str())
                    })
                    .unwrap_or(false),
        );
    }
    {
        let w = with_timestamp(&dn.tag, utc).ok().and_then(|s| pt(&s));
        ck!("빈 꼬리 + timestamp", w.map(|t| t.tail.is_empty() && t.timestamp.as_deref() == Some("t2609290823")).unwrap_or(false));
    }
    ck!("★음성대조 timestamp 두 번 = HINT_TAIL_FORMAT(같은 분 충돌은 차단)", code(with_timestamp(&tsn, utc)).as_deref() == Some("HINT_TAIL_FORMAT"));
    ck!(
        "★음성대조 옛 세대 이름에는 timestamp·기본 이름 ✗",
        code(with_timestamp("hint/0.18.0/m/gb10-sim-h100/qmxfp4-len131072-kvfp8", utc)).as_deref() == Some("HINT_NAME_GRAMMAR_OLD")
            && code(base_name("hint/0.18.0/m/gb10-sim-h100/qmxfp4-len131072-kvfp8")).as_deref() == Some("HINT_NAME_GRAMMAR_OLD"),
    );

    // ③ 파서의 v7/v6 구분
    ck!(
        "파서: v6 모양 = v6(우선) · v6 모양 + timestamp = v7 · 기본 = v7 · 자율 꼬리 = v7",
        go(_TAG2_V6) == GRAMMAR_V6 && go(&format!("{_TAG2_V6}-t2609290823")) == GRAMMAR_V7 && go(&dn.tag) == GRAMMAR_V7 && go(&full) == GRAMMAR_V7,
    );
    ck!(
        "★파서: timestamp 가 맨 끝이 아니면 꼬리 형식 위반(HINT_TAIL_FORMAT) · 세대 unknown",
        rv(&v6r.replace("-spec3", "-t2609290823-spec3")) == Some("HINT_TAIL_FORMAT")
            && go(&_TAG2_V6.replace("-spec3", "-t2609290823-spec3")) == GRAMMAR_UNKNOWN
            && code(validate_new_name(&_TAG2_V6.replace("-spec3", "-t2609290823-spec3"))).as_deref() == Some("HINT_TAIL_FORMAT"),
    );
    ck!(
        "★파서: 꼬리 토큰 25자 · 중복 = HINT_TAIL_FORMAT",
        rv(&format!("qnvfp4-len1-kvauto-{}", "a".repeat(25))) == Some("HINT_TAIL_FORMAT") && rv("qnvfp4-len1-kvauto-x-x") == Some("HINT_TAIL_FORMAT"),
    );
    ck!(
        "split_recipe: 해체 모양",
        split_recipe(&format!("{v6r}-t2609290823")).ok().flatten()
            == Some(crate::jd! {"q" => "nvfp4", "len" => "262144", "kv" => "auto", "base" => "qnvfp4-len262144-kvauto",
                "tail" => jl_strs(&["plemmap", "spec3", "eager"]), "timestamp" => "t2609290823"})
            && split_recipe("x").ok().flatten().is_none(),
    );

    // ④ 페이로드: apply_tail → compose_from_payload
    match apply_tail(&pay, &tail, None) {
        Ok(fin) => {
            ck!(
                "apply_tail → compose_from_payload = compose_name(꼬리)",
                compose_from_payload(&fin).ok().as_deref() == Some(full.as_str())
                    && fin["tail"][0]["meaning"].truthy()
                    && fin["timestamp"].is_null()
                    && fin.get("timestamp").is_some()
                    && fin["segments"]["recipe"]["source"].display().contains("tail(agent"),
            );
            let ts_tok = timestamp_token(utc).unwrap_or_default();
            ck!(
                "apply_tail + timestamp → compose_from_payload = with_timestamp",
                apply_tail(&pay, &tail, Some(&ts_tok)).and_then(|f| compose_from_payload(&f)).ok().as_deref() == Some(tsn.as_str()),
            );
            ck!(
                "apply_tail 은 입력을 바꾸지 않는다(사본)",
                pay["segments"]["recipe"]["value"].as_str() == dn.segment("recipe").map(|a| a.value.as_str()) && pay.get("tail").is_none(),
            );
            let mut tam = fin.clone();
            if let Some(J::List(l)) = tam.get_mut("tail") {
                l[1].set("token", "spec4");
            }
            ck!("★음성대조 PAYLOAD.naming.tail 위조 = 재조립 불일치", code(compose_from_payload(&tam)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
            let mut tam = fin.clone();
            if let Some(J::List(l)) = tam.get_mut("tail") {
                l.push(crate::jd! {"token" => "t2609290823"});
            }
            ck!("★음성대조 PAYLOAD.naming.tail 에 timestamp 모양 = HINT_TAIL_FORMAT", code(compose_from_payload(&tam)).as_deref() == Some("HINT_TAIL_FORMAT"));
        }
        Err(e) => ck!(&format!("apply_tail 실패 {}", e.str()), false),
    }
    ck!("★음성대조 apply_tail 은 v6 페이로드를 받지 않는다", code(apply_tail(&v6p, &tail, None)).as_deref() == Some("HINT_NAMING_INCONSISTENT"));
    ck!("★음성대조 apply_tail timestamp 모양 위반", code(apply_tail(&pay, &tail, Some("2609290823"))).as_deref() == Some("HINT_TAIL_FORMAT"));

    // ⑤ 꼬리 후보
    let mut fc = _tag2_facts();
    set_path(&mut fc, &["ple", "evidence"], ev_default.clone());
    let cc = J::Dict(by_axis(&tail_candidates(&fc, vv).unwrap_or_default()));
    ck!(
        "tail_candidates: v6 뒤 3축 파생 토큰 · 구조화 근거는 그대로 · 없으면 None",
        V6_TAIL_AXES.iter().map(|a| cc[*a]["token"].display()).collect::<Vec<_>>() == ["plemmap", "spec3", "eager"]
            && cc["ple"]["evidence"] == ev_default
            && cc["spec"]["evidence"].is_null()
            && cc["graph"]["evidence"].is_null(),
    );
    let rows3 = J::List(
        V6_TAIL_AXES
            .iter()
            .map(|a| crate::jd! {"token" => cc[*a]["token"].clone(), "meaning" => cc[*a]["meaning"].clone(), "evidence" => ev_default.clone()})
            .collect(),
    );
    ck!("★후보 셋을 그대로 쓰면 v6 모양이라 validate_tail 이 거부한다(후보 ≠ 정답)", codes(&rows3).iter().any(|c| c == "HINT_TAIL_FORMAT"));
    let fbad = with(&_tag2_facts(), "ple", crate::jd! {"raw" => "swap", "source" => "fixture"});
    ck!(
        "★어휘 밖 PLE 후보 = error 행(조용히 건너뛰지 않는다)",
        J::Dict(by_axis(&tail_candidates(&fbad, vv).unwrap_or_default()))["ple"]["error"].as_str() == Some("HINT_VOCAB_UNKNOWN"),
    );

    // ── 어휘표 적재 ──
    {
        let td = core::TempDir::new("hintlib-naming-");
        let r = td.path();
        ck!("★어휘표 부재 = HINT_VOCAB_ABSENT", code(load_vocab(r)).as_deref() == Some("HINT_VOCAB_ABSENT"));
        let _ = std::fs::create_dir_all(r.join("hints"));
        let _ = std::fs::write(r.join(core::REL_VOCAB), "{not json");
        ck!("★어휘표 깨짐 = HINT_VOCAB_ABSENT", code(load_vocab(r)).as_deref() == Some("HINT_VOCAB_ABSENT"));
        let mut broken = _fixture_vocab();
        if let Some(J::List(l)) = broken.get_mut("kv").and_then(|k| k.get_mut("fp8e5m2")) {
            l.push(J::from("FP8_E4M3"));
        }
        let _ = std::fs::write(r.join(core::REL_VOCAB), crate::value::dumps_default(&broken));
        ck!("★원문 하나가 두 토큰에 걸리면 HINT_VOCAB_ABSENT", code(load_vocab(r)).as_deref() == Some("HINT_VOCAB_ABSENT"));
        let push_alias = |axis: &str, tok: &str, a: &str| {
            let mut b = _fixture_vocab();
            if let Some(J::List(l)) = b.get_mut(axis).and_then(|k| k.get_mut(tok)) {
                l.push(J::from(a));
            }
            b
        };
        ck!(
            "★'N/A' 를 값으로 등재하면 구조 검사가 잡는다",
            validate_vocab(&push_alias("quant", "bf16", "N/A")).map(|b| b.iter().any(|p| p.contains("읽지 못함"))).unwrap_or(false),
        );
        ck!(
            "★템플릿 빈칸을 값으로 등재하면 구조 검사가 잡는다",
            validate_vocab(&push_alias("hw", "gb10", "<<FILL>>")).map(|b| b.iter().any(|p| p.contains("빈칸"))).unwrap_or(false),
        );
        ck!(
            "★에디션 모호 원문을 토큰에 합치면 구조 검사가 잡는다",
            validate_vocab(&push_alias("hw", "rtxpro6000srv", "NVIDIA RTX PRO 6000")).map(|b| b.iter().any(|p| p.contains("에디션"))).unwrap_or(false),
        );
        let mut broken = _fixture_vocab();
        set_path(&mut broken, &["graph", "cudagraph"], jl_strs(&["cg"]));
        broken.set("kv_", J::dict());
        ck!("★graph 토큰 추가·오타 키는 구조 검사가 잡는다", validate_vocab(&broken).map(|b| b.len() >= 2).unwrap_or(false));
        let _ = std::fs::write(r.join(core::REL_VOCAB), crate::value::dumps_default(&_fixture_vocab()));
        ck!("정상 어휘표 적재", load_vocab(r).map(|d| d["kv"]["fp8"][0].as_str() == Some("fp8")).unwrap_or(false));
    }

    // 추적 어휘표(배포물)
    match load_vocab(&repo) {
        Err(e) => ck!(&format!("추적 어휘표 적재({}: {})", e.code, e.message), false),
        Ok(real) => {
            let observed = [
                ("NVIDIA GB10", "gb10"),
                ("GB10", "gb10"),
                ("NVIDIA H100", "h100"),
                ("H100", "h100"),
                ("RTX 4090 (simulated on GB10)", "rtx4090"),
                ("NVIDIA RTX PRO 6000 Blackwell Max-Q Workstation Edition x2", "rtxpro6000maxq"),
                ("NVIDIA RTX PRO 6000 Blackwell Server Edition", "rtxpro6000srv"),
            ];
            for (raw, tok) in observed {
                ck!(&format!("추적 어휘표: {} → {tok}", py_repr(raw)), normalize("hw", &J::from(raw), &real).ok().as_deref() == Some(tok));
            }
            ck!(
                "★추적 어휘표: 에디션 모호 원문은 차단",
                code(normalize("hw", &J::from("NVIDIA RTX PRO 6000"), &real)).as_deref() == Some("HINT_VOCAB_UNKNOWN"),
            );
            ck!(
                "추적 어휘표: kv fp8·fp8_e4m3 → fp8(F13)",
                normalize("kv", &J::from("fp8"), &real).ok().as_deref() == Some("fp8")
                    && normalize("kv", &J::from("fp8_e4m3"), &real).ok().as_deref() == Some("fp8"),
            );
            ck!("★추적 어휘표: quant 'N/A' 는 어휘 밖", code(normalize("quant", &J::from("N/A"), &real)).as_deref() == Some("HINT_VOCAB_UNKNOWN"));
            ck!("추적 어휘표로도 태그2 기대 이름", derive_name(&_tag2_facts(), &real).map(|d| d.tag).ok() == Some(tag2_expected.clone()));
            let fr = with(&_tag2_facts(), PLANE_AXIS, crate::jd! {"raw" => "native", "source" => "fixture · serve_proof plane=native"});
            ck!(
                "AC-N6 추적 어휘표: native 셀 → …-cluster-native-bare",
                derive_name(&fr, &real).map(|d| d.tag).ok() == Some(tag2_expected.replace("-cluster-native/", "-cluster-native-bare/")),
            );
        }
    }
    // (import 부수효과 탐침 — Rust 에는 import 시점 실행이 없으므로 생략: 위 doc 주석 참조)
    bad
}
