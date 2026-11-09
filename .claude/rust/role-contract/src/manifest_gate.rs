//! `manifest_contract.py` as a pure library: the terraforming-complete Flag gate (`evaluate_contract`), the manifest-derived tensor
//! parallelism (`manifest_tp`), the per-node `model_source` resolution (`effective_model_source`) and the branch-to-topology
//! helpers (`resolve_topology`, `branch_topology_conflict`), over Python-compatible values ([`PyValue`]).
//!
//! **What is reproduced.** The owner's operator semantics on values it does not control (a manifest is YAML): truthiness of
//! `man.get(k) or default`, `is not True` (so `1` and `"true"` are not the flag), `int(raw)` caught for `TypeError`/`ValueError`
//! only (so `.inf` raises an uncaught `OverflowError`), `len(nodes)` and `enumerate(nodes)` on whatever the YAML built, `%r` of
//! arbitrary values and the key order of the result dictionary. An exception the owner does not handle is returned as data
//! ([`Raised`]) with the owner frames below `main` that Python 3.12 prints for it; the command line builds the traceback text.
//!
//! **What is not claimed.** The captured original (Python 3.12.3, PyYAML 6.0.1) is the oracle; this module carries no second copy
//! of an expectation. The branches it answers are a FINITE set that was captured raw (the 218 original observations of the CLI
//! corpus, among them the unhashable role frame (X03/X04), the bytes `work_dir`/`tiktoken_host_path` branches (Y03-Y06) and the
//! NaN role-key identity (Y07)); that is a list of observed branches, not a proof for every possible YAML shape. Known remaining
//! limits are stated at their sites (the CLI's JSON-depth model for shapes without a raw, the in-process importers, Python
//! objects the YAML port does not build). [`GateFailure::Unsupported`] stays in the contract for a shape a caller must state
//! instead of guess; the gate itself currently has none.
//!
//! Each [`OwnerFrame`] row and each reason string is bound to the bytes of the reviewed owner by a unit check
//! (`source_identity::REVIEWED_MANIFEST_CONTRACT`).

use agent_runtime_support::pynumber::int_call;
use agent_runtime_support::pyops::{compare, multiply, Compare};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{DictKey, PyDict, PyValue, Typed};
use agent_runtime_support::pyyaml::dict_key;
use std::sync::Arc;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_NO_MANIFEST: i32 = 3;
pub const EXIT_NO_FLAG: i32 = 4;
pub const EXIT_MISSING_FIELD: i32 = 5;
pub const EXIT_TOPOLOGY_MISMATCH: i32 = 6;

pub const VALID_MODES: [&str; 3] = ["managed", "ephemeral", "custom"];

/// The owner's canonical redirect text for a Flag that is not issued.
pub const REDIRECT_TEMPLATE: &str = "HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워, 모델의 정확한 서빙전략을 세우기 어렵습니다. terraforming_node 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다.";

const MODES_TEXT: &str = "managed|ephemeral|custom";

// ── the owner's own source lines that appear in a traceback ──────────────────────────────

/// One owner frame: line, function, the stripped source line and the sub-expression Python marks with carets (`""`: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerFrame {
    pub line: u32,
    pub function: &'static str,
    pub code: &'static str,
    pub expr: &'static str,
}

const fn owner(
    line: u32,
    function: &'static str,
    code: &'static str,
    expr: &'static str,
) -> OwnerFrame {
    OwnerFrame {
        line,
        function,
        code,
        expr,
    }
}

const NEXT_CODE: &str = "_owner = next((w for w in _local_of if w and (tik == w or tik.startswith(w.rstrip(\"/\") + \"/\"))), None)";
const NEXT_EXPR: &str = "next((w for w in _local_of if w and (tik == w or tik.startswith(w.rstrip(\"/\") + \"/\"))), None)";

/// `man.get("topology")` while the result dictionary is built: the first use of a manifest that is not a mapping.
pub const MAN_GET: OwnerFrame = owner(
    132,
    "evaluate_contract",
    "\"topology\": man.get(\"topology\"),",
    "man.get",
);
pub const CALL_MANIFEST_TP: OwnerFrame = owner(
    142,
    "evaluate_contract",
    "res[\"manifest_tp\"] = manifest_tp(man, topology)",
    "manifest_tp(man, topology)",
);
pub const TP_STARTSWITH: OwnerFrame = owner(
    117,
    "manifest_tp",
    "if (topology or \"\").startswith(\"single\"):",
    "(topology or \"\").startswith(\"single\")",
);
pub const TP_INT: OwnerFrame = owner(112, "manifest_tp", "gpus = int(raw)", "int(raw)");
pub const TP_LEN: OwnerFrame = owner(
    121,
    "manifest_tp",
    "return max(1, len(nodes)) * gpus",
    "len(nodes)",
);
pub const TERRA_GET: OwnerFrame = owner(
    149,
    "evaluate_contract",
    "if terra.get(\"complete\") is not True:",
    "terra.get",
);
pub const ENUMERATE_NODES: OwnerFrame = owner(
    191,
    "evaluate_contract",
    "for idx, node in enumerate(res[\"nodes\"]):",
    "enumerate(res[\"nodes\"])",
);
/// The `next(...)` call in `evaluate_contract`; the generator frames below it are `<genexpr>` rows on the same line.
pub const NEXT_CALL: OwnerFrame = owner(228, "evaluate_contract", NEXT_CODE, NEXT_EXPR);
/// `tik.startswith` on a `tiktoken_host_path` that is not text (the attribute does not exist).
pub const GENEXPR_STARTSWITH_ATTR: OwnerFrame =
    owner(228, "<genexpr>", NEXT_CODE, "tik.startswith");
/// `w.rstrip` on a `work_dir` that is not text.
pub const GENEXPR_RSTRIP_ATTR: OwnerFrame = owner(228, "<genexpr>", NEXT_CODE, "w.rstrip");
pub const GENEXPR_RSTRIP_CALL: OwnerFrame = owner(228, "<genexpr>", NEXT_CODE, "w.rstrip(\"/\")");
pub const GENEXPR_STARTSWITH_CALL: OwnerFrame = owner(
    228,
    "<genexpr>",
    NEXT_CODE,
    "tik.startswith(w.rstrip(\"/\") + \"/\")",
);

/// The key expression of the `effective_model_source` dictionary comprehension (Python 3.12 inlines comprehensions, so there is
/// no `<dictcomp>` frame). The marked span of an unhashable key is the key-value text without its first character (the opening parenthesis),
/// as the captured original shows (extra-original X03/X04).
pub const EFFECTIVE_KEY: OwnerFrame = owner(
    237,
    "evaluate_contract",
    "(node.get(\"role\") or \"node%d\" % i): effective_model_source(node, ms)",
    "node.get(\"role\") or \"node%d\" % i): effective_model_source(node, ms)",
);

/// Every row above, for the source-binding check.
pub const OWNER_FRAMES: [&OwnerFrame; 12] = [
    &MAN_GET,
    &CALL_MANIFEST_TP,
    &TP_INT,
    &TP_LEN,
    &TERRA_GET,
    &ENUMERATE_NODES,
    &NEXT_CALL,
    &GENEXPR_STARTSWITH_ATTR,
    &GENEXPR_RSTRIP_ATTR,
    &GENEXPR_RSTRIP_CALL,
    &GENEXPR_STARTSWITH_CALL,
    &EFFECTIVE_KEY,
];

// ── results ─────────────────────────────────────────────────────────────────────────────

/// An exception the owner does not handle: its class, `str(exception)` and the owner frames below `main` (outermost first).
#[derive(Clone, Debug, PartialEq)]
pub struct Raised {
    pub class: &'static str,
    pub message: PyStr,
    pub frames: Vec<OwnerFrame>,
    /// `AttributeError.name`, when the exception carries one.
    pub name: Option<String>,
    /// `AttributeError.obj`: where the object sits in the ARGUMENTS of the call (the in-process importer hands over its own object).
    pub obj: Option<Vec<Step>>,
}

/// One step of the way from a call's arguments to an object they hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Arg(usize),
    Key(String),
    Index(usize),
}

fn at(failure: GateFailure, path: Vec<Step>) -> GateFailure {
    match failure {
        GateFailure::Raised(mut raised) => {
            raised.obj = Some(path);
            GateFailure::Raised(raised)
        }
        other => other,
    }
}

/// Why a gate evaluation produced no answer.
#[derive(Clone, Debug, PartialEq)]
pub enum GateFailure {
    Raised(Raised),
    /// A shape with no captured original: stated, never answered by guessing.
    Unsupported {
        boundary: &'static str,
        detail: String,
    },
}

/// The `res` dictionary of `evaluate_contract` in the owner's key order.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub res: PyDict,
}

impl Evaluation {
    pub fn flag(&self) -> bool {
        matches!(
            self.res.get("flag").map(PyValue::peel),
            Some(PyValue::Bool(true))
        )
    }
    pub fn exit_code(&self) -> i32 {
        match self.res.get("exit_code").map(PyValue::peel) {
            Some(PyValue::Int(number)) => number.to_string().parse().unwrap_or(EXIT_NO_FLAG),
            _ => EXIT_NO_FLAG,
        }
    }
    pub fn reason(&self) -> PyStr {
        match self.res.get("reason").map(PyValue::peel) {
            Some(PyValue::Str(text)) => text.clone(),
            _ => PyStr::new(""),
        }
    }
}

fn raised(class: &'static str, message: String, frames: Vec<OwnerFrame>) -> GateFailure {
    GateFailure::Raised(Raised {
        class,
        message: PyStr::new(&message),
        frames,
        name: None,
        obj: None,
    })
}

fn no_attribute(value: &PyValue, name: &str, frames: Vec<OwnerFrame>) -> GateFailure {
    match raised(
        "AttributeError",
        format!("'{}' object has no attribute '{name}'", value.type_name()),
        frames,
    ) {
        GateFailure::Raised(mut raised) => {
            raised.name = Some(name.to_string());
            GateFailure::Raised(raised)
        }
        other => other,
    }
}

/// Builds a Python string out of literal pieces and arbitrary text (lone surrogates survive).
struct Cat(PyStr);

impl Cat {
    fn new() -> Self {
        Cat(PyStr::new(""))
    }
    fn lit(mut self, text: &str) -> Self {
        self.0.push_str(text);
        self
    }
    fn py(mut self, text: &PyStr) -> Self {
        self.0.push_py(text);
        self
    }
    fn done(self) -> PyStr {
        self.0
    }
}

// ── small Python helpers over values ────────────────────────────────────────────────────

fn text_is(value: Option<&PyValue>, expected: &str) -> bool {
    matches!(value.map(PyValue::peel), Some(PyValue::Str(text)) if text.equals(expected))
}

fn truthy(value: Option<&PyValue>) -> bool {
    value.is_some_and(PyValue::is_truthy)
}

/// Python `str(value)`: text is itself; a `datetime.date` is its ISO date; a `datetime.datetime` is `isoformat(" ")`; every other
/// value is its `repr`.
pub fn py_str(value: &PyValue) -> PyStr {
    match value.peel() {
        PyValue::Opaque(opaque) => match &opaque.typed {
            Typed::Date(iso) => PyStr::new(iso),
            Typed::DateTime(iso) => PyStr::new(&iso.replacen('T', " ", 1)),
            _ => value.display_text(),
        },
        _ => value.display_text(),
    }
}

/// Whether Python refuses the value as a dictionary key (`TypeError: unhashable type`).
fn unhashable(value: &PyValue) -> bool {
    match value.peel() {
        PyValue::List(_) | PyValue::Dict(_) => true,
        PyValue::Opaque(opaque) => matches!(opaque.typed, Typed::Set(_)),
        _ => false,
    }
}

/// `man.get(key) or default`.
fn get_or(map: &PyDict, key: &str, default: PyValue) -> PyValue {
    match map.get(key) {
        Some(value) if value.is_truthy() => value.clone(),
        _ => default,
    }
}

fn as_dict(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(map) => Some(map),
        _ => None,
    }
}

/// `len(value)` for the sized types a YAML document can build; `None` for a type that has no length.
pub fn py_len(value: &PyValue) -> Option<usize> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Some(items.len()),
        PyValue::Str(text) => Some(text.len()),
        PyValue::Dict(map) => Some(map.len()),
        PyValue::Bytes(bytes) => Some(bytes.len()),
        PyValue::Opaque(opaque) => match &opaque.typed {
            Typed::Set(members) => Some(members.len()),
            _ => None,
        },
        _ => None,
    }
}

/// What `enumerate(value)` visits that can be a dictionary: the dictionary items of a list or tuple, with their index (strings,
/// dictionaries, byte strings and sets yield no dictionary member). `None` for a value that is not iterable.
fn dict_members(value: &PyValue) -> Option<Vec<(usize, &PyDict)>> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Some(
            items
                .iter()
                .enumerate()
                .filter_map(|(at, item)| as_dict(item).map(|map| (at, map)))
                .collect(),
        ),
        PyValue::Str(_) | PyValue::Dict(_) | PyValue::Bytes(_) => Some(Vec::new()),
        PyValue::Opaque(opaque) if matches!(opaque.typed, Typed::Set(_)) => Some(Vec::new()),
        _ => None,
    }
}

fn mode_valid(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Str(text) if VALID_MODES.iter().any(|mode| text.equals(mode)))
}

// ── branch helpers ──────────────────────────────────────────────────────────────────────

/// `_git_branch(repo)`: the stripped branch name of the checkout, `None` for every failure.
pub trait BranchProbe {
    fn branch(&mut self, repo: &[u8]) -> Option<PyStr>;
}

/// `resolve_topology(repo, explicit)`: the explicit topology wins; otherwise the branch picks the lane.
pub fn resolve_topology(
    probe: &mut dyn BranchProbe,
    repo: &[u8],
    explicit: Option<&str>,
) -> Option<String> {
    if let Some(explicit) = explicit.filter(|text| !text.is_empty()) {
        return Some(explicit.to_string());
    }
    let branch = probe.branch(repo)?;
    if branch.equals("single-node") {
        Some("single".to_string())
    } else if branch.equals("multi-node") {
        Some("multi".to_string())
    } else {
        None
    }
}

/// `branch_topology_conflict(repo, explicit)`: the reason when an explicit topology disagrees with the branch's lane.
pub fn branch_topology_conflict(
    probe: &mut dyn BranchProbe,
    repo: &[u8],
    explicit: Option<&str>,
) -> Option<PyStr> {
    let explicit = explicit.filter(|text| !text.is_empty())?;
    let derived = resolve_topology(probe, repo, None)?;
    if derived == explicit {
        return None;
    }
    Some(PyStr::new(&format!(
        "요청 topology={} 이 체크아웃 브랜치가 고르는 통로 {} 와 다르다 — 브랜치를 바꾸거나 요청을 바꿔라(자동 교정하지 않는다). 정본 술어: topology_parity.py",
        PyStr::new(explicit).repr(),
        PyStr::new(&derived).repr()
    )))
}

// ── effective_model_source / manifest_tp ────────────────────────────────────────────────

/// `effective_model_source(node, top_level_ms)`: the node's own source when it has a truthy one, else the top-level value.
pub fn effective_model_source(node: &PyValue, top_level: &PyValue) -> PyValue {
    if let Some(map) = as_dict(node) {
        if let Some(own) = map.get("model_source").filter(|value| value.is_truthy()) {
            return own.clone();
        }
    }
    top_level.clone()
}

/// Outcome of `manifest_tp`: the TP (a number or `None`), or the owner's `ValueError` text.
enum Tp {
    Value(PyValue),
    ValueError(String),
}

fn manifest_tp(man: &PyDict, topology: &PyValue) -> Result<Tp, GateFailure> {
    let raw = man.get("gpus_per_node").cloned().unwrap_or(PyValue::None);
    let gpus = match int_call(raw.peel()) {
        Ok(number) => Some(PyValue::Int(number)),
        Err(error) if error.is_type_or_value_error() => None,
        Err(error) => {
            return Err(raised(
                error.class,
                error.message,
                vec![CALL_MANIFEST_TP, TP_INT],
            ))
        }
    };
    let gpus = match gpus {
        Some(number) if !compare(Compare::Less, &number, &PyValue::int(1)).unwrap_or(false) => {
            number
        }
        _ => {
            return Ok(Tp::ValueError(format!(
                "gpus_per_node 부재/비정수({}) — TP 는 HW 사실이지 기본값이 아니다",
                raw.repr()
            )))
        }
    };
    // `(topology or "").startswith("single")`: the topology is whatever the caller passed
    let single = if !topology.is_truthy() {
        false
    } else {
        match topology.peel() {
            PyValue::Str(text) => text
                .code_points()
                .starts_with(&"single".chars().map(u32::from).collect::<Vec<u32>>()),
            PyValue::Bytes(_) => {
                return Err(raised(
                    "TypeError",
                    "startswith first arg must be bytes or a tuple of bytes, not str".to_string(),
                    vec![CALL_MANIFEST_TP, TP_STARTSWITH],
                ))
            }
            other => {
                return Err(at(
                    no_attribute(other, "startswith", vec![CALL_MANIFEST_TP, TP_STARTSWITH]),
                    vec![Step::Arg(1)],
                ))
            }
        }
    };
    if single {
        return Ok(Tp::Value(gpus));
    }
    let nodes = get_or(man, "nodes", PyValue::List(Vec::new()));
    if !nodes.is_truthy() {
        return Ok(Tp::Value(PyValue::None));
    }
    let Some(count) = py_len(&nodes) else {
        return Err(raised(
            "TypeError",
            format!("object of type '{}' has no len()", nodes.type_name()),
            vec![CALL_MANIFEST_TP, TP_LEN],
        ));
    };
    let factor = PyValue::int(i64::try_from(count.max(1)).unwrap_or(i64::MAX));
    multiply(&factor, &gpus)
        .map(Tp::Value)
        .map_err(|error| raised(error.class, error.message, vec![CALL_MANIFEST_TP, TP_LEN]))
}

/// `manifest_tp(man, topology)` for a caller that only needs the number: `Ok(Ok(tp))` (a number or `None`), `Ok(Err(text))` for the
/// owner's `ValueError`, `Err` for an exception the owner does not handle.
pub fn manifest_tp_value(
    man: &PyDict,
    topology: &str,
) -> Result<Result<PyValue, String>, GateFailure> {
    manifest_tp(man, &PyValue::str(topology)).map(|tp| match tp {
        Tp::Value(value) => Ok(value),
        Tp::ValueError(text) => Err(text),
    })
}

// ── evaluate_contract ───────────────────────────────────────────────────────────────────

fn finish(mut res: PyDict, reason: &str, code: i32) -> Result<Evaluation, GateFailure> {
    res.set("reason", PyValue::str(reason));
    res.set("exit_code", PyValue::int(code));
    Ok(Evaluation { res })
}

/// `evaluate_contract(man, topology)`: the pure Flag gate. `man` is whatever `yaml.safe_load(...) or {}` produced.
pub fn evaluate_contract(man: &PyValue, topology: &str) -> Result<Evaluation, GateFailure> {
    evaluate_contract_value(man, &PyValue::str(topology))
}

/// `evaluate_contract(man, topology)` for a caller whose `topology` is any value (the importers pass `man.get("topology")`).
pub fn evaluate_contract_value(
    man: &PyValue,
    topology: &PyValue,
) -> Result<Evaluation, GateFailure> {
    let Some(man) = as_dict(man) else {
        return Err(at(
            no_attribute(man.peel(), "get", vec![MAN_GET]),
            vec![Step::Arg(0)],
        ));
    };
    let mut res = PyDict::new();
    res.set("flag", PyValue::Bool(false));
    res.set("reason", PyValue::str(""));
    res.set("exit_code", PyValue::int(EXIT_NO_FLAG));
    let raw = |key: &str| man.get(key).cloned().unwrap_or(PyValue::None);
    res.set("topology", raw("topology"));
    res.set("gpus_per_node", raw("gpus_per_node"));
    res.set("nodes", get_or(man, "nodes", PyValue::List(Vec::new())));
    res.set("model_source", raw("model_source"));
    res.set("nas_model_path", raw("nas_model_path"));
    res.set(
        "custom_model_paths",
        get_or(man, "custom_model_paths", PyValue::Dict(PyDict::new())),
    );
    res.set(
        "quant_model_path",
        get_or(man, "quant_model_path", PyValue::str("")),
    );
    res.set(
        "tiktoken_host_path",
        get_or(man, "tiktoken_host_path", PyValue::str("")),
    );

    let tp_error = match manifest_tp(man, topology)? {
        Tp::Value(value) => {
            res.set("manifest_tp", value);
            None
        }
        Tp::ValueError(text) => {
            res.set("manifest_tp", PyValue::None);
            Some(text)
        }
    };

    let terra_value = get_or(man, "terraforming", PyValue::Dict(PyDict::new()));
    let Some(terra) = as_dict(&terra_value) else {
        return Err(at(
            no_attribute(terra_value.peel(), "get", vec![TERRA_GET]),
            vec![Step::Arg(0), Step::Key("terraforming".to_string())],
        ));
    };
    if !matches!(
        terra.get("complete").map(PyValue::peel),
        Some(PyValue::Bool(true))
    ) {
        return finish(
            res,
            "terraforming.complete != true (Flag 미발급 — info-only)",
            EXIT_NO_FLAG,
        );
    }
    if !matches!(
        terra.get("branch_verified").map(PyValue::peel),
        Some(PyValue::Bool(true))
    ) {
        return finish(
            res,
            "terraforming.branch_verified != true (§1.5 3자일치 미통과 — info-only)",
            EXIT_NO_FLAG,
        );
    }
    if text_is(man.get("self_role"), "sub") && !text_is(terra.get("issued_by"), "main") {
        return finish(
            res,
            "self_role=sub 이지만 terraforming.issued_by != 'main' — 메인이 발급한 서브 manifest가 아니므로 readiness를 인정하지 않는다",
            EXIT_NO_FLAG,
        );
    }

    let missing: Vec<&str> = ["topology", "gpus_per_node"]
        .into_iter()
        .filter(|key| !truthy(man.get(key)))
        .collect();
    if !missing.is_empty() {
        let reason = format!("Flag true 이나 필수 HW필드 누락: {}", missing.join(", "));
        return finish(res, &reason, EXIT_MISSING_FIELD);
    }
    if man
        .get("topology")
        .is_none_or(|declared| declared != topology)
    {
        let declared = man
            .get("topology")
            .map_or_else(|| "None".to_string(), PyValue::repr);
        let reason = format!(
            "manifest 선언 topology={} 과 요청 topology={} 이 다르다 — manifest 가 권위다",
            declared,
            topology.repr()
        );
        return finish(res, &reason, EXIT_TOPOLOGY_MISMATCH);
    }
    if let Some(text) = tp_error.filter(|text| !text.is_empty()) {
        return finish(res, &format!("Flag true 이나 {text}"), EXIT_MISSING_FIELD);
    }

    let ms = raw("model_source");
    if !mode_valid(&ms) {
        let reason = format!(
            "model_source 미설정/오류: {} (기대 {MODES_TEXT})",
            ms.repr()
        );
        return finish(res, &reason, EXIT_MISSING_FIELD);
    }

    let nodes = res.get("nodes").cloned().unwrap_or(PyValue::None);
    let Some(members) = dict_members(&nodes) else {
        return Err(raised(
            "TypeError",
            format!("'{}' object is not iterable", nodes.type_name()),
            vec![ENUMERATE_NODES],
        ));
    };
    let mut warnings: Vec<PyValue> = Vec::new();
    let mut bad: Vec<PyStr> = Vec::new();
    for (idx, node) in &members {
        let role = node.get("role").map_or_else(|| PyStr::new("?"), py_str);
        let Some(node_ms) = node
            .get("model_source")
            .filter(|value| !matches!(value.peel(), PyValue::None))
        else {
            continue;
        };
        if !mode_valid(node_ms) {
            bad.push(
                Cat::new()
                    .lit(&format!("nodes[{idx}]("))
                    .py(&role)
                    .lit(&format!(").model_source={}", node_ms.repr()))
                    .done(),
            );
        } else if text_is(Some(topology), "multi") {
            warnings.push(PyValue::Str(
                Cat::new()
                    .lit(&format!("nodes[{idx}]("))
                    .py(&role)
                    .lit(").model_source override 는 single-node sub-control 한정 — multi 는 무시(단일 정책)")
                    .done(),
            ));
        }
    }
    if !bad.is_empty() {
        let mut joined = Cat::new().lit("per-node model_source 오류: ");
        for (at, entry) in bad.iter().enumerate() {
            if at > 0 {
                joined = joined.lit("; ");
            }
            joined = joined.py(entry);
        }
        let reason = joined.lit(&format!(" (기대 {MODES_TEXT})")).done();
        res.set("reason", PyValue::Str(reason));
        res.set("exit_code", PyValue::int(EXIT_MISSING_FIELD));
        return Ok(Evaluation { res });
    }

    let tik = res
        .get("tiktoken_host_path")
        .cloned()
        .unwrap_or(PyValue::None);
    if tik.is_truthy() && py_len(&nodes).is_some_and(|count| count > 1) {
        if let Some(found) = first_tiktoken_owner(&tik, &members)? {
            warnings.push(PyValue::Str(
                Cat::new()
                    .lit("tiktoken_host_path=")
                    .lit(&tik.repr())
                    .lit(" 가 노드 작업경로(")
                    .py(&py_str(&found))
                    .lit(") 안이다 — 사전적재 자산은 노드-로컬에 두면 다른 노드가 얻을 수 없다(서브는 curl/wget/hf deny). 공유 스토리지(nas_model_path 와 같은 마운트)로 옮기고 이 포인터를 그리로 돌려라.")
                    .done(),
            ));
        }
    }

    res.set("node_warnings", PyValue::List(warnings));
    res.set("effective_model_source", effective_sources(&nodes, &ms)?);
    res.set("flag", PyValue::Bool(true));
    finish(
        res,
        "Flag valid (complete + branch_verified + HW사실 + model_source)",
        EXIT_OK,
    )
}

/// `next((w for w in _local_of if w and (tik == w or tik.startswith(w.rstrip("/") + "/"))), None)` over the `work_dir` values of
/// the dictionary nodes, including the exceptions the generator raises for values that are not text.
fn first_tiktoken_owner(
    tik: &PyValue,
    members: &[(usize, &PyDict)],
) -> Result<Option<PyValue>, GateFailure> {
    let in_generator = |frame: OwnerFrame| vec![NEXT_CALL, frame];
    for (position, node) in members {
        let work_dir = get_or(node, "work_dir", PyValue::str(""));
        if !work_dir.is_truthy() {
            continue;
        }
        if *tik == work_dir {
            return Ok(Some(work_dir));
        }
        // `tik.startswith` is looked up first, then its argument `w.rstrip("/") + "/"` is evaluated, then the call is made.
        if !matches!(tik.peel(), PyValue::Str(_) | PyValue::Bytes(_)) {
            return Err(at(
                no_attribute(
                    tik.peel(),
                    "startswith",
                    in_generator(GENEXPR_STARTSWITH_ATTR),
                ),
                vec![Step::Arg(0), Step::Key("tiktoken_host_path".to_string())],
            ));
        }
        let prefix = match work_dir.peel() {
            PyValue::Str(text) => {
                let mut points = text.code_points().to_vec();
                while points.last() == Some(&u32::from(b'/')) {
                    points.pop();
                }
                points.push(u32::from(b'/'));
                points
            }
            PyValue::Bytes(_) => {
                return Err(raised(
                    "TypeError",
                    "a bytes-like object is required, not 'str'".to_string(),
                    in_generator(GENEXPR_RSTRIP_CALL),
                ))
            }
            other => {
                return Err(at(
                    no_attribute(other, "rstrip", in_generator(GENEXPR_RSTRIP_ATTR)),
                    vec![
                        Step::Arg(0),
                        Step::Key("nodes".to_string()),
                        Step::Index(*position),
                        Step::Key("work_dir".to_string()),
                    ],
                ))
            }
        };
        match tik.peel() {
            PyValue::Str(text) => {
                if text.code_points().starts_with(&prefix) {
                    return Ok(Some(work_dir));
                }
            }
            _ => {
                return Err(raised(
                    "TypeError",
                    "startswith first arg must be bytes or a tuple of bytes, not str".to_string(),
                    in_generator(GENEXPR_STARTSWITH_CALL),
                ))
            }
        }
    }
    Ok(None)
}

/// Which Python object a NaN role is, for the dictionary key identity (`nan != nan`: only the same object is the same key). A
/// loader that makes NaNs objects (`load_yaml_objects`) hands out one shared `.nan`, one object per other NaN node and the
/// same object for an alias: the allocation is the identity. A plain float is an object nobody else refers to.
fn nan_origin(role: &PyValue, index: usize) -> usize {
    match role {
        PyValue::Shared(object) => Arc::as_ptr(object) as *const () as usize,
        _ => usize::MAX - index,
    }
}

/// `{(node.get("role") or "node%d" % i): effective_model_source(node, ms) for i, node in enumerate(nodes) if isinstance(node, dict)}`.
///
/// A key is whatever Python hashes: text, numbers (`1`, `1.0` and `True` are one key), `None`-free (a falsy role falls back to
/// `node<i>`), dates and byte strings (valid keys; only a later `json.dumps` refuses them); a list, dict or set is
/// `TypeError: unhashable type`. A repeated key keeps its first position and the first key object and takes the last value.
fn effective_sources(nodes: &PyValue, ms: &PyValue) -> Result<PyValue, GateFailure> {
    let mut out = PyDict::new();
    for (index, node) in dict_members(nodes).unwrap_or_default() {
        let key = match node.get("role").filter(|value| value.is_truthy()) {
            None => DictKey::Str(PyStr::new(&format!("node{index}"))),
            Some(role) if unhashable(role) => {
                return Err(raised(
                    "TypeError",
                    format!("unhashable type: '{}'", role.type_name()),
                    vec![EFFECTIVE_KEY],
                ))
            }
            Some(role) => dict_key(role, nan_origin(role, index)),
        };
        out.insert_key(
            key,
            effective_model_source(&PyValue::Dict(node.clone()), ms),
        );
    }
    Ok(PyValue::Dict(out))
}

#[path = "manifest_gate_api.rs"]
pub mod api;
