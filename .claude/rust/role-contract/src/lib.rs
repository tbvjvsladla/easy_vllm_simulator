//! `node_role_contract.py` as one pure policy library: the topology-axis node contract of the constitution (invariant A). The source's constants, its
//! `ContractViolation` and its ten functions over Python-compatible values ([`PyValue`]): the topology and role arguments may be of any type, exactly
//! as the original accepts them, and every verdict says where it came from (`value`, `source`).
//!
//! **Layering.** This crate depends only on the common `agent-runtime-support` package (typed values, text, YAML, argument binding); it holds the role
//! policy exactly once and no shell code (no argument parser, no console, no JSON-lines protocol, no network): the command line and the typed service
//! live in the `upstream-runtime` package and call this library.
//!
//! **Python behaviour reproduced** (not "fixed"): truthiness of `topology or doc.get("topology")`, `in` over the closed tuples by Python equality
//! (so `False == 0` finds a `role` of `0`), `isinstance(x, list/dict/str)` on the caller's values, `%r` of arbitrary values, the helper quirks of
//! `evaluate_manifest` (the delivery and tool planes are taken from the sub node alone and are not re-validated as a full topology), the early
//! returns and the fail-closed defaults. Source bugs are not corrected and the policy is not widened.
//!
//! **Seams for a later native caller.** [`evaluate_manifest`] takes the manifest as a borrowed [`PyValue`] (the loaded structure is never serialised
//! first, so an unused 617-digit key or an unused recursive alias costs nothing) and returns a typed [`Evaluation`] whose accessors
//! ([`Evaluation::sub_mode`], [`Evaluation::delivery_plane`], ...) can be read without building the dictionary; [`Evaluation::to_value`] builds it in
//! the source's key order.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
pub mod campaign;
pub mod env_file_mode;
pub mod env_secret;
pub mod identity;
pub mod load;
pub mod manifest_gate;
pub mod manifest_gate_cli;
pub mod manifest_self_test;
pub mod pyprofile;
pub mod scan_node;
pub mod scan_node_files;
pub mod scan_node_manifest;
pub mod scan_node_probe;
pub mod scan_node_selftest;
pub mod self_test;
pub mod source_identity;
pub mod topology;
pub mod topology_cli;
pub mod turn_budget;
#[rustfmt::skip]
pub mod topology_doc;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_CONTRACT_VIOLATION: i32 = 5;

pub const TOPOLOGIES: [&str; 2] = ["single", "multi"];
pub const SUB_MODES: [&str; 2] = ["a2a-agent", "ray-worker"];
pub const MAIN_ROLE: &str = "main";
pub const SUB_ROLE: &str = "sub";
pub const HINT_PUBLISHER_SKILL: &str = "hint-publisher";
pub const WIKI_DESK_SKILL: &str = "wiki-desk";
pub const FIELDS: [&str; 6] = [
    "sub_mode",
    "rank",
    "identity_authority",
    "delivery_plane",
    "tool_plane",
    "topology",
];
const DELIVERY_PLANE_SUB_MODES: [&str; 1] = ["ray-worker"];
/// The `evaluate --field` that reads the manifest pair of the two topologies instead of one manifest (plan_26112012 #2b).
pub const SUB_WORKSPACE_SHARED: &str = "sub_workspace_shared";
/// The source of every [`sub_workspace_shared`] verdict.
pub const SUB_WORKSPACE_SHARED_SOURCE: &str = "measured:manifest-pair";
/// The `--field` choices of `evaluate`: the one-manifest [`FIELDS`] of the source, then the manifest-pair field.
pub const EVALUATE_FIELDS: [&str; 7] = [FIELDS[0], FIELDS[1], FIELDS[2], FIELDS[3], FIELDS[4], FIELDS[5], SUB_WORKSPACE_SHARED];

fn sub_mode_by_topology(topology: &str) -> &'static str {
    match topology {
        "single" => "a2a-agent",
        _ => "ray-worker",
    }
}

fn identity_authority_by_topology(topology: &str) -> &'static str {
    match topology {
        "single" => "ssh-endpoint-auth-and-manifest-role",
        _ => "manifest-rank-and-role",
    }
}

fn tool_plane_by_sub_mode(sub_mode: &str) -> &'static [&'static str] {
    match sub_mode {
        "a2a-agent" => &[
            "vllm-recipe-explorer",
            "adversarial-benchmark",
            "upstream-version-watch",
            HINT_PUBLISHER_SKILL,
            WIKI_DESK_SKILL,
        ],
        _ => &[WIKI_DESK_SKILL],
    }
}

/// The module's constants as the Python values the original holds, in the order of its globals (`TUPLE`s are tuples, the maps are dicts).
pub fn globals() -> Vec<(&'static str, PyValue)> {
    let text = |value: &str| PyValue::str(value);
    let texts = |items: &[&str]| PyValue::Tuple(items.iter().map(|item| text(item)).collect());
    let map = |pairs: Vec<(&str, PyValue)>| {
        let mut dict = PyDict::new();
        for (key, value) in pairs {
            dict.set(key, value);
        }
        PyValue::Dict(dict)
    };
    vec![
        ("EXIT_OK", PyValue::int(EXIT_OK)),
        ("EXIT_USAGE", PyValue::int(EXIT_USAGE)),
        (
            "EXIT_CONTRACT_VIOLATION",
            PyValue::int(EXIT_CONTRACT_VIOLATION),
        ),
        ("TOPOLOGIES", texts(&TOPOLOGIES)),
        (
            "SUB_MODE_BY_TOPOLOGY",
            map(TOPOLOGIES
                .iter()
                .map(|t| (*t, text(sub_mode_by_topology(t))))
                .collect()),
        ),
        ("SUB_MODES", texts(&SUB_MODES)),
        (
            "IDENTITY_AUTHORITY_BY_TOPOLOGY",
            map(TOPOLOGIES
                .iter()
                .map(|t| (*t, text(identity_authority_by_topology(t))))
                .collect()),
        ),
        ("DELIVERY_PLANE_SUB_MODES", texts(&DELIVERY_PLANE_SUB_MODES)),
        ("MAIN_ROLE", text(MAIN_ROLE)),
        ("SUB_ROLE", text(SUB_ROLE)),
        ("HINT_PUBLISHER_SKILL", text(HINT_PUBLISHER_SKILL)),
        ("WIKI_DESK_SKILL", text(WIKI_DESK_SKILL)),
        (
            "TOOL_PLANE_BY_SUB_MODE",
            map(SUB_MODES
                .iter()
                .map(|m| (*m, texts(tool_plane_by_sub_mode(m))))
                .collect()),
        ),
        ("FIELDS", texts(&FIELDS)),
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Python-compatible helpers over the caller's values

/// Python `a == b` for the value kinds a manifest and a caller can hold. Two numbers (`bool`, `int`, `float`) are compared EXACTLY by the common
/// package's comparison (`pydyn::equals`, an exact big-integer/float ordering: adjacent integers above 2**53 are different, an integer equals a
/// float only when they are the same mathematical number, a NaN equals nothing); containers compare element by element with this same rule;
/// everything else by the common equality.
pub fn py_eq(left: &PyValue, right: &PyValue) -> bool {
    let (left, right) = (left.peel(), right.peel());
    let numeric = |value: &PyValue| {
        matches!(
            value,
            PyValue::Bool(_) | PyValue::Int(_) | PyValue::Float(_)
        )
    };
    if numeric(left) && numeric(right) {
        return agent_runtime_support::pydyn::equals(left, right);
    }
    match (left, right) {
        (PyValue::List(a), PyValue::List(b)) | (PyValue::Tuple(a), PyValue::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| py_eq(x, y))
        }
        _ => left == right,
    }
}

fn is_str_in(value: &PyValue, vocabulary: &[&str]) -> bool {
    matches!(value.peel(), PyValue::Str(text) if vocabulary.iter().any(|known| text.equals(known)))
}

fn str_of(value: &PyValue) -> Option<&PyStr> {
    match value.peel() {
        PyValue::Str(text) => Some(text),
        _ => None,
    }
}

fn list_repr(items: &[&str]) -> String {
    PyValue::List(items.iter().map(|i| PyValue::str(i)).collect()).repr()
}

/// A `ContractViolation(code, message, field)`; `args == (message,)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractViolation {
    pub code: &'static str,
    pub message: PyStr,
    pub field: Option<&'static str>,
    /// The owner's own call chain to the `raise`, outermost first: each entry is a function of `node_role_contract.py` and the line it was
    /// executing (the call to the next entry, or the `raise` for the last). Source-mapping data of the original, not a native stack.
    pub trace: Vec<Site>,
}

/// A function of the original owner file and the line of it that was executing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub function: &'static str,
    pub line: u32,
}

/// Where a logical frame's file lives: the owner file itself, the `yaml` package, or a frozen standard-library module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRoot {
    Owner,
    Yaml,
    Frozen,
}

/// One entry of the logical source-compatibility traceback: `file` is relative to its root (`node_role_contract.py`, `yaml/parser.py`,
/// `<frozen posixpath>`), `line` the line that frame was executing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFrame {
    pub root: FrameRoot,
    pub file: &'static str,
    pub line: u32,
    pub function: &'static str,
}

/// The file of the owner module the logical frames name.
pub const OWNER_FILE: &str = "node_role_contract.py";

fn owner_frames(trace: &[Site]) -> Vec<SourceFrame> {
    trace
        .iter()
        .map(|site| SourceFrame {
            root: FrameRoot::Owner,
            file: OWNER_FILE,
            line: site.line,
            function: site.function,
        })
        .collect()
}

impl ContractViolation {
    fn new(
        code: &'static str,
        message: String,
        field: Option<&'static str>,
        raised_at: Site,
    ) -> ContractViolation {
        ContractViolation {
            code,
            message: PyStr::new(&message),
            field,
            trace: vec![raised_at],
        }
    }

    /// The violation propagating out of a call made at `line` of `function`.
    fn called_from(mut self, function: &'static str, line: u32) -> ContractViolation {
        self.trace.insert(0, Site { function, line });
        self
    }

    /// The logical traceback of the owner's frames this violation passed through.
    pub fn frames(&self) -> Vec<SourceFrame> {
        owner_frames(&self.trace)
    }

    /// `as_dict()`: `{"code", "message", "field"}`.
    pub fn as_dict(&self) -> PyValue {
        let mut dict = PyDict::new();
        dict.set("code", PyValue::str(self.code));
        dict.set("message", PyValue::Str(self.message.clone()));
        dict.set("field", self.field.map_or(PyValue::None, PyValue::str));
        PyValue::Dict(dict)
    }
}

/// A `{"value": ..., "source": ...}` verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub value: PyValue,
    pub source: PyStr,
}

impl Verdict {
    fn text(value: &str, source: &str) -> Verdict {
        Verdict {
            value: PyValue::str(value),
            source: PyStr::new(source),
        }
    }

    pub fn to_value(&self) -> PyValue {
        let mut dict = PyDict::new();
        dict.set("value", self.value.clone());
        dict.set("source", PyValue::Str(self.source.clone()));
        PyValue::Dict(dict)
    }

    /// The text of `value` when it is a string.
    pub fn value_text(&self) -> Option<String> {
        str_of(&self.value).and_then(PyStr::to_utf8)
    }
}

type Outcome<T> = Result<T, ContractViolation>;

fn topology_text(topology: &PyValue) -> &'static str {
    // only called after `assert_topology` accepted the value, so it is one of the two vocabulary words
    match str_of(topology) {
        Some(text) if text.equals("single") => "single",
        _ => "multi",
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// the ten functions

/// `assert_topology(topology)`: the caller's own object comes back (the service names it as an alias of the argument).
pub fn assert_topology(topology: &PyValue) -> Outcome<()> {
    if is_str_in(topology, &TOPOLOGIES) {
        return Ok(());
    }
    Err(ContractViolation::new(
        "TOPOLOGY_UNKNOWN",
        format!(
            "topology={} 는 선언 어휘 {} 밖이다 — 토폴로지는 인터뷰로만 확정된다(브랜치 추론 ✗ · SKILL.md §0.5.3).",
            topology.repr(),
            list_repr(&TOPOLOGIES)
        ),
        Some("topology"),
        Site {
            function: "assert_topology",
            line: 118,
        },
    ))
}

/// `resolve_sub_mode(topology, declared=None)`.
pub fn resolve_sub_mode(topology: &PyValue, declared: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("resolve_sub_mode", 128))?;
    let derived = sub_mode_by_topology(topology_text(topology));
    let unset = match declared.peel() {
        PyValue::None => true,
        PyValue::Str(text) => text.strip().is_empty(),
        _ => false,
    };
    if unset {
        return Ok(Verdict::text(derived, "derived-from-topology"));
    }
    let Some(text) = str_of(declared) else {
        return Err(ContractViolation::new(
            "SUB_MODE_UNKNOWN",
            format!(
                "nodes[sub].sub_mode 는 문자열이어야 한다(got {}).",
                declared.repr()
            ),
            Some("nodes[sub].sub_mode"),
            Site {
                function: "resolve_sub_mode",
                line: 133,
            },
        ));
    };
    let stripped = text.strip();
    if !SUB_MODES.iter().any(|known| stripped.equals(known)) {
        return Err(ContractViolation::new(
            "SUB_MODE_UNKNOWN",
            format!(
                "nodes[sub].sub_mode={} 는 어휘 {} 밖이다.",
                stripped.repr(),
                list_repr(&SUB_MODES)
            ),
            Some("nodes[sub].sub_mode"),
            Site {
                function: "resolve_sub_mode",
                line: 139,
            },
        ));
    }
    if !stripped.equals(derived) {
        return Err(ContractViolation::new(
            "SUB_MODE_TOPOLOGY_CONFLICT",
            format!(
                "topology={} 는 sub_mode={} 를 함의하는데 선언은 {} 이다 — 불변식 A 위반(한 스킴의 role:sub 로 두 정체성을 덮지 않는다).",
                topology_text(topology),
                derived,
                stripped.repr()
            ),
            Some("nodes[sub].sub_mode"),
            Site {
                function: "resolve_sub_mode",
                line: 144,
            },
        ));
    }
    Ok(Verdict {
        value: PyValue::Str(stripped),
        source: PyStr::new("declared-and-agrees"),
    })
}

fn as_list(value: &PyValue) -> Option<&Vec<PyValue>> {
    match value.peel() {
        PyValue::List(items) => Some(items),
        _ => None,
    }
}

fn node_role(node: &PyValue) -> Option<PyValue> {
    match node.peel() {
        PyValue::Dict(dict) => Some(dict.get("role").cloned().unwrap_or(PyValue::None)),
        _ => None,
    }
}

fn indexes_with_role(nodes: &[PyValue], role: &PyValue) -> Vec<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node_role(node).is_some_and(|r| py_eq(&r, role)))
        .map(|(index, _)| index)
        .collect()
}

/// `resolve_rank(topology, nodes, role)`: the index of the one node with that role (multi only).
pub fn resolve_rank(topology: &PyValue, nodes: &PyValue, role: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("resolve_rank", 162))?;
    if topology_text(topology) == "single" {
        return Ok(Verdict {
            value: PyValue::None,
            source: PyStr::new("not-applicable:single-a2a-agent"),
        });
    }
    let Some(items) = as_list(nodes).filter(|items| !items.is_empty()) else {
        return Err(ContractViolation::new(
            "RANK_UNRESOLVED",
            "topology=multi 인데 nodes[] 가 비었다 — rank 를 해소할 수 없다.".to_string(),
            Some("nodes"),
            Site {
                function: "resolve_rank",
                line: 167,
            },
        ));
    };
    let hits = indexes_with_role(items, role);
    if hits.is_empty() {
        return Err(ContractViolation::new(
            "RANK_UNRESOLVED",
            format!(
                "nodes[] 에 role={} 항목이 없다 — rank 를 해소할 수 없다.",
                role.repr()
            ),
            Some("nodes"),
            Site {
                function: "resolve_rank",
                line: 174,
            },
        ));
    }
    if hits.len() > 1 {
        return Err(ContractViolation::new(
            "RANK_AMBIGUOUS_ROLE",
            format!(
                "nodes[] 에 role={} 이 {}개다 — rank·node_id 가 겹친다. N>2 는 유일 슬러그가 필요하며 어휘 확장은 node_role_contract·scan_node·node_identity 세 곳이 함께 바뀐다.",
                role.repr(),
                hits.len()
            ),
            Some("nodes"),
            Site {
                function: "resolve_rank",
                line: 183,
            },
        ));
    }
    Ok(Verdict {
        value: PyValue::int(hits[0]),
        source: PyStr::new("manifest-nodes-index"),
    })
}

/// `identity_authority(topology)`.
pub fn identity_authority(topology: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("identity_authority", 196))?;
    Ok(Verdict::text(
        identity_authority_by_topology(topology_text(topology)),
        "constitution-invariant-A",
    ))
}

fn unknown_sub_mode(sub_mode: &PyValue, raised_at: Site) -> ContractViolation {
    ContractViolation::new(
        "SUB_MODE_UNKNOWN",
        format!(
            "sub_mode={} 는 어휘 {} 밖이다.",
            sub_mode.repr(),
            list_repr(&SUB_MODES)
        ),
        Some("nodes[sub].sub_mode"),
        raised_at,
    )
}

/// `delivery_plane(topology, sub_mode)`.
pub fn delivery_plane(topology: &PyValue, sub_mode: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("delivery_plane", 208))?;
    if !is_str_in(sub_mode, &SUB_MODES) {
        return Err(unknown_sub_mode(
            sub_mode,
            Site {
                function: "delivery_plane",
                line: 210,
            },
        ));
    }
    let mode = str_of(sub_mode)
        .and_then(PyStr::to_utf8)
        .unwrap_or_default();
    let active = DELIVERY_PLANE_SUB_MODES.contains(&mode.as_str());
    Ok(Verdict::text(
        if active { "active" } else { "dormant" },
        &format!("sub-mode:{mode}"),
    ))
}

/// `tool_plane(topology, sub_mode)`: the runtime skills the sub receives.
pub fn tool_plane(topology: &PyValue, sub_mode: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("tool_plane", 251))?;
    if !is_str_in(sub_mode, &SUB_MODES) {
        return Err(unknown_sub_mode(
            sub_mode,
            Site {
                function: "tool_plane",
                line: 253,
            },
        ));
    }
    let mode = str_of(sub_mode)
        .and_then(PyStr::to_utf8)
        .unwrap_or_default();
    Ok(Verdict {
        value: PyValue::List(
            tool_plane_by_sub_mode(&mode)
                .iter()
                .map(|skill| PyValue::str(skill))
                .collect(),
        ),
        source: PyStr::new(&format!("sub-mode:{mode}")),
    })
}

/// `sub_workspace_shared(single, multi)` over the two loaded topology manifests (`None` = absent or unreadable), plan_26112012 #2b:
/// `shared` when each registers exactly one `role: sub` node and their `node_id` and `work_dir` are both equal (one sub workspace, so
/// one `envs/.env` for both topologies), `distinct` when either differs, `unknown` when a manifest is missing or its sub, `node_id`
/// or `work_dir` is missing, empty, not a string or not one. The host is not compared (single reaches the sub by its management
/// hostname, multi by its RoCE address). The verdict carries no value of the manifests (no path, no name).
pub fn sub_workspace_shared(single: Option<&PyValue>, multi: Option<&PyValue>) -> Verdict {
    fn sub_identity(doc: Option<&PyValue>) -> Option<(Vec<u32>, Vec<u32>)> {
        let nodes = as_list(dict_get(doc?, "nodes")?)?;
        let mut subs = nodes.iter().filter(|node| {
            node_role(node).is_some_and(|r| matches!(r.peel(), PyValue::Str(s) if s.equals(SUB_ROLE)))
        });
        let (Some(sub), None) = (subs.next(), subs.next()) else {
            return None;
        };
        let text = |key: &str| match dict_get(sub, key).map(|v| v.peel()) {
            Some(PyValue::Str(text)) if !text.is_empty() => Some(text.code_points().to_vec()),
            _ => None,
        };
        Some((text("node_id")?, text("work_dir")?))
    }
    let value = match (sub_identity(single), sub_identity(multi)) {
        (Some(one), Some(other)) if one == other => "shared",
        (Some(_), Some(_)) => "distinct",
        _ => "unknown",
    };
    Verdict::text(value, SUB_WORKSPACE_SHARED_SOURCE)
}

/// `sub_self_publishes(topology, sub_mode=None)`: `hint-publisher` is in the tool plane.
pub fn sub_self_publishes(topology: &PyValue, sub_mode: &PyValue) -> Outcome<Verdict> {
    assert_topology(topology).map_err(|e| e.called_from("sub_self_publishes", 267))?;
    let derived;
    let mode = if matches!(sub_mode.peel(), PyValue::None) {
        derived = PyValue::str(sub_mode_by_topology(topology_text(topology)));
        &derived
    } else {
        sub_mode
    };
    let plane = tool_plane(topology, mode).map_err(|e| e.called_from("sub_self_publishes", 269))?;
    let publishes = matches!(plane.value.peel(), PyValue::List(items)
        if items.iter().any(|item| matches!(item.peel(), PyValue::Str(s) if s.equals(HINT_PUBLISHER_SKILL))));
    Ok(Verdict {
        value: PyValue::Bool(publishes),
        source: PyStr::new(&format!(
            "derived:tool_plane({},{})",
            topology_text(topology),
            str_of(mode).and_then(PyStr::to_utf8).unwrap_or_default()
        )),
    })
}

/// The verdicts of [`evaluate_manifest`]; every field is the one the source's result dictionary holds.
#[derive(Clone, Debug)]
pub struct Evaluation {
    topology: PyValue,
    sub_present: bool,
    sub_mode: Option<Verdict>,
    rank: Option<Verdict>,
    identity_authority: Option<Verdict>,
    delivery_plane: Verdict,
    tool_plane: Verdict,
    notes: Vec<PyStr>,
    violations: Vec<ContractViolation>,
}

impl Evaluation {
    pub fn topology(&self) -> &PyValue {
        &self.topology
    }
    pub fn sub_present(&self) -> bool {
        self.sub_present
    }
    pub fn sub_mode(&self) -> Option<&Verdict> {
        self.sub_mode.as_ref()
    }
    pub fn rank(&self) -> Option<&Verdict> {
        self.rank.as_ref()
    }
    pub fn identity_authority(&self) -> Option<&Verdict> {
        self.identity_authority.as_ref()
    }
    pub fn delivery_plane(&self) -> &Verdict {
        &self.delivery_plane
    }
    pub fn tool_plane(&self) -> &Verdict {
        &self.tool_plane
    }
    pub fn notes(&self) -> &[PyStr] {
        &self.notes
    }
    pub fn violations(&self) -> &[ContractViolation] {
        &self.violations
    }

    /// The result dictionary in the source's key order.
    pub fn to_value(&self) -> PyValue {
        let opt =
            |verdict: &Option<Verdict>| verdict.as_ref().map_or(PyValue::None, Verdict::to_value);
        let mut out = PyDict::new();
        out.set("topology", self.topology.clone());
        out.set("sub_present", PyValue::Bool(self.sub_present));
        out.set("sub_mode", opt(&self.sub_mode));
        out.set("rank", opt(&self.rank));
        out.set("identity_authority", opt(&self.identity_authority));
        out.set("delivery_plane", self.delivery_plane.to_value());
        out.set("tool_plane", self.tool_plane.to_value());
        out.set(
            "notes",
            PyValue::List(self.notes.iter().cloned().map(PyValue::Str).collect()),
        );
        out.set(
            "violations",
            PyValue::List(
                self.violations
                    .iter()
                    .map(ContractViolation::as_dict)
                    .collect(),
            ),
        );
        PyValue::Dict(out)
    }
}

fn dict_get<'a>(doc: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match doc.peel() {
        PyValue::Dict(dict) => dict.get(key),
        _ => None,
    }
}

/// `evaluate_manifest(doc, topology=None, role="sub")`: never raises; violations are collected.
pub fn evaluate_manifest(doc: &PyValue, topology: &PyValue, role: &PyValue) -> Evaluation {
    let manifest = if matches!(doc.peel(), PyValue::Dict(_)) {
        doc
    } else {
        &PyValue::None
    };
    let topo = if topology.is_truthy() {
        topology.clone()
    } else {
        dict_get(manifest, "topology")
            .cloned()
            .unwrap_or(PyValue::None)
    };
    let mut out = Evaluation {
        topology: topo.clone(),
        sub_present: false,
        sub_mode: None,
        rank: None,
        identity_authority: None,
        delivery_plane: Verdict::text("dormant", "fail-closed:unevaluated"),
        tool_plane: Verdict {
            value: PyValue::List(Vec::new()),
            source: PyStr::new("fail-closed:unevaluated"),
        },
        notes: Vec::new(),
        violations: Vec::new(),
    };
    if let Err(violation) = assert_topology(&topo) {
        out.violations.push(violation);
        return out;
    }
    out.identity_authority = identity_authority(&topo).ok();
    let nodes: Vec<PyValue> = dict_get(manifest, "nodes")
        .and_then(as_list)
        .cloned()
        .unwrap_or_default();
    let sub = nodes.iter().find(|node| {
        node_role(node).is_some_and(|r| matches!(r.peel(), PyValue::Str(s) if s.equals(SUB_ROLE)))
    });
    out.sub_present = sub.is_some();
    if !nodes.is_empty() {
        let main_hits = indexes_with_role(&nodes, &PyValue::str(MAIN_ROLE));
        if main_hits.len() == 1 && main_hits[0] != 0 && topology_text(&topo) == "multi" {
            out.notes.push(PyStr::new(&format!(
                "nodes[] 의 role:main 이 인덱스 {} 다(관례상 head=rank 0). rank 는 인덱스 그대로 부여된다.",
                main_hits[0]
            )));
        }
    }
    let Some(sub) = sub else {
        out.delivery_plane = Verdict::text("dormant", "no-sub-registered");
        out.tool_plane = Verdict {
            value: PyValue::List(Vec::new()),
            source: PyStr::new("no-sub-registered"),
        };
        out.rank = Some(Verdict {
            value: PyValue::None,
            source: PyStr::new("not-applicable:no-sub-registered"),
        });
        return out;
    };
    let declared = dict_get(sub, "sub_mode").cloned().unwrap_or(PyValue::None);
    match resolve_sub_mode(&topo, &declared) {
        Ok(verdict) => out.sub_mode = Some(verdict),
        Err(violation) => {
            out.violations.push(violation);
            return out;
        }
    }
    let mode_value = out
        .sub_mode
        .as_ref()
        .map(|v| v.value.clone())
        .unwrap_or(PyValue::None);
    match resolve_rank(&topo, &PyValue::List(nodes.clone()), role) {
        Ok(verdict) => out.rank = Some(verdict),
        Err(violation) => out.violations.push(violation),
    }
    match delivery_plane(&topo, &mode_value) {
        Ok(verdict) => out.delivery_plane = verdict,
        Err(violation) => out.violations.push(violation),
    }
    match tool_plane(&topo, &mode_value) {
        Ok(verdict) => out.tool_plane = verdict,
        Err(violation) => out.violations.push(violation),
    }
    out
}

/// `manifest_path_for(repo, topology)` = `os.path.join(repo, "output", topology, "manifest.yaml")` for the argument types `os.path.join` decides.
pub fn manifest_path_for(repo: &PyValue, topology: &PyValue) -> Result<PyStr, PathJoinError> {
    let parts: [PyValue; 4] = [
        repo.clone(),
        PyValue::str("output"),
        topology.clone(),
        PyValue::str("manifest.yaml"),
    ];
    for part in &parts {
        if !matches!(part.peel(), PyValue::Str(_) | PyValue::Bytes(_)) {
            return Err(PathJoinError::NotPathLike(part.peel().type_name()));
        }
    }
    if parts.iter().any(|p| matches!(p.peel(), PyValue::Bytes(_))) {
        return Err(PathJoinError::MixedStrAndBytes);
    }
    let mut path = PyStr::default();
    for (index, part) in parts.iter().enumerate() {
        let Some(piece) = str_of(part) else { continue };
        // the first part starts the path and an absolute part restarts it
        if index == 0 || piece.code_points().first() == Some(&u32::from('/')) {
            path = piece.clone();
        } else if path.is_empty() || path.ends_with_code_point(u32::from('/')) {
            path.push_py(piece);
        } else {
            path.push_str("/");
            path.push_py(piece);
        }
    }
    Ok(path)
}

/// Why `os.path.join` refused its arguments (`TypeError`, with the context of the failed `os.fspath` suppressed by `from None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathJoinError {
    /// `join() argument must be str, bytes, or os.PathLike object, not 'T'`.
    NotPathLike(&'static str),
    /// `Can't mix strings and bytes in path components`.
    MixedStrAndBytes,
}

impl PathJoinError {
    /// The logical traceback: the owner's `manifest_path_for` (line 110) calling `posixpath.join` (line 90 of the frozen module), whose
    /// `genericpath._check_arg_types` raises (line 164 for an argument that is not a path, 167 for the str/bytes mix) `from None`.
    pub fn frames(&self) -> Vec<SourceFrame> {
        let raise = match self {
            PathJoinError::NotPathLike(_) => 164,
            PathJoinError::MixedStrAndBytes => 167,
        };
        vec![
            SourceFrame {
                root: FrameRoot::Owner,
                file: OWNER_FILE,
                line: 110,
                function: "manifest_path_for",
            },
            SourceFrame {
                root: FrameRoot::Frozen,
                file: "<frozen posixpath>",
                line: 90,
                function: "join",
            },
            SourceFrame {
                root: FrameRoot::Frozen,
                file: "<frozen genericpath>",
                line: raise,
                function: "_check_arg_types",
            },
        ]
    }

    pub fn message(&self) -> String {
        match self {
            PathJoinError::NotPathLike(name) => {
                format!("join() argument must be str, bytes, or os.PathLike object, not '{name}'")
            }
            PathJoinError::MixedStrAndBytes => {
                "Can't mix strings and bytes in path components".to_string()
            }
        }
    }
}
