//! Native `campaign_init.py` writer side: the operations that create or change a campaign instance
//! (`--init`, `--close`, `--phase-set`, `--cell-set`, `--write-brief`) and the assignment query (`--assigned-cells`).
//!
//! The owner program stays the reference (`campaign_init.py`, not edited). This module answers the argument shapes it
//! models natively - same stdout, stderr, exit status and bytes written - and reports [`Outcome::Fallback`] for every
//! shape it does not model *before any side effect*, so the caller (`bin/campaign_init.rs`) can hand the unchanged
//! argument vector to the owner. Nothing is retried once a write has started.
//!
//! Text rules kept from the owner: state files are JSON (`campaign.yaml` too) written with `ensure_ascii=False`,
//! `sort_keys=True`, `indent=1` (writer files) or insertion order, `indent=2` (declaration, pointers); journey lines are
//! compact, key-sorted JSON. A refusal prints `[campaign_init] FAIL <message>` and exits with status 1.

use crate::lite_metrics::read_lite_verdict;
use crate::pyjson;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BOOTSTRAP: &str = "_bootstrap";
pub const RESERVED_IDS: [&str; 2] = ["_template", BOOTSTRAP];
pub const FILL: &str = "<<FILL>>";
pub const JOURNEY_NAME: &str = "journey.jsonl";
pub const LIFECYCLE_V: i64 = 1;
const PHASE_STATES: [&str; 4] = ["pending", "running", "done", "failed"];
const PHASE_NAMES: [&str; 4] = ["build", "serve", "bench", "publish"];
const CELL_OUTCOMES: [&str; 7] = [
    "pending",
    "measured",
    "serve_failed",
    "build_failed",
    "void",
    "measurement_void",
    "not_measured",
];
const CELL_OUTCOMES_PENDING: [&str; 2] = ["pending", "not_measured"];
const LIFECYCLE_KINDS: [&str; 10] = [
    "campaign_opened",
    "campaign_suspended",
    "campaign_resume_pending",
    "campaign_query_answered",
    "campaign_resumed",
    "campaign_closed",
    "cell_interrupted",
    "sub_control_echo",
    "sub_control_accepted",
    "campaign_assignments_revised",
];
const PROGRESS_OPS: [&str; 5] = [
    "phase_set",
    "cell_set",
    "cell_materialize",
    "reentry_decide",
    "assign_set",
];

// ── outcome and refusal ─────────────────────────────────────────────────────────────────────

/// How a native attempt ended.
pub enum Outcome {
    /// The shape is answered: the exit status (output already written).
    Done(i32),
    /// The shape is not modelled; nothing was written or printed.
    Fallback,
}

/// An owner refusal (`PurgeGateRefusal`, `WriterRefusal`, `TransitionRefusal`): printed as `[campaign_init] FAIL <text>`.
pub enum Stop {
    Refuse(String),
    Fallback,
}

type R<T> = Result<T, Stop>;

fn refuse<T>(text: impl Into<String>) -> R<T> {
    Err(Stop::Refuse(text.into()))
}

// ── the process surface ─────────────────────────────────────────────────────────────────────

/// Standard output and error of one run, injectable for tests.
pub struct Sinks<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

impl Sinks<'_> {
    pub fn say(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
    }

    pub fn warn(&mut self, text: &str) {
        let _ = writeln!(self.err, "{text}");
    }
}

// ── argument vector ─────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flag,
    Value,
    Append,
    Optional,
}

/// Every option of the owner's parser (generated from its `add_argument` calls; a unit check compares the list).
pub const OPTIONS: [(&str, &str); 108] = [
    ("--abandon", "value"), ("--accept-drift", "flag"), ("--active", "flag"), ("--answer-query", "value"),
    ("--apply", "flag"), ("--approved-by", "value"), ("--assign-approved-by", "value"), ("--assign-cells", "value"),
    ("--assign-evidence", "value"), ("--assign-reason", "value"), ("--assign-set", "value"),
    ("--assigned-cells", "value"), ("--authored-by", "value"), ("--axis-citation", "value"),
    ("--backfill-from-docs", "flag"), ("--campaign-id", "value"), ("--cell", "value"),
    ("--cell-materialize", "value"), ("--cell-set", "value"), ("--cells", "value"), ("--close", "value"),
    ("--context", "value"), ("--control", "value"), ("--correct-reference", "flag"), ("--decision", "value"),
    ("--derive", "value"), ("--derived-from", "value"), ("--dispose", "append"), ("--emit-slice", "value"),
    ("--ended-utc", "value"), ("--escalation-add", "flag"), ("--escalation-summary", "flag"),
    ("--events", "append"), ("--evidence-add", "flag"), ("--evidence-prune-stubs", "flag"),
    ("--evidence-relocate", "flag"), ("--evidence-untag", "flag"), ("--freeze-evidence", "flag"),
    ("--from-slice", "value"), ("--ground", "flag"), ("--grounding-check", "flag"), ("--hint-approve", "flag"),
    ("--image-digest", "value"), ("--image-tag", "value"), ("--import-sub", "value"), ("--init", "value"),
    ("--kind", "value"), ("--lifecycle", "opt"), ("--lite-raw", "value"), ("--lockset-precheck", "flag"),
    ("--measurement-decode-tps", "value"), ("--measurement-source", "value"), ("--model", "value"),
    ("--next-intent", "value"), ("--no-warm-start", "flag"), ("--node", "value"), ("--out", "value"),
    ("--outcome", "value"), ("--path", "value"), ("--phase-set", "value"), ("--plan-ref", "value"),
    ("--preempt", "flag"), ("--proof-ok", "flag"), ("--proof-predicate", "value"), ("--proof-source", "value"),
    ("--purge-previous", "append"), ("--query-decision", "value"), ("--reason", "value"),
    ("--reentry-decide", "value"), ("--release", "append"), ("--relocate-reason", "value"),
    ("--residue-scan", "flag"), ("--resume-brief", "flag"), ("--resume-campaign", "value"), ("--revise", "flag"),
    ("--revise-approved-by", "value"), ("--revise-evidence", "value"), ("--revise-reason", "value"),
    ("--revise-set", "append"), ("--route", "value"), ("--selftest", "flag"), ("--set", "append"),
    ("--slice-update", "value"), ("--source", "value"), ("--started-utc", "value"), ("--state", "value"),
    ("--sub-echo", "value"), ("--sub-unconverged-accept", "value"), ("--summary-testlog", "value"),
    ("--suspend", "value"), ("--suspend-current", "flag"), ("--sweep", "value"), ("--sweep-state", "value"),
    ("--symptom", "append"), ("--target-topology", "value"), ("--terms", "value"), ("--to", "value"),
    ("--unfreeze", "flag"), ("--untag-reason", "value"), ("--utc", "value"), ("--verify-purge-gate", "value"),
    ("--version", "value"), ("--void-reason", "value"), ("--void-reason-source", "value"),
    ("--warm-start-library", "flag"), ("--window-end-utc", "value"), ("--window-start-utc", "value"),
    ("--write-brief", "flag"),
];

fn kind_of(name: &str) -> Option<Kind> {
    OPTIONS.iter().find(|(n, _)| *n == name).map(|(_, k)| match *k {
        "flag" => Kind::Flag,
        "append" => Kind::Append,
        "opt" => Kind::Optional,
        _ => Kind::Value,
    })
}

/// Options parsed exactly (no abbreviation, no `--opt=value`, no value that looks like an option). `None` = not exact:
/// the owner's `argparse` decides (usage errors, abbreviations, `-h`).
pub struct Args {
    values: BTreeMap<&'static str, Vec<String>>,
}

impl Args {
    pub fn parse(argv: &[String]) -> Option<Args> {
        let mut values: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        let mut at = 0;
        while at < argv.len() {
            let word = &argv[at];
            let (name, _) = OPTIONS.iter().find(|(n, _)| *n == word.as_str())?;
            match kind_of(name)? {
                Kind::Flag => {
                    values.insert(name, vec![String::new()]);
                    at += 1;
                }
                Kind::Optional => return None,
                Kind::Value | Kind::Append => {
                    let value = argv.get(at + 1)?;
                    if value.starts_with('-') && value != "-" {
                        return None;
                    }
                    let entry = values.entry(name).or_default();
                    if kind_of(name) == Some(Kind::Value) {
                        entry.clear();
                    }
                    entry.push(value.clone());
                    at += 2;
                }
            }
        }
        Some(Args { values })
    }

    fn has(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(|v| v.last()).map(String::as_str)
    }

        fn all(&self, name: &str) -> &[String] {
        self.values.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    fn names(&self) -> impl Iterator<Item = &&'static str> {
        self.values.keys()
    }
}

// ── Python-shaped helpers ───────────────────────────────────────────────────────────────────

fn py(text: &str) -> PyValue {
    PyValue::Str(PyStr::new(text))
}

pub(crate) fn py_repr(text: &str) -> String {
    PyStr::new(text).repr()
}

fn py_list_repr(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| py_repr(s)).collect();
    format!("[{}]", inner.join(", "))
}

pub(crate) fn dict_of(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(dict) => Some(dict),
        _ => None,
    }
}

pub(crate) fn list_of(value: &PyValue) -> Option<&Vec<PyValue>> {
    match value.peel() {
        PyValue::List(items) => Some(items),
        _ => None,
    }
}

pub(crate) fn text_of(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8(),
        _ => None,
    }
}

pub(crate) fn get_text(dict: &PyDict, key: &str) -> Option<String> {
    dict.get(key).and_then(text_of)
}

fn get_dict<'a>(dict: &'a PyDict, key: &str) -> Option<&'a PyDict> {
    dict.get(key).and_then(dict_of)
}

fn truthy(value: Option<&PyValue>) -> bool {
    value.map(|v| v.is_truthy()).unwrap_or(false)
}

fn int_of(value: Option<&PyValue>) -> Option<i64> {
    match value.map(PyValue::peel) {
        Some(PyValue::Int(n)) => i64::try_from(n.clone()).ok(),
        Some(PyValue::Bool(b)) => Some(i64::from(*b)),
        _ => None,
    }
}

fn str_list(items: Vec<String>) -> PyValue {
    PyValue::List(items.iter().map(|s| py(s)).collect())
}

fn sorted_value(value: &PyValue) -> PyValue {
    match value.peel() {
        PyValue::Dict(dict) => {
            let mut entries: Vec<(PyStr, PyValue)> =
                dict.iter().map(|(k, v)| (k.clone(), sorted_value(v))).collect();
            entries.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (key, value) in entries {
                out.insert(key, value);
            }
            PyValue::Dict(out)
        }
        PyValue::List(items) => PyValue::List(items.iter().map(sorted_value).collect()),
        PyValue::Tuple(items) => PyValue::List(items.iter().map(sorted_value).collect()),
        other => other.clone(),
    }
}

fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

// ── context: checkout root and the campaign workspace ───────────────────────────────────────

/// `REPO_ROOT` of the owner (`Path(__file__).resolve().parents[4]`) and the files under it the writer touches.
#[derive(Clone)]
pub struct Ctx {
    pub root: PathBuf,
}

impl Ctx {
    pub fn campaigns(&self) -> PathBuf {
        self.root.join("campaigns")
    }

    pub fn active_pointer(&self) -> PathBuf {
        self.campaigns().join("ACTIVE")
    }

    pub fn template(&self) -> PathBuf {
        self.campaigns().join("_template")
    }

    /// `_rel`: relative to the checkout when inside it, else the path as given.
    pub fn rel(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rest) => rest.to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }

    /// `active_campaign_id()`.
    pub fn active_id(&self) -> String {
        let pointer = self.active_pointer();
        if pointer.is_file() {
            if let Ok(bytes) = std::fs::read(&pointer) {
                if let Ok(text) = String::from_utf8(bytes) {
                    let name = text.trim();
                    if !name.is_empty() && is_dir(&self.campaigns().join(name)) {
                        return name.to_string();
                    }
                }
            }
        }
        BOOTSTRAP.to_string()
    }

    /// `_active_pointer_fact()`.
    fn active_pointer_fact(&self) -> String {
        let pointer = self.active_pointer();
        if !pointer.is_file() {
            return "ACTIVE 포인터 부재".to_string();
        }
        let text = match std::fs::read(&pointer) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(error) => return format!("ACTIVE 포인터 읽기 실패({error})"),
        };
        let name = text.trim();
        if name.is_empty() {
            return "ACTIVE 포인터가 비었다".to_string();
        }
        if name == BOOTSTRAP {
            return "ACTIVE=_bootstrap(명시)".to_string();
        }
        if !is_dir(&self.campaigns().join(name)) {
            return format!("ACTIVE={} 무효(campaigns/{name} 디렉터리 없음)", py_repr(name));
        }
        format!("ACTIVE={name}")
    }

    fn manifest_text(&self, topology: &str) -> Option<String> {
        let bytes = std::fs::read(self.root.join("output").join(topology).join("manifest.yaml")).ok()?;
        String::from_utf8(bytes).ok()
    }

    /// `checkout_is_sub()`: a manifest line `self_role: sub`.
    pub fn checkout_is_sub(&self) -> bool {
        ["single", "multi"]
            .iter()
            .filter_map(|t| self.manifest_text(t))
            .any(|text| text.lines().any(|line| key_line_value(line, "self_role").as_deref() == Some("sub")))
    }

    /// `_manifest_topology(root)`: the one `output/<t>/manifest.yaml` whose own `topology:` names its channel.
    fn manifest_topology(&self) -> Option<String> {
        let mut found = Vec::new();
        for t in ["single", "multi"] {
            if let Some(text) = self.manifest_text(t) {
                let hit = text.lines().any(|line| {
                    matches!(key_line_value(line, "topology").as_deref(), Some(v) if v == t)
                });
                if hit {
                    found.push(t);
                }
            }
        }
        if found.len() == 1 {
            Some(found[0].to_string())
        } else {
            None
        }
    }

    /// `_sub_local_topology(root)`: the manifest, then a local branch that is itself a topology token.
    fn sub_local_topology(&self) -> Option<String> {
        if let Some(found) = self.manifest_topology() {
            return Some(found);
        }
        let branch = git_stdout(&self.root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
        if branch == "single" || branch == "multi" {
            Some(branch)
        } else {
            None
        }
    }

    /// `checkout_topology()`.
    pub fn checkout_topology(&self) -> Option<String> {
        let predicate = self.root.join(".claude/policies/runtime/topology_parity.py");
        if !predicate.is_file() {
            return self.sub_local_topology();
        }
        if self.checkout_is_sub() {
            return self.sub_local_topology();
        }
        let branch = git_stdout(&self.root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .filter(|b| !b.is_empty())
            .or_else(|| git_stdout(&self.root, &["rev-parse", "--abbrev-ref", "HEAD"]))
            .unwrap_or_default();
        match branch.as_str() {
            "single-node" => Some("single".to_string()),
            "multi-node" => Some("multi".to_string()),
            _ => None,
        }
    }
}

/// `^key:\s*['"]?(value)['"]?\s*(?:#.*)?$` on one line: the value, or `None`.
fn key_line_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.strip_prefix(':')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(['\'', '"']).unwrap_or(rest);
    let end = rest
        .find(|c: char| c == '\'' || c == '"' || c.is_whitespace() || c == '#')
        .unwrap_or(rest.len());
    let (value, tail) = rest.split_at(end);
    let tail = tail.strip_prefix(['\'', '"']).unwrap_or(tail);
    let tail = tail.trim_start();
    if tail.is_empty() || tail.starts_with('#') {
        Some(value.to_string())
    } else {
        None
    }
}

fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── files ────────────────────────────────────────────────────────────────────────────────────

/// `_read_json(path)`: the value, or `None` when it cannot be read or parsed.
pub fn read_json(path: &Path) -> Option<PyValue> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    pyjson::loads(&text).ok()
}

/// `read_declaration(base)`: the declaration object or an empty dict.
pub fn read_declaration(base: &Path) -> PyDict {
    match read_json(&base.join("campaign.yaml")) {
        Some(value) => match value.peel() {
            PyValue::Dict(dict) => dict.clone(),
            _ => PyDict::new(),
        },
        None => PyDict::new(),
    }
}

fn dump(value: &PyValue, indent: usize) -> R<String> {
    let text = value
        .dumps_indented(indent)
        .map_err(|e| Stop::Refuse(e.to_string()))?;
    text.to_utf8()
        .ok_or_else(|| Stop::Refuse("UnicodeEncodeError: surrogates not allowed".to_string()))
}

fn write_text(path: &Path, text: &str) -> R<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Stop::Refuse(e.to_string()))?;
    }
    std::fs::write(path, text.as_bytes()).map_err(|e| Stop::Refuse(format!("{}: {e}", path.display())))
}

/// `_write_json`: sorted keys, `indent=1`, newline.
pub fn write_json(path: &Path, doc: &PyValue) -> R<()> {
    let text = dump(&sorted_value(doc), 1)?;
    write_text(path, &(text + "\n"))
}

/// `json.dumps(doc, ensure_ascii=False, indent=2) + "\n"` in insertion order.
fn write_json_ordered(path: &Path, doc: &PyValue) -> R<()> {
    let text = dump(doc, 2)?;
    write_text(path, &(text + "\n"))
}

/// `writer_append_journey`: one compact key-sorted line.
pub fn append_journey(base: &Path, entry: &PyValue) -> R<PathBuf> {
    let path = base.join(JOURNEY_NAME);
    let line = sorted_value(entry)
        .dumps_unicode()
        .map_err(|e| Stop::Refuse(e.to_string()))?
        .to_utf8()
        .ok_or_else(|| Stop::Refuse("UnicodeEncodeError: surrogates not allowed".to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Stop::Refuse(e.to_string()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| Stop::Refuse(format!("{}: {e}", path.display())))?;
    file.write_all((line + "\n").as_bytes())
        .map_err(|e| Stop::Refuse(e.to_string()))?;
    Ok(path)
}

/// `read_journey`: parsed lines; an unparsable line becomes `{"_unparsed": line}`.
pub fn read_journey(base: &Path) -> Vec<PyValue> {
    let path = base.join(JOURNEY_NAME);
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match pyjson::loads(line) {
            Ok(value) => out.push(value),
            Err(_) => {
                let mut dict = PyDict::new();
                dict.set("_unparsed", py(line));
                out.push(PyValue::Dict(dict));
            }
        }
    }
    out
}

// ── declaration helpers (the validator's assignment functions) ───────────────────────────────

/// `assignments_of(doc)`: entries of the `assignments` object in declaration order.
pub(crate) fn assignments_of(doc: &PyDict) -> Vec<(String, PyValue)> {
    match get_dict(doc, "assignments") {
        Some(map) => map
            .iter()
            .map(|(k, v)| (k.to_utf8().unwrap_or_default(), v.clone()))
            .collect(),
        None => Vec::new(),
    }
}

/// `assigned_cells(doc, node)`: cells in declaration order, de-duplicated, placeholders skipped.
pub fn assigned_cells(doc: &PyDict, node: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (nid, items) in assignments_of(doc) {
        if let Some(wanted) = node {
            if nid != wanted {
                continue;
            }
        }
        if let Some(items) = list_of(&items) {
            for item in items {
                if let Some(cell) = dict_of(item).and_then(|d| get_text(d, "cell")) {
                    if !cell.is_empty() && !cell.contains(FILL) && !out.contains(&cell) {
                        out.push(cell);
                    }
                }
            }
        }
    }
    out
}

/// `cell_mode(doc, node, cell)`.
fn cell_mode(doc: &PyDict, node: &str, cell: &str) -> String {
    for (nid, items) in assignments_of(doc) {
        if nid != node {
            continue;
        }
        if let Some(items) = list_of(&items) {
            for item in items.iter().filter_map(dict_of) {
                if get_text(item, "cell").as_deref() == Some(cell) {
                    return match get_text(item, "mode") {
                        Some(mode) if ["AUTO", "HITL", "STAY"].contains(&mode.as_str()) => mode,
                        _ => "AUTO".to_string(),
                    };
                }
            }
        }
    }
    "AUTO".to_string()
}

/// `node_of_cell(base, cell)`.
fn node_of_cell(base: &Path, cell: &str) -> Option<String> {
    let doc = read_declaration(base);
    for (node, _) in assignments_of(&doc) {
        if assigned_cells(&doc, Some(&node)).iter().any(|c| c == cell) {
            return Some(node);
        }
    }
    None
}

/// `lockset_observable(decl, node)`.
pub(crate) fn lockset_observable(decl: &PyDict, node: Option<&str>) -> bool {
    if get_text(decl, "self_role").as_deref() == Some("sub") {
        return true;
    }
    let role = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .find(|n| get_text(n, "node_id").as_deref() == node && node.is_some())
        .and_then(|n| get_text(n, "role"));
    role.as_deref() != Some("sub")
}

/// `lockset_provenance_reason(path)`.
pub(crate) fn lockset_provenance_reason(ctx: &Ctx, path: &Path) -> Option<String> {
    const ALLOWED: &str = "('explorer-phase2', 'hand-authored')";
    if !path.is_file() {
        return Some(format!(
            "lockset 부재: {} — 셀 출처(provenance)를 말할 자리가 없다(셀 materialize 는 explorer 소관 · 손작성이면 provenance=hand-authored 로 표시)",
            ctx.rel(path)
        ));
    }
    let bytes = std::fs::read(path).ok()?;
    let doc = match String::from_utf8(bytes).ok().map(|t| pyjson::loads(&t)) {
        Some(Ok(doc)) => doc,
        Some(Err(error)) => return Some(format!("lockset 파손({}): {error}", ctx.rel(path))),
        None => return Some(format!("lockset 파손({}): invalid utf-8", ctx.rel(path))),
    };
    let Some(doc) = dict_of(&doc) else {
        return Some(format!("lockset 최상위가 객체가 아니다: {}", ctx.rel(path)));
    };
    match doc.get("provenance").map(PyValue::peel) {
        None | Some(PyValue::None) => Some(format!(
            "lockset provenance 부재: {} — 값 {ALLOWED} 중 하나로 출처를 표시하라(손작성은 금지가 아니라 표시 대상이다)",
            ctx.rel(path)
        )),
        Some(PyValue::Str(text)) if ["explorer-phase2", "hand-authored"].iter().any(|a| text.equals(a)) => None,
        Some(other) => Some(format!(
            "lockset provenance 무효: {} ({}) — 허용 {ALLOWED}",
            other.repr(),
            ctx.rel(path)
        )),
    }
}

fn lockset_stamp(ldoc: &PyDict) -> PyValue {
    match ldoc.get("provenance").and_then(text_of).as_deref() {
        Some("hand-authored") => py("hand"),
        Some("explorer-phase2") => {
            let trial = ldoc.get("trial_provenance").and_then(text_of);
            if matches!(trial.as_deref(), Some("measured" | "mock" | "dry-run")) {
                py("machine")
            } else {
                py("self-declared")
            }
        }
        _ => PyValue::None,
    }
}

// ── lifecycle fold ───────────────────────────────────────────────────────────────────────────

/// `fold_lifecycle(base, active=...)`: the journey folded into one state (see the owner for the event grammar).
pub struct Fold {
    pub campaign_id: String,
    pub state: String,
    pub close_reason: Option<String>,
    pub parent: PyValue,
    pub topology: Option<String>,
    pub suspended_pins: PyValue,
    pub released_resources: Vec<PyValue>,
    pub interrupted_cells: Vec<String>,
    pub last_transition_event: Option<PyValue>,
    pub answered: bool,
    pub pending_query: PyValue,
    pub sub_convergence: Option<String>,
    pub sub_nodes: Vec<PyValue>,
    pub sub_echoed: Vec<String>,
    pub handoff_pointers: Vec<PyValue>,
    pub parked_devlog: PyValue,
    pub open_pending: PyValue,
    pub sub_accepted: Vec<String>,
    pub reslice_pending: Vec<(String, PyValue)>,
    pub unknown_events: Vec<PyValue>,
}

fn is_lifecycle(entry: &PyValue) -> bool {
    let Some(dict) = dict_of(entry) else {
        return false;
    };
    let kind = match dict.get("kind") {
        Some(v) if v.is_truthy() => v.display_text().to_utf8().unwrap_or_default(),
        _ => String::new(),
    };
    LIFECYCLE_KINDS.contains(&kind.as_str()) || kind.starts_with("campaign_") || dict.contains_key("lifecycle_v")
}

/// `_declared_topology(base)`.
fn declared_topology(base: &Path) -> Option<String> {
    let doc = read_declaration(base);
    let current = list_of(doc.get("revisions").unwrap_or(&PyValue::None))
        .and_then(|revs| revs.last())
        .and_then(dict_of)
        .and_then(|rev| rev.get("values"))
        .filter(|v| v.is_truthy())
        .cloned()
        .or_else(|| doc.get("control_variables").filter(|v| v.is_truthy()).cloned())
        .unwrap_or(PyValue::None);
    let topology = dict_of(&current).and_then(|d| get_text(d, "topology"));
    match topology.as_deref() {
        Some("single" | "multi") => topology,
        _ => None,
    }
}

/// `_sub_nodes(base)`.
fn sub_nodes_of(ctx: &Ctx, base: &Path) -> Vec<PyValue> {
    if declared_topology(base).as_deref() != Some("single") {
        return Vec::new();
    }
    let decl = read_declaration(base);
    if get_text(&decl, "self_role").as_deref() == Some("sub") || ctx.checkout_is_sub() {
        return Vec::new();
    }
    list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .filter(|n| get_text(n, "role").as_deref() == Some("sub"))
        .filter_map(|n| get_text(n, "node_id"))
        .filter(|id| !id.contains(FILL))
        .map(|id| py(&id))
        .collect()
}

fn lifecycle_dict(index: usize, kind: &str, utc: Option<&PyValue>) -> PyValue {
    let mut d = PyDict::new();
    d.set("ref", py(&format!("{index}:{kind}")));
    d.set("index", PyValue::int(index as i64));
    d.set("kind", py(kind));
    d.set("utc", utc.cloned().unwrap_or(PyValue::None));
    PyValue::Dict(d)
}

pub fn fold_lifecycle(ctx: &Ctx, base: &Path, active: Option<&str>) -> Fold {
    let camp = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let active = active.map(str::to_string).unwrap_or_else(|| ctx.active_id());
    let mut out = Fold {
        campaign_id: camp.clone(),
        state: String::new(),
        close_reason: None,
        parent: PyValue::None,
        topology: declared_topology(base),
        suspended_pins: PyValue::None,
        released_resources: Vec::new(),
        interrupted_cells: Vec::new(),
        last_transition_event: None,
        answered: false,
        pending_query: PyValue::None,
        sub_convergence: None,
        sub_nodes: Vec::new(),
        sub_echoed: Vec::new(),
        handoff_pointers: Vec::new(),
        parked_devlog: PyValue::None,
        open_pending: PyValue::None,
        sub_accepted: Vec::new(),
        reslice_pending: Vec::new(),
        unknown_events: Vec::new(),
    };
    let mut state: Option<String> = None;
    let mut control: Option<String> = None;
    let mut echoed: Vec<String> = Vec::new();
    for (i, entry) in read_journey(base).iter().enumerate() {
        if !is_lifecycle(entry) {
            continue;
        }
        let e = dict_of(entry).expect("lifecycle entries are objects");
        let kind_value = e.get("kind").cloned().unwrap_or(PyValue::None);
        let kind = text_of(&kind_value);
        let version = match e.get("lifecycle_v") {
            Some(v) if v.is_truthy() => int_of(Some(v)).unwrap_or(LIFECYCLE_V + 1),
            _ => 1,
        };
        let known = kind.as_deref().map(|k| LIFECYCLE_KINDS.contains(&k)).unwrap_or(false);
        if !known || version > LIFECYCLE_V {
            let mut d = PyDict::new();
            d.set("index", PyValue::int(i as i64));
            d.set("kind", kind_value.clone());
            d.set("lifecycle_v", e.get("lifecycle_v").cloned().unwrap_or(PyValue::None));
            out.unknown_events.push(PyValue::Dict(d));
            continue;
        }
        let kind = kind.expect("known kind is text");
        let node = get_text(e, "node");
        match kind.as_str() {
            "cell_interrupted" => {
                if let Some(cell) = get_text(e, "cell_id").filter(|c| !c.is_empty()) {
                    if !out.interrupted_cells.contains(&cell) {
                        out.interrupted_cells.push(cell);
                    }
                }
                continue;
            }
            "campaign_assignments_revised" => {
                if truthy(e.get("sub_target")) && truthy(e.get("node")) {
                    let node = node.clone().unwrap_or_default();
                    let rev = e.get("rev").cloned().unwrap_or(PyValue::None);
                    match out.reslice_pending.iter_mut().find(|(n, _)| *n == node) {
                        Some(slot) => slot.1 = rev,
                        None => out.reslice_pending.push((node, rev)),
                    }
                }
                continue;
            }
            "sub_control_echo" if get_text(e, "control").as_deref() == Some("reslice") => {
                if let Some(node) = node.clone() {
                    if let Some(at) = out.reslice_pending.iter().position(|(n, _)| *n == node) {
                        let have = int_of(e.get("rev")).filter(|n| *n != 0).unwrap_or(-1);
                        let want = int_of(Some(&out.reslice_pending[at].1)).filter(|n| *n != 0).unwrap_or(0);
                        if have >= want {
                            out.reslice_pending.remove(at);
                        }
                    }
                }
                continue;
            }
            "sub_control_echo" | "sub_control_accepted" => {
                if get_text(e, "control") == control && truthy(e.get("node")) {
                    let node = node.unwrap_or_default();
                    if !echoed.contains(&node) {
                        echoed.push(node.clone());
                    }
                    if kind == "sub_control_accepted" {
                        out.sub_accepted.push(node);
                    }
                }
                continue;
            }
            "campaign_query_answered" => {
                let last_ref = out
                    .last_transition_event
                    .as_ref()
                    .and_then(dict_of)
                    .and_then(|d| d.get("ref").cloned());
                if e.get("answers").cloned().unwrap_or(PyValue::None) == last_ref.unwrap_or(PyValue::None) {
                    out.answered = true;
                    if state.as_deref() == Some("resume_pending")
                        && get_text(e, "decision").as_deref() == Some("keep")
                    {
                        state = Some("suspended".to_string());
                        out.pending_query = PyValue::None;
                    }
                }
                continue;
            }
            "campaign_opened" => {
                state = Some("active".to_string());
                out.parent = e.get("parent").cloned().unwrap_or(PyValue::None);
            }
            "campaign_suspended" => {
                state = Some("suspended".to_string());
                out.suspended_pins = e.get("pins").cloned().unwrap_or(PyValue::None);
                out.released_resources = list_or_empty(e.get("released"));
                out.parked_devlog = e.get("parked_devlog").cloned().unwrap_or(PyValue::None);
                out.open_pending = e.get("next").cloned().unwrap_or(PyValue::None);
                out.sub_nodes = list_or_empty(e.get("sub_nodes"));
                control = Some("suspend".to_string());
                echoed.clear();
            }
            "campaign_resume_pending" => {
                state = Some("resume_pending".to_string());
                let mut q = PyDict::new();
                q.set("trigger", e.get("trigger").cloned().unwrap_or(PyValue::None));
                q.set("child", e.get("child").cloned().unwrap_or(PyValue::None));
                q.set("child_reason", e.get("child_reason").cloned().unwrap_or(PyValue::None));
                q.set("pointers", PyValue::List(list_or_empty(e.get("pointers"))));
                q.set("utc", e.get("utc").cloned().unwrap_or(PyValue::None));
                out.pending_query = PyValue::Dict(q);
            }
            "campaign_resumed" => {
                state = Some("active".to_string());
                out.interrupted_cells.clear();
                out.handoff_pointers = list_or_empty(e.get("handoff_pointers"));
                out.open_pending = PyValue::None;
                out.pending_query = PyValue::None;
                out.sub_nodes = list_or_empty(e.get("sub_nodes"));
                control = Some("resume".to_string());
                echoed.clear();
            }
            "campaign_closed" => {
                state = Some("closed".to_string());
                let reason = match e.get("reason") {
                    Some(v) if v.is_truthy() => v.display_text().to_utf8().unwrap_or_default(),
                    _ => "completed".to_string(),
                };
                out.open_pending = PyValue::None;
                out.pending_query = PyValue::None;
                out.sub_nodes = list_or_empty(e.get("sub_nodes"));
                out.sub_accepted.clear();
                control = Some(if reason == "abandoned" { "abandon" } else { "close" }.to_string());
                out.close_reason = Some(reason);
                echoed.clear();
            }
            _ => unreachable!("kinds are filtered above"),
        }
        out.last_transition_event = Some(lifecycle_dict(i, &kind, e.get("utc")));
        out.answered = false;
    }
    out.state = match state {
        None => {
            if active == camp {
                "active".to_string()
            } else {
                "orphan".to_string()
            }
        }
        Some(s) if s == "active" && active != camp => "orphan".to_string(),
        Some(s) => s,
    };
    if !out.unknown_events.is_empty() {
        out.state = "unknown".to_string();
    }
    if ctx.checkout_is_sub() {
        out.sub_nodes.clear();
        out.reslice_pending.clear();
    }
    if !out.sub_nodes.is_empty() && ["active", "suspended", "resume_pending", "closed"].contains(&out.state.as_str()) {
        let mut sorted = echoed.clone();
        sorted.sort();
        out.sub_echoed = sorted;
        let wanted: Vec<String> = out.sub_nodes.iter().filter_map(text_of).collect();
        out.sub_convergence = Some(
            if wanted.iter().all(|n| echoed.contains(n)) { "converged" } else { "unconverged" }.to_string(),
        );
    }
    out
}

fn list_or_empty(value: Option<&PyValue>) -> Vec<PyValue> {
    match value {
        Some(v) if v.is_truthy() => match v.peel() {
            PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// `list_instances()`.
fn list_instances(ctx: &Ctx) -> Vec<PathBuf> {
    let root = ctx.campaigns();
    let Ok(read) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            !RESERVED_IDS.contains(&name.as_str())
        })
        .collect();
    out.sort();
    out
}

// ── writer target and guards ─────────────────────────────────────────────────────────────────

/// `_writer_target(camp_id)`: the instance to write, or `None` for `_bootstrap` (the no-op signal).
fn writer_target(ctx: &Ctx, camp_id: Option<&str>) -> R<Option<(PathBuf, String)>> {
    let camp = camp_id.map(str::to_string).filter(|c| !c.is_empty()).unwrap_or_else(|| ctx.active_id());
    if camp == BOOTSTRAP {
        return Ok(None);
    }
    let base = ctx.campaigns().join(&camp);
    if !is_dir(&base) {
        return refuse(format!("인스턴스가 없다: campaigns/{camp} — 먼저 --init 하라"));
    }
    Ok(Some((base, camp)))
}

fn no_fill(pairs: &[(&str, &str)]) -> R<()> {
    for (key, value) in pairs {
        if value.contains(FILL) {
            return refuse(format!("{key} 에 {FILL} 이 남아 있다 — 모르는 값을 그럴듯하게 채우지 않는다"));
        }
    }
    Ok(())
}

/// `guard_writer_ops`: progress writes are refused on a suspended or pending campaign, every write on an unknown one.
fn guard_writer_ops(ctx: &Ctx, base: &Path, ops: &[(&str, bool)]) -> R<()> {
    let name = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let fold = fold_lifecycle(ctx, base, Some(&name));
    if fold.state == "unknown" {
        let events = PyValue::List(fold.unknown_events.clone()).repr();
        return refuse(format!(
            "{name} 의 journey 에 이 버전이 모르는 수명 이벤트가 있다 {events} — 상태를 unknown 으로 두고 쓰기를 전부 거부한다. 브랜치 공통층을 동기화한 뒤 다시 하라(오배달 방지)"
        ));
    }
    let hit: Vec<String> = PROGRESS_OPS
        .iter()
        .filter(|k| ops.iter().any(|(n, on)| n == *k && *on))
        .map(|k| format!("--{}", k.replace('_', "-")))
        .collect();
    if !hit.is_empty() && (fold.state == "suspended" || fold.state == "resume_pending") {
        return refuse(format!(
            "{name} 은 {} 다 — 진행성 쓰기({})를 거부한다. 먼저 재개하라(`--resume-campaign`). 증거 정리(--evidence-*·--backfill-from-docs)와 읽기는 허용된다(정상 차단)",
            fold.state,
            hit.join(", ")
        ));
    }
    Ok(())
}

// ── --assigned-cells ─────────────────────────────────────────────────────────────────────────

fn op_assigned_cells(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let node = args.get("--assigned-cells").unwrap_or_default();
    let Some((base, _)) = writer_target(ctx, args.get("--campaign-id"))? else {
        return refuse("활성 캠페인이 없다(_bootstrap) — 배정을 물을 선언이 없다");
    };
    let cells = assigned_cells(&read_declaration(&base), Some(node));
    if cells.is_empty() {
        return refuse(format!("assignments[{}] 가 비었다 — 이 노드에 배정된 셀이 없다", py_repr(node)));
    }
    io.say(&cells.join(","));
    Ok(0)
}

// ── --phase-set ──────────────────────────────────────────────────────────────────────────────

fn op_phase_set(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let phase = args.get("--phase-set").unwrap_or_default().to_string();
    let Some((base, camp)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(0);
    };
    guard_writer_ops(ctx, &base, &[("phase_set", true)])?;
    let cell = args.get("--cell").map(str::to_string);
    let mut node = args.get("--node").map(str::to_string).filter(|n| !n.is_empty());
    if node.is_none() {
        if let Some(cell) = cell.as_deref().filter(|c| !c.is_empty()) {
            node = node_of_cell(&base, cell);
            if node.is_none() {
                return refuse(format!(
                    "셀 {} 이 assignments 에 없다 — 노드를 파생하지 못했다(이름이 곧 증거 연결)",
                    py_repr(cell)
                ));
            }
        }
    }
    let state = args.get("--state").map(str::to_string).filter(|s| !s.is_empty());
    let (Some(node), Some(state)) = (node, state) else {
        return refuse("--phase-set 은 --node(또는 배정된 --cell)와 --state 가 필요하다");
    };
    if phase == "publish" && state == "done" {
        // The owner enters the library here (a process of the wiki tool): not modelled.
        return Err(Stop::Fallback);
    }
    let path = write_phase(ctx, &base, args, &node, &phase, &state, cell.as_deref())?;
    io.say(&format!("[campaign_init] wrote({camp}): {}", ctx.rel(&path)));
    Ok(0)
}

fn write_phase(
    ctx: &Ctx,
    base: &Path,
    args: &Args,
    node: &str,
    phase: &str,
    state: &str,
    cell: Option<&str>,
) -> R<PathBuf> {
    let _ = ctx;
    if !PHASE_NAMES.contains(&phase) {
        return refuse(format!("phase 는 {} 중 하나여야 한다: {}", tuple_repr(&PHASE_NAMES), py_repr(phase)));
    }
    if !PHASE_STATES.contains(&state) {
        return refuse(format!("state 는 {} 중 하나여야 한다: {}", tuple_repr(&PHASE_STATES), py_repr(state)));
    }
    let ok = args.has("--proof-ok");
    let source = args.get("--proof-source").filter(|s| !s.is_empty());
    let predicate = args.get("--proof-predicate").filter(|s| !s.is_empty());
    if ok && source.is_none() {
        return refuse("proof.ok=true 에는 --proof-source 가 필수다 (proof 는 선언이 아니라 관측이다)");
    }
    no_fill(&[
        ("node", node),
        ("cell_id", cell.unwrap_or("")),
        ("predicate", predicate.unwrap_or("")),
        ("source", source.unwrap_or("")),
    ])?;
    let path = base.join("phases").join(node).join(format!("{phase}.status.json"));
    let existing = if path.is_file() { read_json(&path) } else { None };
    let mut doc = match existing.as_ref().and_then(dict_of) {
        Some(d) => d.clone(),
        None => {
            let mut d = PyDict::new();
            d.set("schema_version", PyValue::int(1));
            d
        }
    };
    doc.set("schema_version", PyValue::int(1));
    doc.set("node_id", py(node));
    doc.set("phase", py(phase));
    doc.set("state", py(state));
    doc.set("_state_enum", str_list(PHASE_STATES.iter().map(|s| s.to_string()).collect()));
    let mut proof = PyDict::new();
    proof.set("predicate", predicate.map(py).unwrap_or(PyValue::None));
    proof.set("ok", PyValue::Bool(ok));
    proof.set("source", source.map(py).unwrap_or(PyValue::None));
    doc.set("proof", PyValue::Dict(proof));
    if let Some(cell) = args.get("--cell") {
        doc.set("cell_id", py(cell));
    }
    if let Some(started) = args.get("--started-utc").filter(|s| !s.is_empty()) {
        doc.set("started_utc", py(started));
        if !truthy(doc.get("first_started_utc")) {
            doc.set("first_started_utc", py(started));
        }
    }
    if let Some(ended) = args.get("--ended-utc").filter(|s| !s.is_empty()) {
        doc.set("ended_utc", py(ended));
    }
    if let Some(author) = args.get("--authored-by").filter(|s| !s.is_empty()) {
        doc.set("authored_by", py(author));
    }
    for key in ["authored_by", "first_started_utc", "started_utc", "ended_utc", "cell_id"] {
        if !doc.contains_key(key) {
            doc.set(key, PyValue::None);
        }
    }
    write_json(&path, &PyValue::Dict(doc))?;
    Ok(path)
}

pub(crate) fn tuple_repr(items: &[&str]) -> String {
    let inner: Vec<String> = items.iter().map(|s| py_repr(s)).collect();
    if inner.len() == 1 {
        format!("({},)", inner[0])
    } else {
        format!("({})", inner.join(", "))
    }
}

// ── --cell-set ───────────────────────────────────────────────────────────────────────────────

/// `latest_grounding(base)`: the newest valid record under `grounding/`.
fn latest_grounding(base: &Path) -> Option<(PathBuf, PyDict)> {
    let dir = base.join("grounding");
    let read = std::fs::read_dir(&dir).ok()?;
    let mut names: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    names.sort();
    names.reverse();
    for path in names {
        if let Some(value) = read_json(&path) {
            if let Some(dict) = dict_of(&value) {
                return Some((path, dict.clone()));
            }
        }
    }
    None
}

fn chain_dict<'a>(dict: &'a PyDict, key: &str) -> Option<&'a PyDict> {
    get_dict(dict, key)
}

fn op_cell_set(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let cell = args.get("--cell-set").unwrap_or_default().to_string();
    let Some((base, camp)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(0);
    };
    guard_writer_ops(ctx, &base, &[("cell_set", true)])?;
    let Some(outcome) = args.get("--outcome").filter(|s| !s.is_empty()) else {
        return refuse("--cell-set 은 --outcome 이 필요하다");
    };
    let (cpath, jpath) = write_cell(ctx, &base, args, &cell, outcome)?;
    let mut wrote = vec![ctx.rel(&cpath)];
    if let Some(j) = jpath {
        wrote.push(ctx.rel(&j));
    }
    io.say(&format!("[campaign_init] wrote({camp}): {}", wrote.join(" ")));
    Ok(0)
}

fn write_cell(
    ctx: &Ctx,
    base: &Path,
    args: &Args,
    cell: &str,
    outcome: &str,
) -> R<(PathBuf, Option<PathBuf>)> {
    if !CELL_OUTCOMES.contains(&outcome) {
        return refuse(format!(
            "cell_outcome 은 {} 중 하나여야 한다: {}",
            tuple_repr(&CELL_OUTCOMES),
            py_repr(outcome)
        ));
    }
    let void_reason = args.get("--void-reason").filter(|s| !s.is_empty());
    let void_source = args.get("--void-reason-source").filter(|s| !s.is_empty());
    if void_reason.is_some() && void_source.is_none() {
        return refuse("--void-reason 에는 --void-reason-source 가 필수다(출처 없는 판정 ✗)");
    }
    let node = match args.get("--node") {
        Some(n) => Some(n.to_string()),
        None => node_of_cell(base, cell),
    };
    let version = args.get("--version");
    let model = args.get("--model");
    no_fill(&[
        ("cell", cell),
        ("node", node.as_deref().unwrap_or("")),
        ("version", version.unwrap_or("")),
        ("model", model.unwrap_or("")),
    ])?;
    let path = base.join("cells").join(cell).join("cell.status.json");
    let existing = if path.is_file() { read_json(&path) } else { None };
    let mut doc = match existing.as_ref().and_then(dict_of) {
        Some(d) => d.clone(),
        None => {
            let mut d = PyDict::new();
            d.set("schema_version", PyValue::int(1));
            d
        }
    };
    doc.set("schema_version", PyValue::int(1));
    doc.set("cell_id", py(cell));
    doc.set("cell_outcome", py(outcome));
    doc.set("_cell_outcome_enum", str_list(CELL_OUTCOMES.iter().map(|s| s.to_string()).collect()));
    for (key, val) in [("node_id", node.as_deref()), ("version", version), ("model", model)] {
        if let Some(v) = val {
            doc.set(key, py(v));
        }
        if !doc.contains_key(key) {
            doc.set(key, PyValue::None);
        }
    }
    let sweep_state = args.get("--sweep-state").filter(|s| !s.is_empty()).map(PathBuf::from);
    let mut measurement_source = args.get("--measurement-source").map(str::to_string);
    let mut decode_tps: Option<f64> = match args.get("--measurement-decode-tps") {
        Some(tps) => Some(parse_python_float(tps)?),
        None => None,
    };
    let mut gap: Option<String> = None;
    if args.get("--measurement-decode-tps").is_none() {
        if let Some(sweep) = &sweep_state {
            match sweep_measurement(ctx, sweep, cell) {
                (Some(value), why) => {
                    decode_tps = Some(value);
                    measurement_source = Some(why);
                }
                (None, why) => gap = Some(why),
            }
        }
    }
    let sweep_output = match &sweep_state {
        Some(sweep) => sweep_output(ctx, base, sweep, cell, node.as_deref())?,
        None => None,
    };
    if sweep_output.is_some() && outcome != "measured" {
        return refuse("measured sweep 출력 경로와 셀 outcome이 모순이다");
    }
    let mut meas = match doc.get("measurement").and_then(dict_of) {
        Some(d) => d.clone(),
        None => PyDict::new(),
    };
    if sweep_state.is_some() {
        for key in ["sweep_dir", "sweep_generated_utc", "sweep_config", "sweep_source"] {
            remove_key(&mut meas, key);
        }
        if let Some(output) = &sweep_output {
            for (k, v) in output.iter() {
                meas.insert(k.clone(), v.clone());
            }
        }
    }
    if let Some(number) = decode_tps {
        meas.set("decode_tps_conc1", PyValue::Float(number));
        meas.set("measurement_ok", PyValue::Bool(true));
    }
    if let Some(source) = &measurement_source {
        meas.set("source", py(source));
    }
    for (key, default) in [
        ("decode_tps_conc1", PyValue::None),
        ("measurement_ok", PyValue::Bool(false)),
        ("source", PyValue::None),
    ] {
        if !meas.contains_key(key) {
            meas.set(key, default);
        }
    }
    if truthy(meas.get("measurement_ok")) && !truthy(meas.get("source")) {
        return refuse("측정값을 적으려면 --measurement-source 가 필요하다(출처 표시)");
    }
    if let Some(why) = gap.filter(|g| !g.is_empty()) {
        meas.set("gap", py(&why));
    } else if outcome == "measured" && !truthy(meas.get("measurement_ok")) {
        meas.set(
            "gap",
            py("outcome=measured 인데 측정값이 도착하지 않았다 — --sweep-state 또는 --measurement-decode-tps 미제공(결손 기재)"),
        );
    } else {
        remove_key(&mut meas, "gap");
    }
    doc.set("measurement", PyValue::Dict(meas));

    // image axis (no sweep record in this shape)
    let mut img = match doc.get("image").and_then(dict_of) {
        Some(d) => d.clone(),
        None => PyDict::new(),
    };
    let image_tag = args.get("--image-tag");
    let image_digest = args.get("--image-digest").filter(|s| !s.is_empty());
    let (derived, why) = match &sweep_state {
        Some(sweep) => sweep_image(ctx, sweep, cell),
        None => (None, String::new()),
    };
    if let (Some(d), Some(declared)) = (&derived, image_digest) {
        if get_text(d, "digest").as_deref() != Some(declared) {
            return refuse(format!(
                "이미지 좌표가 모순이다 — 선언={declared} · sweep 파생={} ({}). 결손이 아니라 거짓이므로 기재하지 않는다. 어느 쪽이 이 셀을 실제로 돌린 이미지인지 먼저 가려라.",
                get_text(d, "digest").unwrap_or_default(),
                get_text(d, "source").unwrap_or_default()
            ));
        }
    }
    if let Some(d) = derived {
        for (k, v) in d.iter() {
            img.insert(k.clone(), v.clone());
        }
        remove_key(&mut img, "gap");
    } else if let Some(digest) = image_digest {
        img.set("digest", py(digest));
        img.set("tag", image_tag.map(py).unwrap_or(PyValue::None));
        img.set("source", py("campaign_init --image-digest(선언)"));
        remove_key(&mut img, "gap");
    } else {
        for (key, default) in [
            ("digest", PyValue::None),
            ("tag", image_tag.map(py).unwrap_or(PyValue::None)),
            ("source", PyValue::None),
        ] {
            if !img.contains_key(key) {
                img.set(key, default);
            }
        }
        let gap_text = if why.is_empty() {
            "이미지 digest 가 도착하지 않았다 — --sweep-state 도 --image-digest 도 없다. 셀 간 비교는 같은 digest 안에서만 성립하므로, 이 셀은 다른 셀과 나란히 놓을 수 없다(결손 기재).".to_string()
        } else {
            why
        };
        img.set("gap", py(&gap_text));
    }
    doc.set("image", PyValue::Dict(img));

    let prior = doc.get("provenance").and_then(dict_of).cloned().unwrap_or_default();
    let prior_mismatch = list_or_empty(doc.get("provenance_mismatch"));
    let (provenance, mismatch) =
        cell_provenance(ctx, base, cell, node.as_deref(), sweep_state.as_deref(), &prior, prior_mismatch)?;
    doc.set("provenance", PyValue::Dict(provenance));
    doc.set("provenance_mismatch", PyValue::List(mismatch));

    let mut grounding = PyDict::new();
    match latest_grounding(base) {
        Some((gpath, g)) => {
            let status = chain_dict(&g, "export")
                .and_then(|e| chain_dict(e, "resolution"))
                .and_then(|r| r.get("status").cloned())
                .unwrap_or(PyValue::None);
            let gap = chain_dict(&g, "attestation")
                .and_then(|a| a.get("grounding_gap").cloned())
                .unwrap_or(PyValue::None);
            grounding.set("status", status);
            grounding.set("source", py(&ctx.rel(&gpath)));
            grounding.set("gap", gap);
        }
        None => {
            let mut gap = PyDict::new();
            gap.set("status", py("absent"));
            gap.set("reason", py("그라운딩 기록이 없다(C1 미수행)"));
            grounding.set("status", py("absent"));
            grounding.set("source", PyValue::None);
            grounding.set("gap", PyValue::Dict(gap));
        }
    }
    doc.set("grounding", PyValue::Dict(grounding));
    doc.set("void_reason", void_reason.map(py).unwrap_or(PyValue::None));
    doc.set("void_reason_source", void_source.map(py).unwrap_or(PyValue::None));
    let mut lite_verdict: Option<String> = None;
    let lite_raw = args.get("--lite-raw").filter(|s| !s.is_empty());
    if let Some(raw) = lite_raw {
        lite_verdict = apply_lite_gate(ctx, &mut doc, raw, args.get("--utc"))?;
    }
    let axis = args.get("--axis-citation");
    let intent = args.get("--next-intent");
    if let Some(a) = axis {
        doc.set("axis_citation", py(a));
    }
    if let Some(n) = intent {
        doc.set("next_intent", py(n));
    }
    write_json(&path, &PyValue::Dict(doc))?;

    let mut journey = None;
    if intent.map(|n| !n.is_empty()).unwrap_or(false) {
        let mut entry = PyDict::new();
        entry.set("utc", args.get("--utc").map(py).unwrap_or(PyValue::None));
        entry.set("node_id", node.as_deref().map(py).unwrap_or(PyValue::None));
        entry.set("cell_id", py(cell));
        entry.set("outcome", py(outcome));
        entry.set("axis_citation", axis.map(py).unwrap_or(PyValue::None));
        entry.set("next_intent", intent.map(py).unwrap_or(PyValue::None));
        entry.set("source", py("campaign_init --cell-set"));
        if lite_raw.is_some() {
            entry.set("lite_verdict", lite_verdict.as_deref().map(py).unwrap_or(PyValue::None));
        }
        journey = Some(append_journey(base, &PyValue::Dict(entry))?);
    }
    Ok((path, journey))
}

fn remove_key(dict: &mut PyDict, key: &str) {
    let kept: Vec<(PyStr, PyValue)> = dict.iter().filter(|(k, _)| !k.equals(key)).cloned().collect();
    if kept.len() != dict.len() {
        let mut next = PyDict::new();
        for (k, v) in kept {
            next.insert(k, v);
        }
        *dict = next;
    }
}

/// Python `float(text)` for the spellings Rust parses identically; anything else is the owner's refusal.
fn parse_python_float(text: &str) -> R<f64> {
    if text.contains('_') {
        return Err(Stop::Fallback);
    }
    match text.trim().parse::<f64>() {
        Ok(n) => Ok(n),
        Err(_) => refuse(format!("--measurement-decode-tps 가 수가 아니다: {}", py_repr(text))),
    }
}

/// `_cell_provenance` without a sweep record: what the declaration and the lockset say.
fn cell_provenance(
    ctx: &Ctx,
    base: &Path,
    cell: &str,
    node: Option<&str>,
    sweep_state: Option<&Path>,
    prior: &PyDict,
    prior_mismatch: Vec<PyValue>,
) -> R<(PyDict, Vec<PyValue>)> {
    let decl = read_declaration(base);
    let mut prov = PyDict::new();
    if !lockset_observable(&decl, node) {
        prov.set("observability", py("not_observable"));
        prov.set(
            "observability_reason",
            py(&format!(
                "서브 배정 셀(node={}) — lockset 은 서브 인스턴스가 저작하고 precheck 는 측정 스킬이 tool_plane 으로 열린 서브의 진입에서 집행한다(node_role_contract — tool_plane 이 비는 서브에는 이 precheck 의 집행자가 없고, 서브 배정은 토폴로지 오케스트레이션 전략이 정한다). 메인은 서브 인스턴스를 읽지 않으며 브리핑은 lockset 출처를 싣지 않는다(헌법 노드제어 ①)",
                node.unwrap_or("None")
            )),
        );
        prov.set("lockset", PyValue::None);
        prov.set("lockset_source", PyValue::None);
        prov.set("lockset_gap", PyValue::None);
        prov.set("compared", PyValue::List(Vec::new()));
        prov.set("compare_gaps", PyValue::List(Vec::new()));
        prov.set("pending_knobs", PyValue::List(Vec::new()));
        return Ok((prov, prior_mismatch));
    }
    let lock = base.join("cells").join(cell).join("lockset.json");
    let ldoc = if lock.is_file() { read_json(&lock) } else { None };
    let ldoc = ldoc.as_ref().and_then(dict_of).cloned().unwrap_or_default();
    prov.set("observability", py("observed"));
    prov.set("observability_reason", PyValue::None);
    prov.set(
        "lockset",
        ldoc.get("provenance").filter(|v| text_of(v).is_some()).cloned().unwrap_or(PyValue::None),
    );
    prov.set("lockset_source", if lock.is_file() { py(&ctx.rel(&lock)) } else { PyValue::None });
    prov.set(
        "lockset_gap",
        lockset_provenance_reason(ctx, &lock).map(|s| py(&s)).unwrap_or(PyValue::None),
    );
    prov.set(
        "lockset_trial_provenance",
        ldoc.get("trial_provenance").filter(|v| text_of(v).is_some()).cloned().unwrap_or(PyValue::None),
    );
    prov.set("lockset_stamp", lockset_stamp(&ldoc));
    prov.set("compared", PyValue::List(list_or_empty(prior.get("compared"))));
    prov.set("compare_gaps", PyValue::List(list_or_empty(prior.get("compare_gaps"))));
    prov.set("pending_knobs", PyValue::List(Vec::new()));
    let Some(sweep) = sweep_state else {
        return Ok((prov, prior_mismatch));
    };
    compare_with_sweep(ctx, base, cell, sweep, &ldoc, prov, prior_mismatch)
}

/// `lockset_provenance` text of a lockset field (`lockset_source` is the repository-relative path or null).
fn prov_text(prov: &PyDict, key: &str) -> PyValue {
    prov.get(key).cloned().unwrap_or(PyValue::None)
}

fn as_number(value: &PyValue) -> Option<f64> {
    match value.peel() {
        PyValue::Bool(_) => None,
        PyValue::Int(n) => {
            use num_traits::ToPrimitive;
            n.to_f64()
        }
        PyValue::Float(f) => Some(*f),
        PyValue::Str(text) => {
            let text = text.to_utf8()?;
            let trimmed = text.trim();
            if trimmed.contains('_') {
                return None;
            }
            trimmed.parse::<f64>().ok()
        }
        _ => None,
    }
}

/// `_sweep_coordinate(sweep_path, cell, key, block)`: `(value, source or the reason it is absent)`.
fn sweep_coordinate(ctx: &Ctx, sweep: &Path, cell: &str, key: &str, block: &str) -> (Option<PyValue>, String) {
    let Some(doc) = read_json(sweep) else {
        return (None, format!("sweep 기록을 읽지 못했다: {}", ctx.rel(sweep)));
    };
    let Some(doc) = dict_of(&doc) else {
        return (None, format!("sweep 기록을 읽지 못했다: {}", ctx.rel(sweep)));
    };
    for rec in list_or_empty(doc.get("cells")).iter().filter_map(dict_of) {
        if get_text(rec, "cell_key").as_deref() != Some(cell) {
            continue;
        }
        let coord = get_dict(rec, block).cloned().unwrap_or_default();
        let absent = match coord.get(key) {
            None => true,
            Some(v) => match v.peel() {
                PyValue::None => true,
                PyValue::Str(t) => t.equals("") || t.equals("NA"),
                _ => false,
            },
        };
        if absent {
            return (
                None,
                format!(
                    "sweep 레코드에 {block}.{key} 가 없다: {} cell_key={cell} (서빙 yaml 에 그 키가 없었거나 측정 평면이 싣지 않았다)",
                    ctx.rel(sweep)
                ),
            );
        }
        return (
            coord.get(key).cloned(),
            format!("sweep:{}#cells[cell_key={cell}].{block}.{key}", ctx.rel(sweep)),
        );
    }
    (None, format!("sweep 기록에 셀 {} 이 없다: {}", py_repr(cell), ctx.rel(sweep)))
}

/// `_declared_target_gmu(cfg_path)`: `(value, source or the reason it is absent)`.
fn declared_target_gmu(ctx: &Ctx, cfg: &Path) -> R<(Option<f64>, String)> {
    if !cfg.is_file() {
        return Ok((None, format!("셀 config 부재: {}", ctx.rel(cfg))));
    }
    let loaded = std::fs::read(cfg)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .map(|text| crate::pyyaml::safe_load_text(&PyStr::new(&text), 0));
    let doc = match loaded {
        Some(Ok(doc)) => doc,
        // A YAML error message quotes the source line (a snippet of the text); that rendering is the owner's.
        Some(Err(_)) | None => return Err(Stop::Fallback),
    };
    let value = dict_of(&doc)
        .and_then(|d| get_dict(d, "target_gpu"))
        .and_then(|t| t.get("target_gmu"))
        .and_then(as_number);
    Ok(match value {
        Some(v) => (Some(v), format!("{}#target_gpu.target_gmu", ctx.rel(cfg))),
        None => (
            None,
            format!("셀 config 에 target_gpu.target_gmu 가 수로 선언되지 않았다: {}", ctx.rel(cfg)),
        ),
    })
}

fn number_or_none(v: Option<f64>) -> PyValue {
    v.map(PyValue::Float).unwrap_or(PyValue::None)
}

fn compare_with_sweep(
    ctx: &Ctx,
    base: &Path,
    cell: &str,
    sweep: &Path,
    ldoc: &PyDict,
    mut prov: PyDict,
    prior_mismatch: Vec<PyValue>,
) -> R<(PyDict, Vec<PyValue>)> {
    let mut fresh: Vec<PyValue> = Vec::new();
    let mut compared: Vec<String> = Vec::new();
    let mut gaps: Vec<String> = Vec::new();
    let lockset_prov = prov_text(&prov, "lockset");
    let lockset_source = text_of(&prov_text(&prov, "lockset_source"));
    let (declared, dsrc) = declared_target_gmu(ctx, &base.join("cells").join(cell).join("config.yaml"))?;
    let (observed_raw, osrc) = sweep_coordinate(ctx, sweep, cell, "gpu_memory_utilization", "coordinates");
    let observed = observed_raw.as_ref().and_then(as_number);
    if declared.is_none() {
        gaps.push(format!("gpu_memory_utilization 대조 불가 — {dsrc} (직전 기재 유지)"));
    } else if observed.is_none() {
        let why = match &observed_raw {
            None => osrc.clone(),
            Some(v) => format!("서빙 gmu 가 수가 아니다: {} ({osrc})", v.repr()),
        };
        gaps.push(format!("gpu_memory_utilization 대조 불가 — {why} (직전 기재 유지)"));
    } else {
        compared.push("gpu_memory_utilization".to_string());
        if declared != observed {
            let mut m = PyDict::new();
            m.set("knob", py("gpu_memory_utilization"));
            m.set("declared", number_or_none(declared));
            m.set("declared_source", py(&dsrc));
            m.set("observed", number_or_none(observed));
            m.set("observed_source", py(&osrc));
            m.set("lockset_provenance", lockset_prov.clone());
            m.set("lockset_gmu_source", ldoc.get("gmu_source").cloned().unwrap_or(PyValue::None));
            fresh.push(PyValue::Dict(m));
        }
    }
    for (lkey, block, skey, src_key) in [
        ("batch", "coordinates", "max_num_seqs", "batch_source"),
        ("kv_cache_memory_bytes", "capacity", "kv_cache_memory_bytes", "kv_source"),
    ] {
        let declared_k = ldoc.get(lkey).and_then(as_number);
        let (observed_raw_k, osrc_k) = sweep_coordinate(ctx, sweep, cell, skey, block);
        let observed_k = observed_raw_k.as_ref().and_then(as_number);
        let Some(observed_k) = observed_k else {
            let why = match &observed_raw_k {
                None => osrc_k.clone(),
                Some(v) => format!("서빙 값이 수가 아니다: {} ({osrc_k})", v.repr()),
            };
            let tail = if declared_k.is_some() {
                String::new()
            } else {
                format!(
                    " · lockset {lkey} 도 수로 잠겨 있지 않다({})",
                    lockset_source.clone().unwrap_or_else(|| "lockset 부재".to_string())
                )
            };
            gaps.push(format!("{lkey} 대조 불가 — {why}{tail} (직전 기재 유지)"));
            continue;
        };
        compared.push(lkey.to_string());
        let source_key = format!("lockset_{src_key}");
        match declared_k {
            None => {
                let mut m = PyDict::new();
                m.set("knob", py(lkey));
                m.set("declared", PyValue::None);
                m.set("declared_state", py("undetermined"));
                m.set(
                    "declared_source",
                    match &lockset_source {
                        Some(src) => py(&format!("{src}#{lkey}")),
                        None => py("lockset 부재"),
                    },
                );
                m.set("observed", PyValue::Float(observed_k));
                m.set("observed_source", py(&osrc_k));
                m.set("lockset_provenance", lockset_prov.clone());
                m.set(&source_key, ldoc.get(src_key).cloned().unwrap_or(PyValue::None));
                fresh.push(PyValue::Dict(m));
            }
            Some(d) if d != observed_k => {
                let mut m = PyDict::new();
                m.set("knob", py(lkey));
                m.set("declared", PyValue::Float(d));
                m.set(
                    "declared_source",
                    py(&format!("{}#{lkey}", lockset_source.clone().unwrap_or_else(|| "None".to_string()))),
                );
                m.set("observed", PyValue::Float(observed_k));
                m.set("observed_source", py(&osrc_k));
                m.set("lockset_provenance", lockset_prov.clone());
                m.set(&source_key, ldoc.get(src_key).cloned().unwrap_or(PyValue::None));
                fresh.push(PyValue::Dict(m));
            }
            Some(_) => {}
        }
    }
    if get_text(ldoc, "batch_source").as_deref() == Some("hand-lever") {
        let der = get_dict(ldoc, "batch_derivation").cloned().unwrap_or_default();
        let hand = ldoc.get("batch").and_then(as_number);
        let would = der.get("derived_batch_would_be").and_then(as_number);
        match (hand, would) {
            (Some(hand), Some(would)) => {
                compared.push("batch_hand_lever".to_string());
                if hand != would {
                    let src = lockset_source.clone().unwrap_or_else(|| "None".to_string());
                    let mut m = PyDict::new();
                    m.set("knob", py("batch_hand_lever"));
                    m.set("declared", PyValue::Float(hand));
                    m.set("declared_source", py(&format!("{src}#batch(batch_source=hand-lever)")));
                    m.set("observed", PyValue::Float(would));
                    m.set(
                        "observed_source",
                        py(&format!("{src}#batch_derivation.derived_batch_would_be")),
                    );
                    m.set(
                        "observed_source_kind",
                        der.get("derived_batch_source_would_be").cloned().unwrap_or(PyValue::None),
                    );
                    m.set("lockset_provenance", lockset_prov.clone());
                    fresh.push(PyValue::Dict(m));
                }
            }
            _ => gaps.push(
                "batch_hand_lever 대조 불가 — lockset 손레버 batch 또는 explorer 재계산값(batch_derivation.derived_batch_would_be)이 수가 아니다 (직전 기재 유지)".to_string(),
            ),
        }
    }
    prov.set("compared", str_list(compared.clone()));
    prov.set("compare_gaps", str_list(gaps));
    let mut kept: Vec<PyValue> = prior_mismatch
        .into_iter()
        .filter(|m| match dict_of(m) {
            Some(d) => !get_text(d, "knob").map(|k| compared.contains(&k)).unwrap_or(false),
            None => false,
        })
        .collect();
    kept.extend(fresh);
    Ok((prov, kept))
}

/// `_sweep_measurement(sweep_path, cell)`.
fn sweep_measurement(ctx: &Ctx, sweep: &Path, cell: &str) -> (Option<f64>, String) {
    let doc = read_json(sweep);
    let Some(doc) = doc.as_ref().and_then(dict_of) else {
        return (None, format!("sweep 기록을 읽지 못했다: {}", ctx.rel(sweep)));
    };
    for rec in list_or_empty(doc.get("cells")).iter().filter_map(dict_of) {
        if get_text(rec, "cell_key").as_deref() != Some(cell) {
            continue;
        }
        let value = get_dict(rec, "concurrency_vector").and_then(|v| v.get("1"));
        let number = match value.map(PyValue::peel) {
            Some(PyValue::Int(_)) | Some(PyValue::Float(_)) => value.and_then(as_number),
            _ => None,
        };
        if let Some(number) = number {
            return (
                Some(number),
                format!("sweep:{}#cells[cell_key={cell}].concurrency_vector.1", ctx.rel(sweep)),
            );
        }
        return (None, format!("sweep 레코드에 동시성 1 측정이 없다: {} cell_key={cell}", ctx.rel(sweep)));
    }
    (None, format!("sweep 기록에 셀 {} 이 없다: {}", py_repr(cell), ctx.rel(sweep)))
}

/// `_sweep_image(sweep_path, cell)`.
fn sweep_image(ctx: &Ctx, sweep: &Path, cell: &str) -> (Option<PyDict>, String) {
    let doc = read_json(sweep);
    let Some(doc) = doc.as_ref().and_then(dict_of) else {
        return (None, format!("sweep 기록을 읽지 못했다: {}", ctx.rel(sweep)));
    };
    for rec in list_or_empty(doc.get("cells")).iter().filter_map(dict_of) {
        if get_text(rec, "cell_key").as_deref() != Some(cell) {
            continue;
        }
        let coord = get_dict(rec, "coordinates").cloned().unwrap_or_default();
        let digest = coord.get("image_digest").filter(|v| v.is_truthy());
        let Some(digest) = digest else {
            return (
                None,
                format!(
                    "sweep 레코드에 image_digest 가 없다: {} cell_key={cell} (측정 평면이 그 좌표를 싣지 않았다)",
                    ctx.rel(sweep)
                ),
            );
        };
        let tag = coord.get("image_tag").filter(|v| v.is_truthy());
        let mut out = PyDict::new();
        out.set("digest", py(&digest.display_text().to_utf8().unwrap_or_default()));
        out.set(
            "tag",
            tag.map(|t| py(&t.display_text().to_utf8().unwrap_or_default())).unwrap_or(PyValue::None),
        );
        out.set(
            "source",
            py(&format!("sweep:{}#cells[cell_key={cell}].coordinates.image_digest", ctx.rel(sweep))),
        );
        return (Some(out), String::new());
    }
    (None, format!("sweep 기록에 셀 {} 이 없다: {}", py_repr(cell), ctx.rel(sweep)))
}

/// `_sweep_output(base, sweep_path, cell, node)`: the measured output path the producer carried.
fn sweep_output(ctx: &Ctx, base: &Path, sweep: &Path, cell: &str, node: Option<&str>) -> R<Option<PyDict>> {
    let state = read_json(sweep);
    let records = state.as_ref().and_then(dict_of).map(|d| list_or_empty(d.get("cells"))).unwrap_or_default();
    let hits: Vec<&PyDict> = records
        .iter()
        .filter_map(dict_of)
        .filter(|r| get_text(r, "cell_key").as_deref() == Some(cell))
        .collect();
    if hits.is_empty() || hits.iter().all(|r| !r.contains_key("sweep_dir")) {
        return Ok(None);
    }
    if hits.len() != 1 {
        return refuse(format!("sweep 출력 경로의 셀 기록이 유일하지 않다: {cell}"));
    }
    let rec = hits[0];
    let rel_value = rec.get("sweep_dir").filter(|v| !matches!(v.peel(), PyValue::None));
    let stamp_value = rec.get("sweep_generated_utc").filter(|v| !matches!(v.peel(), PyValue::None));
    if rel_value.is_none() && stamp_value.is_none() {
        return Ok(None);
    }
    let rel = rel_value.and_then(text_of);
    let stamp = stamp_value.and_then(text_of).filter(|s| !s.is_empty());
    let (Some(rel), Some(stamp)) = (rel, stamp) else {
        return refuse("sweep 출력 경로는 measured 셀의 실제 경로·generated_utc 짝이어야 한다");
    };
    if get_text(rec, "cell_outcome").as_deref() != Some("measured") {
        return refuse("sweep 출력 경로는 measured 셀의 실제 경로·generated_utc 짝이어야 한다");
    }
    let decl = read_declaration(base);
    let from_node = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .find(|n| get_text(n, "node_id").as_deref() == node && node.is_some())
        .and_then(|n| get_text(n, "topology"))
        .filter(|t| !t.is_empty());
    let topology = from_node.or_else(|| declared_topology(base));
    let absolute = rel.starts_with('/');
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let name_ok = |name: &str| {
        let Some(rest) = name.strip_prefix("sweep_") else { return false };
        let mut chars = rest.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    };
    let topo_ok = matches!(topology.as_deref(), Some("single" | "multi"));
    if !topo_ok
        || absolute
        || parts.contains(&"..")
        || parts.len() != 4
        || parts[0] != "output"
        || Some(parts[1]) != topology.as_deref()
        || parts[2] != "benchlog"
        || !name_ok(parts[3])
    {
        return refuse(format!("sweep 출력 경로는 이 topology의 benchlog 아래여야 한다: {}", py_repr(&rel)));
    }
    let topology = topology.expect("checked");
    let path = ctx.root.join(&rel);
    let bench_root = ctx.root.join("output").join(&topology).join("benchlog");
    let mut inside: Vec<&Path> = vec![path.as_path()];
    inside.extend(path.ancestors().skip(1));
    if inside.iter().any(|p| *p != ctx.root && p.starts_with(&ctx.root) && p.is_symlink()) {
        return refuse(format!("sweep 출력 경로에 symlink가 있다: {rel}"));
    }
    let resolved = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let resolved_root = std::fs::canonicalize(&bench_root).unwrap_or_else(|_| bench_root.clone());
    if !resolved.starts_with(&resolved_root) {
        return refuse(format!("sweep 출력 경로가 benchlog 밖이다: {rel}"));
    }
    let index_path = path.join("sweep_index.json");
    if index_path.is_symlink() || !index_path.is_file() {
        return refuse("sweep 출력 index는 실재 정규파일이어야 한다(symlink 불가)");
    }
    let index = read_json(&index_path);
    let index = index.as_ref().and_then(dict_of);
    let meta = index.and_then(|i| get_dict(i, "meta")).cloned().unwrap_or_default();
    let config = rec.get("config").and_then(text_of).filter(|c| !c.is_empty());
    let consistent = match (index, &config) {
        (Some(index), Some(config)) => {
            let declared = meta
                .get("config_name")
                .filter(|v| v.is_truthy())
                .cloned()
                .or_else(|| index.get("config").cloned())
                .unwrap_or(PyValue::None);
            text_of(&declared).as_deref() == Some(config.as_str())
                && get_text(index, "generated_utc").as_deref() == Some(stamp.as_str())
                && match meta.get("topology") {
                    None => true,
                    Some(v) => matches!(v.peel(), PyValue::None) || text_of(v).as_deref() == Some(topology.as_str()),
                }
        }
        _ => false,
    };
    if !consistent {
        return refuse("sweep 출력 index의 config/UTC가 공식 셀 기록과 일치하지 않는다");
    }
    let mut out = PyDict::new();
    out.set("sweep_dir", py(&rel));
    out.set("sweep_generated_utc", py(&stamp));
    out.set("sweep_config", py(&config.expect("checked")));
    out.set("sweep_source", py(&format!("sweep:{}#cells[cell_key={cell}].sweep_dir", ctx.rel(sweep))));
    Ok(Some(out))
}

const REENTRY_ROUTES: [(&str, &str); 2] = [
    ("explorer", "vllm-recipe-explorer(서빙전략 재수립)"),
    ("upstream", "upstream-version-watch(재빌드)"),
];

/// `_read_lite_raw(raw_path)`: `(verdict, measured_utc, source)`.
fn read_lite_raw(ctx: &Ctx, raw_path: &str) -> R<(Option<String>, PyValue, String)> {
    let raw = read_json(Path::new(raw_path));
    let Some(raw_value) = raw.filter(|v| dict_of(v).is_some()) else {
        return refuse(format!("--lite-raw 를 JSON 객체로 읽을 수 없다: {raw_path}"));
    };
    let measured = dict_of(&raw_value).and_then(|d| d.get("measured_utc").cloned()).unwrap_or(PyValue::None);
    let owner = ctx.root.join(".claude/skills/adversarial-benchmark/scripts/lite_metrics_native.py");
    if !owner.is_file() {
        return Ok((
            None,
            measured,
            format!("not_evaluated(판정 읽기 소유자 부재: {})", owner.display()),
        ));
    }
    let resolved = std::fs::canonicalize(raw_path).unwrap_or_else(|_| PathBuf::from(raw_path));
    let verdict = read_lite_verdict(&raw_value).map(|v| v.label().to_string());
    Ok((
        verdict,
        measured,
        format!("lite_raw({}) · lite_metrics.read_lite_verdict", ctx.rel(&resolved)),
    ))
}

/// `_apply_lite_gate(doc, lite_raw, utc)`: the lite verdict, with the charge and the re-entry proposal of a `server_failed`.
fn apply_lite_gate(ctx: &Ctx, doc: &mut PyDict, raw: &str, utc: Option<&str>) -> R<Option<String>> {
    let (verdict, measured, source) = read_lite_raw(ctx, raw)?;
    let mut gate = PyDict::new();
    gate.set("verdict", verdict.as_deref().map(py).unwrap_or(PyValue::None));
    gate.set("measured_utc", measured.clone());
    gate.set("source", py(&source));
    doc.set("lite_gate", PyValue::Dict(gate));
    if verdict.as_deref() != Some("server_failed") {
        return Ok(verdict);
    }
    let rec = doc.get("reconciliation").and_then(dict_of).cloned().unwrap_or_default();
    let mut charges: Vec<PyValue> = list_or_empty(rec.get("charges")).into_iter().filter(|c| dict_of(c).is_some()).collect();
    let key = if measured.is_truthy() { measured.clone() } else { py(&source) };
    let fresh = !charges.iter().any(|c| dict_of(c).and_then(|d| d.get("key")) == Some(&key));
    if fresh {
        let mut c = PyDict::new();
        c.set("key", key.clone());
        c.set("utc", utc.map(py).unwrap_or(PyValue::None));
        c.set("reason", py("lite_verdict=server_failed(실사용 불가)"));
        c.set("source", py(&source));
        charges.push(PyValue::Dict(c));
    }
    let mut reconciliation = PyDict::new();
    let spent = charges.len() as i64;
    reconciliation.set("charges", PyValue::List(charges));
    reconciliation.set("spent", PyValue::int(spent));
    reconciliation.set(
        "cap_owner",
        py("vllm-recipe-explorer recipe.py --cap — 잔여 = cap − spent(cap 값은 여기에 적지 않는다)"),
    );
    doc.set("reconciliation", PyValue::Dict(reconciliation));
    let prior = doc.get("reentry").and_then(dict_of).is_some();
    if fresh || !prior {
        let mut r = PyDict::new();
        r.set("status", py("proposed"));
        r.set("reason", py("lite_verdict=server_failed"));
        r.set("charge_key", key);
        r.set("proposed_utc", utc.map(py).unwrap_or(PyValue::None));
        r.set("routes", str_list(REENTRY_ROUTES.iter().map(|(_, v)| v.to_string()).collect()));
        r.set("decision", PyValue::None);
        r.set("gate", py("사람 승인(대화 평면) → campaign_init --reentry-decide 로 기록 · 자동 실행 ✗"));
        doc.set("reentry", PyValue::Dict(r));
    }
    Ok(verdict)
}

// ── --write-brief ────────────────────────────────────────────────────────────────────────────

fn campaign_brief(ctx: &Ctx, base: &Path, node: &str, utc: &str) -> PyValue {
    let decl = read_declaration(base);
    let cells_dir = base.join("cells");
    let mut assigned = assigned_cells(&decl, Some(node));
    if assigned.is_empty() {
        assigned = assigned_cells(&decl, None);
    }
    let mut others: Vec<String> = Vec::new();
    for (nid, _) in assignments_of(&decl) {
        if nid != node {
            others.extend(assigned_cells(&decl, Some(&nid)));
        }
    }
    let mut on_disk: Vec<String> = match std::fs::read_dir(&cells_dir) {
        Ok(read) => read
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "_cell")
            .collect(),
        Err(_) => Vec::new(),
    };
    on_disk.sort();
    let mut listed = assigned.clone();
    for cell in on_disk {
        if !assigned.contains(&cell) && !others.contains(&cell) {
            listed.push(cell);
        }
    }
    let mut stamps: Vec<String> = Vec::new();
    let mut cells = Vec::new();
    let mut pending = Vec::new();
    for cell in &listed {
        let st = read_json(&cells_dir.join(cell).join("cell.status.json"));
        let st = st.as_ref().and_then(dict_of).cloned().unwrap_or_default();
        let meas = get_dict(&st, "measurement").cloned().unwrap_or_default();
        let outcome = st.get("cell_outcome").cloned().unwrap_or(PyValue::None);
        let mut c = PyDict::new();
        c.set("cell_id", py(cell));
        c.set("mode", py(&cell_mode(&decl, node, cell)));
        c.set("cell_outcome", outcome.clone());
        c.set("decode_tps_conc1", meas.get("decode_tps_conc1").cloned().unwrap_or(PyValue::None));
        c.set("measurement_gap", meas.get("gap").cloned().unwrap_or(PyValue::None));
        c.set("axis_citation", st.get("axis_citation").cloned().unwrap_or(PyValue::None));
        c.set("void_reason", st.get("void_reason").cloned().unwrap_or(PyValue::None));
        let is_pending = match text_of(&outcome) {
            _ if matches!(outcome.peel(), PyValue::None) => true,
            Some(o) => CELL_OUTCOMES_PENDING.contains(&o.as_str()),
            None => false,
        };
        if is_pending {
            pending.push(py(cell));
        }
        cells.push(PyValue::Dict(c));
    }
    let mut phases = PyDict::new();
    for phase in PHASE_NAMES {
        let st = read_json(&base.join("phases").join(node).join(format!("{phase}.status.json")));
        let Some(st) = st.as_ref().and_then(dict_of) else {
            phases.set(phase, PyValue::None);
            continue;
        };
        let proof = get_dict(st, "proof").cloned().unwrap_or_default();
        let mut p = PyDict::new();
        p.set("state", st.get("state").cloned().unwrap_or(PyValue::None));
        p.set("proof_ok", PyValue::Bool(truthy(proof.get("ok"))));
        p.set("proof_source", proof.get("source").cloned().unwrap_or(PyValue::None));
        p.set("first_started_utc", st.get("first_started_utc").cloned().unwrap_or(PyValue::None));
        p.set("started_utc", st.get("started_utc").cloned().unwrap_or(PyValue::None));
        p.set("ended_utc", st.get("ended_utc").cloned().unwrap_or(PyValue::None));
        for key in ["started_utc", "ended_utc"] {
            if let Some(t) = st.get(key).and_then(text_of) {
                stamps.push(t);
            }
        }
        phases.set(phase, PyValue::Dict(p));
    }
    let journey = read_journey(base);
    for j in &journey {
        if let Some(t) = dict_of(j).and_then(|d| d.get("utc")).and_then(text_of) {
            stamps.push(t);
        }
    }
    let tail: Vec<PyValue> = journey.iter().rev().take(3).rev().cloned().collect();
    let ep = read_json(&base.join("evidence_pointers.json"));
    let pointers = ep
        .as_ref()
        .and_then(dict_of)
        .and_then(|d| d.get("pointers"))
        .and_then(list_of)
        .cloned();
    let claims: Vec<PyValue> = pointers
        .iter()
        .flatten()
        .filter(|p| dict_of(p).map(|d| get_text(d, "node_id").as_deref() == Some(node)).unwrap_or(false))
        .cloned()
        .collect();
    let self_role = match decl.get("self_role") {
        Some(v) if v.is_truthy() => v.clone(),
        _ => list_of(decl.get("nodes").unwrap_or(&PyValue::None))
            .into_iter()
            .flatten()
            .filter_map(dict_of)
            .find(|n| get_text(n, "node_id").as_deref() == Some(node))
            .and_then(|n| n.get("role").cloned())
            .unwrap_or(PyValue::None),
    };
    let fold = fold_lifecycle(ctx, base, None);
    let mut life = PyDict::new();
    life.set("state", py(&fold.state));
    life.set("close_reason", fold.close_reason.as_deref().map(py).unwrap_or(PyValue::None));
    life.set("parent", fold.parent.clone());
    life.set("last_transition_event", fold.last_transition_event.clone().unwrap_or(PyValue::None));
    life.set("interrupted_cells", str_list(fold.interrupted_cells.clone()));
    stamps.sort();
    let mut doc = PyDict::new();
    doc.set("schema_version", PyValue::int(1));
    doc.set("generated_utc", py(utc));
    doc.set(
        "campaign_id",
        py(&base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
    );
    doc.set("node_id", py(node));
    doc.set("self_role", self_role);
    doc.set("authored_by", py(node));
    doc.set("plan_ref", decl.get("plan_ref").cloned().unwrap_or(PyValue::None));
    doc.set("cells", PyValue::List(cells));
    doc.set("pending_cells", PyValue::List(pending));
    doc.set("phases", PyValue::Dict(phases));
    doc.set("journey_tail", PyValue::List(tail));
    doc.set("lifecycle", PyValue::Dict(life));
    doc.set("evidence_pointer_count", PyValue::int(pointers.as_ref().map(Vec::len).unwrap_or(0) as i64));
    doc.set("evidence_pointers", PyValue::List(claims));
    doc.set("last_utc", stamps.last().map(|s| py(s)).unwrap_or(PyValue::None));
    PyValue::Dict(doc)
}

fn op_write_brief(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let (Some(node), Some(utc)) = (
        args.get("--node").filter(|s| !s.is_empty()),
        args.get("--utc").filter(|s| !s.is_empty()),
    ) else {
        return refuse("--write-brief 는 --node 와 --utc 가 필요하다");
    };
    let Some((base, _)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에는 진행이 없다)");
        return Ok(0);
    };
    let path = ctx.root.join("docs").join("logs").join(node).join("campaign_brief.json");
    write_json(&path, &campaign_brief(ctx, &base, node, utc))?;
    io.say(&format!("[campaign_init] brief → {}", ctx.rel(&path)));
    Ok(0)
}

// ── evidence pointers, revisions ────────────────────────────────────────────────────────────

const EVIDENCE_KINDS: [&str; 8] = [
    "certificate", "bench_report", "sweep_map", "plan", "testlog", "devlog", "simlog", "relay_summary",
];

/// The shared start of the writer operations that are not progress writes: the `_bootstrap` no-op and the lifecycle guard.
fn writer_prelude(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<Option<(PathBuf, String)>> {
    let Some(target) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(None);
    };
    guard_writer_ops(ctx, &target.0, &[])?;
    Ok(Some(target))
}

fn wrote_line(io: &mut Sinks<'_>, camp: &str, paths: &[String]) {
    io.say(&format!("[campaign_init] wrote({camp}): {}", paths.join(" ")));
}

fn load_pointer_doc(ctx: &Ctx, ep: &Path, missing: &str) -> R<PyDict> {
    let _ = ctx;
    match read_json(ep) {
        Some(v) => match v.peel() {
            PyValue::Dict(d) => Ok(d.clone()),
            _ => refuse(missing.to_string()),
        },
        None => refuse(missing.to_string()),
    }
}

fn refuse_if_frozen(doc: &PyDict, message: impl FnOnce(&str) -> String, unfreeze: bool) -> R<()> {
    if let Some(frozen) = doc.get("frozen_utc").filter(|v| v.is_truthy()) {
        if !unfreeze {
            return refuse(message(&frozen.display_text().to_utf8().unwrap_or_default()));
        }
    }
    Ok(())
}

fn pointer_matches(ptr: &PyValue, kind: &str, path: &str) -> bool {
    dict_of(ptr)
        .map(|d| get_text(d, "path").as_deref() == Some(path) && get_text(d, "kind").as_deref() == Some(kind))
        .unwrap_or(false)
}

fn op_evidence_add(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let (Some(kind), Some(path_rel)) = (
        args.get("--kind").filter(|s| !s.is_empty()),
        args.get("--path").filter(|s| !s.is_empty()),
    ) else {
        return refuse("--evidence-add 는 --kind 와 --path 가 필요하다");
    };
    let cell_id = args.get("--cell").map(str::to_string);
    if !EVIDENCE_KINDS.contains(&kind) {
        return refuse(format!("kind 는 {} 중 하나여야 한다: {}", tuple_repr(&EVIDENCE_KINDS), py_repr(kind)));
    }
    no_fill(&[("path_rel", path_rel)])?;
    let mut node = args.get("--node").filter(|s| !s.is_empty()).map(str::to_string);
    if node.is_none() {
        if let Some(cell) = cell_id.as_deref().filter(|c| !c.is_empty()) {
            node = node_of_cell(&base, cell);
        }
    }
    let declaration = read_declaration(&base);
    if get_text(&declaration, "self_role").as_deref() == Some("sub") {
        return Err(Stop::Fallback);
    }
    let declared = list_of(declaration.get("nodes").unwrap_or(&PyValue::None))
        .map(|l| l.iter().filter(|n| dict_of(n).is_some()).count())
        .unwrap_or(0);
    if !ctx.root.join(path_rel).exists() {
        return refuse(format!(
            "증거가 실재하지 않는다: {path_rel} (증거는 docs 평면에서 태어나고 여기엔 포인터만 둔다)"
        ));
    }
    if declared > 1 && node.is_none() {
        return refuse(format!(
            "--node 가 필요하다 — 이 캠페인은 노드가 {declared}개다. 노드 태그 없는 증거는 '어느 노드가 낸 것인지 모르는 증거'이고, P2 는 그런 포인터를 검사하지 못해 공허하게 통과한다(2026-09-08 실측). 셀을 함께 주면(--cell) 배정에서 파생한다."
        ));
    }
    let ep = base.join("evidence_pointers.json");
    let existing = if ep.is_file() { read_json(&ep) } else { None };
    let mut doc = match existing.as_ref().and_then(dict_of) {
        Some(d) => d.clone(),
        None => {
            let mut d = PyDict::new();
            d.set("schema_version", PyValue::int(1));
            d.set("pointers", PyValue::List(Vec::new()));
            d
        }
    };
    refuse_if_frozen(
        &doc,
        |f| format!("이 스냅샷은 {f} 에 동결됐다 — 동결 이후에는 입력이 바뀌지 않는다(사람이 명시로 건 캠페인 전체 스냅샷이다).\n  → 정말 바꿔야 하면 사람이 --unfreeze 를 붙인다."),
        args.has("--unfreeze"),
    )?;
    let mut pointers = list_or_empty(doc.get("pointers"));
    if doc.get("pointers").and_then(list_of).is_none() {
        pointers = Vec::new();
    }
    if let Some(at) = pointers.iter().position(|p| pointer_matches(p, kind, path_rel)) {
        let mut ptr = dict_of(&pointers[at]).expect("matched").clone();
        let mut changed = false;
        for (key, val) in [("node_id", node.as_deref()), ("cell_id", cell_id.as_deref())] {
            let Some(val) = val else { continue };
            let have = ptr.get(key).cloned().unwrap_or(PyValue::None);
            let empty = matches!(have.peel(), PyValue::None) || text_of(&have).as_deref() == Some("");
            if empty {
                ptr.set(key, py(val));
                changed = true;
            } else if text_of(&have).as_deref() != Some(val) {
                return refuse(format!(
                    "이 포인터의 {key} 는 이미 {} 다 — {} 로 바꾸려면 사람이 판단해야 한다(증거의 귀속을 조용히 바꾸지 않는다): {path_rel}",
                    have.repr(),
                    py_repr(val)
                ));
            }
        }
        if changed {
            pointers[at] = PyValue::Dict(ptr);
            doc.set("pointers", PyValue::List(pointers));
            write_json(&ep, &PyValue::Dict(doc))?;
        }
        wrote_line(io, &camp, &[ctx.rel(&ep)]);
        return Ok(0);
    }
    let mut ptr = PyDict::new();
    ptr.set("kind", py(kind));
    ptr.set("path", py(path_rel));
    ptr.set("cell_id", cell_id.as_deref().map(py).unwrap_or(PyValue::None));
    ptr.set("node_id", node.as_deref().map(py).unwrap_or(PyValue::None));
    pointers.push(PyValue::Dict(ptr));
    doc.set("pointers", PyValue::List(pointers));
    if !doc.contains_key("campaign_id") {
        doc.set("campaign_id", py(&camp));
    }
    write_json(&ep, &PyValue::Dict(doc))?;
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

fn op_evidence_untag(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let (Some(kind), Some(path_rel), Some(cell)) = (
        args.get("--kind").filter(|s| !s.is_empty()),
        args.get("--path").filter(|s| !s.is_empty()),
        args.get("--cell").filter(|s| !s.is_empty()),
    ) else {
        return refuse("--evidence-untag 는 --kind --path --cell 이 필요하다");
    };
    let reason = args.get("--untag-reason").filter(|r| !r.trim().is_empty());
    let utc = args.get("--utc").filter(|u| !u.is_empty());
    let (Some(reason), Some(utc)) = (reason, utc) else {
        return refuse("--evidence-untag 는 --untag-reason 과 --utc 가 필요하다(사유 없는 귀속 해제는 드리프트다)");
    };
    let ep = base.join("evidence_pointers.json");
    let mut doc = load_pointer_doc(ctx, &ep, &format!("증거 포인터 파일이 없다: {}", ctx.rel(&ep)))?;
    refuse_if_frozen(&doc, |f| format!("이 스냅샷은 {f} 에 동결됐다 — 사람이 --unfreeze 를 붙여야 한다"), args.has("--unfreeze"))?;
    let mut pointers = list_or_empty(doc.get("pointers"));
    let hits: Vec<usize> = pointers.iter().enumerate().filter(|(_, p)| pointer_matches(p, kind, path_rel)).map(|(i, _)| i).collect();
    if hits.len() != 1 {
        return refuse(format!(
            "(kind={}, path={}) 포인터가 정확히 1건이 아니다: {}건",
            py_repr(kind),
            py_repr(path_rel),
            hits.len()
        ));
    }
    let mut ptr = dict_of(&pointers[hits[0]]).expect("matched").clone();
    let have = ptr.get("cell_id").cloned().unwrap_or(PyValue::None);
    if text_of(&have).as_deref() != Some(cell) {
        return refuse(format!(
            "이 포인터의 cell_id 는 {} 다 — --cell {} 와 다르다(엉뚱한 해제 ✗)",
            have.repr(),
            py_repr(cell)
        ));
    }
    let mut history = list_or_empty(ptr.get("untagged"));
    let mut entry = PyDict::new();
    entry.set("cell_id", py(cell));
    entry.set("reason", py(reason.trim()));
    entry.set("utc", py(utc));
    history.push(PyValue::Dict(entry));
    ptr.set("untagged", PyValue::List(history));
    ptr.set("cell_id", PyValue::None);
    pointers[hits[0]] = PyValue::Dict(ptr);
    doc.set("pointers", PyValue::List(pointers));
    write_json(&ep, &PyValue::Dict(doc))?;
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

fn op_evidence_relocate(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let (Some(kind), Some(path_from), Some(path_to)) = (
        args.get("--kind").filter(|s| !s.is_empty()),
        args.get("--path").filter(|s| !s.is_empty()),
        args.get("--to").filter(|s| !s.is_empty()),
    ) else {
        return refuse("--evidence-relocate 는 --kind --path --to 가 필요하다");
    };
    let reason = args.get("--relocate-reason").filter(|r| !r.trim().is_empty());
    let utc = args.get("--utc").filter(|u| !u.is_empty());
    let (Some(reason), Some(utc)) = (reason, utc) else {
        return refuse("--evidence-relocate 는 --relocate-reason 과 --utc 가 필요하다");
    };
    let ep = base.join("evidence_pointers.json");
    let mut doc = load_pointer_doc(ctx, &ep, &format!("증거 포인터 파일이 없다: {}", ctx.rel(&ep)))?;
    refuse_if_frozen(&doc, |f| format!("이 스냅샷은 {f} 에 동결됐다 — 사람이 --unfreeze 를 붙여야 한다"), args.has("--unfreeze"))?;
    if path_to.starts_with('/') || path_to.split('/').any(|p| p == "..") {
        return refuse(format!("새 경로는 저장소 상대여야 한다: {}", py_repr(path_to)));
    }
    if ctx.root.join(path_from).exists() {
        return refuse(format!("옛 경로가 아직 실재한다({path_from}) — 개명이 아니다(증거를 옮기지 않는다)"));
    }
    if !ctx.root.join(path_to).exists() {
        return refuse(format!("새 경로가 실재하지 않는다: {path_to}"));
    }
    let mut pointers = list_or_empty(doc.get("pointers"));
    let hits: Vec<usize> = pointers.iter().enumerate().filter(|(_, p)| pointer_matches(p, kind, path_from)).map(|(i, _)| i).collect();
    if hits.len() != 1 {
        return refuse(format!(
            "(kind={}, path={}) 포인터가 정확히 1건이 아니다: {}건",
            py_repr(kind),
            py_repr(path_from),
            hits.len()
        ));
    }
    if pointers.iter().any(|p| pointer_matches(p, kind, path_to)) {
        return refuse(format!("(kind={}, path={}) 포인터가 이미 있다 — 중복 ✗", py_repr(kind), py_repr(path_to)));
    }
    if get_text(&read_declaration(&base), "self_role").as_deref() == Some("sub") {
        return Err(Stop::Fallback);
    }
    let mut ptr = dict_of(&pointers[hits[0]]).expect("matched").clone();
    let mut history = list_or_empty(ptr.get("relocated"));
    let mut h = PyDict::new();
    h.set("from", py(path_from));
    h.set("reason", py(reason.trim()));
    h.set("utc", py(utc));
    history.push(PyValue::Dict(h));
    ptr.set("relocated", PyValue::List(history));
    ptr.set("path", py(path_to));
    pointers[hits[0]] = PyValue::Dict(ptr);
    doc.set("pointers", PyValue::List(pointers));
    write_json(&ep, &PyValue::Dict(doc))?;
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

fn op_evidence_prune_stubs(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let ep = base.join("evidence_pointers.json");
    let mut doc = load_pointer_doc(ctx, &ep, "evidence_pointers.json 이 없거나 파손")?;
    refuse_if_frozen(&doc, |f| format!("이 스냅샷은 {f} 에 동결됐다 — 사람이 --unfreeze 를 붙인다"), args.has("--unfreeze"))?;
    let (mut kept, mut dropped): (Vec<PyValue>, Vec<String>) = (Vec::new(), Vec::new());
    for ptr in list_or_empty(doc.get("pointers")) {
        let blob = ptr.dumps_unicode().map_err(|e| Stop::Refuse(e.to_string()))?.to_utf8().unwrap_or_default();
        if blob.contains(FILL) {
            dropped.push(blob);
        } else {
            kept.push(ptr);
        }
    }
    doc.set("pointers", PyValue::List(kept));
    if get_text(&doc, "campaign_id").as_deref() == Some(FILL) {
        doc.set("campaign_id", py(&camp));
    }
    write_json(&ep, &PyValue::Dict(doc))?;
    io.warn(&format!("[campaign_init] 스텁 포인터 {}건 제거:", dropped.len()));
    for d in &dropped {
        io.warn(&format!("  - {d}"));
    }
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

fn op_freeze_evidence(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--freeze-evidence 는 --utc 가 필요하다(벽시계 금지)");
    };
    let ep = base.join("evidence_pointers.json");
    let mut doc = load_pointer_doc(ctx, &ep, "evidence_pointers.json 이 없거나 파손 — 동결할 스냅샷이 없다")?;
    doc.set("frozen_utc", py(utc));
    write_json(&ep, &PyValue::Dict(doc))?;
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

fn op_revise(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let mut values = PyDict::new();
    for kv in args.all("--revise-set") {
        let (k, v) = match kv.split_once('=') {
            Some((k, v)) if !k.is_empty() => (k, v),
            _ => return refuse(format!("--revise-set 은 K=V 형식이다: {}", py_repr(kv))),
        };
        values.set(k.trim(), py(v.trim()));
    }
    if values.is_empty() {
        return refuse("--revise 는 --revise-set K=V 가 하나 이상 필요하다");
    }
    let reason = args.get("--revise-reason").filter(|s| !s.is_empty());
    let evidence = args.get("--revise-evidence").filter(|s| !s.is_empty());
    let approved = args.get("--revise-approved-by").filter(|s| !s.is_empty());
    for (need, val) in [("--revise-reason", reason), ("--revise-evidence", evidence), ("--revise-approved-by", approved)] {
        if val.is_none() {
            return refuse(format!("{need} 는 필수다 — 사유·근거·승인자 없는 개정은 개정이 아니다"));
        }
    }
    let path = base.join("campaign.yaml");
    let mut doc = load_pointer_doc(ctx, &path, "campaign.yaml 이 없거나 파손")?;
    let mut revs = list_or_empty(doc.get("revisions"));
    if doc.get("revisions").and_then(list_of).is_none() {
        revs = Vec::new();
    }
    if revs.is_empty() {
        let base_vals = match doc.get("control_variables") {
            Some(v) if v.is_truthy() => v.clone(),
            _ => PyValue::Dict(PyDict::new()),
        };
        let mut r = PyDict::new();
        r.set("rev", PyValue::int(0));
        r.set("declared_utc", doc.get("declared_utc").cloned().unwrap_or(PyValue::None));
        r.set("values", base_vals);
        r.set("reason", py("t0 선언"));
        r.set("evidence", doc.get("plan_ref").cloned().unwrap_or(PyValue::None));
        r.set("approved_by", py("t0"));
        revs.push(PyValue::Dict(r));
    }
    let mut merged = revs
        .last()
        .and_then(dict_of)
        .and_then(|r| r.get("values").filter(|v| v.is_truthy()))
        .and_then(dict_of)
        .cloned()
        .unwrap_or_default();
    for (k, v) in values.iter() {
        merged.insert(k.clone(), v.clone());
    }
    let mut r = PyDict::new();
    r.set("rev", PyValue::int(revs.len() as i64));
    r.set("declared_utc", args.get("--utc").map(py).unwrap_or(PyValue::None));
    r.set("values", PyValue::Dict(merged));
    r.set("reason", py(reason.expect("checked")));
    r.set("evidence", py(evidence.expect("checked")));
    r.set("approved_by", py(approved.expect("checked")));
    revs.push(PyValue::Dict(r));
    doc.set("revisions", PyValue::List(revs));
    write_json(&path, &PyValue::Dict(doc))?;
    wrote_line(io, &camp, &[ctx.rel(&path)]);
    Ok(0)
}

// ── purge gate ───────────────────────────────────────────────────────────────────────────────

/// `purge_gate_reasons(previous_id)`: why the previous instance may not be removed (empty = the gate is open). A closed
/// `abandoned` instance asks the owner's reduction gate: that shape is not modelled.
fn purge_gate_reasons(ctx: &Ctx, previous_id: &str) -> R<Vec<String>> {
    let prev = ctx.campaigns().join(previous_id);
    if !is_dir(&prev) {
        return Ok(Vec::new());
    }
    let life = fold_lifecycle(ctx, &prev, None);
    match life.state.as_str() {
        "suspended" | "resume_pending" => {
            return Ok(vec![format!(
                "{previous_id}: 보류 상태({})는 purge 할 수 없다 — 사람이 포기를 고르면 `--abandon {previous_id}` 로 종결 사유를 받은 뒤 지운다",
                life.state
            )])
        }
        "orphan" => {
            return Ok(vec![format!(
                "{previous_id}: 이름 없는 잔재(orphan · 수명 이벤트 없음 · ACTIVE 미지목)다 — 재개할 수 없고 `--abandon {previous_id}`(축소 게이트)로만 정리한다"
            )])
        }
        "unknown" => {
            return Ok(vec![format!(
                "{previous_id}: journey 에 이 버전이 모르는 수명 이벤트가 있다 — 브랜치 공통층 동기화 후 다시"
            )])
        }
        _ => {}
    }
    if life.state == "closed" && life.sub_convergence.as_deref() == Some("unconverged") {
        let wanted: Vec<String> = life.sub_nodes.iter().filter_map(text_of).collect();
        let mut missing: Vec<String> = wanted.into_iter().filter(|n| !life.sub_echoed.contains(n)).collect();
        missing.sort();
        missing.dedup();
        let control = if life.close_reason.as_deref() == Some("abandoned") { "abandon" } else { "close" };
        return Ok(vec![format!(
            "{previous_id}: 서브 종결이 수렴하지 않았다({}) — `relay.py --campaign-control {control} --campaign-id {previous_id} --node <sub>` 로 전하고 echo 를 받거나, 서브 미도달이면 사람이 `--sub-unconverged-accept {previous_id} --node <sub> --approved-by <전사> --utc <T>` 로 수용한다",
            missing.join(", ")
        )]);
    }
    if life.state == "closed" && life.close_reason.as_deref() == Some("abandoned") {
        return Err(Stop::Fallback);
    }
    let mut reasons: Vec<String> = Vec::new();
    let pointers_path = prev.join("evidence_pointers.json");
    let Some(doc) = read_json(&pointers_path) else {
        reasons.push(format!(
            "{previous_id}: evidence_pointers.json 이 없거나 파손 — 증거가 docs 평면에 도착했는지 물을 수단이 없다"
        ));
        return Ok(reasons);
    };
    let pointers = dict_of(&doc).and_then(|d| d.get("pointers")).and_then(list_of).filter(|l| !l.is_empty());
    let Some(pointers) = pointers else {
        reasons.push(format!(
            "{previous_id}: 증거 포인터가 0건이다 — 캠페인을 돌고 증거가 하나도 없다는 기록은 그 자체가 결손이다(빈 목록으로 게이트를 통과시키지 않는다)"
        ));
        return Ok(reasons);
    };
    let mut kinds: Vec<String> = Vec::new();
    for (i, ptr) in pointers.iter().enumerate() {
        let Some(ptr) = dict_of(ptr) else {
            reasons.push(format!("{previous_id}: pointers[{i}] 형태 위반"));
            continue;
        };
        let shown = |key: &str| {
            match ptr.get(key) {
                Some(v) if v.is_truthy() => v.display_text().to_utf8().unwrap_or_default().trim().to_string(),
                _ => String::new(),
            }
        };
        let (rel, kind) = (shown("path"), shown("kind"));
        if !kinds.contains(&kind) {
            kinds.push(kind.clone());
        }
        if rel.is_empty() || rel.contains(FILL) {
            reasons.push(format!("{previous_id}: pointers[{i}] 경로 미기재"));
            continue;
        }
        if !ctx.root.join(&rel).exists() {
            reasons.push(format!("{previous_id}: 증거가 실재하지 않는다 — {rel} (kind={kind})"));
        }
    }
    let relay = prev.join("relay");
    let has_ledger = std::fs::read_dir(&relay)
        .map(|read| {
            read.filter_map(Result::ok).any(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.ends_with(".json") && name != "pending_hitl.json"
            })
        })
        .unwrap_or(false);
    if has_ledger && !kinds.iter().any(|k| k == "relay_summary") {
        reasons.push(format!(
            "{previous_id}: relay_summary 포인터가 없다 — 원장 원문은 휘발이므로 요약 testlog 가 없으면 왕복이 통째로 사라진다"
        ));
    }
    let validator = ctx.root.join(".claude/skills/terraforming_node/scripts/campaign_template_validator.py");
    if !validator.is_file() {
        reasons.push(format!(
            "{previous_id}: 검증기(campaign_template_validator.py)를 찾지 못했다 — P1~P3 을 물을 수단이 없다"
        ));
    } else {
        reasons.extend(
            crate::campaign_validator::instance_predicates(ctx, &prev)
                .into_iter()
                .map(|r| format!("{previous_id}: {r}")),
        );
    }
    Ok(reasons)
}

/// `do_purge(previous_id, apply)`: the removed path (empty when there was nothing to remove).
fn do_purge(ctx: &Ctx, previous_id: &str, apply: bool) -> R<Vec<String>> {
    let reasons = purge_gate_reasons(ctx, previous_id)?;
    if !reasons.is_empty() {
        return refuse(format!("purge 게이트가 닫혀 있다:\n  - {}", reasons.join("\n  - ")));
    }
    let prev = ctx.campaigns().join(previous_id);
    if !is_dir(&prev) {
        return Ok(Vec::new());
    }
    if apply {
        std::fs::remove_dir_all(&prev).map_err(|e| Stop::Refuse(format!("{}: {e}", prev.display())))?;
    }
    Ok(vec![ctx.rel(&prev)])
}

fn op_verify_purge_gate(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let reasons = purge_gate_reasons(ctx, args.get("--verify-purge-gate").unwrap_or_default())?;
    if !reasons.is_empty() {
        io.warn("[campaign_init] purge 게이트 닫힘");
        for reason in reasons {
            io.warn(&format!("  - {reason}"));
        }
        return Ok(1);
    }
    io.say("[campaign_init] purge 게이트 열림");
    Ok(0)
}

/// `_parent_on_close(base, reason, utc)`: a closed child puts a pending resume question on its suspended parent.
fn parent_on_close(ctx: &Ctx, base: &Path, reason: &str, utc: &str) -> R<Option<String>> {
    let fold = fold_lifecycle(ctx, base, Some(BOOTSTRAP));
    let Some(parent) = text_of(&fold.parent).filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    let pbase = ctx.campaigns().join(&parent);
    if !is_dir(&pbase) {
        return Ok(None);
    }
    if fold_lifecycle(ctx, &pbase, Some(BOOTSTRAP)).state != "suspended" {
        return Ok(None);
    }
    let ep = read_json(&base.join("evidence_pointers.json"));
    let pointers: Vec<PyValue> = ep
        .as_ref()
        .and_then(dict_of)
        .and_then(|d| d.get("pointers"))
        .and_then(list_of)
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .filter(|p| matches!(get_text(p, "kind").as_deref(), Some("testlog" | "bench_report" | "sweep_map" | "relay_summary")))
        .filter_map(|p| p.get("path").filter(|v| v.is_truthy()).cloned())
        .collect();
    let mut event = PyDict::new();
    event.set("utc", py(utc));
    event.set("kind", py("campaign_resume_pending"));
    event.set("lifecycle_v", PyValue::int(LIFECYCLE_V));
    event.set("source", py("campaign_init"));
    event.set("trigger", py("child_closed"));
    event.set("child", py(&base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    event.set("child_reason", py(reason));
    event.set("pointers", PyValue::List(pointers));
    append_journey(&pbase, &PyValue::Dict(event))?;
    Ok(Some(parent))
}

fn sub_close_hint(camp_id: &str, subs: &[String], control: &str) -> String {
    if subs.is_empty() {
        return String::new();
    }
    format!(
        " · 서브 종결은 A2A 제어명령으로 전한다: relay.py --campaign-control {control} --campaign-id {camp_id} --node {} … (echo 가 오기 전까지 '서브 미수렴' · purge 게이트가 이것을 묻는다)",
        subs.join("|")
    )
}

// ── --close and --init ───────────────────────────────────────────────────────────────────────

fn op_close(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp_id = args.get("--close").unwrap_or_default();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    if RESERVED_IDS.contains(&camp_id) {
        return refuse(format!("예약 id 는 종결 대상이 아니다: {}", py_repr(camp_id)));
    }
    let active = ctx.active_id();
    if active == BOOTSTRAP {
        io.say(&format!(
            "[campaign_init] {mode} no-op — 활성 캠페인이 없다(ACTIVE=_bootstrap · {})",
            ctx.active_pointer_fact()
        ));
        return Ok(0);
    }
    if active != camp_id {
        return refuse(format!(
            "활성 캠페인은 {} 다 — {} 를 종결할 수 없다(이름이 곧 증거 연결이다)",
            py_repr(&active),
            py_repr(camp_id)
        ));
    }
    if args.get("--utc").map(str::is_empty).unwrap_or(true) {
        return refuse("--close 는 --utc 가 필요하다(시각은 주입만 받는다 — 벽시계 금지)");
    }
    let utc = args.get("--utc").unwrap_or_default();
    let reasons = purge_gate_reasons(ctx, camp_id)?;
    if !reasons.is_empty() {
        return refuse(format!("종결 선행조건(= purge 게이트)이 닫혀 있다:\n  - {}", reasons.join("\n  - ")));
    }
    let base = ctx.campaigns().join(camp_id);
    let plain = format!("closed {camp_id} → ACTIVE=_bootstrap (인스턴스 보존: campaigns/{camp_id})");
    if !args.has("--apply") {
        io.say(&format!("[campaign_init] {mode} {plain}"));
        return Ok(0);
    }
    let subs: Vec<String> = sub_nodes_of(ctx, &base).iter().filter_map(text_of).collect();
    let mut event = PyDict::new();
    event.set("utc", py(utc));
    event.set("kind", py("campaign_closed"));
    event.set("reason", py("completed"));
    event.set("lifecycle_v", PyValue::int(LIFECYCLE_V));
    event.set("source", py("campaign_init --close"));
    event.set("sub_nodes", str_list(subs.clone()));
    event.set("note", py("ACTIVE → _bootstrap (인스턴스 보존 · 다음 --init --purge-previous 가 지운다)"));
    append_journey(&base, &PyValue::Dict(event))?;
    write_text(&ctx.active_pointer(), &format!("{BOOTSTRAP}\n"))?;
    let tail = sub_close_hint(camp_id, &subs, "close");
    let message = match parent_on_close(ctx, &base, "completed", utc)? {
        Some(parent) => format!("{plain} · 부모 {parent} 에 재개 미결(계기 a){tail}"),
        None => format!("{plain}{tail}"),
    };
    io.say(&format!("[campaign_init] {mode} {message}"));
    Ok(0)
}

fn require_transition(utc: Option<&str>, approved_by: Option<&str>, what: &str) -> R<()> {
    if utc.map(str::is_empty).unwrap_or(true) {
        return refuse(format!("{what} 는 --utc 가 필요하다(시각은 주입만 받는다 — 벽시계 금지)"));
    }
    if approved_by.map(str::is_empty).unwrap_or(true) {
        return refuse(format!(
            "{what} 는 --approved-by 가 필요하다(선언 확인 팝업의 사람 발화 전사 — 승인 없는 전이 ✗)"
        ));
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> R<()> {
    std::fs::create_dir_all(to).map_err(|e| Stop::Refuse(e.to_string()))?;
    let read = std::fs::read_dir(from).map_err(|e| Stop::Refuse(e.to_string()))?;
    for entry in read.filter_map(Result::ok) {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| Stop::Refuse(format!("{}: {e}", src.display())))?;
        }
    }
    Ok(())
}

fn without_notes(doc: &PyValue) -> PyDict {
    let mut out = PyDict::new();
    if let Some(d) = dict_of(doc) {
        for (k, v) in d.iter() {
            if !k.to_utf8().map(|s| s.starts_with('_')).unwrap_or(false) {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

fn op_init(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let new_id = args.get("--init").unwrap_or_default().to_string();
    let apply = args.has("--apply");
    let mode = if apply { "APPLIED" } else { "DRY-RUN" };
    let Some(plan_ref) = args.get("--plan-ref").filter(|s| !s.is_empty()) else {
        io.warn("[campaign_init] FAIL --init 은 --plan-ref 가 필요하다(승인 없는 개설 ✗)");
        return Ok(2);
    };
    // Shapes with a suspension, a lineage, a slice, a purge or a disposition are the owner's.
    let unmodelled = [
        "--suspend-current", "--derived-from", "--preempt", "--release", "--dispose", "--from-slice",
    ];
    if unmodelled.iter().any(|o| args.has(o)) {
        return Err(Stop::Fallback);
    }
    let utc = args.get("--utc");
    let approved_by = args.get("--approved-by");
    let cur = ctx.active_id();
    let instances = list_instances(ctx);
    let table: Vec<(String, Fold)> = {
        let active = ctx.active_id();
        instances
            .iter()
            .map(|p| {
                (
                    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    fold_lifecycle(ctx, p, Some(&active)),
                )
            })
            .collect()
    };
    let here = ctx.checkout_topology();
    if table.iter().any(|(_, f)| {
        f.state == "suspended"
            && dict_of(&f.open_pending).and_then(|d| get_text(d, "id")).as_deref() == Some(new_id.as_str())
    }) {
        return Err(Stop::Fallback);
    }
    let purge_previous: Vec<String> = args.all("--purge-previous").to_vec();
    if cur != BOOTSTRAP && !purge_previous.contains(&cur) {
        return refuse(format!(
            "활성 캠페인 {cur} 이 있다 — 처분 없이 덮어쓰는 --init 은 거부한다(옛 허점: ACTIVE 만 바뀌고 {cur} 이 이름 없는 잔재가 된다). 셋 중 하나를 고른다: ① `--suspend-current [--derived-from {cur}]` 로 보류+개설 ② `--close {cur}`(완주) ③ `--abandon {cur}`(포기) — 정상 차단"
        ));
    }
    let need = table
        .iter()
        .any(|(c, f)| *c != cur && (f.state == "suspended" || f.state == "resume_pending") && !f.answered);
    if need {
        return Err(Stop::Fallback);
    }
    if utc.is_some() || approved_by.is_some() {
        require_transition(utc, approved_by, "수명 전이(--init)")?;
    }
    let target = args.get("--target-topology").map(str::to_string).or_else(|| here.clone());
    let target = match target.as_deref() {
        Some("single" | "multi") => target.expect("checked"),
        other => {
            return refuse(format!(
                "대상 토폴로지를 정할 수 없다({}) — `--target-topology single|multi`",
                other.map(py_repr).unwrap_or_else(|| "None".to_string())
            ))
        }
    };
    if table.iter().any(|(c, _)| *c == new_id) || ctx.campaigns().join(&new_id).exists() {
        return refuse(format!("이미 있는 인스턴스다: campaigns/{new_id}"));
    }
    if RESERVED_IDS.contains(&new_id.as_str()) {
        return refuse(format!("예약 id 로는 캠페인을 열 수 없다: {}", py_repr(&new_id)));
    }
    if Some(target.as_str()) != here.as_deref() {
        return refuse(format!(
            "대상 토폴로지 {} 가 체크아웃({})과 다르다 — 반대 토폴로지 캠페인을 이 브랜치에서 열 수 없다(4자일치). 보류할 활성 캠페인이 있으면 `--suspend-current` 로 1단을 남기고, 없으면 대상 브랜치에서 연다",
            py_repr(&target),
            here.as_deref().map(py_repr).unwrap_or_else(|| "None".to_string())
        ));
    }
    // purge: an abandoned previous instance is the owner's reduction gate; decide that before the first removal
    for prev in &purge_previous {
        let probe = ctx.campaigns().join(prev);
        if is_dir(&probe) {
            let life = fold_lifecycle(ctx, &probe, None);
            if life.state == "closed" && life.close_reason.as_deref() == Some("abandoned") && life.sub_convergence.as_deref() != Some("unconverged") {
                return Err(Stop::Fallback);
            }
        }
    }
    let mut removed: Vec<String> = Vec::new();
    for prev in &purge_previous {
        removed.extend(do_purge(ctx, prev, apply)?);
    }
    // sweep_bootstrap_relay
    let relay = ctx.campaigns().join(BOOTSTRAP).join("relay");
    let mut swept: Vec<String> = Vec::new();
    if relay.is_dir() {
        let mut children: Vec<PathBuf> = std::fs::read_dir(&relay)
            .map_err(|e| Stop::Refuse(e.to_string()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            if child.file_name().map(|n| n == ".gitkeep").unwrap_or(false) {
                continue;
            }
            swept.push(ctx.rel(&child));
            if apply {
                if child.is_dir() {
                    std::fs::remove_dir_all(&child).map_err(|e| Stop::Refuse(e.to_string()))?;
                } else {
                    std::fs::remove_file(&child).map_err(|e| Stop::Refuse(e.to_string()))?;
                }
            }
        }
    }
    // scaffold
    let dest = ctx.campaigns().join(&new_id);
    if apply {
        copy_tree(&ctx.template(), &dest)?;
        let decl = read_json(&dest.join("campaign.yaml"))
            .ok_or_else(|| Stop::Refuse("campaign.yaml 을 읽지 못했다".to_string()))?;
        let mut doc = without_notes(&decl);
        doc.set("id", py(&new_id));
        doc.set("plan_ref", py(plan_ref));
        write_json_ordered(&dest.join("campaign.yaml"), &PyValue::Dict(doc))?;
        let ep_path = dest.join("evidence_pointers.json");
        let ep = read_json(&ep_path).ok_or_else(|| Stop::Refuse("evidence_pointers.json 을 읽지 못했다".to_string()))?;
        let mut ep = without_notes(&ep);
        ep.set("campaign_id", py(&new_id));
        if !ep.contains_key("pointers") {
            ep.set("pointers", PyValue::List(Vec::new()));
        }
        write_json_ordered(&ep_path, &PyValue::Dict(ep))?;
        write_text(&ctx.active_pointer(), &format!("{new_id}\n"))?;
        // topology and the opened event
        let mut doc = read_declaration(&dest);
        let mut cv = get_dict(&doc, "control_variables").cloned().unwrap_or_default();
        let unset = match cv.get("topology") {
            None => true,
            Some(v) => matches!(v.peel(), PyValue::None) || text_of(v).as_deref() == Some(FILL),
        };
        if unset {
            cv.set("topology", py(&target));
            doc.set("control_variables", PyValue::Dict(cv));
            write_json_ordered(&dest.join("campaign.yaml"), &PyValue::Dict(doc))?;
        }
        if let Some(utc) = utc.filter(|u| !u.is_empty()) {
            let mut event = PyDict::new();
            event.set("utc", py(utc));
            event.set("kind", py("campaign_opened"));
            event.set("lifecycle_v", PyValue::int(LIFECYCLE_V));
            event.set("source", py("campaign_init"));
            event.set("parent", PyValue::None);
            event.set("topology", py(&target));
            event.set("approved_by", approved_by.map(py).unwrap_or(PyValue::None));
            event.set("plan_ref", py(plan_ref));
            append_journey(&dest, &PyValue::Dict(event))?;
        }
    }
    io.say(&format!(
        "[campaign_init] {mode} purge={} init=['campaigns/{new_id}']",
        py_list_repr(&removed)
    ));
    if !swept.is_empty() {
        io.say(&format!("[campaign_init] {mode} _bootstrap/relay 정리 {}건", swept.len()));
    }
    Ok(0)
}

// ── --reentry-decide ─────────────────────────────────────────────────────────────────────────

fn op_reentry_decide(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let cell = args.get("--reentry-decide").unwrap_or_default().to_string();
    let Some((base, camp)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(0);
    };
    guard_writer_ops(ctx, &base, &[("reentry_decide", true)])?;
    let decision = args.get("--decision");
    let route = args.get("--route");
    let approved_by = args.get("--approved-by").map(str::trim).filter(|s| !s.is_empty());
    let utc = args.get("--utc").filter(|s| !s.is_empty());
    let decision = match decision {
        Some(d @ ("approve" | "decline")) => d,
        other => {
            return refuse(format!(
                "--decision 은 approve|decline 이다: {}",
                other.map(py_repr).unwrap_or_else(|| "None".to_string())
            ))
        }
    };
    let route_text = route.and_then(|r| REENTRY_ROUTES.iter().find(|(k, _)| *k == r).map(|(_, v)| *v));
    if decision == "approve" && route_text.is_none() {
        return refuse("승인에는 --route explorer|upstream 가 필요하다(누가 재발동하는가를 먼저 적는다)");
    }
    let (Some(approved_by), Some(utc)) = (approved_by, utc) else {
        return refuse("--reentry-decide 는 --approved-by(사람 발화 전사)와 --utc 가 필요하다");
    };
    let path = base.join("cells").join(&cell).join("cell.status.json");
    let existing = if path.is_file() { read_json(&path) } else { None };
    let mut doc = existing.as_ref().and_then(dict_of).cloned();
    let reentry = doc.as_ref().and_then(|d| get_dict(d, "reentry")).cloned();
    let Some(mut reentry) = reentry.filter(|r| get_text(r, "status").as_deref() == Some("proposed")) else {
        let status = doc
            .as_ref()
            .and_then(|d| get_dict(d, "reentry"))
            .map(|r| r.get("status").map(PyValue::repr).unwrap_or_else(|| "None".to_string()))
            .unwrap_or_else(|| "None".to_string());
        return refuse(format!(
            "셀 {cell} 에 대기 중인 재발동 제안이 없다(status={status}) — 제안은 lite ② 가 셀 상태에 실릴 때 생긴다(--cell-set --lite-raw)"
        ));
    };
    let mut doc_dict = doc.take().expect("reentry came from the document");
    let status = if decision == "approve" { "approved" } else { "declined" };
    reentry.set("status", py(status));
    let mut d = PyDict::new();
    d.set("decision", py(decision));
    d.set("route", route_text.map(py).unwrap_or(PyValue::None));
    d.set("approved_by", py(approved_by));
    d.set("utc", py(utc));
    reentry.set("decision", PyValue::Dict(d));
    doc_dict.set("reentry", PyValue::Dict(reentry));
    write_json(&path, &PyValue::Dict(doc_dict.clone()))?;
    let mut entry = PyDict::new();
    entry.set("utc", py(utc));
    entry.set("node_id", doc_dict.get("node_id").cloned().unwrap_or(PyValue::None));
    entry.set("cell_id", py(&cell));
    entry.set("outcome", doc_dict.get("cell_outcome").cloned().unwrap_or(PyValue::None));
    entry.set("reentry", py(status));
    entry.set("route", route_text.map(py).unwrap_or(PyValue::None));
    entry.set("source", py("campaign_init --reentry-decide"));
    let journey = append_journey(&base, &PyValue::Dict(entry))?;
    io.say(&format!(
        "[campaign_init] wrote({camp}): {} {}",
        ctx.rel(&path),
        ctx.rel(&journey)
    ));
    Ok(0)
}

// ── --lockset-precheck ───────────────────────────────────────────────────────────────────────

fn op_lockset_precheck(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some(cell) = args.get("--cell").filter(|s| !s.is_empty()) else {
        return refuse("--lockset-precheck 는 --cell 이 필요하다(셀 없는 출처는 없다)");
    };
    let camp_id = args.get("--campaign-id").filter(|s| !s.is_empty());
    let camp = camp_id.map(str::to_string).unwrap_or_else(|| ctx.active_id());
    let mut verdict = PyDict::new();
    if camp == BOOTSTRAP {
        let fact = if camp_id.is_none() {
            ctx.active_pointer_fact()
        } else {
            "--campaign-id=_bootstrap".to_string()
        };
        verdict.set("status", py("not_applicable"));
        verdict.set("campaign_id", py(BOOTSTRAP));
        verdict.set("cell", py(cell));
        verdict.set("lockset", PyValue::None);
        verdict.set("provenance", PyValue::None);
        verdict.set(
            "reason",
            py(&format!(
                "{fact} → _bootstrap — 캠페인 셀 문맥이 없어 lockset 의 거처가 없다(writer no-op 과 같은 경계). 셀 출처 precheck 대상이 아니다"
            )),
        );
        let line = PyValue::Dict(verdict).dumps_unicode().map_err(|e| Stop::Refuse(e.to_string()))?;
        io.say(&line.to_utf8().unwrap_or_default());
        io.warn(&format!(
            "[campaign_init] ⓘ 셀 출처 precheck 대상 아님 — {fact} → _bootstrap — 캠페인 셀 문맥이 없어 lockset 의 거처가 없다(writer no-op 과 같은 경계). 셀 출처 precheck 대상이 아니다"
        ));
        return Ok(0);
    }
    let base = ctx.campaigns().join(&camp);
    if !is_dir(&base) {
        return refuse(format!("인스턴스가 없다: campaigns/{camp} — 출처를 물을 셀이 없다"));
    }
    let path = base.join("cells").join(cell).join("lockset.json");
    let mut why = lockset_provenance_reason(ctx, &path);
    let doc = if path.is_file() { read_json(&path) } else { None };
    let prov = doc.as_ref().and_then(dict_of).and_then(|d| d.get("provenance")).filter(|v| text_of(v).is_some()).cloned();
    if let Some(reason) = why.as_mut() {
        if node_of_cell(&base, cell).is_none() {
            reason.push_str(&format!(
                " · 셀 키 {} 가 campaigns/{camp}/campaign.yaml 의 assignments 에도 없다(스윕 셀 키가 캠페인 셀 id 와 같은 이름인지 확인하라)",
                py_repr(cell)
            ));
        }
    }
    verdict.set("status", py(if why.is_some() { "refused" } else { "passed" }));
    verdict.set("campaign_id", py(&camp));
    verdict.set("cell", py(cell));
    verdict.set("lockset", py(&ctx.rel(&path)));
    verdict.set("provenance", prov.unwrap_or(PyValue::None));
    verdict.set("reason", why.as_deref().map(py).unwrap_or(PyValue::None));
    let line = PyValue::Dict(verdict).dumps_unicode().map_err(|e| Stop::Refuse(e.to_string()))?;
    io.say(&line.to_utf8().unwrap_or_default());
    if let Some(reason) = why {
        io.warn(&format!("[campaign_init] 셀 출처 precheck 거부 — {reason}"));
        return Ok(2);
    }
    Ok(0)
}

// ── --grounding-check and --escalation-summary ───────────────────────────────────────────────

/// `grounding_reasons(base)`: why the grounding does not hold (empty = it does).
fn grounding_reasons(ctx: &Ctx, base: &Path) -> Vec<String> {
    let Some((path, doc)) = latest_grounding(base) else {
        return vec![format!(
            "그라운딩 기록이 없다: {}/ — 캠페인 착수·셀 축 변경 시 통제변인 파생 질의로 사서를 부른 기록이 있어야 한다(policy:LIBRARY_GROUNDING_FAIL_CLOSED C1). `campaign_init.py --ground --utc <t>` 로 물어라.",
            ctx.rel(&base.join("grounding"))
        )];
    };
    let status = chain_dict(&doc, "export")
        .and_then(|e| chain_dict(e, "resolution"))
        .and_then(|r| r.get("status").cloned())
        .unwrap_or(PyValue::None);
    match text_of(&status).as_deref() {
        Some("refused") => vec![format!(
            "사서가 질의를 **거절**했다({}) — 공백과 거절은 다른 사실이고 거절은 차단이다(D5)",
            ctx.rel(&path)
        )],
        Some("resolved" | "unresolved") => Vec::new(),
        _ => vec![format!(
            "그라운딩 기록의 판정이 알 수 없는 값이다({}): {}",
            ctx.rel(&path),
            status.repr()
        )],
    }
}

fn op_grounding_check(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, _)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] 그라운딩 검사 생략 — 활성 캠페인 없음(_bootstrap)");
        return Ok(0);
    };
    let reasons = grounding_reasons(ctx, &base);
    if !reasons.is_empty() {
        io.warn("[campaign_init] 그라운딩 백스톱: 통과하지 못했다 (policy:LIBRARY_GROUNDING_FAIL_CLOSED C2)");
        for reason in reasons {
            io.warn(&format!("  - {reason}"));
        }
        return Ok(4);
    }
    io.say("[campaign_init] 그라운딩 백스톱 통과");
    Ok(0)
}

fn op_escalation_summary(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, _)) = writer_target(ctx, args.get("--campaign-id"))? else {
        let mut d = PyDict::new();
        d.set("campaign_id", PyValue::None);
        d.set("total", PyValue::int(0));
        d.set("hitl_required", PyValue::Bool(false));
        d.set("_note", py("ACTIVE=_bootstrap — 캠페인 밖에는 후보가 없다"));
        let line = PyValue::Dict(d).dumps_unicode().map_err(|e| Stop::Refuse(e.to_string()))?;
        io.say(&line.to_utf8().unwrap_or_default());
        return Ok(0);
    };
    let rows: Vec<PyValue> = match std::fs::read(base.join("escalation_candidates.jsonl")) {
        Ok(bytes) => String::from_utf8_lossy(&bytes)
            .lines()
            .filter_map(|l| pyjson::loads(l.trim()).ok().filter(|v| !l.trim().is_empty() && dict_of(v).is_some()))
            .collect(),
        Err(_) => Vec::new(),
    };
    let flag = |r: &PyValue, key: &str, want: bool| {
        dict_of(r).and_then(|d| d.get(key)).map(|v| matches!(v.peel(), PyValue::Bool(b) if *b == want)).unwrap_or(false)
    };
    let standing: Vec<&PyValue> = rows.iter().filter(|r| flag(r, "predicate_1", true)).collect();
    let host: Vec<&PyValue> = rows
        .iter()
        .filter(|r| {
            flag(r, "predicate_1", false)
                && dict_of(r)
                    .map(|d| truthy(d.get("host_plane_ignored")) || truthy(d.get("model_plane_refuted")))
                    .unwrap_or(false)
        })
        .collect();
    let cells = |items: &[&PyValue]| {
        let mut out: Vec<String> = items
            .iter()
            .map(|r| {
                dict_of(r)
                    .and_then(|d| d.get("cell_id"))
                    .map(|v| v.display_text().to_utf8().unwrap_or_default())
                    .unwrap_or_else(|| "None".to_string())
            })
            .collect();
        out.sort();
        out.dedup();
        out
    };
    let mut d = PyDict::new();
    d.set("campaign_id", py(&base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    d.set("total", PyValue::int(rows.len() as i64));
    d.set("predicate_1_standing", PyValue::int(standing.len() as i64));
    d.set("host_plane_only", PyValue::int(host.len() as i64));
    d.set("cells_standing", str_list(cells(&standing)));
    d.set("cells_host_plane", str_list(cells(&host)));
    d.set("hitl_required", PyValue::Bool(!standing.is_empty()));
    d.set("rows", PyValue::List(rows.clone()));
    d.set(
        "_note",
        py("술어 ① 이 선 후보가 하나라도 있으면 캠페인 종결 시 사람에게 묻는다 — 빌드 평면 재전략(포크 핀·빌드 패치·이미지 재빌드)은 통제변인을 바꾸므로 캠페인 중간에 발동하지 않는다. ① 이 서지 않은 줄은 예산·호스트 평면의 일이며 그 처방은 R1~R3 의 게이트가 이미 갖고 있다."),
    );
    let text = PyValue::Dict(d).dumps_indented(2).map_err(|e| Stop::Refuse(e.to_string()))?;
    io.say(&text.to_utf8().unwrap_or_default());
    Ok(0)
}

// ── dispatch ─────────────────────────────────────────────────────────────────────────────────

/// The operations this module answers, by the owner's dispatch order.
const NATIVE_OPS: [&str; 17] = [
    "--evidence-add",
    "--evidence-untag",
    "--evidence-relocate",
    "--evidence-prune-stubs",
    "--freeze-evidence",
    "--revise",
    "--verify-purge-gate",
    "--reentry-decide",
    "--lockset-precheck",
    "--grounding-check",
    "--escalation-summary",
    "--assigned-cells",
    "--write-brief",
    "--phase-set",
    "--cell-set",
    "--close",
    "--init",
];

/// Options that an operation of this module understands (anything else in the vector means the owner's shape).
fn allowed_options(op: &str) -> &'static [&'static str] {
    match op {
        "--assigned-cells" => &["--assigned-cells", "--campaign-id"],
        "--verify-purge-gate" => &["--verify-purge-gate"],
        "--evidence-add" => &["--evidence-add", "--kind", "--path", "--cell", "--node", "--unfreeze", "--campaign-id"],
        "--evidence-untag" => &["--evidence-untag", "--kind", "--path", "--cell", "--untag-reason", "--utc", "--unfreeze", "--campaign-id"],
        "--evidence-relocate" => &["--evidence-relocate", "--kind", "--path", "--to", "--relocate-reason", "--utc", "--unfreeze", "--campaign-id"],
        "--evidence-prune-stubs" => &["--evidence-prune-stubs", "--unfreeze", "--campaign-id"],
        "--freeze-evidence" => &["--freeze-evidence", "--utc", "--campaign-id"],
        "--revise" => &["--revise", "--revise-set", "--revise-reason", "--revise-evidence", "--revise-approved-by", "--utc", "--campaign-id"],
        "--reentry-decide" => &["--reentry-decide", "--campaign-id", "--decision", "--route", "--approved-by", "--utc"],
        "--lockset-precheck" => &["--lockset-precheck", "--cell", "--campaign-id"],
        "--grounding-check" => &["--grounding-check", "--campaign-id"],
        "--escalation-summary" => &["--escalation-summary", "--campaign-id"],
        "--write-brief" => &["--write-brief", "--campaign-id", "--node", "--utc"],
        "--phase-set" => &[
            "--phase-set", "--campaign-id", "--node", "--cell", "--state", "--proof-predicate", "--proof-ok",
            "--proof-source", "--started-utc", "--ended-utc", "--authored-by",
        ],
        "--cell-set" => &[
            "--cell-set", "--campaign-id", "--outcome", "--node", "--version", "--model",
            "--measurement-decode-tps", "--measurement-source", "--void-reason", "--void-reason-source",
            "--axis-citation", "--next-intent", "--utc", "--image-digest", "--image-tag", "--sweep-state",
            "--lite-raw",
        ],
        "--close" => &["--close", "--utc", "--apply"],
        "--init" => &[
            "--init", "--plan-ref", "--target-topology", "--apply", "--utc", "--approved-by", "--reason",
            "--suspend-current", "--derived-from", "--preempt", "--release", "--dispose", "--from-slice",
            "--purge-previous",
        ],
        _ => &[],
    }
}

/// `ap.print_help()` for the no-argument run (status 2) and for `-h`/`--help` (status 0), at the width the help was captured
/// for; any other terminal width is the owner's.
fn help(argv: &[String], io: &mut Sinks<'_>) -> Option<Outcome> {
    let status = match argv {
        [] => 2,
        [flag] if flag == "-h" || flag == "--help" => 0,
        _ => return None,
    };
    if crate::terminal::columns() != crate::campaign_init_help::COLUMNS {
        return Some(Outcome::Fallback);
    }
    let _ = io.out.write_all(crate::campaign_init_help::HELP.as_bytes());
    Some(Outcome::Done(status))
}

/// Answer the vector natively when it is a modelled shape.
pub fn run(ctx: &Ctx, argv: &[String], io: &mut Sinks<'_>) -> Outcome {
    if let Some(outcome) = help(argv, io) {
        return outcome;
    }
    let Some(args) = Args::parse(argv) else {
        return Outcome::Fallback;
    };
    let ops: Vec<&&str> = NATIVE_OPS.iter().filter(|op| args.has(op)).collect();
    if ops.len() != 1 {
        return Outcome::Fallback;
    }
    let op: &str = *ops[0];
    if args.names().any(|n| !allowed_options(op).contains(n)) {
        return Outcome::Fallback;
    }
    for (option, allowed) in [("--decision", &["approve", "decline"][..]), ("--route", &["explorer", "upstream"][..])] {
        if let Some(value) = args.get(option) {
            if !allowed.contains(&value) {
                return Outcome::Fallback;
            }
        }
    }
    if let Some(topology) = args.get("--target-topology") {
        if topology != "single" && topology != "multi" {
            return Outcome::Fallback;
        }
    }
    let result = match op {
        "--assigned-cells" => op_assigned_cells(ctx, &args, io),
        "--verify-purge-gate" => op_verify_purge_gate(ctx, &args, io),
        "--evidence-add" => op_evidence_add(ctx, &args, io),
        "--evidence-untag" => op_evidence_untag(ctx, &args, io),
        "--evidence-relocate" => op_evidence_relocate(ctx, &args, io),
        "--evidence-prune-stubs" => op_evidence_prune_stubs(ctx, &args, io),
        "--freeze-evidence" => op_freeze_evidence(ctx, &args, io),
        "--revise" => op_revise(ctx, &args, io),
        "--reentry-decide" => op_reentry_decide(ctx, &args, io),
        "--lockset-precheck" => op_lockset_precheck(ctx, &args, io),
        "--grounding-check" => op_grounding_check(ctx, &args, io),
        "--escalation-summary" => op_escalation_summary(ctx, &args, io),
        "--write-brief" => op_write_brief(ctx, &args, io),
        "--phase-set" => op_phase_set(ctx, &args, io),
        "--cell-set" => op_cell_set(ctx, &args, io),
        "--close" => op_close(ctx, &args, io),
        "--init" => op_init(ctx, &args, io),
        _ => return Outcome::Fallback,
    };
    match result {
        Ok(status) => Outcome::Done(status),
        Err(Stop::Fallback) => Outcome::Fallback,
        Err(Stop::Refuse(text)) => {
            io.warn(&format!("[campaign_init] FAIL {text}"));
            Outcome::Done(1)
        }
    }
}
