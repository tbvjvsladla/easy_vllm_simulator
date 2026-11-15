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
    fn say(&mut self, text: &str) {
        let _ = writeln!(self.out, "{text}");
    }

    fn warn(&mut self, text: &str) {
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

    #[allow(dead_code)]
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

fn py_repr(text: &str) -> String {
    PyStr::new(text).repr()
}

#[allow(dead_code)]
fn py_list_repr(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| py_repr(s)).collect();
    format!("[{}]", inner.join(", "))
}

fn dict_of(value: &PyValue) -> Option<&PyDict> {
    match value.peel() {
        PyValue::Dict(dict) => Some(dict),
        _ => None,
    }
}

fn list_of(value: &PyValue) -> Option<&Vec<PyValue>> {
    match value.peel() {
        PyValue::List(items) => Some(items),
        _ => None,
    }
}

fn text_of(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8(),
        _ => None,
    }
}

fn get_text(dict: &PyDict, key: &str) -> Option<String> {
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
fn assignments_of(doc: &PyDict) -> Vec<(String, PyValue)> {
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
fn lockset_observable(decl: &PyDict, node: Option<&str>) -> bool {
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
fn lockset_provenance_reason(ctx: &Ctx, path: &Path) -> Option<String> {
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
#[allow(dead_code)]
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

fn tuple_repr(items: &[&str]) -> String {
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
    // The sweep-record and lite-gate branches read other files; they stay with the owner for now.
    if args.has("--sweep-state") || args.has("--lite-raw") {
        return Err(Stop::Fallback);
    }
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
    let mut meas = match doc.get("measurement").and_then(dict_of) {
        Some(d) => d.clone(),
        None => PyDict::new(),
    };
    if let Some(tps) = args.get("--measurement-decode-tps") {
        let number = parse_python_float(tps)?;
        meas.set("decode_tps_conc1", PyValue::Float(number));
        meas.set("measurement_ok", PyValue::Bool(true));
    }
    if let Some(source) = args.get("--measurement-source") {
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
    if outcome == "measured" && !truthy(meas.get("measurement_ok")) {
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
    if let Some(digest) = args.get("--image-digest").filter(|s| !s.is_empty()) {
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
        img.set(
            "gap",
            py("이미지 digest 가 도착하지 않았다 — --sweep-state 도 --image-digest 도 없다. 셀 간 비교는 같은 digest 안에서만 성립하므로, 이 셀은 다른 셀과 나란히 놓을 수 없다(결손 기재)."),
        );
    }
    doc.set("image", PyValue::Dict(img));

    let prior = doc.get("provenance").and_then(dict_of).cloned().unwrap_or_default();
    let prior_mismatch = list_or_empty(doc.get("provenance_mismatch"));
    let (provenance, mismatch) = cell_provenance(ctx, base, cell, node.as_deref(), &prior, prior_mismatch);
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
    prior: &PyDict,
    prior_mismatch: Vec<PyValue>,
) -> (PyDict, Vec<PyValue>) {
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
        return (prov, prior_mismatch);
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
    (prov, prior_mismatch)
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
    // The purge gate reads the instance predicates of the validator: still the owner's.
    Err(Stop::Fallback)
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
        "--purge-previous",
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
    if cur != BOOTSTRAP {
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
        "[campaign_init] {mode} purge=[] init=['campaigns/{new_id}']"
    ));
    if !swept.is_empty() {
        io.say(&format!("[campaign_init] {mode} _bootstrap/relay 정리 {}건", swept.len()));
    }
    Ok(0)
}

// ── dispatch ─────────────────────────────────────────────────────────────────────────────────

/// The operations this module answers, by the owner's dispatch order.
const NATIVE_OPS: [&str; 6] = [
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

/// Answer the vector natively when it is a modelled shape.
pub fn run(ctx: &Ctx, argv: &[String], io: &mut Sinks<'_>) -> Outcome {
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
    if let Some(topology) = args.get("--target-topology") {
        if topology != "single" && topology != "multi" {
            return Outcome::Fallback;
        }
    }
    let result = match op {
        "--assigned-cells" => op_assigned_cells(ctx, &args, io),
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
