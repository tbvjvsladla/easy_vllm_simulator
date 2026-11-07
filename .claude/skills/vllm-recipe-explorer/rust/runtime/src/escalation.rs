//! `escalation_predicate.py`: the deterministic judge of escalation predicate one ("the model cannot be served").
//!
//! The judgment is one sentence enforced in code: **a host-plane sign never makes predicate one**. A watchdog or thermal kill, an
//! earlyoom kill or an OOM kill says "this host cannot hold that much resident memory now", not "this vLLM cannot take this model";
//! an executed kill inside the window also refutes the model-plane symptom "dies at serve init" (the death was a kill). The kill
//! vocabulary is owned by the classifier (`KILL_EVENT_KINDS`), reached through the [`Dependency`]; it is never copied here.
//!
//! The order is the original's: arguments are bound, `list(symptoms or [])`, `list(kill_hits or [])`, the classifier is loaded, the
//! kill hits are filtered by kind, then collisions, unknown, host, model, refuted, standing, reasons and the verdict. A bad argument
//! fails before the dependency is touched; a bad kill hit only after it. Every value is handled as Python handles it (truthiness,
//! `dict`/`list`/`str` membership, `%s` of any value, the exceptions of `.get` on a non-dict and of hashing a container).

use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{as_get_target, ensure_hashable, equals, iterate};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;

/// The module name of the original and of its classifier dependency, as exceptions and traces name them.
pub const MODULE: &str = "escalation_predicate";
pub const CLASSIFIER_MODULE: &str = "_classify_cell";

/// `MODEL_PLANE_SYMPTOMS`: the symptoms that can make predicate one (a closed list: a new symptom is an edit and a review).
pub const MODEL_PLANE_SYMPTOMS: [(&str, &str); 3] = [
    (
        "config_arch_unsupported",
        "config 의 architectures/quant_method 를 이 vLLM 이 모른다",
    ),
    (
        "serve_init_immediate_death",
        "serve init 즉사 — 가중치 로드 이전에 죽는다",
    ),
    (
        "transformers_only_fallback",
        "transformers-only 폴백 신호(네이티브 경로 부재)",
    ),
];

/// `HOST_PLANE_SIGNS`: the signs that cannot make predicate one (host resource plane, independent of model and version).
pub const HOST_PLANE_SIGNS: [(&str, &str); 7] = [
    ("watchdog_kill_ack", "호스트 메모리 워치독이 집행한 사살"),
    ("watchdog_trip", "호스트 메모리 워치독의 트립 판정"),
    ("thermal_kill_ack", "SoC 열 임계 사살"),
    ("thermal_trip", "SoC 열 임계 트립 판정"),
    ("earlyoom_kill", "earlyoom 사살"),
    ("docker_kill", "호스트 감시자가 수행한 컨테이너 kill"),
    ("oom_killer", "커널 OOM killer"),
];

/// `REFUTED_BY_KILL`: the model-plane symptoms an executed kill refutes.
pub const REFUTED_BY_KILL: [&str; 1] = ["serve_init_immediate_death"];

/// The three module globals the judgment reads, as the caller holds them (a caller may mutate them; the judgment reads the
/// values given to each call).
#[derive(Clone, Debug)]
pub struct Vocabulary {
    pub model: PyDict,
    pub host: PyDict,
    pub refuted: Vec<PyValue>,
}

impl Vocabulary {
    /// The globals as the module defines them.
    pub fn canonical() -> Vocabulary {
        let table = |rows: &[(&str, &str)]| {
            let mut map = PyDict::new();
            for (name, why) in rows {
                map.set(name, PyValue::str(why));
            }
            map
        };
        Vocabulary {
            model: table(&MODEL_PLANE_SYMPTOMS),
            host: table(&HOST_PLANE_SIGNS),
            refuted: REFUTED_BY_KILL
                .iter()
                .map(|name| PyValue::str(name))
                .collect(),
        }
    }

    /// The globals as one JSON object (`{"MODEL_PLANE_SYMPTOMS": {...}, "HOST_PLANE_SIGNS": {...}, "REFUTED_BY_KILL": [...]}`).
    pub fn to_value(&self) -> PyValue {
        let mut all = PyDict::new();
        all.set("MODEL_PLANE_SYMPTOMS", PyValue::Dict(self.model.clone()));
        all.set("HOST_PLANE_SIGNS", PyValue::Dict(self.host.clone()));
        all.set("REFUTED_BY_KILL", PyValue::List(self.refuted.clone()));
        PyValue::Dict(all)
    }

    /// Replaces the globals the object names (`MODEL_PLANE_SYMPTOMS`/`HOST_PLANE_SIGNS`: objects, `REFUTED_BY_KILL`: array); a member
    /// that is not one of the three, or of the wrong shape, is the error text.
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        for (name, value) in members.iter() {
            match (name.to_utf8().as_deref(), value) {
                (Some("MODEL_PLANE_SYMPTOMS"), PyValue::Dict(map)) => self.model = map.clone(),
                (Some("HOST_PLANE_SIGNS"), PyValue::Dict(map)) => self.host = map.clone(),
                (Some("REFUTED_BY_KILL"), PyValue::List(items)) => self.refuted = items.clone(),
                _ => return Err(format!("globals member {} is not usable", name.repr())),
            }
        }
        Ok(())
    }
}

// ── exceptions with a chain ──────────────────────────────────────────────────────────────────────

/// An exception as the caller sees it: its class, module and message, the exception it was raised from (`raise ... from`), and the
/// `code` of a `SystemExit`.
#[derive(Clone, Debug)]
pub struct Raised {
    pub class: String,
    pub module: String,
    pub message: PyStr,
    pub cause: Option<Box<Raised>>,
    /// The link to `cause` is the implicit context (`__context__`), not an explicit `raise ... from`.
    pub via_context: bool,
    pub suppress_context: bool,
    pub code: Option<Box<PyValue>>,
}

impl From<Failure> for Raised {
    fn from(failure: Failure) -> Raised {
        let module = match failure.class {
            "JSONDecodeError" => "json.decoder",
            _ => "builtins",
        };
        Raised::new(failure.class, module, failure.message)
    }
}

impl Raised {
    pub fn new(class: &str, module: &str, message: PyStr) -> Raised {
        Raised {
            class: class.to_string(),
            module: module.to_string(),
            message,
            cause: None,
            via_context: false,
            suppress_context: false,
            code: None,
        }
    }

    pub fn plain(class: &str, message: &str) -> Raised {
        Raised::new(class, "builtins", PyStr::new(message))
    }

    /// `SystemExit(message)`: its code is the message.
    pub fn system_exit(message: PyStr) -> Raised {
        let mut raised = Raised::new("SystemExit", "builtins", message.clone());
        raised.code = Some(Box::new(PyValue::Str(message)));
        raised
    }

    /// `raise self from cause`.
    pub fn from_cause(mut self, cause: Raised) -> Raised {
        self.cause = Some(Box::new(cause));
        self.suppress_context = true;
        self
    }

    /// `exception_chain`: this exception first, then each cause.
    pub fn chain_value(&self) -> PyValue {
        let mut rows = Vec::new();
        let mut current = Some(self);
        while let Some(link) = current {
            let mut row = PyDict::new();
            row.set("type", PyValue::str(&link.class));
            row.set("module", PyValue::str(&link.module));
            row.set("message", PyValue::Str(link.message.clone()));
            row.set("suppress_context", PyValue::Bool(link.suppress_context));
            row.set(
                "relation",
                if link.cause.is_some() {
                    PyValue::str(if link.via_context { "context" } else { "cause" })
                } else {
                    PyValue::None
                },
            );
            if let Some(code) = &link.code {
                row.set("code", (**code).clone());
            }
            rows.push(PyValue::Dict(row));
            current = link.cause.as_deref();
        }
        PyValue::List(rows)
    }

    /// The exception a chain encodes, decoded strictly against the encoding [`Raised::chain_value`] writes: a non-empty list of objects,
    /// top exception first, each with exactly `type`, `module`, `message` (text), `suppress_context` (a bool), `relation` (`null`,
    /// `"cause"` or `"context"`), and `code` if and only if `type` is `SystemExit`. A row has a relation exactly when another row follows
    /// it; `"cause"` is an explicit `__cause__` (with either `suppress_context`: Python sets it on assignment and the caller may set it
    /// back) and `"context"` the implicit one (`suppress_context` false); a last row may have either (`raise ... from None`). Anything else (an empty or
    /// non-list value, a row that is not an object, a missing, extra or wrongly typed member, an unknown relation, an inconsistent link)
    /// is `None`: a caller's acknowledgement is never coerced into an ordinary exception.
    pub fn from_chain(rows: &PyValue) -> Option<Raised> {
        let PyValue::List(rows) = rows else {
            return None;
        };
        if rows.is_empty() {
            return None;
        }
        let mut chain: Option<Raised> = None;
        for (position, row) in rows.iter().enumerate().rev() {
            let PyValue::Dict(map) = row else {
                return None;
            };
            let text = |key: &str| match map.get(key) {
                Some(PyValue::Str(text)) => Some(text.clone()),
                _ => None,
            };
            let class = text("type")?.to_utf8()?;
            let has_code = class == "SystemExit";
            if map.len() != 5 + usize::from(has_code) || has_code != map.contains_key("code") {
                return None;
            }
            let suppress_context = match map.get("suppress_context") {
                Some(PyValue::Bool(flag)) => *flag,
                _ => return None,
            };
            let is_last = position + 1 == rows.len();
            let via_context = match (map.get("relation")?, is_last) {
                (PyValue::None, true) => false,
                // An explicit `__cause__` is legal with either flag: Python sets `__suppress_context__` when it is assigned, and the
                // caller may set it back; the collector reports `cause` whenever `__cause__` is set.
                (PyValue::Str(relation), false) if relation.equals("cause") => false,
                (PyValue::Str(relation), false)
                    if relation.equals("context") && !suppress_context =>
                {
                    true
                }
                _ => return None,
            };
            let mut raised = Raised::new(&class, &text("module")?.to_utf8()?, text("message")?);
            raised.suppress_context = suppress_context;
            raised.via_context = via_context;
            raised.code = map.get("code").cloned().map(Box::new);
            raised.cause = chain.take().map(Box::new);
            chain = Some(raised);
        }
        chain
    }

    /// `Class: message` as an uncaught report names the class (a class of another module is qualified by it).
    fn final_line(&self) -> PyStr {
        let name = if self.module == "builtins" || self.module == "__main__" {
            self.class.clone()
        } else {
            format!("{}.{}", self.module, self.class)
        };
        let mut line = PyStr::new(&name);
        if !self.message.is_empty() {
            line.push_str(": ");
            line.push_py(&self.message);
        }
        line
    }

    /// The report of an uncaught exception without the traceback frames: each exception's final `Class: message` block, a cause
    /// before the exception raised from it, with the interpreter's separators.
    pub fn report(&self) -> PyStr {
        let mut blocks = Vec::new();
        let mut current = Some(self);
        while let Some(link) = current {
            blocks.push(link);
            current = link.cause.as_deref();
        }
        let mut text = PyStr::default();
        for (position, link) in blocks.iter().rev().enumerate() {
            if position > 0 {
                // The link joining this block to the one before it is the relation of the block raised from it (top first).
                let joined = blocks[blocks.len() - 1 - position];
                text.push_str(if joined.via_context {
                    "\nDuring handling of the above exception, another exception occurred:\n\n"
                } else {
                    "\nThe above exception was the direct cause of the following exception:\n\n"
                });
            }
            text.push_py(&link.final_line());
            text.push_str("\n");
        }
        text
    }
}

// ── the dependency ───────────────────────────────────────────────────────────────────────────────

/// What `_load_classifier()` returned that the judgment uses: the `KILL_EVENT_KINDS` the module holds (`None` when it holds none:
/// reading it is an `AttributeError`, raised only where the original reads it).
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub kill_event_kinds: Option<Vec<PyValue>>,
}

/// The classifier module as the escalation owner reaches it: its load, and the two functions the command line calls.
pub trait Dependency {
    /// `_load_classifier()`: a fresh load every call.
    fn load(&mut self) -> Result<Loaded, Raised>;
    /// `classify._utc(value, field)`: the owner's UTC wire object `{"datetime_utc": ISO}`.
    fn utc(&mut self, value: &PyStr, field: &str) -> Result<PyValue, Raised>;
    /// `classify.kill_events_in_window(lines, started, ended, tolerance_s)`: the hits (a JSON array).
    fn kill_events(
        &mut self,
        lines: &[PyStr],
        started: &PyValue,
        ended: &PyValue,
        tolerance_s: &BigInt,
    ) -> Result<PyValue, Raised>;
    /// What the dependency wrote to standard error since the last call (a transport record's own `stderr`).
    fn take_stderr(&mut self) -> Vec<PyStr> {
        Vec::new()
    }
    /// Records an ordered observation of the caller (a check's control record); nothing in a production run.
    fn record_event(&mut self, _name: &str, _fields: Vec<(&str, PyValue)>) {}
}

// ── judge ────────────────────────────────────────────────────────────────────────────────────────

/// `list(value or [])`.
fn listed(value: &PyValue) -> Res<Vec<PyValue>> {
    if value.is_truthy() {
        iterate(value)
    } else {
        Ok(Vec::new())
    }
}

/// `key in dict` for a dictionary whose keys are strings: only an equal string is a member (a container is unhashable).
fn in_names(names: &PyDict, key: &PyValue) -> Res<bool> {
    ensure_hashable(key)?;
    Ok(match key.peel() {
        PyValue::Str(text) => names
            .iter()
            .any(|(name, _)| name.code_points() == text.code_points()),
        _ => false,
    })
}

fn in_values(values: &[PyValue], item: &PyValue) -> bool {
    values.iter().any(|value| equals(value, item))
}

fn attribute_error() -> Raised {
    Raised::plain(
        "AttributeError",
        &format!("module '{CLASSIFIER_MODULE}' has no attribute 'KILL_EVENT_KINDS'"),
    )
}

/// `%s` of the description a table holds for a name that is in it.
fn describe(names: &PyDict, key: &PyValue) -> PyStr {
    match key.peel() {
        PyValue::Str(text) => names
            .iter()
            .find(|(name, _)| name.code_points() == text.code_points())
            .map(|(_, value)| value.display_text())
            .unwrap_or_default(),
        _ => PyStr::default(),
    }
}

fn quoted(parts: &[&PyStr]) -> PyStr {
    let mut text = PyStr::default();
    for part in parts {
        text.push_py(part);
    }
    text
}

/// `judge(symptoms, kill_hits, events_scanned="none")`: the verdict dictionary (`predicate_1` and the evidence behind it).
pub fn judge(
    dependency: &mut dyn Dependency,
    vocabulary: &Vocabulary,
    symptoms: &PyValue,
    kill_hits: &PyValue,
    events_scanned: &PyValue,
) -> Result<PyDict, Raised> {
    let symptoms = listed(symptoms)?;
    let kill_hits = listed(kill_hits)?;
    let loaded = dependency.load()?;
    let mut executed: Vec<PyValue> = Vec::new();
    for hit in &kill_hits {
        let kind = as_get_target(hit)?
            .get("kind")
            .cloned()
            .unwrap_or(PyValue::None);
        let kinds = loaded
            .kill_event_kinds
            .as_ref()
            .ok_or_else(attribute_error)?;
        if in_values(kinds, &kind) {
            executed.push(hit.clone());
        }
    }
    let (model, host) = (&vocabulary.model, &vocabulary.host);
    let mut collisions: Vec<PyStr> = model
        .iter()
        .filter(|(name, _)| {
            host.iter()
                .any(|(other, _)| other.code_points() == name.code_points())
        })
        .map(|(name, _)| name.clone())
        .collect();
    collisions.sort_by(|a, b| a.code_points().cmp(b.code_points()));
    let mut unknown = Vec::new();
    for symptom in &symptoms {
        if !in_names(model, symptom)? && !in_names(host, symptom)? {
            unknown.push(symptom.clone());
        }
    }
    let mut host_used = Vec::new();
    let mut model_claimed = Vec::new();
    for symptom in &symptoms {
        if in_names(host, symptom)? {
            host_used.push(symptom.clone());
        }
    }
    for symptom in &symptoms {
        if in_names(model, symptom)? && !in_names(host, symptom)? {
            model_claimed.push(symptom.clone());
        }
    }
    let refuted: Vec<PyValue> = if executed.is_empty() {
        Vec::new()
    } else {
        model_claimed
            .iter()
            .filter(|symptom| in_values(&vocabulary.refuted, symptom))
            .cloned()
            .collect()
    };
    let model_standing: Vec<PyValue> = model_claimed
        .iter()
        .filter(|symptom| !in_values(&refuted, symptom))
        .cloned()
        .collect();

    let mut reasons: Vec<PyValue> = Vec::new();
    for symptom in &host_used {
        let name = symptom.display_text();
        let why = describe(host, symptom);
        reasons.push(PyValue::Str(quoted(&[
            &PyStr::new("호스트 평면 사인 '"),
            &name,
            &PyStr::new("'("),
            &why,
            &PyStr::new(
                ")은 술어 ① 을 만들지 못한다 — 그것은 '이 모델을 이 vLLM 이 못 받는다' 가 아니라 '이 호스트가 지금 이만큼의 상주를 감당하지 못한다' 는 진술이다. 처방은 예산·KV 클램프·PLE 모드이지 버전핀·포크가 아니다.",
            ),
        ])));
    }
    for symptom in &refuted {
        let first = as_get_target(&executed[0])?;
        let field = |name: &str| first.get(name).cloned().unwrap_or(PyValue::None);
        reasons.push(PyValue::Str(quoted(&[
            &PyStr::new("'"),
            &symptom.display_text(),
            &PyStr::new("' 은 구간 안에 집행된 사살("),
            &field("kind").display_text(),
            &PyStr::new(" @ "),
            &field("ts").display_text(),
            &PyStr::new(")이 있어 **반증된다** — 그 죽음은 즉사가 아니라 사살이다."),
        ])));
    }
    for symptom in &unknown {
        reasons.push(PyValue::Str(quoted(&[
            &PyStr::new("'"),
            &symptom.display_text(),
            &PyStr::new(
                "' 은 닫힌 목록 어디에도 없다 — 증상 어휘를 늘리려면 MODEL_PLANE_SYMPTOMS 를 고쳐라(조용히 통과시키지 않는다).",
            ),
        ])));
    }
    for symptom in &model_standing {
        reasons.push(PyValue::Str(quoted(&[
            &PyStr::new("모델 평면 증상 '"),
            &symptom.display_text(),
            &PyStr::new("'("),
            &describe(model, symptom),
            &PyStr::new(") 성립."),
        ])));
    }
    for name in &collisions {
        reasons.push(PyValue::Str(quoted(&[
            &PyStr::new("배선 오류: '"),
            name,
            &PyStr::new(
                "' 이 MODEL_PLANE_SYMPTOMS 와 HOST_PLANE_SIGNS 양쪽에 있다 — 판정에서는 호스트 평면이 이긴다(가드는 목록 위생에 의존하지 않는다). 두 목록 중 한쪽에서 지워라.",
            ),
        ])));
    }
    let ok = !model_standing.is_empty() && unknown.is_empty();
    if !ok && reasons.is_empty() {
        reasons.push(PyValue::str(
            "증상이 하나도 제시되지 않았다 — ① 은 부재로 미충족이다(추측 ✗).",
        ));
    }
    let mut result = PyDict::new();
    result.set("predicate_1", PyValue::Bool(ok));
    result.set(
        "vocabulary_collisions",
        PyValue::List(collisions.into_iter().map(PyValue::Str).collect()),
    );
    result.set("model_plane_standing", PyValue::List(model_standing));
    result.set("model_plane_refuted", PyValue::List(refuted));
    result.set("host_plane_ignored", PyValue::List(host_used));
    result.set("unknown_symptoms", PyValue::List(unknown));
    result.set("kill_events_executed", PyValue::List(executed));
    result.set("evidence_source", events_scanned.clone());
    result.set("reasons", PyValue::List(reasons));
    result.set(
        "note",
        PyValue::str(
            "술어 ① 만 판정한다. escalation 발동은 ① ∧ ② 이며 ② 는 외부 교차검증이다(§5.5).",
        ),
    );
    Ok(result)
}
