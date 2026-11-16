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
#[derive(Clone)]
pub struct Args {
    values: BTreeMap<&'static str, Vec<String>>,
}

impl Args {
    /// The namespace `campaign_args::parse` built (option string -> values; a flag holds one empty value).
    pub fn from_values(values: BTreeMap<&'static str, Vec<String>>) -> Args {
        Args { values }
    }

    /// Python truthiness of the attribute: a flag is set, a value option holds a non-empty text.
    pub fn on(&self, name: &str) -> bool {
        match kind_of(name) {
            Some(Kind::Flag) => self.has(name),
            Some(Kind::Append) => !self.all(name).is_empty(),
            _ => self.get(name).map(|v| !v.is_empty()).unwrap_or(false),
        }
    }

    /// `a.<name> = value` (a value option).
    fn set(&mut self, name: &str, value: String) {
        if let Some((known, _)) = OPTIONS.iter().find(|(n, _)| *n == name) {
            self.values.insert(known, vec![value]);
        }
    }

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
                Kind::Optional => {
                    match argv.get(at + 1).filter(|v| !v.starts_with('-')) {
                        Some(v) => {
                            values.insert(name, vec![v.clone()]);
                            at += 2;
                        }
                        None => {
                            values.insert(name, vec!["*".to_string()]);
                            at += 1;
                        }
                    }
                }
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

    /// Arguments built by the program itself (an import re-enters the writers with the values of a brief).
    fn synthetic(pairs: Vec<(&str, String)>) -> Args {
        let mut values: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for (name, value) in pairs {
            let (known, _) = OPTIONS.iter().find(|(n, _)| *n == name).expect("a parser option");
            values.insert(known, vec![value]);
        }
        Args { values }
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

    /// The option strings present on the command line (sorted).
    pub fn names(&self) -> impl Iterator<Item = &&'static str> {
        self.values.keys()
    }
}

// ── Python-shaped helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn py(text: &str) -> PyValue {
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
        // The predicate is installed when its program is: the original `topology_parity.py` or - with the original retired -
        // its Rust launcher `topology_parity.sh` beside it (plan_26111615 pass 3: removing the `.py` must not turn a main
        // checkout into a sub-local reading).
        let runtime = self.root.join(".claude/policies/runtime");
        if !runtime.join("topology_parity.py").is_file() && !runtime.join("topology_parity.sh").is_file() {
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
        .filter(|n| get_text(n, "node_id").as_deref() == node && node.is_some())
        .last()
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
pub(crate) fn sub_nodes_of(ctx: &Ctx, base: &Path) -> Vec<PyValue> {
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

/// `validator()`: the owner loads the validator beside it; without that file nothing that asks for an assignment can answer.
fn need_validator(ctx: &Ctx) -> R<()> {
    // The validator is this program's own Rust module (`campaign_validator`), always present: the original's refusal for a
    // missing `campaign_template_validator.py` beside it cannot occur any more.
    let _ = ctx;
    Ok(())
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
    need_validator(ctx)?;
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
    let Some((base, camp)) = progress_prelude(ctx, args, io, "phase_set")? else { return Ok(0) };
    let cell = args.get("--cell").map(str::to_string);
    let mut node = args.get("--node").map(str::to_string).filter(|n| !n.is_empty());
    if node.is_none() {
        if let Some(cell) = cell.as_deref().filter(|c| !c.is_empty()) {
            need_validator(ctx)?;
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
        io.warn(&format!("[campaign_init] 서가 입고(C4) — {}", warm_start_library(ctx)?));
    }
    let path = write_phase(ctx, &base, args, &node, &phase, &state, cell.as_deref())?;
    wrote_line(io, &camp, &[ctx.rel(&path)]);
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
    let Some((base, camp)) = progress_prelude(ctx, args, io, "cell_set")? else { return Ok(0) };
    let Some(outcome) = args.get("--outcome").filter(|s| !s.is_empty()) else {
        return refuse("--cell-set 은 --outcome 이 필요하다");
    };
    let (cpath, jpath) = write_cell(ctx, &base, args, &cell, outcome)?;
    let mut wrote = vec![ctx.rel(&cpath)];
    if let Some(j) = jpath {
        wrote.push(ctx.rel(&j));
    }
    wrote_line(io, &camp, &wrote);
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
        None => {
            need_validator(ctx)?;
            node_of_cell(base, cell)
        }
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
    need_validator(ctx)?;
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
    // The lite verdict owner is installed when its launcher is (the reading rule itself is native: read_lite_verdict).
    let owner = ctx.root.join(".claude/skills/adversarial-benchmark/scripts/lite_metrics.sh");
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
    need_validator(ctx)?;
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
    if let Some(target) = group_target() {
        return Ok(Some(target));
    }
    let Some(target) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(None);
    };
    guard_writer_ops(ctx, &target.0, &[])?;
    Ok(Some(target))
}

fn wrote_line(io: &mut Sinks<'_>, camp: &str, paths: &[String]) {
    if group_collect(paths) {
        return;
    }
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
    if args.get("--kind").filter(|s| !s.is_empty()).is_none() || args.get("--path").filter(|s| !s.is_empty()).is_none() {
        return refuse("--evidence-add 는 --kind 와 --path 가 필요하다");
    }
    let ep = add_evidence(ctx, &base, &camp, args)?;
    wrote_line(io, &camp, &[ctx.rel(&ep)]);
    Ok(0)
}

/// `writer_add_evidence`: the pointer file it wrote (or left as it was).
fn add_evidence(ctx: &Ctx, base: &Path, camp: &str, args: &Args) -> R<PathBuf> {
    let base = base.to_path_buf();
    let kind = args.get("--kind").unwrap_or_default();
    let path_rel = args.get("--path").unwrap_or_default();
    let cell_id = args.get("--cell").map(str::to_string);
    if !EVIDENCE_KINDS.contains(&kind) {
        return refuse(format!("kind 는 {} 중 하나여야 한다: {}", tuple_repr(&EVIDENCE_KINDS), py_repr(kind)));
    }
    no_fill(&[("path_rel", path_rel)])?;
    let mut node = args.get("--node").filter(|s| !s.is_empty()).map(str::to_string);
    if node.is_none() {
        if let Some(cell) = cell_id.as_deref().filter(|c| !c.is_empty()) {
            need_validator(ctx)?;
            node = node_of_cell(&base, cell);
        }
    }
    let declaration = read_declaration(&base);
    let declared_nodes: Vec<&PyDict> = list_of(declaration.get("nodes").unwrap_or(&PyValue::None)).into_iter().flatten().filter_map(dict_of).collect();
    let declared = declared_nodes.len();
    if get_text(&declaration, "self_role").as_deref() == Some("sub") {
        if node.is_none() && declared == 1 {
            node = declared_nodes[0].get("node_id").and_then(text_of);
        }
        let node_value = node.as_deref().map(py).unwrap_or(PyValue::None);
        sub_evidence_binding(&base, &node_value, &cell_id.as_deref().map(py).unwrap_or(PyValue::None))?;
        let local = sub_evidence_path(&py(kind), &py(path_rel), &ctx.root.join("docs"))?;
        if !local.is_file() && !(kind == "simlog" && local.is_dir()) {
            return refuse(format!("서브 증거는 실재 정규파일 또는 simlog 디렉터리여야 한다: {path_rel}"));
        }
    }
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
        return Ok(ep);
    }
    let mut ptr = PyDict::new();
    ptr.set("kind", py(kind));
    ptr.set("path", py(path_rel));
    ptr.set("cell_id", cell_id.as_deref().map(py).unwrap_or(PyValue::None));
    ptr.set("node_id", node.as_deref().map(py).unwrap_or(PyValue::None));
    pointers.push(PyValue::Dict(ptr));
    doc.set("pointers", PyValue::List(pointers));
    if !doc.contains_key("campaign_id") {
        doc.set("campaign_id", py(camp));
    }
    write_json(&ep, &PyValue::Dict(doc))?;
    Ok(ep)
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
    let correct = args.has("--correct-reference");
    if ctx.root.join(path_from).exists() && !correct {
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
    let is_sub = get_text(&read_declaration(&base), "self_role").as_deref() == Some("sub");
    let mut ptr = dict_of(&pointers[hits[0]]).expect("matched").clone();
    let mut history = list_or_empty(ptr.get("relocated"));
    let mut h = PyDict::new();
    h.set("from", py(path_from));
    h.set("reason", py(reason.trim()));
    h.set("utc", py(utc));
    if correct {
        let (node, cell) = (args.get("--node").filter(|s| !s.is_empty()), args.get("--cell").filter(|s| !s.is_empty()));
        let (true, true, Some(node), Some(cell)) = (kind == "relay_summary", is_sub, node, cell) else {
            return refuse("--correct-reference 는 sub relay_summary 정정이며 --node --cell이 필요하다");
        };
        sub_evidence_binding(&base, &py(node), &py(cell))?;
        if get_text(&ptr, "node_id").as_deref() != Some(node) || get_text(&ptr, "cell_id").as_deref() != Some(cell) {
            return refuse("정정할 포인터의 node/cell 귀속이 --node/--cell과 다르다");
        }
        let source = ctx.root.join(path_from);
        let relay_dir = std::fs::canonicalize(base.join("relay")).ok();
        let source_ok = !path_from.starts_with('/')
            && !path_from.split('/').any(|p| p == "..")
            && source.is_file()
            && !source.is_symlink()
            && source.parent().and_then(|p| std::fs::canonicalize(p).ok()) == relay_dir
            && relay_dir.is_some();
        if !source_ok {
            return refuse("정정 원본은 이 캠페인의 보존할 relay 정규파일이어야 한다");
        }
        let target = sub_evidence_path(&py(kind), &py(path_to), &ctx.root.join("docs"))?;
        if !path_to.starts_with("docs/testlog/") || !target.is_file() || target.extension().map(|e| e != "md").unwrap_or(true) {
            return refuse("정정 목적지는 실제 docs/testlog Markdown 요약 정규파일이어야 한다");
        }
        h.set("mode", py("reference-correction"));
        h.set("source_preserved", PyValue::Bool(true));
        h.set("node_id", py(node));
        h.set("cell_id", py(cell));
    } else if is_sub {
        sub_evidence_binding(&base, &ptr.get("node_id").cloned().unwrap_or(PyValue::None), &ptr.get("cell_id").cloned().unwrap_or(PyValue::None))?;
        sub_evidence_path(&py(kind), &py(path_to), &ctx.root.join("docs"))?;
    }
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

fn op_hint_approve(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    use crate::campaign_validator::{hint_approval_reason, hint_target_reasons, APPROVAL_DEFAULT_SOURCE};
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    need_validator(ctx)?;
    let node = args.get("--node");
    let approved_by = args.get("--approved-by");
    let utc = args.get("--utc");
    let source = args.get("--source").filter(|s| !s.is_empty()).unwrap_or(APPROVAL_DEFAULT_SOURCE);
    for (need, val) in [("--node", node), ("--approved-by", approved_by), ("--utc", utc)] {
        if val.map(|v| v.trim().is_empty()).unwrap_or(true) {
            return refuse(format!(
                "{need} 는 필수다 — node·cells·approved-by·utc 넷 중 하나라도 없으면 승인이 아니라 드리프트다(--revise 와 같은 규율)"
            ));
        }
    }
    let (node, approved_by, utc) = (node.expect("checked"), approved_by.expect("checked"), utc.expect("checked"));
    let mut wanted: Vec<String> = Vec::new();
    for c in args.get("--cells").unwrap_or("").split(',') {
        let c = c.trim();
        if !c.is_empty() && !wanted.iter().any(|w| w == c) {
            wanted.push(c.to_string());
        }
    }
    if wanted.is_empty() {
        return refuse("--cells 는 필수다 — 승인할 셀을 명시 열거한다(빈 목록 = 전 셀 해석은 폐기 · 백지 승인 ✗)");
    }
    no_fill(&[("node", node), ("approved_by", approved_by), ("utc", utc), ("source", source), ("cells", &wanted.join(","))])?;
    let mut approval = PyDict::new();
    approval.set("approved_by", py(approved_by));
    approval.set("approved_utc", py(utc));
    approval.set("source", py(source));
    let approval = PyValue::Dict(approval);
    if let Some(why) = hint_approval_reason(&approval) {
        return refuse(format!("승인이 완결되지 않았다 — {why}"));
    }
    let path = base.join("campaign.yaml");
    let mut doc = load_pointer_doc(ctx, &path, "campaign.yaml 이 없거나 파손 — 승인을 적을 선언이 없다")?;
    if get_text(&doc, "self_role").as_deref() == Some("sub") {
        return refuse("self_role=sub 인스턴스다 — 발행 승인은 메인 선언에만 적는다. 서브 셀이면 메인에서 `--hint-approve --node <그 서브 노드>` 로 적고, 파생 선언(--emit-slice)이 그 노드 몫을 운반한다");
    }
    let declared = list_of(doc.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .any(|n| get_text(n, "node_id").as_deref() == Some(node));
    if !declared {
        return refuse(format!("nodes[] 에 {} 가 없다 — 선언되지 않은 노드의 셀은 승인할 수 없다", py_repr(node)));
    }
    let assigned = assigned_cells(&doc, Some(node));
    let outside: Vec<String> = wanted.iter().filter(|c| !assigned.contains(c)).cloned().collect();
    if !outside.is_empty() {
        return refuse(format!(
            "--cells 의 {} 는 assignments[{}] 에 없다(배정: {}) — 배정 SSOT 는 assignments 이고 hint_targets 는 파생이다(배정의 노드가 승인의 노드다)",
            py_list_repr(&outside),
            py_repr(node),
            py_list_repr(&assigned)
        ));
    }
    let before = hint_target_reasons(&doc);
    if !before.is_empty() {
        return refuse(format!(
            "선언의 hint_targets 에 이미 결함이 있다 — 결함 위에 승인을 얹지 않는다. 먼저 고친다:\n{}",
            before.iter().map(|b| format!("  - {b}")).collect::<Vec<_>>().join("\n")
        ));
    }
    let mut targets = match doc.get("hint_targets").and_then(list_of) {
        Some(t) => t.clone(),
        None => Vec::new(),
    };
    let target_cells = |t: &PyValue| -> Vec<String> {
        dict_of(t).and_then(|d| d.get("cells")).and_then(list_of).map(|l| l.iter().filter_map(text_of).collect()).unwrap_or_default()
    };
    let mut approved_at: Vec<(String, usize)> = Vec::new();
    for (i, t) in targets.iter().enumerate() {
        for c in target_cells(t) {
            match approved_at.iter_mut().find(|(n, _)| *n == c) {
                Some(slot) => slot.1 = i,
                None => approved_at.push((c, i)),
            }
        }
    }
    let at = |c: &str| approved_at.iter().find(|(n, _)| n == c).map(|(_, i)| *i);
    let same = targets.iter().position(|t| {
        dict_of(t).map(|d| get_text(d, "node_id").as_deref() == Some(node) && d.get("approval") == Some(&approval)).unwrap_or(false)
    });
    let added: Vec<String> = wanted.iter().filter(|c| at(c).is_none()).cloned().collect();
    let already: Vec<String> = wanted.iter().filter(|c| at(c).is_some() && at(c) == same).cloned().collect();
    let kept: Vec<(String, usize)> = wanted.iter().filter_map(|c| at(c).filter(|i| Some(*i) != same).map(|i| (c.clone(), i))).collect();
    let mut index = same;
    let mut wrote: Vec<String> = Vec::new();
    if !added.is_empty() {
        match same {
            Some(i) => {
                let mut t = dict_of(&targets[i]).expect("same is an object").clone();
                let mut cells = list_or_empty(t.get("cells"));
                cells.extend(added.iter().map(|c| py(c)));
                t.set("cells", PyValue::List(cells));
                targets[i] = PyValue::Dict(t);
            }
            None => {
                let mut t = PyDict::new();
                t.set("node_id", py(node));
                t.set("cells", str_list(added.clone()));
                t.set("approval", approval.clone());
                targets.push(PyValue::Dict(t));
                index = Some(targets.len() - 1);
            }
        }
        doc.set("hint_targets", PyValue::List(targets.clone()));
        let after = hint_target_reasons(&doc);
        if !after.is_empty() {
            return refuse(format!(
                "승인 기록이 검증기를 통과하지 못한다(writer↔검증기 갈라짐 — 쓰지 않았다):\n{}",
                after.iter().map(|x| format!("  - {x}")).collect::<Vec<_>>().join("\n")
            ));
        }
        write_json(&path, &PyValue::Dict(doc))?;
        wrote.push(ctx.rel(&path));
        io.warn(&format!(
            "[campaign_init] hint 사전승인 hint_targets[{}] ← {}",
            index.expect("set above"),
            py_list_repr(&added)
        ));
    }
    if !already.is_empty() {
        io.warn(&format!("[campaign_init] 같은 승인으로 이미 적힌 셀(멱등): {}", py_list_repr(&already)));
    }
    for (cell, i) in kept {
        let a = dict_of(&targets[i]).and_then(|d| get_dict(d, "approval")).cloned().unwrap_or_default();
        let shown = |k: &str| a.get(k).map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or_else(|| "None".to_string());
        io.warn(&format!(
            "[campaign_init] ⓘ {} 은 이미 hint_targets[{i}] 가 승인했다({} · {}) — 옛 승인을 지킨다(귀속을 조용히 바꾸지 않는다)",
            py_repr(&cell),
            shown("source"),
            shown("approved_utc")
        ));
    }
    wrote_line(io, &camp, &wrote);
    Ok(0)
}

// ── --resume-brief ───────────────────────────────────────────────────────────────────────────

fn shown(value: Option<&PyValue>) -> String {
    value.map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or_else(|| "None".to_string())
}

/// `_short(text, limit)`: one line, cut to `limit` characters.
fn short(value: Option<&PyValue>, limit: usize) -> String {
    let text = match value {
        Some(v) if v.is_truthy() => v.display_text().to_utf8().unwrap_or_default(),
        _ => String::new(),
    };
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= limit {
        joined
    } else {
        let cut: String = joined.chars().take(limit - 1).collect();
        format!("{cut}…")
    }
}

fn lifecycle_table(ctx: &Ctx) -> Vec<(String, Fold)> {
    let active = ctx.active_id();
    list_instances(ctx)
        .iter()
        .map(|p| {
            (
                p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                fold_lifecycle(ctx, p, Some(&active)),
            )
        })
        .collect()
}

fn pending_query_field(f: &Fold, key: &str) -> String {
    shown(dict_of(&f.pending_query).and_then(|d| d.get(key)).or(None))
}

/// `lifecycle_line(table)`: the first line of the brief.
fn lifecycle_line(ctx: &Ctx, table: &[(String, Fold)]) -> String {
    let active = ctx.active_id();
    let susp: Vec<String> = table
        .iter()
        .filter(|(_, f)| f.state == "suspended" || f.state == "resume_pending")
        .map(|(c, f)| match f.parent.is_truthy() {
            true => format!("{c}⊂{}", shown(Some(&f.parent))),
            false => c.clone(),
        })
        .collect();
    let pend: Vec<String> = table
        .iter()
        .filter(|(_, f)| f.state == "resume_pending" && !f.answered)
        .map(|(c, f)| {
            format!(
                "{c}←{} 종결({}): 재개?",
                pending_query_field(f, "child"),
                pending_query_field(f, "child_reason")
            )
        })
        .collect();
    let table_names: Vec<&str> = table.iter().map(|(c, _)| c.as_str()).collect();
    let opening: Vec<String> = table
        .iter()
        .filter(|(_, f)| f.state == "suspended" && f.open_pending.is_truthy())
        .filter(|(_, f)| {
            let id = dict_of(&f.open_pending).and_then(|d| get_text(d, "id"));
            !id.map(|i| table_names.contains(&i.as_str())).unwrap_or(false)
        })
        .map(|(_, f)| {
            let d = dict_of(&f.open_pending);
            format!("{}@{}", shown(d.and_then(|d| d.get("id"))), shown(d.and_then(|d| d.get("topology"))))
        })
        .collect();
    let unconv: Vec<String> = table
        .iter()
        .filter(|(_, f)| f.sub_convergence.as_deref() == Some("unconverged"))
        .map(|(c, f)| {
            let mut missing: Vec<String> =
                f.sub_nodes.iter().filter_map(text_of).filter(|n| !f.sub_echoed.contains(n)).collect();
            missing.sort();
            missing.dedup();
            format!("{c}@{}", missing.join(","))
        })
        .collect();
    let reslice: Vec<String> = table
        .iter()
        .filter(|(_, f)| !f.reslice_pending.is_empty() && f.state != "closed")
        .map(|(c, f)| {
            let mut names: Vec<String> = f.reslice_pending.iter().map(|(n, _)| n.clone()).collect();
            names.sort();
            format!("{c}@{}", names.join(","))
        })
        .collect();
    let odd: Vec<String> = table
        .iter()
        .filter(|(_, f)| f.state == "orphan" || f.state == "unknown")
        .map(|(c, f)| format!("{c}={}", f.state))
        .collect();
    let mut parts = vec![
        format!("ACTIVE={active}"),
        format!("보류 {}{}", susp.len(), if susp.is_empty() { String::new() } else { format!("({})", susp.join(", ")) }),
        format!("미결 질의 {}{}", pend.len(), if pend.is_empty() { String::new() } else { format!("({})", pend.join("; ")) }),
    ];
    if !opening.is_empty() {
        parts.push(format!("개설 대기 {}", opening.join(", ")));
    }
    if !unconv.is_empty() {
        parts.push(format!("서브 미수렴 {}", unconv.join(", ")));
    }
    if !reslice.is_empty() {
        parts.push(format!("재배정 미수렴 {}", reslice.join(", ")));
    }
    if !odd.is_empty() {
        parts.push(format!("정리 대상 {}", odd.join(", ")));
    }
    format!("상태: {}", parts.join(" · "))
}

fn resume_brief(ctx: &Ctx, camp_id: Option<&str>) -> R<String> {
    let camp = camp_id.filter(|c| !c.is_empty()).map(str::to_string).unwrap_or_else(|| ctx.active_id());
    let base = ctx.campaigns().join(&camp);
    let mut l: Vec<String> = Vec::new();
    let table = lifecycle_table(ctx);
    l.push(lifecycle_line(ctx, &table));
    l.push(format!("# resume-brief · campaigns/{camp}"));
    let life: Vec<&(String, Fold)> = table.iter().filter(|(_, f)| f.state != "closed").collect();
    if !life.is_empty() {
        l.push(String::new());
        l.push("## 0. 캠페인 수명 (journey fold · 질의 규칙: 계기 a=파생 자식 종결 · d=무관 개설뿐)".to_string());
        for (c, f) in &life {
            let mut bits = vec![f.state.clone()];
            if f.parent.is_truthy() {
                bits.push(format!("parent={}", shown(Some(&f.parent))));
            }
            if !f.interrupted_cells.is_empty() {
                bits.push(format!("interrupted={}", f.interrupted_cells.join(",")));
            }
            if let Some(conv) = &f.sub_convergence {
                bits.push(format!("서브 {conv}"));
            }
            if f.state == "suspended" || f.state == "resume_pending" {
                bits.push(if f.answered { "답함" } else { "미답" }.to_string());
            }
            l.push(format!("- `{c}` — {}", bits.join(" · ")));
        }
        if life.iter().any(|(_, f)| f.state == "resume_pending" && !f.answered) {
            l.push("- 미결 질의는 **사람이 있는 세션에서 한 번** 묻는다(무인 리볼빙은 이 줄만 보이고 묻지 않는다 · D6). 답: `--resume-campaign` · `--answer-query <id> --decision keep` · `--abandon`".to_string());
        }
    }
    if camp == BOOTSTRAP {
        l.push(String::new());
        l.push("활성 캠페인이 **없다**(ACTIVE=_bootstrap). 여기는 캠페인 밖 릴레이 대기실이며 진행 상태를".to_string());
        l.push("들지 않는다 — campaigns writer 는 no-op 이고, 재개할 캠페인도 없다.".to_string());
        return Ok(l.join("\n") + "\n");
    }
    if !is_dir(&base) {
        l.push(String::new());
        l.push(format!("인스턴스 디렉터리가 없다: campaigns/{camp}"));
        return Ok(l.join("\n") + "\n");
    }
    let doc = read_declaration(&base);
    let revs: Vec<PyValue> = doc.get("revisions").and_then(list_of).cloned().unwrap_or_default();
    let cur: PyDict = revs
        .last()
        .and_then(dict_of)
        .and_then(|r| r.get("values").filter(|v| v.is_truthy()))
        .and_then(dict_of)
        .cloned()
        .or_else(|| doc.get("control_variables").filter(|v| v.is_truthy()).and_then(dict_of).cloned())
        .unwrap_or_default();
    l.push(String::new());
    l.push("## 1. layer-1 선언 (통제변인)".to_string());
    if !cur.is_empty() {
        let mut keys: Vec<&(PyStr, PyValue)> = cur.iter().collect();
        keys.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
        for (k, v) in keys {
            l.push(format!("- {}: {}", k.to_utf8().unwrap_or_default(), shown(Some(v))));
        }
    } else {
        l.push("- (선언 없음 — 결손)".to_string());
    }
    let tail = if revs.len() > 1 {
        format!(" · 최신 사유: {}", shown(revs.last().and_then(dict_of).and_then(|r| r.get("reason"))))
    } else {
        String::new()
    };
    l.push(format!("- 개정 수: {}{tail}", revs.len().saturating_sub(1)));
    l.push(format!("- plan_ref: {}", shown(doc.get("plan_ref"))));

    need_validator(ctx)?;
    let mut plan: Vec<(Option<String>, String, Option<String>)> = Vec::new();
    for (node, _) in assignments_of(&doc) {
        for cell in assigned_cells(&doc, Some(&node)) {
            let mode = cell_mode(&doc, &node, &cell);
            plan.push((Some(node.clone()), cell, Some(mode)));
        }
    }
    let cells_dir = base.join("cells");
    let mut found: Vec<String> = std::fs::read_dir(&cells_dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != "_cell")
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    let known: Vec<String> = plan.iter().map(|(_, c, _)| c.clone()).collect();
    let mut listed = plan.clone();
    for c in found {
        if !known.contains(&c) {
            listed.push((None, c, None));
        }
    }
    l.push(String::new());
    l.push("## 2. 셀 진행표 (노드별 배정 순 — **다른 노드의 리스트는 동시에 돈다**)".to_string());
    let mut pending: Vec<String> = Vec::new();
    let mut refuted: Vec<(String, Option<PyValue>, Option<PyValue>)> = Vec::new();
    let mut reentry_pending: Vec<(String, Option<PyValue>, Option<PyValue>)> = Vec::new();
    for (node_of, cell, mode) in &listed {
        let st = read_json(&cells_dir.join(cell).join("cell.status.json"));
        let st = st.as_ref().and_then(dict_of).cloned().unwrap_or_default();
        let outcome_value = st.get("cell_outcome").filter(|v| v.is_truthy()).cloned();
        let outcome = match &outcome_value {
            Some(v) => v.display_text().to_utf8().unwrap_or_default(),
            None if cells_dir.join(cell).is_dir() => "(상태파일 없음)".to_string(),
            None => "(미개설)".to_string(),
        };
        let tps = get_dict(&st, "measurement").and_then(|m| m.get("decode_tps_conc1")).filter(|v| !matches!(v.peel(), PyValue::None));
        let mut extra: Vec<String> = Vec::new();
        if let Some(t) = tps {
            extra.push(format!("decode {} t/s", shown(Some(t))));
        }
        if truthy(st.get("void_reason")) {
            extra.push(format!("void: {}", short(st.get("void_reason"), 110)));
        }
        let rc = get_dict(&st, "reconciliation").cloned().unwrap_or_default();
        let re = get_dict(&st, "reentry").cloned().unwrap_or_default();
        if truthy(rc.get("spent")) {
            let status = match re.get("status") {
                Some(v) if v.is_truthy() => shown(Some(v)),
                _ => "기록 없음".to_string(),
            };
            extra.push(format!("lite ② · cap 차감 {} · 재발동 {status}", shown(rc.get("spent"))));
        }
        if get_text(&re, "status").as_deref() == Some("proposed") {
            reentry_pending.push((cell.clone(), rc.get("spent").cloned(), re.get("proposed_utc").cloned()));
        }
        let tag = match node_of {
            Some(n) => format!("[{n} · {}]", mode.clone().unwrap_or_default()),
            None => "[미배정]".to_string(),
        };
        let suffix = if extra.is_empty() { String::new() } else { format!("  ({})", extra.join(" · ")) };
        l.push(format!("- `{cell}` {tag} — {outcome}{suffix}"));
        if CELL_OUTCOMES_PENDING.contains(&outcome.as_str()) || outcome == "(상태파일 없음)" || outcome == "(미개설)" {
            pending.push(cell.clone());
        }
        if ["serve_failed", "build_failed", "void", "measurement_void"].contains(&outcome.as_str()) {
            refuted.push((cell.clone(), st.get("axis_citation").cloned(), st.get("void_reason").cloned()));
        }
    }
    if listed.is_empty() {
        l.push("- (배정된 셀 없음)".to_string());
    }
    if !reentry_pending.is_empty() {
        l.push(String::new());
        l.push("### 재발동 대기(사람 승인) — lite ② 실사용 불가".to_string());
        let routes: Vec<&str> = REENTRY_ROUTES.iter().map(|(_, v)| *v).collect();
        for (cell, spent, when) in &reentry_pending {
            let when_text = match when {
                Some(w) if w.is_truthy() => shown(Some(w)),
                _ => "시각 미상".to_string(),
            };
            l.push(format!(
                "- `{cell}` — cap 차감 누계 {} · 제안 {when_text} · 경로: {} · 결정은 `--reentry-decide` 로 기록(자동 실행 ✗)",
                shown(spent.as_ref()),
                routes.join(" / ")
            ));
        }
    }

    l.push(String::new());
    l.push("## 3. 노드별 phase 진행표".to_string());
    let phases_dir = base.join("phases");
    let mut nodes: Vec<String> = list_of(doc.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .filter_map(|n| get_text(n, "node_id"))
        .filter(|id| !id.contains(FILL))
        .collect();
    let mut seen: Vec<String> = std::fs::read_dir(&phases_dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != "_node")
                .collect()
        })
        .unwrap_or_default();
    seen.sort();
    for n in seen {
        if !nodes.contains(&n) {
            nodes.push(n);
        }
    }
    if nodes.is_empty() {
        nodes.push("(선언 노드 없음)".to_string());
    }
    for node in &nodes {
        let mut row: Vec<String> = Vec::new();
        for ph in PHASE_NAMES {
            let st = read_json(&phases_dir.join(node).join(format!("{ph}.status.json")));
            match st.as_ref().and_then(dict_of) {
                None => row.push(format!("{ph}=—")),
                Some(st) => {
                    let ok = get_dict(st, "proof").map(|p| truthy(p.get("ok"))).unwrap_or(false);
                    row.push(format!("{ph}={}{}", shown(st.get("state")), if ok { "✓" } else { "·" }));
                }
            }
        }
        let missing = if phases_dir.join(node).is_dir() { "" } else { "  ⚠ phases/ 디렉터리 없음" };
        l.push(format!("- {node}: {}{missing}", row.join(" ")));
    }

    let jour = read_journey(&base);
    l.push(String::new());
    l.push("## 4. 여정 (마지막 3줄)".to_string());
    for e in jour.iter().rev().take(3).rev() {
        let d = dict_of(e).cloned().unwrap_or_default();
        let axis = if truthy(d.get("axis_citation")) {
            format!("   (축: {})", short(d.get("axis_citation"), 90))
        } else {
            String::new()
        };
        l.push(format!(
            "- [{}] {} → {}{axis}",
            shown(d.get("utc")),
            shown(d.get("cell_id")),
            short(d.get("next_intent"), 160)
        ));
    }
    if jour.is_empty() {
        l.push("- (여정 줄 없음 — **결손**. 이 체인에서 유일하게 복원 불가능한 정보다)".to_string());
    }

    l.push(String::new());
    l.push("## 5. 다음에 할 일".to_string());
    let last_intent = jour.last().and_then(dict_of).and_then(|d| d.get("next_intent")).filter(|v| v.is_truthy());
    let more = if pending.len() > 1 { format!("   · 이후 {}건", pending.len() - 1) } else { String::new() };
    l.push(format!(
        "- 다음 pending 셀: {}{more}",
        pending.first().cloned().unwrap_or_else(|| "없음(전 셀 종결)".to_string())
    ));
    l.push(format!(
        "- 마지막 여정이 지목한 다음 의도: {}",
        last_intent.map(|v| shown(Some(v))).unwrap_or_else(|| "(없음)".to_string())
    ));
    l.push(String::new());
    l.push("## 6. 반증된 축 — **재시도 금지**".to_string());
    if refuted.is_empty() {
        l.push("- (없음)".to_string());
    }
    for (cell, cite, why) in &refuted {
        let why_text = match why {
            Some(w) if w.is_truthy() => short(Some(w), 160),
            _ => "(사유 미기재)".to_string(),
        };
        let cite_text = match cite {
            Some(c) if c.is_truthy() => format!("   [축: {}]", short(Some(c), 110)),
            _ => String::new(),
        };
        l.push(format!("- `{cell}`: {why_text}{cite_text}"));
    }
    let ep = read_json(&base.join("evidence_pointers.json"));
    let ep_dict = ep.as_ref().and_then(dict_of);
    let ptrs: Vec<PyValue> = ep_dict.and_then(|d| d.get("pointers")).and_then(list_of).cloned().unwrap_or_default();
    let mut kinds: Vec<String> = ptrs
        .iter()
        .filter_map(dict_of)
        .map(|p| shown(p.get("kind")))
        .collect();
    kinds.sort();
    kinds.dedup();
    l.push(String::new());
    l.push("## 7. 증거".to_string());
    l.push(format!(
        "- 포인터 {}건 · 종류 {}",
        ptrs.len(),
        if kinds.is_empty() { "없음".to_string() } else { py_list_repr(&kinds) }
    ));
    l.push(format!(
        "- relay_summary: {}",
        if kinds.iter().any(|k| k == "relay_summary") { "있음" } else { "**없음(purge 선행조건 미충족)**" }
    ));
    if let Some(frozen) = ep_dict.and_then(|d| d.get("frozen_utc")).filter(|v| v.is_truthy()) {
        l.push(format!("- 스냅샷 동결: {} (이후 --evidence-add 는 거부된다)", shown(Some(frozen))));
    }
    Ok(l.join("\n") + "\n")
}

fn op_resume_brief(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let _ = io.out.write_all(resume_brief(ctx, args.get("--campaign-id"))?.as_bytes());
    Ok(0)
}

// ── --emit-slice ─────────────────────────────────────────────────────────────────────────────

fn op_emit_slice(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let node = args.get("--emit-slice").unwrap_or_default().to_string();
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--emit-slice 는 --utc 가 필요하다(시각은 주입만 받는다)");
    };
    let Some((base, _)) = writer_target(ctx, args.get("--campaign-id"))? else {
        return refuse("활성 캠페인이 없다(_bootstrap) — 자를 선언이 없다");
    };
    let decl = read_declaration(&base);
    if decl.is_empty() {
        return refuse(format!("선언을 읽지 못했다: {}", ctx.rel(&base.join("campaign.yaml"))));
    }
    need_validator(ctx)?;
    let all_nodes: Vec<&PyDict> = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .collect();
    let nodes: Vec<&PyDict> = all_nodes.iter().copied().filter(|n| get_text(n, "node_id").as_deref() == Some(node.as_str())).collect();
    if nodes.is_empty() {
        return refuse(format!("nodes[] 에 {} 가 없다 — 없는 노드에 배정을 자를 수 없다", py_repr(&node)));
    }
    let items: Vec<PyValue> = assignments_of(&decl)
        .into_iter()
        .find(|(n, _)| *n == node)
        .and_then(|(_, v)| list_of(&v).map(|l| l.iter().filter(|x| dict_of(x).is_some()).cloned().collect()))
        .unwrap_or_default();
    if items.is_empty() {
        return refuse(format!("assignments[{}] 가 비었다 — 보낼 배정이 없는 지시서는 지시서가 아니다", py_repr(&node)));
    }
    let mains: Vec<String> = all_nodes
        .iter()
        .filter(|n| get_text(n, "role").as_deref() == Some("main"))
        .filter_map(|n| get_text(n, "node_id"))
        .collect();
    let own_targets: Vec<PyValue> = list_or_empty(decl.get("hint_targets"))
        .into_iter()
        .filter(|t| dict_of(t).map(|d| get_text(d, "node_id").as_deref() == Some(node.as_str())).unwrap_or(false))
        .collect();
    let mut carried: Vec<PyValue> = Vec::new();
    if !own_targets.is_empty() && get_text(nodes[0], "role").as_deref() == Some("sub") {
        let why = crate::campaign_validator::hint_target_reasons(&decl);
        if !why.is_empty() {
            return refuse(format!(
                "메인 선언의 hint_targets 에 결함이 있다 — 결함 있는 승인을 운반하지 않는다:\n{}",
                why.iter().map(|b| format!("  - {b}")).collect::<Vec<_>>().join("\n")
            ));
        }
        // carrying an approval asks the role contract whether this topology publishes by itself
        // the contract program is its launcher (the retired node_role_contract.py beside it may be absent)
        let contract = ctx.root.join(".claude/skills/terraforming_node/scripts/node_role_contract.sh");
        if !contract.is_file() {
            return refuse(format!(
                "계약기를 찾지 못했다: {} — 파생 선언은 메인에서만 자른다",
                ctx.rel(&contract)
            ));
        }
        let topology = nodes[0].get("topology").cloned().unwrap_or(PyValue::None);
        let known = text_of(&topology).map(|t| ["single", "multi"].contains(&t.as_str())).unwrap_or(false);
        if !known {
            return refuse(format!(
                "노드 topology {} 를 모른다 — 그 노드 몫 hint 승인을 운반할지 판정할 수 없다(어휘 ['single', 'multi'] · 추측 ✗)",
                topology.repr()
            ));
        }
        use agent_runtime_support::pyvalue::PyValue as ContractValue;
        let topology_name = text_of(&topology).unwrap_or_default();
        let verdict = role_contract::sub_self_publishes(&ContractValue::str(&topology_name), &ContractValue::None)
            .map_err(|_| Stop::Fallback)?;
        if matches!(verdict.value.peel(), ContractValue::Bool(true)) {
            carried = own_targets.clone();
        }
    }
    let truthy_or = |key: &str, default: PyValue| match decl.get(key) {
        Some(v) if v.is_truthy() => v.clone(),
        _ => default,
    };
    let revisions = truthy_or("revisions", PyValue::List(Vec::new()));
    let assignment_revision = list_or_empty(decl.get("assignment_revisions"))
        .iter()
        .rev()
        .filter_map(dict_of)
        .find(|r| get_text(r, "node").as_deref() == Some(node.as_str()))
        .map(|r| {
            let mut d = PyDict::new();
            for k in ["rev", "reason", "evidence", "approved_by", "declared_utc"] {
                d.set(k, r.get(k).cloned().unwrap_or(PyValue::None));
            }
            PyValue::Dict(d)
        })
        .unwrap_or(PyValue::None);
    let mut out = PyDict::new();
    out.set("schema_version", PyValue::int(1));
    out.set("id", decl.get("id").cloned().unwrap_or(PyValue::None));
    out.set("plan_ref", decl.get("plan_ref").cloned().unwrap_or(PyValue::None));
    out.set("declared_utc", decl.get("declared_utc").cloned().unwrap_or(PyValue::None));
    out.set("nodes", PyValue::List(nodes.iter().map(|n| PyValue::Dict((*n).clone())).collect()));
    out.set("matrix", decl.get("matrix").cloned().unwrap_or(PyValue::None));
    let mut assignments = PyDict::new();
    assignments.set(&node, PyValue::List(items));
    out.set("assignments", PyValue::Dict(assignments));
    out.set("budgets", decl.get("budgets").cloned().unwrap_or(PyValue::None));
    out.set("control_variables", decl.get("control_variables").cloned().unwrap_or(PyValue::None));
    out.set("revisions", revisions);
    out.set("hint_targets", PyValue::List(carried));
    out.set("topology_sections", truthy_or("topology_sections", PyValue::Dict(PyDict::new())));
    out.set("self_role", nodes[0].get("role").cloned().unwrap_or(PyValue::None));
    out.set("authored_by", py("main-derived"));
    out.set("derived_from", mains.first().map(|m| py(m)).unwrap_or(PyValue::None));
    out.set("derived_utc", py(utc));
    out.set("_lifecycle_parent", fold_lifecycle(ctx, &base, None).parent);
    out.set("_assignment_revision", assignment_revision);
    let blob = dump(&PyValue::Dict(out), 2)? + "\n";
    match args.get("--out") {
        Some(path) => {
            write_text(Path::new(path), &blob)?;
            io.say(&format!("[campaign_init] slice({node}) → {path}"));
        }
        None => {
            let _ = io.out.write_all(blob.as_bytes());
        }
    }
    Ok(0)
}

// ── lifecycle reads and echoes ───────────────────────────────────────────────────────────────

fn lifecycle_event(kind: &str, utc: &str, extra: Vec<(&str, PyValue)>) -> PyValue {
    let mut e = PyDict::new();
    e.set("utc", py(utc));
    e.set("kind", py(kind));
    e.set("lifecycle_v", PyValue::int(LIFECYCLE_V));
    e.set("source", py("campaign_init"));
    for (k, v) in extra {
        e.set(k, v);
    }
    PyValue::Dict(e)
}

fn transition_refuse<T>(text: impl Into<String>) -> R<T> {
    refuse(text)
}

fn require_transition_pair(utc: Option<&str>, approved_by: Option<&str>, what: &str) -> R<()> {
    require_transition(utc.filter(|s| !s.is_empty()), approved_by.filter(|s| !s.is_empty()), what)
}

fn last_ref(f: &Fold) -> PyValue {
    f.last_transition_event
        .as_ref()
        .and_then(dict_of)
        .and_then(|d| d.get("ref").cloned())
        .unwrap_or(PyValue::None)
}

fn print_messages(io: &mut Sinks<'_>, mode: &str, messages: &[String]) {
    for m in messages {
        io.say(&format!("[campaign_init] {mode} {m}"));
    }
}

fn op_answer_query(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--answer-query").unwrap_or_default().to_string();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    require_transition_pair(args.get("--utc"), args.get("--approved-by"), "--answer-query")?;
    if args.get("--query-decision") != Some("keep") {
        return transition_refuse("--answer-query 는 keep 만 받는다 — 재개는 --resume-campaign, 포기는 --abandon");
    }
    let base = ctx.campaigns().join(&camp);
    let f = fold_lifecycle(ctx, &base, None);
    if f.state != "suspended" && f.state != "resume_pending" {
        return transition_refuse(format!("{camp} 은 {} 다 — 답할 보류 질의가 없다", f.state));
    }
    if f.answered {
        print_messages(
            io,
            mode,
            &[format!("no-op — {camp} 의 마지막 전이({})는 이미 답했다(멱등)", shown(Some(&last_ref(&f))))],
        );
        return Ok(0);
    }
    if args.has("--apply") {
        let trigger = dict_of(&f.pending_query)
            .and_then(|d| d.get("trigger"))
            .filter(|v| v.is_truthy())
            .cloned()
            .unwrap_or_else(|| py("explicit"));
        append_journey(
            &base,
            &lifecycle_event(
                "campaign_query_answered",
                args.get("--utc").unwrap_or_default(),
                vec![
                    ("decision", py("keep")),
                    ("approved_by", args.get("--approved-by").map(py).unwrap_or(PyValue::None)),
                    ("trigger", trigger),
                    ("answers", last_ref(&f)),
                ],
            ),
        )?;
    }
    print_messages(io, mode, &[format!("answered keep {camp}")]);
    Ok(0)
}

fn op_sub_echo(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--sub-echo").unwrap_or_default().to_string();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    let (Some(node), Some(control)) = (
        args.get("--node").filter(|s| !s.is_empty()),
        args.get("--control").filter(|s| !s.is_empty()),
    ) else {
        return transition_refuse("--sub-echo 는 --node 와 --control 이 필요하다");
    };
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return transition_refuse("--sub-echo 는 --utc 가 필요하다");
    };
    let base = ctx.campaigns().join(&camp);
    let f = fold_lifecycle(ctx, &base, None);
    if control == "reslice" {
        match f.reslice_pending.iter().find(|(n, _)| n == node) {
            None => {
                let pending = PyValue::Dict({
                    let mut d = PyDict::new();
                    for (n, v) in &f.reslice_pending {
                        d.set(n, v.clone());
                    }
                    d
                });
                return transition_refuse(format!("{node} 에 미수렴 재배정이 없다({})", pending.repr()));
            }
            Some(_) => {
                // `--rev` is not an option of the owner parser: the rev is always None
                let want = f.reslice_pending.iter().find(|(n, _)| n == node).map(|(_, v)| shown(Some(v))).unwrap_or_default();
                return transition_refuse(format!("reslice echo 의 rev None 가 최신 재배정 rev {want} 보다 낡았다"));
            }
        }
    } else if !f.sub_nodes.iter().filter_map(text_of).any(|n| n == node) {
        return transition_refuse(format!(
            "{node} 는 {camp} 의 마지막 전이가 지목한 서브가 아니다({})",
            PyValue::List(f.sub_nodes.clone()).repr()
        ));
    }
    if args.has("--apply") {
        append_journey(
            &base,
            &lifecycle_event(
                "sub_control_echo",
                utc,
                vec![("node", py(node)), ("control", py(control))],
            ),
        )?;
    }
    print_messages(io, mode, &[format!("sub echo {camp}@{node}:{control}")]);
    Ok(0)
}

fn op_sub_unconverged_accept(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--sub-unconverged-accept").unwrap_or_default().to_string();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    let Some(node) = args.get("--node").filter(|s| !s.is_empty()) else {
        return transition_refuse("--sub-unconverged-accept 는 --node 가 필요하다");
    };
    require_transition_pair(args.get("--utc"), args.get("--approved-by"), "--sub-unconverged-accept")?;
    let base = ctx.campaigns().join(&camp);
    if !is_dir(&base) {
        return transition_refuse(format!("인스턴스가 없다: campaigns/{camp}"));
    }
    let f = fold_lifecycle(ctx, &base, None);
    if f.state != "closed" {
        return transition_refuse(format!(
            "{camp} 은 {} 다 — 수용은 종결된 캠페인의 서브 미수렴에만 한다(보류·재개 미수렴은 차단이 아니다)",
            f.state
        ));
    }
    if !f.sub_nodes.iter().filter_map(text_of).any(|n| n == node) {
        return transition_refuse(format!(
            "{node} 는 {camp} 의 종결이 지목한 서브가 아니다({})",
            PyValue::List(f.sub_nodes.clone()).repr()
        ));
    }
    if f.sub_echoed.iter().any(|n| n == node) {
        print_messages(io, mode, &[format!("no-op — {camp}@{node} 는 이미 수렴했다(멱등)")]);
        return Ok(0);
    }
    if args.has("--apply") {
        let control = if f.close_reason.as_deref() == Some("abandoned") { "abandon" } else { "close" };
        append_journey(
            &base,
            &lifecycle_event(
                "sub_control_accepted",
                args.get("--utc").unwrap_or_default(),
                vec![
                    ("node", py(node)),
                    ("approved_by", args.get("--approved-by").map(py).unwrap_or(PyValue::None)),
                    ("control", py(control)),
                ],
            ),
        )?;
    }
    print_messages(io, mode, &[format!("accepted sub-unconverged {camp}@{node}")]);
    Ok(0)
}

fn fold_value(ctx: &Ctx, base: &Path, f: &Fold) -> PyValue {
    let mut d = PyDict::new();
    d.set("campaign_id", py(&f.campaign_id));
    d.set("state", py(&f.state));
    d.set("close_reason", f.close_reason.as_deref().map(py).unwrap_or(PyValue::None));
    d.set("parent", f.parent.clone());
    d.set("topology", f.topology.as_deref().map(py).unwrap_or(PyValue::None));
    d.set("suspended_pins", f.suspended_pins.clone());
    d.set("released_resources", PyValue::List(f.released_resources.clone()));
    d.set("interrupted_cells", str_list(f.interrupted_cells.clone()));
    d.set("last_transition_event", f.last_transition_event.clone().unwrap_or(PyValue::None));
    d.set("answered", PyValue::Bool(f.answered));
    d.set("pending_query", f.pending_query.clone());
    d.set("sub_convergence", f.sub_convergence.as_deref().map(py).unwrap_or(PyValue::None));
    d.set("sub_nodes", PyValue::List(f.sub_nodes.clone()));
    d.set("sub_echoed", str_list(f.sub_echoed.clone()));
    d.set("handoff_pointers", PyValue::List(f.handoff_pointers.clone()));
    d.set("parked_devlog", f.parked_devlog.clone());
    d.set("open_pending", f.open_pending.clone());
    d.set("sub_accepted", str_list(f.sub_accepted.clone()));
    let mut r = PyDict::new();
    for (n, v) in &f.reslice_pending {
        r.set(n, v.clone());
    }
    d.set("reslice_pending", PyValue::Dict(r));
    d.set("unknown_events", PyValue::List(f.unknown_events.clone()));
    d.set("source", py(&format!("{} fold", ctx.rel(&base.join(JOURNEY_NAME)))));
    PyValue::Dict(d)
}

fn op_lifecycle(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let which = args.get("--lifecycle").unwrap_or("*");
    let value = if which == "*" {
        let table = lifecycle_table(ctx);
        let mut campaigns = PyDict::new();
        for (name, f) in &table {
            campaigns.set(name, fold_value(ctx, &ctx.campaigns().join(name), f));
        }
        let mut d = PyDict::new();
        d.set("line", py(&lifecycle_line(ctx, &table)));
        d.set("campaigns", PyValue::Dict(campaigns));
        PyValue::Dict(d)
    } else {
        let base = ctx.campaigns().join(which);
        fold_value(ctx, &base, &fold_lifecycle(ctx, &base, None))
    };
    io.say(&dump(&value, 2)?);
    Ok(0)
}

// ── --residue-scan ───────────────────────────────────────────────────────────────────────────

/// `fnmatch.fnmatch(name, pattern)` on POSIX (case-sensitive): `*`, `?`, `[seq]`, `[!seq]`.
fn fnmatch(name: &str, pattern: &str) -> bool {
    fn go(n: &[char], p: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|k| go(&n[k..], &p[1..])),
            Some('?') => !n.is_empty() && go(&n[1..], &p[1..]),
            Some('[') => {
                let mut i = 1;
                if p.get(i) == Some(&'!') {
                    i += 1;
                }
                if p.get(i) == Some(&']') {
                    i += 1;
                }
                while i < p.len() && p[i] != ']' {
                    i += 1;
                }
                if i >= p.len() {
                    return !n.is_empty() && n[0] == '[' && go(&n[1..], &p[1..]);
                }
                if n.is_empty() {
                    return false;
                }
                let (negate, body) = match p[1] {
                    '!' => (true, &p[2..i]),
                    _ => (false, &p[1..i]),
                };
                let mut hit = false;
                let mut k = 0;
                while k < body.len() {
                    if k + 2 < body.len() && body[k + 1] == '-' {
                        hit |= body[k] <= n[0] && n[0] <= body[k + 2];
                        k += 3;
                    } else {
                        hit |= body[k] == n[0];
                        k += 1;
                    }
                }
                hit != negate && go(&n[1..], &p[i + 1..])
            }
            Some(c) => !n.is_empty() && n[0] == *c && go(&n[1..], &p[1..]),
        }
    }
    let (n, p): (Vec<char>, Vec<char>) = (name.chars().collect(), pattern.chars().collect());
    go(&n, &p)
}

fn residue_scan(ctx: &Ctx) -> PyDict {
    let registry = read_json(&ctx.root.join(".claude/policies/root_registry.json"));
    let registry = registry.as_ref().and_then(dict_of).cloned().unwrap_or_default();
    let declared: Vec<String> = list_or_empty(registry.get("entries"))
        .iter()
        .filter_map(dict_of)
        .filter_map(|e| get_text(e, "name"))
        .collect();
    let tombs: Vec<String> = list_or_empty(registry.get("tombstones"))
        .iter()
        .filter_map(dict_of)
        .filter_map(|t| get_text(t, "name"))
        .filter(|n| !n.is_empty())
        .collect();
    let (mut workspace, mut leak, mut unclassified, mut volatile) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut children: Vec<String> = std::fs::read_dir(&ctx.root)
        .map(|r| r.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    children.sort();
    for name in children {
        if declared.contains(&name) {
            workspace.push(name);
        } else if tombs.iter().any(|pat| fnmatch(&name, pat)) {
            leak.push(name);
        } else {
            unclassified.push(name);
        }
    }
    let mut camps: Vec<String> = std::fs::read_dir(ctx.campaigns())
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    camps.sort();
    for name in camps {
        if name != "_template" {
            volatile.push(format!("campaigns/{name}"));
        }
    }
    let mut out = PyDict::new();
    out.set("workspace", str_list(workspace));
    out.set("evidence", PyValue::List(Vec::new()));
    out.set("volatile", str_list(volatile));
    out.set("leak", str_list(leak));
    out.set("unclassified", str_list(unclassified));
    out
}

fn op_residue_scan(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let out = residue_scan(ctx);
    io.say(&dump(&PyValue::Dict(out.clone()), 2)?);
    match args.get("--utc").filter(|u| !u.is_empty()) {
        Some(utc) => {
            let camp = args.get("--campaign-id").filter(|c| !c.is_empty()).map(str::to_string).unwrap_or_else(|| ctx.active_id());
            let base = ctx.campaigns().join(&camp);
            if camp == BOOTSTRAP || !is_dir(&base) {
                io.warn("[campaign_init] residue → (활성 캠페인 없음 — 미기재)");
            } else {
                let path = base.join("residue.json");
                let mut doc = PyDict::new();
                if let Some(old) = read_json(&path).as_ref().and_then(dict_of) {
                    for (k, v) in old.iter() {
                        if k.to_utf8().map(|s| s.starts_with('_')).unwrap_or(false) {
                            doc.insert(k.clone(), v.clone());
                        }
                    }
                }
                doc.set("schema_version", PyValue::int(1));
                doc.set("scanned_utc", py(utc));
                for (k, v) in out.iter() {
                    doc.insert(k.clone(), v.clone());
                }
                write_json(&path, &PyValue::Dict(doc))?;
                io.warn(&format!("[campaign_init] residue → {}", ctx.rel(&path)));
            }
        }
        None => io.warn("[campaign_init] --utc 가 없어 residue.json 에 남기지 않았다(시각은 주입만 받는다 — 벽시계 금지)"),
    }
    Ok(0)
}

// ── --backfill-from-docs and --import-sub ────────────────────────────────────────────────────

fn number_text(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Int(n) => PyValue::Float(i64::try_from(n.clone()).map(|v| v as f64).unwrap_or(0.0)).repr(),
        other => other.repr(),
    }
}

fn plain_number(value: Option<&PyValue>) -> Option<&PyValue> {
    value.filter(|v| matches!(v.peel(), PyValue::Int(_) | PyValue::Float(_)))
}

fn sorted_files(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| { let n = n.to_string_lossy(); n.starts_with(prefix) && n.ends_with(suffix) }).unwrap_or(false))
        .collect();
    out.sort();
    out
}

fn op_backfill(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    need_validator(ctx)?;
    let utc = args.get("--utc").filter(|u| !u.is_empty()).map(str::to_string);
    let decl = read_declaration(&base);
    let mut sources = sorted_files(&base.join("sweeps"), "", ".json");
    for dir in [ctx.root.join("docs").join("benchmark"), ctx.root.join("sync_staging").join("sub_docs").join("benchmark")] {
        sources.extend(sorted_files(&dir, "sweep_map_", ".json"));
    }
    let mut records: Vec<(String, (PyValue, PyValue, String))> = Vec::new();
    for src in sources {
        let Some(doc) = read_json(&src) else { continue };
        let Some(doc) = dict_of(&doc) else { continue };
        for rec in list_or_empty(doc.get("cells")).iter().filter_map(dict_of) {
            let key = get_text(rec, "cell_key");
            let val = get_dict(rec, "concurrency_vector").and_then(|v| v.get("1"));
            if let (Some(key), Some(val)) = (key, plain_number(val)) {
                if !records.iter().any(|(k, _)| *k == key) {
                    records.push((
                        key,
                        (val.clone(), rec.get("cell_outcome").cloned().unwrap_or(PyValue::None), ctx.rel(&src)),
                    ));
                }
            }
        }
    }
    let cells_dir = base.join("cells");
    let mut names: Vec<String> = assigned_cells(&decl, None);
    if let Ok(read) = std::fs::read_dir(&cells_dir) {
        for e in read.filter_map(Result::ok).filter(|e| e.path().is_dir()) {
            let n = e.file_name().to_string_lossy().into_owned();
            if n != "_cell" && !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names.sort();
    let (mut repaired, mut gaps): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for cell in names {
        let st = read_json(&cells_dir.join(&cell).join("cell.status.json"));
        let st = st.as_ref().and_then(dict_of).cloned().unwrap_or_default();
        let meas = get_dict(&st, "measurement").cloned().unwrap_or_default();
        let need_tps = meas.get("decode_tps_conc1").map(|v| matches!(v.peel(), PyValue::None)).unwrap_or(true);
        let need_node = !truthy(st.get("node_id"));
        if !(need_tps || need_node || st.is_empty()) {
            continue;
        }
        let rec = records.iter().find(|(k, _)| *k == cell).map(|(_, r)| r);
        if need_tps && rec.is_none() {
            gaps.push(format!("{cell}: 측정값이 어느 자리에도 없다(스윕 기록·sweep map 전수 조회)"));
            if !st.is_empty() && !need_node {
                continue;
            }
        }
        let outcome = match st.get("cell_outcome").filter(|v| v.is_truthy()).or(rec.map(|r| &r.1).filter(|v| v.is_truthy())) {
            Some(v) => text_of(v).filter(|t| CELL_OUTCOMES.contains(&t.as_str())).unwrap_or_else(|| "pending".to_string()),
            None => "pending".to_string(),
        };
        let mut pairs: Vec<(&str, String)> = vec![("--outcome", outcome.clone())];
        if let Some(n) = st.get("node_id").and_then(text_of) {
            pairs.push(("--node", n));
        }
        for (flag, key) in [("--version", "version"), ("--model", "model"), ("--void-reason", "void_reason"), ("--void-reason-source", "void_reason_source"), ("--axis-citation", "axis_citation")] {
            if let Some(v) = st.get(key).and_then(text_of) {
                pairs.push((flag, v));
            }
        }
        if let (true, Some(r)) = (need_tps, rec) {
            pairs.push(("--measurement-decode-tps", number_text(&r.0)));
            pairs.push(("--measurement-source", format!("backfill:{}#cells[cell_key={cell}].concurrency_vector.1", r.2)));
        }
        if let Some(u) = &utc {
            pairs.push(("--utc", u.clone()));
        }
        let (cpath, _) = write_cell(ctx, &base, &Args::synthetic(pairs), &cell, &outcome)?;
        let after = read_json(&cpath);
        if !after.as_ref().and_then(dict_of).map(|d| truthy(d.get("node_id"))).unwrap_or(false) {
            gaps.push(format!("{cell}: 노드를 배정에서 파생하지 못했다(assignments 에 없는 셀)"));
        }
        repaired.push(ctx.rel(&cpath));
    }
    for g in &gaps {
        io.warn(&format!("[campaign_init] ⚠ 남은 결손 — {g}"));
    }
    wrote_line(io, &camp, &repaired);
    Ok(0)
}

const SUB_ID_OK: fn(&str) -> bool = |t| {
    let mut c = t.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphanumeric()) && c.all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '.' || x == '-')
};

/// `_sub_evidence_binding(base, node, cell_id)`.
fn sub_evidence_binding(base: &Path, node: &PyValue, cell_id: &PyValue) -> R<()> {
    let decl = read_declaration(base);
    let own: Vec<&PyDict> = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .filter(|n| n.get("node_id") == Some(node))
        .collect();
    let node_text = text_of(node);
    if !node_text.as_deref().map(SUB_ID_OK).unwrap_or(false) || own.len() != 1 || get_text(own[0], "role").as_deref() != Some("sub") {
        return refuse(format!("서브 증거 node_id가 선언된 sub가 아니다: {}", node.repr()));
    }
    if !matches!(cell_id.peel(), PyValue::None) {
        let cell_text = text_of(cell_id);
        let ok = cell_text.as_deref().map(|c| SUB_ID_OK(c) && node_of_cell(base, c) == node_text).unwrap_or(false);
        if !ok {
            return refuse(format!("서브 증거 셀이 이 노드 배정과 다르다: node={}, cell={}", node.repr(), cell_id.repr()));
        }
    }
    Ok(())
}

/// `Path.resolve()` (non-strict): absolute, symlinks of the part that exists followed, the rest kept.
fn resolve_nonstrict(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|c| c.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut head = absolute.clone();
    loop {
        if let Ok(real) = std::fs::canonicalize(&head) {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (head.file_name().map(|n| n.to_os_string()), head.parent().map(Path::to_path_buf)) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                head = parent;
            }
            _ => return absolute,
        }
    }
}

/// `_sub_evidence_path(kind, path_rel, docs_root)`.
fn sub_evidence_path(kind: &PyValue, path_rel: &PyValue, docs_root: &Path) -> R<PathBuf> {
    let kind_text = text_of(kind).filter(|k| EVIDENCE_KINDS.contains(&k.as_str()));
    let path = text_of(path_rel);
    let parts_ok = path.as_deref().map(|p| {
        let parts: Vec<&str> = p.split('/').filter(|x| !x.is_empty() && *x != ".").collect();
        p.starts_with("docs/") && !parts.contains(&"..") && !p.starts_with('/') && parts.len() >= 3
    });
    let (Some(_), Some(path), Some(true)) = (kind_text, path, parts_ok) else {
        return refuse(format!(
            "서브 evidence claim은 docs 아래의 유효 kind/상대경로여야 한다: {}, {}",
            kind.repr(),
            path_rel.repr()
        ));
    };
    no_fill(&[("path_rel", &path)])?;
    let local = docs_root.join(path.strip_prefix("docs/").unwrap_or(&path));
    let mut chain: Vec<&Path> = vec![local.as_path()];
    chain.extend(local.ancestors().skip(1));
    if docs_root.is_symlink() || chain.iter().any(|p| *p != docs_root && p.starts_with(docs_root) && p.is_symlink()) {
        return refuse(format!("서브 evidence 경로에 symlink가 있다: {path}"));
    }
    if !resolve_nonstrict(&local).starts_with(resolve_nonstrict(docs_root)) {
        return refuse(format!("서브 evidence 경로가 docs/mirror 밖으로 벗어난다: {path}"));
    }
    Ok(local)
}

fn journey_key(entry: &PyDict) -> String {
    let mut d = PyDict::new();
    for (k, v) in entry.iter() {
        if !k.equals("imported_from") && !k.equals("imported_utc") {
            d.insert(k.clone(), v.clone());
        }
    }
    sorted_value(&PyValue::Dict(d)).dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default()
}

fn op_import_sub(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--import-sub 는 --utc 가 필요하다(시각은 주입만 받는다)");
    };
    need_validator(ctx)?;
    if get_text(&read_declaration(&base), "self_role").as_deref() == Some("sub") {
        return Err(Stop::Fallback);
    }
    let mirror = PathBuf::from(args.get("--import-sub").unwrap_or_default());
    let mut discovered: Vec<PathBuf> = Vec::new();
    if let Ok(read) = std::fs::read_dir(mirror.join("logs")) {
        for e in read.filter_map(Result::ok) {
            let b = e.path().join("campaign_brief.json");
            if b.is_file() {
                discovered.push(b);
            }
        }
    }
    discovered.sort();
    if discovered.is_empty() {
        return refuse(format!(
            "회수 미러에 브리핑이 없다: {}/logs/*/campaign_brief.json — 서브가 `campaign_init --write-brief` 를 돌지 않았거나 회수 범위가 docs/logs 를 뺐다.",
            ctx.rel(&mirror)
        ));
    }
    let name = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut briefs: Vec<(PathBuf, PyDict)> = Vec::new();
    for bpath in &discovered {
        let doc = read_json(bpath);
        let Some(doc) = doc.as_ref().and_then(dict_of) else {
            return refuse(format!("브리핑 파손: {}", ctx.rel(bpath)));
        };
        if get_text(doc, "campaign_id").as_deref() != Some(name.as_str()) {
            continue;
        }
        if get_text(doc, "node_id").filter(|n| !n.is_empty()).is_none() {
            return refuse(format!("브리핑에 node_id 가 없다: {}", ctx.rel(bpath)));
        }
        briefs.push((bpath.clone(), doc.clone()));
    }
    if briefs.is_empty() {
        return refuse(format!("현재 캠페인({name})의 브리핑이 회수 미러에 없다 — 다른 캠페인의 역사로 현재 진행표를 만들지 않는다"));
    }
    let falsy_or = |v: Option<&PyValue>, default: PyValue| match v {
        Some(x) if x.is_truthy() => x.clone(),
        _ => default,
    };
    let mut claim_paths: Vec<((usize, usize), PathBuf)> = Vec::new();
    for (bi, (bpath, doc)) in briefs.iter().enumerate() {
        let node = doc.get("node_id").cloned().unwrap_or(PyValue::None);
        sub_evidence_binding(&base, &node, &PyValue::None)?;
        let phases = falsy_or(doc.get("phases"), PyValue::Dict(PyDict::new()));
        let cells = falsy_or(doc.get("cells"), PyValue::List(Vec::new()));
        let journey = falsy_or(doc.get("journey_tail"), PyValue::List(Vec::new()));
        let (Some(phases), Some(cells), Some(journey)) = (dict_of(&phases), list_of(&cells), list_of(&journey)) else {
            return refuse(format!("브리핑의 phases/cells/journey_tail 형식이 무효다: {}", ctx.rel(bpath)));
        };
        for (phase, st) in phases.iter() {
            let phase = phase.to_utf8().unwrap_or_default();
            let Some(st) = dict_of(st) else { continue };
            if !PHASE_NAMES.contains(&phase.as_str()) {
                continue;
            }
            let state = falsy_or(st.get("state"), py("pending"));
            if !text_of(&state).map(|t| PHASE_STATES.contains(&t.as_str())).unwrap_or(false) {
                return refuse(format!("브리핑 phase state가 무효다: {phase}={}", st.get("state").map(PyValue::repr).unwrap_or_else(|| "None".to_string())));
            }
            if let Some(src) = st.get("proof_source").and_then(text_of) {
                no_fill(&[("source", &src)])?;
            }
        }
        for cell in cells.iter().filter_map(dict_of) {
            let known = text_of(&cell.get("cell_outcome").cloned().unwrap_or(PyValue::None)).map(|o| CELL_OUTCOMES.contains(&o.as_str())).unwrap_or(false);
            if truthy(cell.get("cell_id")) && known {
                sub_evidence_binding(&base, &node, cell.get("cell_id").expect("truthy"))?;
            }
        }
        for entry in journey.iter() {
            if let Some(d) = dict_of(entry) {
                if !is_lifecycle(entry) {
                    let who = d.get("node_id").cloned().unwrap_or(PyValue::None);
                    if !matches!(who.peel(), PyValue::None) && who != node {
                        return refuse("브리핑 journey의 node_id가 brief node와 다르다");
                    }
                    sub_evidence_binding(&base, &node, &d.get("cell_id").cloned().unwrap_or(PyValue::None))?;
                }
            }
        }
        let Some(claims) = doc.get("evidence_pointers").filter(|v| !matches!(v.peel(), PyValue::None)) else { continue };
        let Some(claims) = list_of(claims) else {
            return refuse(format!("브리핑 evidence_pointers 가 목록이 아니다: {}", ctx.rel(bpath)));
        };
        for (i, claim) in claims.iter().enumerate() {
            let Some(claim) = dict_of(claim) else {
                return refuse(format!("브리핑 evidence claim 이 객체가 아니다: {}", ctx.rel(bpath)));
            };
            let claim_node = claim.get("node_id").cloned().unwrap_or(PyValue::None);
            if claim_node != node {
                return refuse(format!("브리핑 evidence claim node_id({})가 brief node({})와 다르다", claim_node.repr(), node.repr()));
            }
            sub_evidence_binding(&base, &node, &claim.get("cell_id").cloned().unwrap_or(PyValue::None))?;
            let local = sub_evidence_path(&claim.get("kind").cloned().unwrap_or(PyValue::None), &claim.get("path").cloned().unwrap_or(PyValue::None), &mirror)?;
            if !local.starts_with(&ctx.root) {
                return refuse(format!("회수 evidence mirror는 저장소 안이어야 한다: {}", ctx.rel(&mirror)));
            }
            claim_paths.push(((bi, i), local));
        }
    }
    let (mut wrote, mut warnings): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for (bi, (bpath, doc)) in briefs.iter().enumerate() {
        let node = get_text(doc, "node_id").unwrap_or_default();
        let brel = ctx.rel(bpath);
        for (phase, st) in get_dict(doc, "phases").cloned().unwrap_or_default().iter() {
            let phase = phase.to_utf8().unwrap_or_default();
            let Some(st) = dict_of(st) else { continue };
            if !PHASE_NAMES.contains(&phase.as_str()) {
                continue;
            }
            let state = text_of(&falsy_or(st.get("state"), py("pending"))).unwrap_or_default();
            let mut pairs: Vec<(&str, String)> = vec![
                ("--proof-predicate", format!("서브 회수(brief {})", shown(doc.get("generated_utc")))),
                ("--proof-source", match st.get("proof_source").filter(|v| v.is_truthy()) {
                    Some(v) => shown(Some(v)),
                    None => format!("{brel}#phases.{phase}"),
                }),
                ("--authored-by", node.clone()),
            ];
            if truthy(st.get("proof_ok")) {
                pairs.push(("--proof-ok", String::new()));
            }
            if let Some(v) = st.get("first_started_utc").filter(|v| v.is_truthy()).or(st.get("started_utc").filter(|v| v.is_truthy())) {
                pairs.push(("--started-utc", shown(Some(v))));
            }
            if let Some(v) = st.get("ended_utc").filter(|v| v.is_truthy()) {
                pairs.push(("--ended-utc", shown(Some(v))));
            }
            let path = write_phase(ctx, &base, &Args::synthetic(pairs), &node, &phase, &state, None)?;
            wrote.push(ctx.rel(&path));
        }
        for cell in list_or_empty(doc.get("cells")).iter().filter_map(dict_of) {
            let outcome = cell.get("cell_outcome").and_then(text_of).filter(|o| CELL_OUTCOMES.contains(&o.as_str()));
            let (true, Some(outcome)) = (truthy(cell.get("cell_id")), outcome) else { continue };
            let cid = shown(cell.get("cell_id"));
            let mut pairs: Vec<(&str, String)> = vec![("--outcome", outcome.clone()), ("--node", node.clone()), ("--utc", utc.to_string())];
            if let Some(t) = plain_number(cell.get("decode_tps_conc1")) {
                pairs.push(("--measurement-decode-tps", number_text(t)));
                pairs.push(("--measurement-source", format!("{brel}#cells[{cid}]")));
            }
            if let Some(v) = cell.get("void_reason").filter(|v| v.is_truthy()) {
                pairs.push(("--void-reason", shown(Some(v))));
                pairs.push(("--void-reason-source", format!("{brel}#cells[{cid}].void_reason")));
            }
            if let Some(v) = cell.get("axis_citation").and_then(text_of) {
                pairs.push(("--axis-citation", v));
            }
            let (cpath, jpath) = write_cell(ctx, &base, &Args::synthetic(pairs), &cid, &outcome)?;
            wrote.push(ctx.rel(&cpath));
            if let Some(j) = jpath {
                wrote.push(ctx.rel(&j));
            }
        }
        let mut have: Vec<String> = read_journey(&base).iter().filter_map(dict_of).map(journey_key).collect();
        for entry in list_or_empty(doc.get("journey_tail")).iter() {
            if is_lifecycle(entry) {
                continue;
            }
            let Some(d) = dict_of(entry) else { continue };
            let key = journey_key(d);
            if have.contains(&key) {
                continue;
            }
            have.push(key);
            let mut e = d.clone();
            e.set("imported_from", py(&brel));
            e.set("imported_utc", py(utc));
            wrote.push(ctx.rel(&append_journey(&base, &PyValue::Dict(e))?));
        }
        let Some(claims) = doc.get("evidence_pointers").filter(|v| !matches!(v.peel(), PyValue::None)) else {
            warnings.push(format!("브리핑에 scoped evidence_pointers 가 없다: {brel} — 진행표만 편입"));
            continue;
        };
        for (i, claim) in claims.peel_list().iter().enumerate() {
            let claim = dict_of(claim).expect("validated");
            let kind = get_text(claim, "kind").unwrap_or_default();
            let local = claim_paths.iter().find(|(k, _)| *k == (bi, i)).map(|(_, p)| p.clone()).expect("validated");
            if !local.is_file() {
                warnings.push(format!("증거 미등재 {} (brief가 선언했으나 미러에 없다)", ctx.rel(&local)));
                continue;
            }
            let mut pairs: Vec<(&str, String)> = vec![
                ("--kind", kind.clone()),
                ("--path", local.strip_prefix(&ctx.root).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
                ("--node", node.clone()),
            ];
            if let Some(c) = claim.get("cell_id").and_then(text_of) {
                pairs.push(("--cell", c));
            }
            if args.has("--unfreeze") {
                pairs.push(("--unfreeze", String::new()));
            }
            match add_evidence(ctx, &base, &camp, &Args::synthetic(pairs)) {
                Ok(p) => wrote.push(ctx.rel(&p)),
                Err(Stop::Refuse(why)) => warnings.push(format!("증거 미등재 {} (kind={kind}) — {why}", ctx.rel(&local))),
                Err(Stop::Fallback) => return Err(Stop::Fallback),
            }
        }
    }
    for w in &warnings {
        io.warn(&format!("[campaign_init] ⚠ {w}"));
    }
    wrote_line(io, &camp, &wrote);
    Ok(0)
}

trait PeelList {
    fn peel_list(&self) -> Vec<PyValue>;
}

impl PeelList for PyValue {
    fn peel_list(&self) -> Vec<PyValue> {
        list_of(self).cloned().unwrap_or_default()
    }
}

// ── the library (wiki-desk): intake, query, grounding ───────────────────────────────────────

const WIKI_CLI: &str = ".claude/skills/wiki-desk/src/cli.ts";
const GROUNDING_TOP: usize = 6;

struct WikiRun {
    status: i32,
    stdout: String,
    stderr: String,
}

/// `_wiki(root, *args)`: the node-run wiki-desk command line (not Python).
fn wiki(ctx: &Ctx, args: &[String]) -> R<WikiRun> {
    let output = std::process::Command::new("node")
        .arg(ctx.root.join(WIKI_CLI))
        .args(args)
        .arg("--root")
        .arg(&ctx.root)
        .current_dir(&ctx.root)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Stop::Refuse(format!("node 를 시작할 수 없다: {e}")))?;
    Ok(WikiRun {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn tail_chars(text: &str, n: usize) -> String {
    let t = text.trim();
    let count = t.chars().count();
    t.chars().skip(count.saturating_sub(n)).collect()
}

/// `warm_start_library()`: the one-line outcome of the intake.
fn warm_start_library(ctx: &Ctx) -> R<String> {
    if !ctx.root.join(WIKI_CLI).is_file() {
        return Ok(format!("입고 건너뜀 — 사서 도구 부재({WIKI_CLI})"));
    }
    let run = wiki(ctx, &["sync".to_string(), "--apply".to_string()])?;
    if run.status != 0 && run.status != 4 {
        return Ok(format!("입고 실패(rc={}): {}", run.status, tail_chars(&run.stderr, 200)));
    }
    let out = match pyjson::loads(&run.stdout) {
        Ok(v) => v,
        Err(_) => return Ok("입고 완료(출력 판독 불가)".to_string()),
    };
    let out = match out.peel() {
        PyValue::Dict(d) => d.clone(),
        _ => PyDict::new(),
    };
    if get_text(&out, "ingest_status").as_deref() == Some("not-yet-ingested") {
        return Ok(format!(
            "not-yet-ingested — 서고 미도달(stage={}) · 다음 입고가 따라잡는다",
            shown(out.get("stage"))
        ));
    }
    let n = |k: &str| list_or_empty(out.get(k)).len();
    Ok(format!("입고 완료 — created={} updated={} archived={}", n("created"), n("updated"), n("archived")))
}

fn resolution(status: &str, reason: PyValue, extra: Vec<(&str, PyValue)>) -> PyDict {
    let mut r = PyDict::new();
    r.set("status", py(status));
    r.set("librarian", py("wiki-desk"));
    r.set("reason", reason);
    for (k, v) in extra {
        r.set(k, v);
    }
    let mut out = PyDict::new();
    out.set("resolution", PyValue::Dict(r));
    out.set("references", PyValue::List(Vec::new()));
    out
}

/// `query_library(terms)`: the exported pointers, an honest gap or a refusal.
fn query_library(ctx: &Ctx, terms: &[String]) -> R<PyDict> {
    if !ctx.root.join(WIKI_CLI).is_file() {
        return Ok(resolution("unresolved", py(&format!("사서 도구 부재({WIKI_CLI})")), vec![]));
    }
    let mut argv = vec!["query".to_string(), "--limit".to_string(), GROUNDING_TOP.to_string()];
    for t in terms {
        argv.push("--terms".to_string());
        argv.push(t.clone());
    }
    let run = wiki(ctx, &argv)?;
    if run.status == 126 {
        return Ok(resolution("refused", py(&tail_chars(&run.stderr, 300)), vec![]));
    }
    let out = if run.status == 0 { pyjson::loads(&run.stdout).ok() } else { None };
    let Some(out) = out else {
        return Ok(resolution(
            "unresolved",
            py(&format!("사서 질의 실패(rc={}): {}", run.status, tail_chars(&run.stderr, 200))),
            vec![],
        ));
    };
    let out = dict_of(&out).cloned().unwrap_or_default();
    let mut refs: Vec<PyValue> = Vec::new();
    for (i, r) in list_or_empty(out.get("results")).iter().enumerate() {
        let r = dict_of(r).cloned().unwrap_or_default();
        let mut d = PyDict::new();
        d.set("ref_id", py(&format!("R{}", i + 1)));
        d.set("path", r.get("path").cloned().unwrap_or(PyValue::None));
        d.set("score", r.get("score").cloned().unwrap_or(PyValue::None));
        let spans: Vec<PyValue> = list_or_empty(r.get("spans"))
            .iter()
            .map(|sp| dict_of(sp).and_then(|d| d.get("ref").cloned()).unwrap_or(PyValue::None))
            .collect();
        d.set("spans", PyValue::List(spans));
        refs.push(PyValue::Dict(d));
    }
    let resolved = !refs.is_empty();
    let mut res = resolution(
        if resolved { "resolved" } else { "unresolved" },
        if resolved { PyValue::None } else { py("질의 결과 0건(정직한 공백)") },
        vec![
            ("unmatched_tokens", out.get("unmatched_tokens").cloned().unwrap_or(PyValue::None)),
            ("stale", out.get("stale").cloned().unwrap_or(PyValue::None)),
        ],
    );
    res.set("references", PyValue::List(refs));
    Ok(res)
}

/// `_self_node_id()`: `main`, or the node id the main issued in the sub's manifest `library:` block.
fn self_node_id(ctx: &Ctx) -> String {
    if !ctx.checkout_is_sub() {
        return "main".to_string();
    }
    for t in ["single", "multi"] {
        let Some(text) = ctx.manifest_text(t) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_end() != "library:" {
                continue;
            }
            for next in &lines[i + 1..] {
                if !(next.starts_with(' ') || next.starts_with('\t')) {
                    break;
                }
                if let Some(rest) = next.trim_start().strip_prefix("node_id:") {
                    let rest = rest.trim_start();
                    let rest = rest.strip_prefix(['\'', '"']).unwrap_or(rest);
                    let id: String = rest
                        .chars()
                        .enumerate()
                        .take_while(|(k, c)| {
                            if *k == 0 { c.is_ascii_lowercase() || c.is_ascii_digit() } else { c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-' }
                        })
                        .map(|(_, c)| c)
                        .collect();
                    if !id.is_empty() {
                        return id;
                    }
                }
            }
        }
    }
    "sub".to_string()
}

fn grounding_terms(decl: &PyDict) -> Vec<String> {
    let cv = decl.get("control_variables").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    let mut keys: Vec<&(PyStr, PyValue)> = cv.iter().collect();
    keys.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
    let mut out = Vec::new();
    for (k, v) in keys {
        if k.to_utf8().map(|s| s.starts_with('_')).unwrap_or(false) {
            continue;
        }
        if matches!(v.peel(), PyValue::Str(_) | PyValue::Int(_) | PyValue::Float(_)) {
            let text = v.display_text().to_utf8().unwrap_or_default().trim().to_string();
            if !text.is_empty() && !text.contains(FILL) && text.chars().count() <= 80 {
                out.push(text);
            }
        }
    }
    out
}

fn op_warm_start_library(ctx: &Ctx, io: &mut Sinks<'_>) -> R<i32> {
    io.say(&format!("[campaign_init] {}", warm_start_library(ctx)?));
    Ok(0)
}

fn op_ground(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--ground 는 --utc 가 필요하다(시각은 주입만 받는다)");
    };
    need_validator(ctx)?;
    let decl = read_declaration(&base);
    let topology_value = get_dict(&decl, "control_variables").and_then(|c| c.get("topology")).cloned().unwrap_or(PyValue::None);
    let topology = match text_of(&topology_value).filter(|t| t == "single" || t == "multi") {
        Some(t) => t,
        None => {
            return refuse(format!(
                "--ground 는 활성 캠페인의 control_variables.topology 를 요구한다 — 현재 값 {}. 기본값으로 대체하지 않는다(그라운딩 기록에 거짓 사실이 실리면 fail-closed 백스톱이 무의미해진다). 선언을 먼저 고쳐라.",
                topology_value.repr()
            ))
        }
    };
    let given: Vec<String> = args
        .get("--terms")
        .unwrap_or("")
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let terms = if given.is_empty() { grounding_terms(&decl) } else { given };
    let node_id = self_node_id(ctx);
    if terms.is_empty() {
        return refuse("통제변인에서 질의어를 파생하지 못했다 — 선언이 아직 빈칸이면 그라운딩은 선언을 채운 뒤에 한다(진입 백스톱이 그때까지 서빙을 막는다).");
    }
    let note = if args.has("--no-warm-start") { "입고 생략(호출부 지시)".to_string() } else { warm_start_library(ctx)? };
    let exchange = format!("{camp}-{utc}");
    let mut query = PyDict::new();
    query.set("terms", str_list(terms.clone()));
    let mut request = PyDict::new();
    request.set("schema_version", PyValue::int(1));
    request.set("kind", py("library.resolution.request"));
    request.set("exchange_id", py(&exchange));
    request.set("node_id", py(&node_id));
    request.set("topology", py(&topology));
    request.set("query", PyValue::Dict(query));
    request.set("purpose", py("캠페인 착수 그라운딩 — 이 통제변인으로 참고할 내부 자료가 있는가"));
    let export = query_library(ctx, &terms)?;
    let status = get_dict(&export, "resolution").and_then(|r| r.get("status").cloned()).unwrap_or(PyValue::None);
    let refs = list_or_empty(export.get("references"));
    let cited: Vec<PyValue> = refs.iter().map(|r| dict_of(r).and_then(|d| d.get("ref_id").cloned()).unwrap_or(PyValue::None)).collect();
    let gap = if text_of(&status).as_deref() == Some("resolved") {
        PyValue::None
    } else {
        let mut g = PyDict::new();
        g.set("status", status.clone());
        g.set("asked_utc", py(utc));
        g.set("reason", get_dict(&export, "resolution").and_then(|r| r.get("reason").cloned()).unwrap_or(PyValue::None));
        PyValue::Dict(g)
    };
    let mut attestation = PyDict::new();
    attestation.set("kind", py("library.resolution.attestation"));
    attestation.set("exchange_id", py(&exchange));
    attestation.set("node_id", py(&node_id));
    attestation.set("status", status.clone());
    attestation.set("cited_refs", PyValue::List(cited));
    attestation.set("grounding_gap", gap);
    let mut doc = PyDict::new();
    doc.set("schema_version", PyValue::int(1));
    doc.set("campaign_id", py(&camp));
    doc.set("asked_utc", py(utc));
    doc.set("library_intake", py(&note));
    doc.set("request", PyValue::Dict(request));
    doc.set("export", PyValue::Dict(export));
    doc.set("attestation", PyValue::Dict(attestation));
    let path = base.join("grounding").join(format!("{}.json", utc.replace(':', "").replace('-', "")));
    write_json(&path, &PyValue::Dict(doc))?;
    io.warn(&format!(
        "[campaign_init] 그라운딩 · {note} · 판정={} 참조={}건",
        shown(Some(&status)),
        refs.len()
    ));
    wrote_line(io, &camp, &[ctx.rel(&path)]);
    Ok(0)
}

// ── --assign-set ─────────────────────────────────────────────────────────────────────────────

/// `_parse_assign_cells(spec)`: `c1,c2:HITL,c3:STAY` as assignment items (an omitted mode is not written).
fn parse_assign_cells(spec: &str) -> R<Vec<PyValue>> {
    let mut out = Vec::new();
    for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let (cell, mode) = match tok.split_once(':') {
            Some((c, m)) => (c, m),
            None => (tok, ""),
        };
        let mut item = PyDict::new();
        item.set("cell", py(cell.trim()));
        if !mode.is_empty() {
            if !["AUTO", "HITL", "STAY"].contains(&mode) {
                return refuse(format!("--assign-cells 의 mode 는 ('AUTO', 'HITL', 'STAY') 중 하나다: {}", py_repr(tok)));
            }
            item.set("mode", py(mode));
        }
        out.push(PyValue::Dict(item));
    }
    Ok(out)
}

/// `_cell_lock_reason(base, cell)`: why the cell's assignment may no longer change (`None` = it is still pending).
fn cell_lock_reason(base: &Path, cell: &str) -> Option<String> {
    if let Some(st) = read_json(&base.join("cells").join(cell).join("cell.status.json")).as_ref().and_then(dict_of) {
        if let Some(oc) = st.get("cell_outcome").filter(|v| v.is_truthy()) {
            let shown_oc = oc.display_text().to_utf8().unwrap_or_default();
            if !shown_oc.contains(FILL) && !CELL_OUTCOMES_PENDING.contains(&shown_oc.as_str()) {
                return Some(format!("cell_outcome={shown_oc}"));
            }
        }
        let meas = get_dict(st, "measurement").cloned().unwrap_or_default();
        if meas.get("decode_tps_conc1").map(|v| !matches!(v.peel(), PyValue::None)).unwrap_or(false) {
            return Some("측정값이 기록됐다".to_string());
        }
        if truthy(st.get("void_reason")) {
            return Some("사유가 기록됐다".to_string());
        }
    }
    let phases = base.join("phases");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&phases)
        .map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    dirs.sort();
    for ndir in dirs {
        let name = name_of_path(&ndir);
        if !ndir.is_dir() || name.starts_with('_') {
            continue;
        }
        for ph in PHASE_NAMES {
            let doc = read_json(&ndir.join(format!("{ph}.status.json")));
            if let Some(d) = doc.as_ref().and_then(dict_of) {
                let state = get_text(d, "state");
                if get_text(d, "cell_id").as_deref() == Some(cell) && matches!(state.as_deref(), Some("running" | "done")) {
                    return Some(format!("phase {name}:{ph}={}", state.unwrap_or_default()));
                }
            }
        }
    }
    None
}

fn name_of_path(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `writer_assign_set`: the revision written to `campaign.yaml`.
fn assign_set(
    ctx: &Ctx,
    base: &Path,
    node: &str,
    items: Vec<PyValue>,
    values: [Option<&str>; 4],
    source: &str,
) -> R<(PathBuf, PyDict)> {
    let [reason, evidence, approved_by, utc] = values.map(|v| v.filter(|s| !s.is_empty()));
    for (need, val) in [("--assign-reason", reason), ("--assign-evidence", evidence), ("--assign-approved-by", approved_by), ("--utc", utc)] {
        if val.is_none() {
            return refuse(format!("{need} 는 필수다 — 사유·근거·승인자·시각 없는 배정 변경은 드리프트다"));
        }
    }
    let (reason, evidence, approved_by, utc) = (reason.expect("c"), evidence.expect("c"), approved_by.expect("c"), utc.expect("c"));
    let path = base.join("campaign.yaml");
    let mut doc = read_declaration(base);
    if doc.is_empty() {
        return refuse("campaign.yaml 이 없거나 파손");
    }
    let node_ids: Vec<String> = list_of(doc.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .filter_map(|n| get_text(n, "node_id"))
        .collect();
    if !node_ids.iter().any(|n| n == node) {
        return refuse(format!("nodes[] 에 {} 가 없다", py_repr(node)));
    }
    let old_items: Vec<PyValue> = assignments_of(&doc)
        .into_iter()
        .find(|(n, _)| n == node)
        .and_then(|(_, v)| list_of(&v).map(|l| l.iter().filter(|x| dict_of(x).is_some()).cloned().collect()))
        .unwrap_or_default();
    let mode_of = |item: &PyValue| -> PyValue { dict_of(item).and_then(|d| d.get("mode").cloned()).unwrap_or_else(|| py("AUTO")) };
    let cell_of = |item: &PyValue| -> PyValue { dict_of(item).and_then(|d| d.get("cell").cloned()).unwrap_or(PyValue::None) };
    let old_mode: Vec<(PyValue, PyValue)> = old_items.iter().map(|i| (cell_of(i), mode_of(i))).collect();
    let new_cells: Vec<String> = items.iter().filter_map(|i| text_of(&cell_of(i))).collect();
    let mut seen: Vec<&String> = Vec::new();
    for c in &new_cells {
        if seen.contains(&c) {
            return refuse(format!("--assign-cells 에 같은 셀이 두 번 있다: {}", py_list_repr(&new_cells)));
        }
        seen.push(c);
    }
    for cell in &new_cells {
        if RESERVED_CELL_NAMES_W.contains(&cell.as_str()) || cell.is_empty() || cell.contains(FILL) {
            return refuse(format!("셀 이름으로 쓸 수 없다: {}", py_repr(cell)));
        }
        let other = assignments_of(&doc).into_iter().map(|(n, _)| n).find(|n| n != node && assigned_cells(&doc, Some(n)).contains(cell));
        if let Some(other) = other {
            return refuse(format!(
                "셀 {} 은 노드 {} 에 배정돼 있다 — 이동은 그 노드에서 먼저 빼고 넣는다(제거+추가 · pending 일 때만)",
                py_repr(cell),
                py_repr(&other)
            ));
        }
    }
    let new_cell_values: Vec<PyValue> = new_cells.iter().map(|c| py(c)).collect();
    let removed: Vec<String> = old_mode.iter().filter(|(c, _)| !new_cell_values.contains(c)).filter_map(|(c, _)| text_of(c)).collect();
    let changed: Vec<String> = items
        .iter()
        .filter(|i| old_mode.iter().any(|(c, m)| *c == cell_of(i) && *m != mode_of(i)))
        .filter_map(|i| text_of(&cell_of(i)))
        .collect();
    let locked: Vec<String> = removed
        .iter()
        .chain(changed.iter())
        .filter_map(|c| cell_lock_reason(base, c).map(|r| format!("{c}({r})")))
        .collect();
    if !locked.is_empty() {
        return refuse(format!("pending 이 아닌 셀은 빼거나 mode 를 바꿀 수 없다(증거 연결 보존): {}", locked.join("; ")));
    }
    let mut approved: Vec<String> = Vec::new();
    for c in &removed {
        for t in list_or_empty(doc.get("hint_targets")).iter().filter_map(dict_of) {
            let cells: Vec<String> = list_or_empty(t.get("cells")).iter().filter_map(text_of).collect();
            if cells.contains(c) && truthy(t.get("approval")) {
                approved.push(c.clone());
            }
        }
    }
    if !approved.is_empty() {
        approved.sort();
        approved.dedup();
        return refuse(format!("hint 사전승인을 든 셀은 뺄 수 없다: {} — 승인을 먼저 다시 받는다(무인 재태깅 ✗)", py_list_repr(&approved)));
    }
    let mut revs = list_or_empty(doc.get("assignment_revisions"));
    if doc.get("assignment_revisions").and_then(list_of).is_none() {
        revs = Vec::new();
    }
    let current_assignments = {
        let mut d = PyDict::new();
        for (n, v) in assignments_of(&doc) {
            d.set(&n, v);
        }
        d
    };
    if revs.is_empty() {
        let mut t0 = PyDict::new();
        t0.set("rev", PyValue::int(0));
        t0.set("declared_utc", doc.get("declared_utc").cloned().unwrap_or(PyValue::None));
        t0.set("assignments", PyValue::Dict(current_assignments.clone()));
        t0.set("reason", py("t0 선언"));
        t0.set("evidence", doc.get("plan_ref").cloned().unwrap_or(PyValue::None));
        t0.set("approved_by", py("t0"));
        revs.push(PyValue::Dict(t0));
    }
    let mut assigns = current_assignments.clone();
    assigns.set(node, PyValue::List(items.clone()));
    let mut trial = doc.clone();
    trial.set("assignments", PyValue::Dict(assigns.clone()));
    let tmp = base.join(".assign_trial.campaign.yaml");
    write_json_ordered(&tmp, &PyValue::Dict(trial))?;
    let verdict = (|| -> Result<Vec<String>, crate::campaign_validator::Failure> {
        let before = crate::campaign_validator::validate_campaign(ctx, &path, false)?;
        let after = crate::campaign_validator::validate_campaign(ctx, &tmp, false)?;
        Ok(after
            .into_iter()
            .map(|x| x.replace(".assign_trial.campaign.yaml", "campaign.yaml"))
            .filter(|w| !before.contains(w))
            .collect())
    })();
    let _ = std::fs::remove_file(&tmp);
    let why = verdict.map_err(|_| Stop::Fallback)?;
    if !why.is_empty() {
        return refuse(format!(
            "배정 변경 뒤 선언이 계약을 깬다:\n{}",
            why.iter().map(|w| format!("  - {w}")).collect::<Vec<_>>().join("\n")
        ));
    }
    let added: Vec<String> = new_cells.iter().filter(|c| !old_mode.iter().any(|(o, _)| *o == py(c))).cloned().collect();
    let mut rev = PyDict::new();
    rev.set("rev", PyValue::int(revs.len() as i64));
    rev.set("declared_utc", py(utc));
    rev.set("node", py(node));
    rev.set("assignments", PyValue::Dict(assigns.clone()));
    rev.set("removed", str_list(removed.clone()));
    rev.set("added", str_list(added.clone()));
    rev.set("reason", py(reason));
    rev.set("evidence", py(evidence));
    rev.set("approved_by", py(approved_by));
    rev.set("source", py(source));
    let rev_number = revs.len() as i64;
    revs.push(PyValue::Dict(rev.clone()));
    doc.set("assignments", PyValue::Dict(assigns));
    doc.set("assignment_revisions", PyValue::List(revs));
    write_json_ordered(&path, &PyValue::Dict(doc))?;
    let sub_target = sub_nodes_of(ctx, base).iter().filter_map(text_of).any(|n| n == node);
    append_journey(
        base,
        &lifecycle_event(
            "campaign_assignments_revised",
            utc,
            vec![
                ("node", py(node)),
                ("rev", PyValue::int(rev_number)),
                ("cells", str_list(new_cells)),
                ("removed", str_list(removed)),
                ("added", str_list(added)),
                ("approved_by", py(approved_by)),
                ("source", py(source)),
                ("sub_target", PyValue::Bool(sub_target)),
            ],
        ),
    )?;
    Ok((path, rev))
}

const RESERVED_CELL_NAMES_W: [&str; 2] = ["_cell", "_node"];

fn op_assign_set(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let node = args.get("--assign-set").unwrap_or_default().to_string();
    let Some((base, camp)) = progress_prelude(ctx, args, io, "assign_set")? else { return Ok(0) };
    let Some(spec) = args.get("--assign-cells") else {
        return refuse("--assign-set 은 --assign-cells 가 필요하다(빈 목록은 \"\" 로 명시)");
    };
    need_validator(ctx)?;
    let items = parse_assign_cells(spec)?;
    let values = [args.get("--assign-reason"), args.get("--assign-evidence"), args.get("--assign-approved-by"), args.get("--utc")];
    let (path, rev) = assign_set(ctx, &base, &node, items, values, "campaign_init --assign-set")?;
    if sub_nodes_of(ctx, &base).iter().filter_map(text_of).any(|n| Some(&n) == get_text(&rev, "node").as_ref()) {
        io.warn(&format!(
            "[campaign_init] 서브 재배정은 A2A 제어명령으로 전한다: relay.py --campaign-control reslice --campaign-id {camp} --node {node} … (echo 가 오기 전까지 '재배정 미수렴')"
        ));
    }
    wrote_line(io, &camp, &[ctx.rel(&path)]);
    Ok(0)
}

// ── --slice-update ───────────────────────────────────────────────────────────────────────────

/// Python `==` on JSON values: dicts compare without order, `1 == 1.0`.
fn py_equal(a: &PyValue, b: &PyValue) -> bool {
    match (a.peel(), b.peel()) {
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| y.get(&k.to_utf8().unwrap_or_default()).map(|w| py_equal(v, w)).unwrap_or(false))
        }
        (PyValue::List(x), PyValue::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_equal(p, q)),
        (PyValue::Int(x), PyValue::Float(y)) | (PyValue::Float(y), PyValue::Int(x)) => {
            use num_traits::ToPrimitive;
            x.to_f64() == Some(*y)
        }
        (PyValue::Bool(x), PyValue::Int(y)) | (PyValue::Int(y), PyValue::Bool(x)) => *y == i64::from(*x).into(),
        (x, y) => x == y,
    }
}

fn nonblank(value: Option<&PyValue>) -> bool {
    value.and_then(text_of).map(|t| !t.trim().is_empty()).unwrap_or(false)
}

/// `_slice_revision_extension(decl, sd)`: the control revisions the main added after the sub's prefix.
fn slice_revision_extension(decl: &PyDict, sd: &PyDict) -> R<Vec<PyValue>> {
    for key in ["id", "plan_ref", "declared_utc", "control_variables", "derived_from"] {
        let (a, b) = (decl.get(key).cloned().unwrap_or(PyValue::None), sd.get(key).cloned().unwrap_or(PyValue::None));
        if !py_equal(&a, &b) {
            return refuse(format!("slice-update control identity mismatch: {key}"));
        }
    }
    let Some(control) = decl.get("control_variables").filter(|v| dict_of(v).is_some()).cloned() else {
        return refuse("slice-update original control variables are missing");
    };
    if !nonblank(decl.get("derived_from")) {
        return refuse("slice-update main origin is missing");
    }
    let local_value = decl.get("revisions").cloned().unwrap_or_else(|| PyValue::List(Vec::new()));
    let incoming_value = sd.get("revisions").cloned().unwrap_or(PyValue::None);
    let (Some(local), Some(incoming)) = (list_of(&local_value).cloned(), list_of(&incoming_value).cloned()) else {
        return refuse("slice-update revision history is missing or malformed");
    };
    if local.is_empty() && incoming.is_empty() {
        return Ok(Vec::new());
    }
    if !nonblank(decl.get("declared_utc")) || !nonblank(decl.get("plan_ref")) {
        return refuse("slice-update original revision identity is incomplete");
    }
    let mut t0 = PyDict::new();
    t0.set("rev", PyValue::int(0));
    t0.set("declared_utc", decl.get("declared_utc").cloned().unwrap_or(PyValue::None));
    t0.set("values", control.clone());
    t0.set("reason", py("t0 선언"));
    t0.set("evidence", decl.get("plan_ref").cloned().unwrap_or(PyValue::None));
    t0.set("approved_by", py("t0"));
    let t0 = PyValue::Dict(t0);
    let validate = |history: &[PyValue]| -> R<()> {
        if history.is_empty() {
            return Ok(());
        }
        if !py_equal(&history[0], &t0) {
            return refuse("slice-update t0 revision conflicts with original declaration");
        }
        let mut previous: PyDict = dict_of(&control).cloned().unwrap_or_default();
        for (index, item) in history.iter().enumerate().skip(1) {
            let bad = (|| {
                let d = dict_of(item)?;
                let want = ["rev", "declared_utc", "values", "reason", "evidence", "approved_by"];
                if d.len() != want.len() || !want.iter().all(|k| d.contains_key(k)) {
                    return Some(());
                }
                if !matches!(d.get("rev").map(PyValue::peel), Some(PyValue::Int(n)) if *n == (index as i64).into()) {
                    return Some(());
                }
                let values = d.get("values").and_then(dict_of)?;
                if !previous.iter().all(|(k, _)| values.contains_key(&k.to_utf8().unwrap_or_default())) {
                    return Some(());
                }
                if ["declared_utc", "reason", "evidence", "approved_by"].iter().any(|k| !nonblank(d.get(k))) || get_text(d, "approved_by").as_deref() == Some("t0") {
                    return Some(());
                }
                previous = values.clone();
                None
            })();
            if bad.is_some() || dict_of(item).is_none() || dict_of(item).and_then(|d| d.get("values")).and_then(dict_of).is_none() {
                return refuse("slice-update malformed or unapproved control revision");
            }
        }
        Ok(())
    };
    validate(&local)?;
    validate(&incoming)?;
    if incoming.len() < local.len() || !incoming.iter().zip(&local).all(|(a, b)| py_equal(a, b)) {
        return refuse("slice-update control history conflicts or downgrades the existing prefix");
    }
    Ok(incoming[local.len()..].to_vec())
}

fn op_slice_update(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let source = args.get("--slice-update").unwrap_or_default();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--slice-update 는 --utc 가 필요하다");
    };
    let sd = load_slice(source)?;
    if get_text(&sd, "self_role").as_deref() != Some("sub") || get_text(&sd, "authored_by").as_deref() != Some("main-derived") {
        return refuse("--slice-update 는 메인 파생 선언(self_role=sub · authored_by=main-derived)만 받는다");
    }
    let camp_value = sd.get("id").cloned().unwrap_or(PyValue::None);
    let camp = camp_value.display_text().to_utf8().unwrap_or_default();
    let base = ctx.campaigns().join(&camp);
    need_validator(ctx)?;
    let decl = read_declaration(&base);
    if decl.is_empty() {
        return refuse(format!("인스턴스가 없다: campaigns/{camp} — 먼저 메인 파생 선언으로 개설(--init --from-slice)"));
    }
    if get_text(&decl, "self_role").as_deref() != Some("sub") {
        return refuse(format!("campaigns/{camp} 은 서브 자기 인스턴스가 아니다 — 메인 배정은 --assign-set 으로 바꾼다"));
    }
    let assign_keys: Vec<String> = match sd.get("assignments").filter(|v| v.is_truthy()).and_then(dict_of) {
        Some(d) => d.iter().map(|(k, _)| k.to_utf8().unwrap_or_default()).collect(),
        None => Vec::new(),
    };
    let own: Vec<PyValue> = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .map(|n| n.get("node_id").cloned().unwrap_or(PyValue::None))
        .collect();
    if assign_keys.len() != 1 || !own.iter().any(|o| text_of(o).as_deref() == Some(assign_keys[0].as_str())) {
        return refuse(format!(
            "선언의 배정 노드 {} 가 이 인스턴스의 노드 {} 가 아니다(오배달 방지)",
            py_list_repr(&assign_keys),
            PyValue::List(own).repr()
        ));
    }
    let node = assign_keys[0].clone();
    let sd_nodes = sd.get("nodes").cloned().unwrap_or(PyValue::None);
    let decl_nodes = decl.get("nodes").cloned().unwrap_or(PyValue::None);
    let any_not_sub = list_of(&sd_nodes).into_iter().flatten().filter_map(dict_of).any(|n| get_text(n, "role").as_deref() != Some("sub"));
    if !py_equal(&sd_nodes, &decl_nodes) || any_not_sub {
        return refuse("slice-update node identity or role differs");
    }
    let extension = slice_revision_extension(&decl, &sd)?;
    let meta = sd.get("_assignment_revision").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    guard_writer_ops(ctx, &base, &[("assign_set", true)])?;
    let raw = get_dict(&sd, "assignments").and_then(|a| a.get(&node)).cloned().unwrap_or(PyValue::None);
    let Some(items) = list_of(&raw).filter(|l| l.iter().all(|i| dict_of(i).is_some())).cloned() else {
        return refuse("slice-update assignment list is malformed");
    };
    let current: Vec<PyValue> = assignments_of(&decl)
        .into_iter()
        .find(|(n, _)| *n == node)
        .and_then(|(_, v)| list_of(&v).map(|l| l.iter().filter(|x| dict_of(x).is_some()).cloned().collect()))
        .unwrap_or_default();
    let changed = !py_equal(&PyValue::List(current), &PyValue::List(items.clone()));
    if changed && ["reason", "evidence", "approved_by", "declared_utc"].iter().any(|k| !nonblank(meta.get(k))) {
        return refuse("slice-update assignment change lacks main approval evidence");
    }
    let cells: Vec<PyValue> = items.iter().map(|i| dict_of(i).and_then(|d| d.get("cell").cloned()).unwrap_or(PyValue::None)).collect();
    let cells_repr = PyValue::List(cells).repr();
    if !args.has("--apply") {
        print_messages(io, mode, &[format!("dry-run slice-update {camp}@{node} → {cells_repr}; control revisions +{}", extension.len())]);
        return Ok(0);
    }
    let rev_text = if changed {
        let approved = format!("main-a2a: {}", shown(meta.get("approved_by")));
        let source_note = format!("campaign_init --slice-update (main rev {})", shown(meta.get("rev")));
        let (_, rev) = assign_set(
            ctx,
            &base,
            &node,
            items,
            [meta.get("reason").and_then(text_of).as_deref(), meta.get("evidence").and_then(text_of).as_deref(), Some(&approved), Some(utc)],
            &source_note,
        )?;
        shown(rev.get("rev"))
    } else {
        "unchanged".to_string()
    };
    if !extension.is_empty() {
        let mut current = read_declaration(&base);
        let mut revisions = list_or_empty(current.get("revisions"));
        revisions.extend(extension.iter().cloned());
        current.set("revisions", PyValue::List(revisions));
        write_json(&base.join("campaign.yaml"), &PyValue::Dict(current))?;
    }
    print_messages(
        io,
        mode,
        &[format!("slice-update {camp}@{node} assignment rev {rev_text}; control revisions +{} → {cells_repr}", extension.len())],
    );
    Ok(0)
}

// ── --escalation-add ─────────────────────────────────────────────────────────────────────────

/// Predicate ① of the escalation: asked of the explorer's native `escalation_predicate` (an explicit path in
/// `CAMPAIGN_ESCALATION_BIN` wins), never judged here.
fn judge_escalation(ctx: &Ctx, symptoms: &[String], events: &[String], window: Option<(&str, &str)>) -> R<PyDict> {
    // Delivery evidence of the owner = its launcher (the retired escalation_predicate.py is not consulted).
    let owner = ctx.root.join(".claude/skills/vllm-recipe-explorer/scripts/escalation_predicate.sh");
    if !owner.is_file() {
        let mut d = PyDict::new();
        d.set("predicate_1", PyValue::None);
        d.set("model_plane_standing", PyValue::List(Vec::new()));
        d.set("model_plane_refuted", PyValue::List(Vec::new()));
        d.set("host_plane_ignored", PyValue::List(Vec::new()));
        d.set("unknown_symptoms", str_list(symptoms.to_vec()));
        d.set("evidence_source", py("none"));
        d.set(
            "reasons",
            str_list(vec![format!(
                "술어 ① 판정기가 없다({}) — 판정 불가를 false 로 적지 않는다. vllm-recipe-explorer 배달을 확인하라.",
                owner.display()
            )]),
        );
        return Ok(d);
    }
    let bin = std::env::var_os("CAMPAIGN_ESCALATION_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.root.join(".claude/skills/vllm-recipe-explorer/rust/runtime/target/release/escalation_predicate"));
    let mut command = std::process::Command::new(&bin);
    for s in symptoms {
        command.arg("--symptom").arg(s);
    }
    if let (false, Some((start, end))) = (events.is_empty(), window) {
        for e in events {
            command.arg("--events").arg(e);
        }
        command.arg("--started-utc").arg(start).arg("--ended-utc").arg(end);
    }
    command.arg("--json").stdin(std::process::Stdio::null());
    let output = command
        .output()
        .map_err(|e| Stop::Refuse(format!("술어 ① 네이티브 판정기를 시작할 수 없다({}): {e}", bin.display())))?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        return refuse(format!(
            "술어 ① 판정기가 판정하지 못했다(rc={}): {}",
            output.status.code().unwrap_or(-1),
            tail_chars(&String::from_utf8_lossy(&output.stderr), 200)
        ));
    }
    let doc = pyjson::loads(&String::from_utf8_lossy(&output.stdout))
        .map_err(|e| Stop::Refuse(format!("술어 ① 판정기 출력을 읽지 못했다: {e}")))?;
    dict_of(&doc).cloned().ok_or_else(|| Stop::Refuse("술어 ① 판정기 출력이 객체가 아니다".to_string()))
}

fn op_escalation_add(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_prelude(ctx, args, io)? else { return Ok(0) };
    let (Some(cell), Some(utc)) = (
        args.get("--cell").filter(|s| !s.is_empty()),
        args.get("--utc").filter(|s| !s.is_empty()),
    ) else {
        return refuse("--escalation-add 는 --cell 과 --utc 가 필요하다");
    };
    let symptoms: Vec<String> = args.all("--symptom").to_vec();
    let events: Vec<String> = args.all("--events").to_vec();
    let window = match (args.get("--window-start-utc").filter(|s| !s.is_empty()), args.get("--window-end-utc").filter(|s| !s.is_empty())) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    };
    let verdict = judge_escalation(ctx, &symptoms, &events, window)?;
    let mut entry = PyDict::new();
    entry.set("utc", py(utc));
    entry.set("cell_id", py(cell));
    entry.set("node_id", args.get("--node").map(py).unwrap_or(PyValue::None));
    entry.set("symptoms", str_list(symptoms));
    for key in ["predicate_1", "model_plane_standing", "model_plane_refuted", "host_plane_ignored", "unknown_symptoms", "evidence_source", "reasons"] {
        entry.set(key, verdict.get(key).cloned().unwrap_or(PyValue::None));
    }
    entry.set("source", py("campaign_init --escalation-add"));
    let line = sorted_value(&PyValue::Dict(entry)).dumps_unicode().map_err(|e| Stop::Refuse(e.to_string()))?.to_utf8().unwrap_or_default();
    let path = base.join("escalation_candidates.jsonl");
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| Stop::Refuse(e.to_string()))?;
    file.write_all((line + "\n").as_bytes()).map_err(|e| Stop::Refuse(e.to_string()))?;
    wrote_line(io, &camp, &[ctx.rel(&path)]);
    Ok(0)
}

// ── --cell-materialize ───────────────────────────────────────────────────────────────────────

const MATERIALIZE_SET_KEYS: [&str; 13] = [
    "vram_budget_gb", "per_card_vram_gib", "ple_mode", "control_variable", "concurrency_requirement",
    "typical_request_tokens", "tiktoken_host_path", "nas_host_root", "target_gmu", "variant",
    "test_device_total_gib", "kv_cache_dtype_bytes", "port",
];

fn yaml_scalar(v: &PyValue) -> String {
    match v.peel() {
        PyValue::None => "null".to_string(),
        PyValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        PyValue::Int(_) | PyValue::Float(_) => v.repr(),
        _ => {
            let t = v.display_text().to_utf8().unwrap_or_default();
            let plain = !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || "_./+-".contains(c));
            if plain && !["null", "true", "false", "yes", "no"].contains(&t.to_lowercase().as_str()) {
                t
            } else {
                py(&t).dumps_unicode().ok().and_then(|x| x.to_utf8()).unwrap_or_default()
            }
        }
    }
}

fn env_value(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok())?;
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
            return Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn meminfo_gib() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: f64 = line["MemTotal:".len()..].trim().strip_suffix("kB")?.trim().parse().ok()?;
    format!("{:.1}", kb / (1024.0 * 1024.0)).parse().ok()
}

/// `recipe._memory_model(man)`: `unified`/`discrete`/unknown from observed facts only, and the source sentence.
fn memory_model(ctx: &Ctx, man: &PyDict) -> (Option<&'static str>, String) {
    let strip = |k: &str| man.get(k).filter(|v| v.is_truthy()).map(|v| v.display_text().to_utf8().unwrap_or_default().trim().trim_matches('"').to_string()).unwrap_or_default();
    match strip("gpu_memory_model").as_str() {
        "unified" => return (Some("unified"), "manifest.gpu_memory_model".to_string()),
        "discrete" => return (Some("discrete"), "manifest.gpu_memory_model".to_string()),
        _ => {}
    }
    let gpu_model = strip("gpu_model");
    let references = ctx.root.join(".claude/skills/wiki-desk/reference/references.md");
    if !gpu_model.trim().is_empty() {
        if let Some(text) = std::fs::read(&references).ok().and_then(|b| String::from_utf8(b).ok()) {
            let key = gpu_model.trim().to_lowercase();
            let header = text.lines().find(|l| {
                let Some(rest) = l.strip_prefix("###") else { return false };
                let mut chars = rest.chars();
                matches!(chars.next(), Some(c) if c.is_whitespace()) && chars.as_str().to_lowercase().contains(&key)
            });
            if let Some(h) = header {
                let unified = h.contains("통합메모리");
                return (
                    Some(if unified { "unified" } else { "discrete" }),
                    format!("references.md §4 헤더(gpu_model={gpu_model})"),
                );
            }
        }
    }
    (None, "미상(manifest gpu_memory_model 부재 · references.md 미등재)".to_string())
}

fn device_total_gib(ctx: &Ctx, mf: &PyDict) -> (Option<f64>, String) {
    // The memory-model rule's owner is the explorer's Rust `recipe` (`recipe_cmd::memory_model`), not the retired recipe.py.
    let owner = ctx.root.join(".claude/skills/vllm-recipe-explorer/rust/runtime/src/recipe_cmd.rs");
    if !owner.is_file() {
        return (None, format!("미상(판정 소유자 부재: {})", owner.display()));
    }
    let (mm, source) = memory_model(ctx, mf);
    match mm {
        Some("unified") => (meminfo_gib(), format!("measured /proc/meminfo MemTotal(통합메모리 ← {source})")),
        Some("discrete") if truthy(mf.get("gpu_vram_mib")) => {
            let mib = mf.get("gpu_vram_mib").and_then(as_number).unwrap_or(0.0);
            (format!("{:.1}", mib / 1024.0).parse().ok(), format!("measured manifest gpu_vram_mib(discrete ← {source})"))
        }
        _ => (None, source),
    }
}

fn load_yaml(path: &Path) -> R<Option<PyValue>> {
    let Some(text) = std::fs::read(path).ok().and_then(|b| String::from_utf8(b).ok()) else {
        return Err(Stop::Fallback);
    };
    match crate::pyyaml::safe_load_text(&PyStr::new(&text), 0) {
        Ok(PyValue::None) => Ok(None),
        Ok(v) => Ok(Some(v)),
        Err(_) => Err(Stop::Fallback),
    }
}

fn op_cell_materialize(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let cell = args.get("--cell-materialize").unwrap_or_default().to_string();
    let Some((base, camp)) = progress_prelude(ctx, args, io, "cell_materialize")? else { return Ok(0) };
    let mut sets: Vec<(String, String)> = Vec::new();
    for kv in args.all("--set") {
        let Some((k, v)) = kv.split_once('=') else {
            return refuse(format!("--set 은 K=V 다: {}", py_repr(kv)));
        };
        match sets.iter_mut().find(|(n, _)| n == k.trim()) {
            Some(slot) => slot.1 = v.to_string(),
            None => sets.push((k.trim().to_string(), v.to_string())),
        }
    }
    let Some(utc) = args.get("--utc").filter(|u| !u.is_empty()) else {
        return refuse("--cell-materialize 는 --utc 가 필요하다(시각은 주입만)");
    };
    need_validator(ctx)?;
    let Some(node) = node_of_cell(&base, &cell) else {
        return refuse(format!("셀 {} 이 assignments 에 없다 — 배정되지 않은 셀은 만들지 않는다(이름이 곧 증거 연결)", py_repr(&cell)));
    };
    let mut bad: Vec<String> = sets.iter().map(|(k, _)| k.clone()).filter(|k| !MATERIALIZE_SET_KEYS.contains(&k.as_str())).collect();
    bad.sort();
    if !bad.is_empty() {
        return refuse(format!("--set 닫힌 목록 밖 키 {} — 허용 {}", py_list_repr(&bad), py_list_repr(&MATERIALIZE_SET_KEYS.iter().map(|s| s.to_string()).collect::<Vec<_>>())));
    }
    let cdir = base.join("cells").join(&cell);
    let names = ["config.yaml", "lockset.json", "cell.status.json"];
    let have: Vec<String> = names.iter().filter(|n| cdir.join(n).exists()).map(|n| n.to_string()).collect();
    if !have.is_empty() {
        return refuse(format!("cells/{cell}/ 에 이미 {} 가 있다 — 덮어쓰지 않는다", py_list_repr(&have)));
    }
    let decl = read_declaration(&base);
    let node_decl = list_of(decl.get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .find(|n| get_text(n, "node_id").as_deref() == Some(node.as_str()))
        .cloned()
        .unwrap_or_default();
    let topo_value = node_decl
        .get("topology")
        .filter(|v| v.is_truthy())
        .cloned()
        .or_else(|| get_dict(&decl, "control_variables").and_then(|c| c.get("topology").cloned()).filter(|v| v.is_truthy()))
        .unwrap_or(PyValue::None);
    let topo = match text_of(&topo_value).filter(|t| t == "single" || t == "multi") {
        Some(t) => t,
        None => return refuse(format!("노드 {} 의 topology 를 선언에서 해소하지 못했다: {}(추측 ✗)", py_repr(&node), topo_value.repr())),
    };
    let syaml = ctx.root.join("output").join(&topo).join("configs").join(format!("{cell}.yaml"));
    if !syaml.is_file() {
        return refuse(format!("서빙 yaml {} 부재 — 셀 id = config 이름이어야 한다", ctx.rel(&syaml)));
    }
    let sv_value = load_yaml(&syaml)?;
    let sv: PyDict = match sv_value.as_ref() {
        None => PyDict::new(),
        Some(v) => dict_of(v).cloned().ok_or(Stop::Fallback)?,
    };
    let env = ctx.root.join("output").join(&topo).join("envs").join(format!(".env.{cell}"));
    let mf_path = ctx.root.join("output").join(&topo).join("manifest.yaml");
    let mf: PyDict = if mf_path.is_file() { load_yaml(&mf_path)?.as_ref().and_then(dict_of).cloned().unwrap_or_default() } else { PyDict::new() };
    let kv_name = match sv.get("kv-cache-dtype") { Some(v) => v.display_text().to_utf8().unwrap_or_default(), None => "auto".to_string() };
    let kv_bytes = match kv_name.as_str() {
        "auto" | "bfloat16" | "float16" | "half" => Some(2),
        "fp8" | "fp8_e4m3" | "fp8_e5m2" | "fp8_inc" | "fp8_ds_mla" => Some(1),
        _ => None,
    };
    let get_or = |k: &str, d: PyValue| sv.get(k).cloned().unwrap_or(d);
    let (dev, dev_src) = device_total_gib(ctx, &mf);
    let env_variant = env_value(&env, "VARIANT").filter(|v| !v.is_empty());
    let env_name = env_value(&env, "SERVING_MODEL_NAME").filter(|v| !v.is_empty());
    let derived: Vec<(&str, PyValue, String)> = vec![
        ("path", get_or("model", PyValue::None), format!("output/{topo}/configs/{cell}.yaml#model")),
        ("context_len", get_or("max-model-len", PyValue::None), "서빙 yaml#max-model-len".to_string()),
        ("kv_cache_dtype", get_or("kv-cache-dtype", py("auto")), "서빙 yaml#kv-cache-dtype(부재 = vLLM 기본 auto)".to_string()),
        ("tp", get_or("tensor-parallel-size", PyValue::int(1)), "서빙 yaml#tensor-parallel-size(부재 = 1)".to_string()),
        ("target_gmu", get_or("gpu-memory-utilization", PyValue::None), "서빙 yaml#gpu-memory-utilization(서빙된 값)".to_string()),
        ("variant", py(env_variant.as_deref().unwrap_or("stock")), format!(".env.{cell}#VARIANT(부재 = stock)")),
        (
            "served_model_name",
            env_name.as_deref().map(py).unwrap_or_else(|| sv.get("served-model-name").filter(|v| v.is_truthy()).cloned().unwrap_or(PyValue::None)),
            format!(".env.{cell}#SERVING_MODEL_NAME"),
        ),
        ("config_name", py(&cell), "셀 id".to_string()),
        ("gpu_model", mf.get("gpu_model").cloned().unwrap_or(PyValue::None), format!("output/{topo}/manifest.yaml#gpu_model")),
        (
            "nas_host_root",
            if get_text(&mf, "model_source").as_deref() == Some("managed") { mf.get("nas_model_path").cloned().unwrap_or(PyValue::None) } else { PyValue::None },
            format!("output/{topo}/manifest.yaml#nas_model_path(model_source=managed)"),
        ),
        ("test_device_total_gib", dev.map(PyValue::Float).unwrap_or(PyValue::None), dev_src),
        (
            "kv_cache_dtype_bytes",
            kv_bytes.map(PyValue::int).unwrap_or(PyValue::None),
            "서빙 yaml#kv-cache-dtype → 원소 바이트(auto = 모델 dtype bf16/fp16 전제)".to_string(),
        ),
        ("port", get_or("port", PyValue::int(8000)), "서빙 yaml#port(부재 = vLLM 기본 8000)".to_string()),
    ];
    let mut vals: Vec<(String, PyValue)> = Vec::new();
    let mut src: Vec<(String, String)> = Vec::new();
    let put_val = |k: &str, v: PyValue, why: String, vals: &mut Vec<(String, PyValue)>, src: &mut Vec<(String, String)>| {
        match vals.iter_mut().find(|(n, _)| n == k) { Some(slot) => slot.1 = v, None => vals.push((k.to_string(), v)) }
        match src.iter_mut().find(|(n, _)| n == k) { Some(slot) => slot.1 = why, None => src.push((k.to_string(), why)) }
    };
    for (k, v, why) in derived {
        if !matches!(v.peel(), PyValue::None) {
            put_val(k, v, why, &mut vals, &mut src);
        }
    }
    for (k, raw) in &sets {
        let v = if raw.trim().is_empty() {
            PyValue::None
        } else {
            match crate::pyyaml::safe_load_text(&PyStr::new(raw), 0) { Ok(v) => v, Err(_) => return Err(Stop::Fallback) }
        };
        put_val(k, v, "--set(사람 선언)".to_string(), &mut vals, &mut src);
    }
    let template_text = std::fs::read(ctx.template().join("cells/_cell/config.yaml")).ok().and_then(|b| String::from_utf8(b).ok()).ok_or(Stop::Fallback)?;
    let mut left: Vec<String> = Vec::new();
    let mut text = String::new();
    for line in template_text.split_inclusive('\n') {
        let body = line.trim_start();
        let lead = &line[..line.len() - body.len()];
        let key_len = body.chars().take_while(|c| c.is_ascii_lowercase() || *c == '_').count();
        let mut replaced = None;
        if key_len > 0 && !lead.contains('\n') {
            let after = &body[key_len..];
            if let Some(rest) = after.strip_prefix(':') {
                let rest_trim = rest.trim_start();
                if let Some(tail) = rest_trim.strip_prefix(FILL) {
                    let key = &body[..key_len];
                    let gap = &rest[..rest.len() - rest_trim.len()];
                    replaced = Some(match vals.iter().find(|(n, _)| n == key) {
                        Some((_, v)) => format!("{lead}{key}:{gap}{}{tail}", yaml_scalar(v)),
                        None => {
                            left.push(key.to_string());
                            line.to_string()
                        }
                    });
                }
            }
        }
        text.push_str(&replaced.unwrap_or_else(|| line.to_string()));
    }
    if !left.is_empty() {
        return refuse(format!("채우지 못한 빈칸 {} — 파생할 수 없는 선언 축이다. `--set 키=값`(요구 없음 = `키=null`)으로 적는다", py_list_repr(&left)));
    }
    let mut sorted_src = src.clone();
    sorted_src.sort_by(|a, b| a.0.cmp(&b.0));
    let head = format!(
        "# materialized: campaign_init --cell-materialize {cell} · {utc} · node {node}\n{}",
        sorted_src.iter().map(|(k, why)| format!("#   {k}: {why}\n")).collect::<String>()
    );
    let mut lock = read_json(&ctx.template().join("cells/_cell/lockset.json")).as_ref().and_then(dict_of).cloned().ok_or(Stop::Fallback)?;
    lock.set("id", py(&cell));
    lock.set("provenance", py("hand-authored"));
    lock.set("max_model_len", vals.iter().find(|(n, _)| n == "context_len").map(|(_, v)| v.clone()).unwrap_or(PyValue::None));
    lock.set("quantization", sv.get("quantization").cloned().unwrap_or(PyValue::None));
    let mut st = read_json(&ctx.template().join("cells/_cell/cell.status.json")).as_ref().and_then(dict_of).cloned().ok_or(Stop::Fallback)?;
    st.set("cell_id", py(&cell));
    st.set("node_id", py(&node));
    write_text(&cdir.join("config.yaml"), &(head + &text))?;
    write_json(&cdir.join("lockset.json"), &PyValue::Dict(lock))?;
    write_json(&cdir.join("cell.status.json"), &PyValue::Dict(st))?;
    wrote_line(io, &camp, &names.iter().map(|n| ctx.rel(&cdir.join(n))).collect::<Vec<_>>());
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
        let closed = read_journey(&prev)
            .into_iter()
            .rev()
            .find(|e| dict_of(e).map(|d| get_text(d, "kind").as_deref() == Some("campaign_closed")).unwrap_or(false));
        let summary = closed.as_ref().and_then(dict_of).and_then(|d| get_text(d, "summary_testlog"));
        return Ok(abandoned_gate_reasons(ctx, previous_id, summary.as_deref())
            .into_iter()
            .map(|r| format!("{previous_id}: {r}"))
            .collect());
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
    // P1~P3 are asked of the built-in validator (`campaign_validator`): the retired campaign_template_validator.py beside the
    // program is no longer needed for the means to exist.
    reasons.extend(
        crate::campaign_validator::instance_predicates(ctx, &prev)
            .into_iter()
            .map(|r| format!("{previous_id}: {r}")),
    );
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

/// `_load_slice(source)`: one main-derived declaration from a file or from standard input (`-`).
fn load_slice(source: &str) -> R<PyDict> {
    let value = if source == "-" {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes);
        let text = String::from_utf8_lossy(&bytes).into_owned();
        match pyjson::loads(&text) {
            Ok(v) => Some(v),
            Err(e) => return refuse(format!("stdin 파생 선언 JSON을 읽지 못했다: {e}")),
        }
    } else {
        read_json(Path::new(source))
    };
    match value.as_ref().and_then(dict_of) {
        Some(d) => Ok(d.clone()),
        None => refuse(format!("파생 선언을 읽지 못했다: {source}")),
    }
}

// ── lifecycle transitions: pins, resources, suspension, resumption, abandonment, opening ─────

fn running_cells(base: &Path) -> Vec<(String, String, PyValue)> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(base.join("phases")).map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    dirs.sort();
    for ndir in dirs {
        let name = name_of_path(&ndir);
        if !ndir.is_dir() || name == "_node" {
            continue;
        }
        for ph in PHASE_NAMES {
            if let Some(st) = read_json(&ndir.join(format!("{ph}.status.json"))).as_ref().and_then(dict_of) {
                if get_text(st, "state").as_deref() == Some("running") {
                    out.push((name.clone(), ph.to_string(), st.get("cell_id").cloned().unwrap_or(PyValue::None)));
                }
            }
        }
    }
    out
}

fn capture_pins(base: &Path) -> PyValue {
    let decl = read_declaration(base);
    let revs = list_or_empty(decl.get("revisions"));
    let cur = revs
        .last()
        .and_then(dict_of)
        .and_then(|r| r.get("values").filter(|v| v.is_truthy()))
        .cloned()
        .or_else(|| decl.get("control_variables").filter(|v| v.is_truthy()).cloned())
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let cells = base.join("cells");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cells).map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    dirs.sort();
    let mut images: Vec<PyValue> = Vec::new();
    for cdir in dirs {
        if let Some(st) = read_json(&cdir.join("cell.status.json")).as_ref().and_then(dict_of) {
            if let Some(img) = get_dict(st, "image") {
                if truthy(img.get("tag")) && truthy(img.get("digest")) {
                    let mut e = PyDict::new();
                    e.set("tag", img.get("tag").cloned().unwrap_or(PyValue::None));
                    e.set("digest", img.get("digest").cloned().unwrap_or(PyValue::None));
                    let e = PyValue::Dict(e);
                    if !images.iter().any(|x| py_equal(x, &e)) {
                        images.push(e);
                    }
                }
            }
        }
    }
    let mut d = PyDict::new();
    d.set("topology", declared_topology(base).as_deref().map(py).unwrap_or(PyValue::None));
    d.set("vllm_version", dict_of(&cur).and_then(|c| c.get("vllm_version").cloned()).unwrap_or(PyValue::None));
    d.set("images", PyValue::List(images));
    d.set("source", py("campaign.yaml(현행 개정) · cells/*/cell.status.json#image"));
    PyValue::Dict(d)
}

/// `_image_digest(tag)`: `docker image inspect --format {{.Id}}`, `None` when it cannot answer.
fn image_digest(tag: &str) -> Option<String> {
    let out = std::process::Command::new("docker")
        .args(["image", "inspect", "--format", "{{.Id}}", tag])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn pin_drift(pins: &PyValue) -> Vec<String> {
    let mut drift = Vec::new();
    for img in dict_of(pins).map(|p| list_or_empty(p.get("images"))).unwrap_or_default() {
        let Some(img) = dict_of(&img) else { continue };
        let (tag, digest) = (shown(img.get("tag")), shown(img.get("digest")));
        match image_digest(&tag) {
            None => drift.push(format!("이미지 {tag}: 현재 digest 를 확인할 수 없다(로컬 부재 또는 docker 불가) — 보류 당시 {digest}")),
            Some(now) if now != digest => drift.push(format!("이미지 {tag}: 보류 당시 {digest} → 현재 {now}")),
            Some(_) => {}
        }
    }
    drift
}

fn release_resources(ctx: &Ctx, topology: Option<&str>, releases: &[String], apply: bool) -> R<Vec<PyValue>> {
    if releases.is_empty() {
        return transition_refuse("보류는 자원을 반납한다 — `--release <config>`(반복) 로 이 캠페인이 띄운 서빙을 지목하거나, 띄운 것이 없으면 `--release none` 으로 그 사실을 선언하라(정상 차단: 침묵 skip 은 고아 서빙을 남긴다)");
    }
    let one = |config: PyValue, status: &str, extra: Vec<(&str, PyValue)>| {
        let mut d = PyDict::new();
        d.set("config", config);
        d.set("status", py(status));
        for (k, v) in extra {
            d.set(k, v);
        }
        PyValue::Dict(d)
    };
    if releases == ["none".to_string()] {
        return Ok(vec![one(PyValue::None, "none-declared", vec![])]);
    }
    if releases.iter().any(|r| r == "none") {
        return transition_refuse("`--release none` 은 다른 --release 와 함께 올 수 없다(의도 모순)");
    }
    let (script, extra): (&str, &[&str]) = match topology {
        Some("single") => (".claude/skills/upstream-version-watch/scripts/single_serve_down.sh", &[]),
        Some("multi") => (".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh", &["--down"]),
        other => {
            return transition_refuse(format!(
                "캠페인 토폴로지가 선언되지 않았다({}) — 어느 teardown 진입점을 쓸지 고를 수 없다",
                other.map(py_repr).unwrap_or_else(|| "None".to_string())
            ))
        }
    };
    let mut out = Vec::new();
    for cfg in releases {
        let script_path = ctx.root.join(script);
        if !apply {
            let mut cmd = vec![script_path.to_string_lossy().into_owned(), cfg.clone()];
            cmd.extend(extra.iter().map(|e| e.to_string()));
            out.push(one(py(cfg), "dry-run", vec![("command", py(&cmd.join(" ")))]));
            continue;
        }
        let output = std::process::Command::new("bash")
            .arg(&script_path)
            .arg(cfg)
            .args(extra)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| Stop::Refuse(format!("bash 를 시작할 수 없다: {e}")))?;
        let rc = output.status.code().unwrap_or(-1);
        if rc != 0 {
            let both = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            let tail600: String = {
                let n = both.chars().count();
                both.chars().skip(n.saturating_sub(600)).collect()
            };
            return transition_refuse(format!(
                "teardown 실패(rc={rc}) — {cfg}. 전환은 fail-closed 다: ACTIVE 와 journey 는 바뀌지 않았다. 자원을 정리한 뒤 다시 전환하라(정상 차단 · 우회 ✗).\n  {}",
                tail_chars(&tail600, 400)
            ));
        }
        out.push(one(py(cfg), "released", vec![("script", py(script)), ("rc", PyValue::int(rc as i64))]));
    }
    Ok(out)
}

fn devlog_name(ctx: &Ctx, utc: &str, topic: &str) -> R<(PathBuf, String)> {
    let ts = ctx.root.join(".claude/skills/wiki-desk/src/doc_naming.ts");
    let ddir = ctx.root.join("docs").join("devlog");
    let mut existing: Vec<String> = std::fs::read_dir(&ddir).map(|r| r.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    existing.sort();
    if !ts.is_file() {
        return transition_refuse(format!("문서 명명 SSOT 를 적재하지 못했다: {}", ctx.rel(&ts)));
    }
    let arg = PyValue::List(vec![py("devlog"), py(utc), py(topic), str_list(existing)])
        .dumps_unicode()
        .map_err(|e| Stop::Refuse(e.to_string()))?
        .to_utf8()
        .unwrap_or_default();
    let output = std::process::Command::new("node")
        .arg(&ts)
        .arg("dated_doc_basename")
        .arg(arg)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Stop::Refuse(format!("node 를 시작할 수 없다: {e}")))?;
    let out = pyjson::loads(&String::from_utf8_lossy(&output.stdout)).ok().and_then(|v| dict_of(&v).cloned()).unwrap_or_default();
    if !truthy(out.get("ok")) {
        let why = match out.get("message").filter(|v| v.is_truthy()) {
            Some(m) => shown(Some(m)),
            None => tail_chars(&String::from_utf8_lossy(&output.stderr), 200),
        };
        return transition_refuse(format!("문서 명명 SSOT 실패(rc={}): {why}", output.status.code().unwrap_or(-1)));
    }
    let name = shown(out.get("value"));
    Ok((ddir.join(&name), name))
}

fn write_parked_devlog(ctx: &Ctx, camp: &str, event: &PyDict, apply: bool) -> R<Option<String>> {
    let slug: String = camp.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '.' { c } else { '_' }).collect();
    let utc = shown(event.get("utc"));
    let (path, name) = devlog_name(ctx, &utc, &format!("PARKED_캠페인보류_{slug}"))?;
    if !apply {
        return Ok(Some(ctx.rel(&path)));
    }
    let released: Vec<String> = list_or_empty(event.get("released")).iter().filter_map(dict_of).map(|r| shown(r.get("config"))).collect();
    let released = if released.is_empty() { "없음".to_string() } else { released.join(", ") };
    let nxt = event.get("next").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    let join_or_none = |key: &str| {
        let items: Vec<String> = list_or_empty(event.get(key)).iter().map(|v| shown(Some(v))).collect();
        if items.is_empty() { "없음".to_string() } else { items.join(", ") }
    };
    let pins = event.get("pins").cloned().unwrap_or(PyValue::None).dumps_unicode().ok().and_then(|t| t.to_utf8()).unwrap_or_default();
    let reason = match event.get("reason").filter(|v| v.is_truthy()) { Some(r) => shown(Some(r)), None => "(사유 미기재)".to_string() };
    let preempted = match event.get("preempted_by").filter(|v| v.is_truthy()) { Some(p) => shown(Some(p)), None => "(없음 — 단독 보류)".to_string() };
    let wait = if nxt.is_empty() { String::new() } else { format!(" · 개설 대기 {}@{}", shown(nxt.get("id")), shown(nxt.get("topology"))) };
    let body = vec![
        format!("# {}", name.strip_suffix(".md").unwrap_or(&name)),
        String::new(),
        format!("PARKED: 캠페인 `{camp}` 보류 — {reason}"),
        String::new(),
        format!("- 시각(UTC): {utc} · 승인: {}", shown(event.get("approved_by"))),
        format!("- 선점한 캠페인: {preempted}{wait}"),
        format!("- 내린 자원: {released}"),
        format!("- 중단된 셀(재개 시 pending 으로 복귀): {}", join_or_none("interrupted_cells")),
        format!("- 보류 당시 핀: `{pins}`"),
        format!("- 서브(A2A 보류 대상): {}", join_or_none("sub_nodes")),
        String::new(),
        "## 재개".to_string(),
        String::new(),
        format!("- 재개는 `campaign_init.py --resume-campaign {camp} --approved-by <발화 전사> --utc <UTC> --apply`."),
        "- 재개 게이트: 보류 당시 핀·이미지 digest 대조(어긋나면 `--accept-drift` 또는 abandoned) · 토폴로지는 체크아웃과 같아야 한다(drift ✗).".to_string(),
        format!("- 권위는 `campaigns/{camp}/journey.jsonl` 이다 — 이 문서는 인스턴스가 사라졌을 때 사람이 재구성하기 위한 서사다."),
        String::new(),
    ];
    write_text(&path, &body.join("\n"))?;
    Ok(Some(ctx.rel(&path)))
}

#[allow(clippy::too_many_arguments)]
fn suspend_campaign(
    ctx: &Ctx,
    camp: &str,
    utc: &str,
    approved_by: &str,
    reason: &str,
    releases: &[String],
    preempt: bool,
    preempted_by: Option<&str>,
    nxt: Option<PyValue>,
    apply: bool,
) -> R<(PyDict, Vec<String>)> {
    let base = ctx.campaigns().join(camp);
    if !is_dir(&base) {
        return transition_refuse(format!("인스턴스가 없다: campaigns/{camp}"));
    }
    let f = fold_lifecycle(ctx, &base, None);
    if f.state != "active" {
        return transition_refuse(format!("{camp} 은 {} 다 — 보류는 active 캠페인에만 한다", f.state));
    }
    let run = running_cells(&base);
    if !run.is_empty() && !preempt {
        let list: Vec<String> = run.iter().map(|(n, p, c)| format!("{}({n}:{p})", shown(Some(c)))).collect();
        return transition_refuse(format!(
            "진행 중인 셀이 있다 — {}. 협조 전환이 기본이다: 셀을 끝낸 뒤 전환하거나, 사람이 강제를 고르면 `--preempt` 로 다시 부른다(측정 중단 · 서빙 내림 · 그 셀은 interrupted — 부분 측정값은 증거로 쓰지 않는다). 정상 차단이다",
            list.join(", ")
        ));
    }
    let released = release_resources(ctx, f.topology.as_deref(), releases, apply)?;
    let mut interrupted: Vec<String> = run.iter().filter_map(|(_, _, c)| text_of(c).filter(|t| !t.is_empty())).collect();
    interrupted.sort();
    interrupted.dedup();
    let mut event = match lifecycle_event(
        "campaign_suspended",
        utc,
        vec![
            ("approved_by", py(approved_by)),
            ("reason", py(reason)),
            ("preempted_by", preempted_by.map(py).unwrap_or(PyValue::None)),
            ("preempt", PyValue::Bool(!run.is_empty() && preempt)),
            ("released", PyValue::List(released.clone())),
            ("pins", capture_pins(&base)),
            ("interrupted_cells", str_list(interrupted)),
            ("sub_nodes", PyValue::List(sub_nodes_of(ctx, &base))),
            ("next", nxt.unwrap_or(PyValue::None)),
        ],
    ) {
        PyValue::Dict(d) => d,
        _ => PyDict::new(),
    };
    let parked = write_parked_devlog(ctx, camp, &event, apply)?;
    event.set("parked_devlog", parked.as_deref().map(py).unwrap_or(PyValue::None));
    let mut wrote = Vec::new();
    if apply {
        for (node, phase, cell) in &run {
            let mut pairs: Vec<(&str, String)> = vec![
                ("--proof-predicate", format!("preempted — {} 로 강제 전환", preempted_by.unwrap_or("단독 보류"))),
                ("--proof-source", "campaign_init --preempt".to_string()),
                ("--ended-utc", utc.to_string()),
            ];
            if let Some(c) = text_of(cell) {
                pairs.push(("--cell", c));
            }
            write_phase(ctx, &base, &Args::synthetic(pairs), node, phase, "failed", text_of(cell).as_deref())?;
            if cell.is_truthy() {
                append_journey(
                    &base,
                    &lifecycle_event(
                        "cell_interrupted",
                        utc,
                        vec![
                            ("cell_id", cell.clone()),
                            ("node", py(node)),
                            ("phase", py(phase)),
                            ("preempted_by", preempted_by.map(py).unwrap_or(PyValue::None)),
                        ],
                    ),
                )?;
            }
        }
        wrote.push(ctx.rel(&append_journey(&base, &PyValue::Dict(event.clone()))?));
        if ctx.active_id() == camp {
            write_text(&ctx.active_pointer(), &format!("{BOOTSTRAP}\n"))?;
        }
        if let Some(p) = parked {
            wrote.push(p);
        }
    }
    Ok((event, wrote))
}

fn op_suspend(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--suspend").unwrap_or_default().to_string();
    let mode = if args.has("--apply") { "APPLIED" } else { "DRY-RUN" };
    require_transition_pair(args.get("--utc"), args.get("--approved-by"), "--suspend")?;
    let (event, wrote) = suspend_campaign(
        ctx,
        &camp,
        args.get("--utc").unwrap_or_default(),
        args.get("--approved-by").unwrap_or_default(),
        args.get("--reason").filter(|r| !r.is_empty()).unwrap_or("단독 보류"),
        args.all("--release"),
        args.has("--preempt"),
        None,
        None,
        args.has("--apply"),
    )?;
    io.say(&format!(
        "[campaign_init] {mode} suspended {camp} · 내린 자원 {} · 중단 셀 {} · {}",
        shown_repr(event.get("released")),
        shown_repr(event.get("interrupted_cells")),
        wrote.join(" ")
    ));
    Ok(0)
}

fn shown_repr(v: Option<&PyValue>) -> String {
    v.map(PyValue::repr).unwrap_or_else(|| "None".to_string())
}

fn op_resume_campaign(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--resume-campaign").unwrap_or_default().to_string();
    let apply = args.has("--apply");
    let mode = if apply { "APPLIED" } else { "DRY-RUN" };
    require_transition_pair(args.get("--utc"), args.get("--approved-by"), "--resume-campaign")?;
    let (utc, approved_by) = (args.get("--utc").unwrap_or_default(), args.get("--approved-by").unwrap_or_default());
    let base = ctx.campaigns().join(&camp);
    if !is_dir(&base) {
        return transition_refuse(format!("인스턴스가 없다: campaigns/{camp}"));
    }
    let f = fold_lifecycle(ctx, &base, None);
    if f.state != "suspended" && f.state != "resume_pending" {
        let tail = if f.state == "orphan" { " (orphan 은 재개할 수 없다 — `--abandon` 으로만 정리)" } else { "" };
        return transition_refuse(format!("{camp} 은 {} 다 — 재개는 보류 캠페인에만 한다{tail}", f.state));
    }
    let here = ctx.checkout_topology();
    let want = dict_of(&f.suspended_pins).and_then(|p| get_text(p, "topology")).filter(|t| !t.is_empty()).or_else(|| f.topology.clone());
    if want != here {
        return transition_refuse(format!(
            "토폴로지 불일치 — {camp} 은 topology={} 인데 체크아웃은 {} 다. 이것은 drift 가 아니라 4자일치 불변식 위반이다(--accept-drift 로 넘길 수 없다). 미커밋 0 을 확인하고 해당 브랜치로 전환한 뒤 재개하라(오배달 방지)",
            want.as_deref().map(py_repr).unwrap_or_else(|| "None".to_string()),
            here.as_deref().map(py_repr).unwrap_or_else(|| "None".to_string())
        ));
    }
    let cur = ctx.active_id();
    if cur != BOOTSTRAP {
        return transition_refuse(format!("활성 캠페인 {cur} 이 있다 — 먼저 그것을 보류하거나 종결하라(ACTIVE 슬롯은 하나다)"));
    }
    let drift = pin_drift(&f.suspended_pins);
    if !drift.is_empty() && !args.has("--accept-drift") {
        return transition_refuse(format!(
            "보류 동안 환경이 바뀌었다:\n  - {}\n  → 사람이 수용하면 `--accept-drift`, 아니면 `--abandon`(정상 차단)",
            drift.join("\n  - ")
        ));
    }
    if !apply {
        let note = if drift.is_empty() { String::new() } else { format!(" (drift 수용 {}건)", drift.len()) };
        print_messages(io, mode, &[format!("dry-run resume {camp}{note}")]);
        return Ok(0);
    }
    let pq = dict_of(&f.pending_query).cloned().unwrap_or_default();
    for cell in &f.interrupted_cells {
        let pairs: Vec<(&str, String)> = vec![
            ("--outcome", "pending".to_string()),
            ("--next-intent", format!("preempt 로 중단된 셀 — 재개({utc})에 처음부터 재실행")),
            ("--utc", utc.to_string()),
        ];
        write_cell(ctx, &base, &Args::synthetic(pairs), cell, "pending")?;
    }
    let handoff = if truthy(pq.get("child")) {
        let mut h = PyDict::new();
        h.set("child", pq.get("child").cloned().unwrap_or(PyValue::None));
        h.set("child_reason", pq.get("child_reason").cloned().unwrap_or(PyValue::None));
        h.set("pointers", pq.get("pointers").cloned().unwrap_or(PyValue::None));
        vec![PyValue::Dict(h)]
    } else {
        Vec::new()
    };
    let subs = sub_nodes_of(ctx, &base);
    append_journey(
        &base,
        &lifecycle_event(
            "campaign_resumed",
            utc,
            vec![
                ("approved_by", py(approved_by)),
                ("accepted_drift", str_list(drift)),
                ("handoff_pointers", PyValue::List(handoff)),
                ("requeued_cells", str_list(f.interrupted_cells.clone())),
                ("sub_nodes", PyValue::List(subs.clone())),
                ("answers", last_ref(&f)),
            ],
        ),
    )?;
    write_text(&ctx.active_pointer(), &format!("{camp}\n"))?;
    let mut out = vec![format!("resumed {camp} → ACTIVE={camp}")];
    if !subs.is_empty() {
        out.push(format!("서브 재개는 A2A 제어명령으로 전한다: relay.py --campaign-control resume --campaign-id {camp} --node <sub> … (echo 가 오기 전까지 '서브 미수렴')"));
    }
    print_messages(io, mode, &out);
    Ok(0)
}

/// `abandoned_gate_reasons(camp, summary_testlog)`.
fn abandoned_gate_reasons(ctx: &Ctx, camp: &str, summary: Option<&str>) -> Vec<String> {
    let base = ctx.campaigns().join(camp);
    let mut reasons = Vec::new();
    match summary.filter(|s| !s.is_empty()) {
        None => reasons.push("포기 요약 testlog 가 없다 — `--summary-testlog <docs/testlog/...md>`(포기 사유·미완 셀 목록)".to_string()),
        Some(s) if !ctx.root.join(s).is_file() => reasons.push(format!("포기 요약 testlog 가 실재하지 않는다: {s}")),
        Some(_) => {}
    }
    let ep = read_json(&base.join("evidence_pointers.json"));
    let ptrs: Vec<PyDict> = ep.as_ref().and_then(dict_of).map(|d| list_or_empty(d.get("pointers"))).unwrap_or_default().iter().filter_map(dict_of).cloned().collect();
    let cells = base.join("cells");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cells).map(|r| r.filter_map(Result::ok).map(|e| e.path()).collect()).unwrap_or_default();
    dirs.sort();
    for cdir in dirs {
        let name = name_of_path(&cdir);
        let measured = read_json(&cdir.join("cell.status.json")).as_ref().and_then(dict_of).map(|st| get_text(st, "cell_outcome").as_deref() == Some("measured")).unwrap_or(false);
        if !measured {
            continue;
        }
        let mine: Vec<&PyDict> = ptrs
            .iter()
            .filter(|p| get_text(p, "cell_id").as_deref() == Some(name.as_str()) && truthy(p.get("path")) && !shown(p.get("path")).contains(FILL))
            .collect();
        if mine.is_empty() {
            reasons.push(format!("측정된 셀 {name} 의 증거 포인터가 없다 — 측정은 docs 평면에 도착해야 지울 수 있다"));
        }
        for p in mine {
            let path = shown(p.get("path"));
            if !ctx.root.join(&path).exists() {
                reasons.push(format!("측정된 셀 {name} 의 증거가 실재하지 않는다: {path}"));
            }
        }
    }
    reasons
}

fn op_abandon(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let camp = args.get("--abandon").unwrap_or_default().to_string();
    let apply = args.has("--apply");
    let mode = if apply { "APPLIED" } else { "DRY-RUN" };
    require_transition_pair(args.get("--utc"), args.get("--approved-by"), "--abandon")?;
    let (utc, approved_by) = (args.get("--utc").unwrap_or_default(), args.get("--approved-by").unwrap_or_default());
    let Some(reason) = args.get("--reason").filter(|r| !r.is_empty()) else {
        return transition_refuse("--abandon 은 --reason 이 필요하다(사유 없는 포기는 종결이 아니라 증발이다)");
    };
    let base = ctx.campaigns().join(&camp);
    if !is_dir(&base) {
        return transition_refuse(format!("인스턴스가 없다: campaigns/{camp}"));
    }
    let f = fold_lifecycle(ctx, &base, None);
    if f.state == "closed" || f.state == "unknown" {
        let tail = if f.state == "unknown" { " (모르는 이벤트: 브랜치 공통층 동기화 필요)" } else { "" };
        return transition_refuse(format!("{camp} 은 {} 다 — 포기할 수 없다{tail}", f.state));
    }
    let mut released: Vec<PyValue> = Vec::new();
    if f.state == "active" {
        if !running_cells(&base).is_empty() {
            return transition_refuse("진행 중인 셀이 있다 — 셀을 끝내거나 먼저 `--suspend --preempt` 로 내린 뒤 포기하라");
        }
        released = release_resources(ctx, f.topology.as_deref(), args.all("--release"), apply)?;
    }
    let summary = args.get("--summary-testlog");
    let reasons = abandoned_gate_reasons(ctx, &camp, summary);
    if !reasons.is_empty() {
        return transition_refuse(format!("abandoned 축소 게이트가 닫혀 있다:\n  - {}", reasons.join("\n  - ")));
    }
    if !apply {
        print_messages(io, mode, &[format!("dry-run abandon {camp}")]);
        return Ok(0);
    }
    let main_node = list_of(read_declaration(&base).get("nodes").unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .find(|n| get_text(n, "role").as_deref() == Some("main") && !shown(n.get("node_id")).contains(FILL))
        .and_then(|n| n.get("node_id").cloned());
    let summary_text = summary.unwrap_or_default();
    let mut pairs: Vec<(&str, String)> = vec![("--kind", "testlog".to_string()), ("--path", summary_text.to_string()), ("--unfreeze", String::new())];
    if let Some(n) = main_node.as_ref().and_then(text_of) {
        pairs.push(("--node", n));
    }
    add_evidence(ctx, &base, &camp, &Args::synthetic(pairs))?;
    let subs = sub_nodes_of(ctx, &base);
    append_journey(
        &base,
        &lifecycle_event(
            "campaign_closed",
            utc,
            vec![
                ("reason", py("abandoned")),
                ("note", py(reason)),
                ("approved_by", py(approved_by)),
                ("summary_testlog", py(summary_text)),
                ("released", PyValue::List(released)),
                ("sub_nodes", PyValue::List(subs.clone())),
                ("answers", last_ref(&f)),
            ],
        ),
    )?;
    if ctx.active_id() == camp {
        write_text(&ctx.active_pointer(), &format!("{BOOTSTRAP}\n"))?;
    }
    let sub_names: Vec<String> = subs.iter().filter_map(text_of).collect();
    let mut out = vec![format!("closed(abandoned) {camp}{}", sub_close_hint(&camp, &sub_names, "abandon"))];
    if let Some(parent) = parent_on_close(ctx, &base, "abandoned", utc)? {
        out.push(format!("부모 {parent} 에 재개 미결(계기 a)"));
    }
    print_messages(io, mode, &out);
    Ok(0)
}

fn pending_dispositions(table: &[(String, Fold)], exclude: &[&str]) -> Vec<String> {
    table
        .iter()
        .filter(|(c, f)| !exclude.contains(&c.as_str()) && (f.state == "suspended" || f.state == "resume_pending") && !f.answered)
        .map(|(c, _)| c.clone())
        .collect()
}

fn apply_dispositions(
    ctx: &Ctx,
    dispositions: &[(String, String)],
    need: &[String],
    utc: &str,
    approved_by: &str,
    table: &[(String, Fold)],
    apply: bool,
) -> R<Vec<String>> {
    let missing: Vec<&String> = need.iter().filter(|c| !dispositions.iter().any(|(d, _)| d == *c)).collect();
    if !missing.is_empty() {
        return transition_refuse(format!(
            "무관한 캠페인 개설(계기 d) — 답이 없는 보류 캠페인의 처분이 필요하다: {}. 선언 확인 팝업에서 묻고 `--dispose <id>=keep` 로 전사하라. 포기는 먼저 `--abandon <id>` 로 종결 사유를 받는다(정상 차단 — 묻지 않고 넘어가면 보류가 잊힌다)",
            missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    let mut wrote = Vec::new();
    for (camp, decision) in dispositions {
        if decision != "keep" {
            return transition_refuse(format!("--dispose 는 keep 만 받는다({}={}) — 포기는 `--abandon {camp}` 로 종결한다", camp, py_repr(decision)));
        }
        let found = table.iter().find(|(c, _)| c == camp).map(|(_, f)| f);
        match found {
            Some(f) if f.state == "suspended" || f.state == "resume_pending" => {
                if apply {
                    let path = append_journey(
                        &ctx.campaigns().join(camp),
                        &lifecycle_event(
                            "campaign_query_answered",
                            utc,
                            vec![
                                ("decision", py("keep")),
                                ("trigger", py("unrelated_open")),
                                ("answers", last_ref(f)),
                                ("approved_by", py(approved_by)),
                            ],
                        ),
                    )?;
                    wrote.push(ctx.rel(&path));
                }
            }
            other => {
                return transition_refuse(format!(
                    "--dispose 대상 {camp} 은 보류 상태가 아니다({})",
                    other.map(|f| py_repr(&f.state)).unwrap_or_else(|| "None".to_string())
                ))
            }
        }
    }
    Ok(wrote)
}

fn op_init(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let new_id = args.get("--init").unwrap_or_default().to_string();
    let apply = args.has("--apply");
    let mode = if apply { "APPLIED" } else { "DRY-RUN" };
    let Some(plan_ref) = args.get("--plan-ref").filter(|s| !s.is_empty()) else {
        io.warn("[campaign_init] FAIL --init 은 --plan-ref 가 필요하다(승인 없는 개설 ✗)");
        return Ok(2);
    };
    let mut dispositions: Vec<(String, String)> = Vec::new();
    for kv in args.all("--dispose") {
        match kv.split_once('=') {
            Some((k, v)) if !k.is_empty() => match dispositions.iter_mut().find(|(n, _)| n == k.trim()) {
                Some(slot) => slot.1 = v.trim().to_string(),
                None => dispositions.push((k.trim().to_string(), v.trim().to_string())),
            },
            _ => return transition_refuse(format!("--dispose 는 CAMP=keep 형식이다: {}", py_repr(kv))),
        }
    }
    let messages = open_campaign(ctx, args, &new_id, plan_ref, dispositions)?;
    print_messages(io, mode, &messages);
    Ok(0)
}

fn open_campaign(ctx: &Ctx, args: &Args, new_id: &str, plan_ref: &str, dispositions: Vec<(String, String)>) -> R<Vec<String>> {
    let apply = args.has("--apply");
    let utc = args.get("--utc");
    let mut approved_by: Option<String> = args.get("--approved-by").map(str::to_string);
    let suspend_current = args.has("--suspend-current");
    let preempt = args.has("--preempt");
    let releases: Vec<String> = args.all("--release").to_vec();
    let purge_previous: Vec<String> = args.all("--purge-previous").to_vec();
    let mut derived_from: PyValue = args.get("--derived-from").filter(|d| !d.is_empty()).map(py).unwrap_or(PyValue::None);
    let cur = ctx.active_id();
    let table = lifecycle_table(ctx);
    let here = ctx.checkout_topology();
    let mut msgs: Vec<String> = Vec::new();
    let slice: Option<PyDict> = match args.get("--from-slice") {
        Some(source) => Some(load_slice(source)?),
        None => None,
    };
    let sub_slice = slice.as_ref().filter(|d| get_text(d, "self_role").as_deref() == Some("sub"));
    if let Some(sd) = sub_slice {
        if !derived_from.is_truthy() {
            derived_from = sd.get("_lifecycle_parent").cloned().unwrap_or(PyValue::None);
        }
    }
    let waiting = table.iter().find(|(_, f)| {
        f.state == "suspended" && dict_of(&f.open_pending).and_then(|d| get_text(d, "id")).as_deref() == Some(new_id)
    });
    let mut target_arg: Option<String> = args.get("--target-topology").map(str::to_string);
    let mut need: Vec<String> = Vec::new();
    if let (Some((_, wf)), false) = (waiting, suspend_current) {
        let pending = dict_of(&wf.open_pending).cloned().unwrap_or_default();
        let want = pending.get("topology").cloned().unwrap_or(PyValue::None);
        if cur != BOOTSTRAP {
            return transition_refuse(format!("개설 대기 {new_id} 를 이어받으려면 ACTIVE 가 비어 있어야 한다(현재 {cur})"));
        }
        if text_of(&want) != here {
            return transition_refuse(format!(
                "{new_id} 는 topology={} 로 개설 대기 중인데 체크아웃은 {} 다 — 대상 브랜치로 전환한 뒤 같은 명령을 다시 쳐라(4자일치 · 오배달 방지)",
                want.repr(),
                here.as_deref().map(py_repr).unwrap_or_else(|| "None".to_string())
            ));
        }
        if !derived_from.is_truthy() {
            derived_from = pending.get("parent").cloned().unwrap_or(PyValue::None);
        }
        target_arg = text_of(&want);
        if approved_by.as_deref().map(str::is_empty).unwrap_or(true) {
            approved_by = pending.get("approved_by").and_then(text_of);
        }
        require_transition_pair(utc, approved_by.as_deref(), "개설 대기 이어받기(2단)")?;
    } else {
        if cur != BOOTSTRAP && !suspend_current && !purge_previous.contains(&cur) {
            return transition_refuse(format!(
                "활성 캠페인 {cur} 이 있다 — 처분 없이 덮어쓰는 --init 은 거부한다(옛 허점: ACTIVE 만 바뀌고 {cur} 이 이름 없는 잔재가 된다). 셋 중 하나를 고른다: ① `--suspend-current [--derived-from {cur}]` 로 보류+개설 ② `--close {cur}`(완주) ③ `--abandon {cur}`(포기) — 정상 차단"
            ));
        }
        if suspend_current && cur == BOOTSTRAP {
            return transition_refuse("--suspend-current 를 줬지만 활성 캠페인이 없다(ACTIVE=_bootstrap) — 플래그를 빼라(의도 모순)");
        }
        if derived_from.is_truthy() && sub_slice.is_none() {
            let parent = text_of(&derived_from).unwrap_or_default();
            let pf = table.iter().find(|(c, _)| *c == parent).map(|(_, f)| f);
            let ok_states: &[&str] = if suspend_current && parent == cur { &["active"] } else { &["suspended", "resume_pending"] };
            if !pf.map(|f| ok_states.contains(&f.state.as_str())).unwrap_or(false) {
                return transition_refuse(format!(
                    "--derived-from {} 은 {} 다 — 파생 부모는 지금 보류되는 활성 캠페인이거나 보류 중인 캠페인이어야 한다",
                    shown(Some(&derived_from)),
                    pf.map(|f| f.state.clone()).unwrap_or_else(|| "None".to_string())
                ));
            }
        }
        need = if derived_from.is_truthy() || sub_slice.is_some() { Vec::new() } else { pending_dispositions(&table, &[cur.as_str()]) };
        if suspend_current || utc.is_some() || approved_by.is_some() || !need.is_empty() || !dispositions.is_empty() {
            require_transition_pair(utc, approved_by.as_deref(), "수명 전이(--init)")?;
        }
    }
    let target = target_arg.or_else(|| here.clone());
    let target = match target.as_deref() {
        Some("single" | "multi") => target.clone().expect("checked"),
        other => {
            return transition_refuse(format!(
                "대상 토폴로지를 정할 수 없다({}) — `--target-topology single|multi`",
                other.map(py_repr).unwrap_or_else(|| "None".to_string())
            ))
        }
    };
    if table.iter().any(|(c, _)| c == new_id) || ctx.campaigns().join(new_id).exists() {
        return transition_refuse(format!("이미 있는 인스턴스다: campaigns/{new_id}"));
    }
    if RESERVED_IDS.contains(&new_id) {
        return transition_refuse(format!("예약 id 로는 캠페인을 열 수 없다: {}", py_repr(new_id)));
    }
    let (utc_text, approved_text) = (utc.unwrap_or_default().to_string(), approved_by.clone().unwrap_or_default());
    if !need.is_empty() {
        apply_dispositions(ctx, &dispositions, &need, &utc_text, &approved_text, &table, false)?;
    }
    let stage1_only = Some(target.as_str()) != here.as_deref();
    if suspend_current {
        let nxt = if stage1_only {
            let mut n = PyDict::new();
            n.set("id", py(new_id));
            n.set("topology", py(&target));
            n.set("plan_ref", py(plan_ref));
            n.set("parent", derived_from.clone());
            n.set("approved_by", approved_by.as_deref().map(py).unwrap_or(PyValue::None));
            Some(PyValue::Dict(n))
        } else {
            None
        };
        let default_reason = format!("{new_id} 개설로 전환");
        let (event, wrote) = suspend_campaign(
            ctx,
            &cur,
            &utc_text,
            &approved_text,
            args.get("--reason").filter(|r| !r.is_empty()).unwrap_or(&default_reason),
            &releases,
            preempt,
            Some(new_id),
            nxt,
            apply,
        )?;
        msgs.push(format!(
            "suspended {cur} (내린 자원 {} · 중단 셀 {})",
            list_or_empty(event.get("released")).len(),
            list_or_empty(event.get("interrupted_cells")).len()
        ));
        msgs.extend(wrote);
    } else if stage1_only {
        return transition_refuse(format!(
            "대상 토폴로지 {} 가 체크아웃({})과 다르다 — 반대 토폴로지 캠페인을 이 브랜치에서 열 수 없다(4자일치). 보류할 활성 캠페인이 있으면 `--suspend-current` 로 1단을 남기고, 없으면 대상 브랜치에서 연다",
            py_repr(&target),
            here.as_deref().map(py_repr).unwrap_or_else(|| "None".to_string())
        ));
    }
    if !need.is_empty() {
        msgs.extend(apply_dispositions(ctx, &dispositions, &need, &utc_text, &approved_text, &table, apply)?);
    }
    if stage1_only {
        msgs.push(format!(
            "1단 완료 — {new_id} 개설 대기(topology={target}). 미커밋 0 확인 → {target}-node 체크아웃 → 같은 `--init {new_id} --plan-ref {plan_ref} --apply` 로 2단"
        ));
        return Ok(msgs);
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
    let dest = ctx.campaigns().join(new_id);
    if apply {
        copy_tree(&ctx.template(), &dest)?;
        let decl = read_json(&dest.join("campaign.yaml"))
            .ok_or_else(|| Stop::Refuse("campaign.yaml 을 읽지 못했다".to_string()))?;
        let mut doc = without_notes(&decl);
        if let Some(sd) = &slice {
            if get_text(sd, "id").as_deref() != Some(new_id) {
                return refuse(format!(
                    "파생 선언의 id({})와 개설 이름({})이 다르다 — 이름이 곧 증거 연결이다",
                    sd.get("id").map(PyValue::repr).unwrap_or_else(|| "None".to_string()),
                    py_repr(new_id)
                ));
            }
            doc = without_notes(&PyValue::Dict(sd.clone()));
        }
        doc.set("id", py(new_id));
        if slice.is_none() {
            doc.set("plan_ref", py(plan_ref));
        } else if !doc.contains_key("plan_ref") {
            doc.set("plan_ref", PyValue::None);
        }
        write_json_ordered(&dest.join("campaign.yaml"), &PyValue::Dict(doc))?;
        let ep_path = dest.join("evidence_pointers.json");
        let ep = read_json(&ep_path).ok_or_else(|| Stop::Refuse("evidence_pointers.json 을 읽지 못했다".to_string()))?;
        let mut ep = without_notes(&ep);
        ep.set("campaign_id", py(new_id));
        if !ep.contains_key("pointers") {
            ep.set("pointers", PyValue::List(Vec::new()));
        }
        write_json_ordered(&ep_path, &PyValue::Dict(ep))?;
        write_text(&ctx.active_pointer(), &format!("{new_id}\n"))?;
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
        if !utc_text.is_empty() {
            append_journey(
                &dest,
                &lifecycle_event(
                    "campaign_opened",
                    &utc_text,
                    vec![
                        ("parent", derived_from.clone()),
                        ("topology", py(&target)),
                        ("approved_by", approved_by.as_deref().map(py).unwrap_or(PyValue::None)),
                        ("plan_ref", py(plan_ref)),
                    ],
                ),
            )?;
        }
    }
    let parent_note = if derived_from.is_truthy() { format!(" parent={}", shown(Some(&derived_from))) } else { String::new() };
    msgs.push(format!("purge={} init=['campaigns/{new_id}']{parent_note}", py_list_repr(&removed)));
    if !swept.is_empty() {
        msgs.push(format!("_bootstrap/relay 정리 {}건", swept.len()));
    }
    if suspend_current && cur != BOOTSTRAP && !sub_nodes_of(ctx, &ctx.campaigns().join(&cur)).is_empty() {
        msgs.push(format!("서브 보류는 A2A 제어명령으로 전한다: relay.py --campaign-control suspend --campaign-id {cur} --node <sub> … (echo 가 오기 전까지 '서브 미수렴' · 메인은 진행)"));
    }
    Ok(msgs)
}

// ── --reentry-decide ─────────────────────────────────────────────────────────────────────────

fn op_reentry_decide(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let cell = args.get("--reentry-decide").unwrap_or_default().to_string();
    let Some((base, camp)) = progress_prelude(ctx, args, io, "reentry_decide")? else { return Ok(0) };
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
    wrote_line(io, &camp, &[ctx.rel(&path), ctx.rel(&journey)]);
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
    need_validator(ctx)?;
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

// ── one writer run (several writer operations of one command line) ──────────────────────────

/// The writer section of the original `main`: one target, one lifecycle guard, every requested writer in a fixed order, and
/// one `wrote(<camp>): ...` line with everything they wrote. The per-operation functions keep their single-operation shape;
/// while a run is open they take its target ([`writer_prelude`], [`progress_prelude`]) and add to its list ([`wrote_line`]).
struct Group {
    base: PathBuf,
    camp: String,
    wrote: Vec<String>,
}

thread_local! {
    static GROUP: std::cell::RefCell<Option<Group>> = const { std::cell::RefCell::new(None) };
}

fn group_target() -> Option<(PathBuf, String)> {
    GROUP.with(|g| g.borrow().as_ref().map(|g| (g.base.clone(), g.camp.clone())))
}

fn group_collect(paths: &[String]) -> bool {
    GROUP.with(|g| match g.borrow_mut().as_mut() {
        Some(group) => {
            group.wrote.extend(paths.iter().cloned());
            true
        }
        None => false,
    })
}

/// The start of a progress write (`--phase-set` ... `--assign-set`) outside a writer run: the `_bootstrap` no-op and the
/// lifecycle guard for that one operation.
fn progress_prelude(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>, op: &str) -> R<Option<(PathBuf, String)>> {
    if let Some(target) = group_target() {
        return Ok(Some(target));
    }
    let Some((base, camp)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(None);
    };
    guard_writer_ops(ctx, &base, &[(op, true)])?;
    Ok(Some((base, camp)))
}

type Step = fn(&Ctx, &Args, &mut Sinks<'_>) -> R<i32>;

/// The writer operations in the original's order (`if a.phase_set: ...` through `if a.hint_approve: ...`).
const WRITER_STEPS: [(&str, Step); 16] = [
    ("--phase-set", op_phase_set),
    ("--cell-materialize", op_cell_materialize),
    ("--cell-set", op_cell_set),
    ("--reentry-decide", op_reentry_decide),
    ("--escalation-add", op_escalation_add),
    ("--evidence-add", op_evidence_add),
    ("--import-sub", op_import_sub),
    ("--ground", op_ground),
    ("--backfill-from-docs", op_backfill),
    ("--evidence-untag", op_evidence_untag),
    ("--evidence-relocate", op_evidence_relocate),
    ("--evidence-prune-stubs", op_evidence_prune_stubs),
    ("--freeze-evidence", op_freeze_evidence),
    ("--assign-set", op_assign_set),
    ("--revise", op_revise),
    ("--hint-approve", op_hint_approve),
];

/// `writer_ops = (a.phase_set, a.cell_set, a.evidence_add, ...)`: is any of them set?
fn any_writer(args: &Args) -> bool {
    WRITER_STEPS.iter().any(|(name, _)| args.on(name))
}

fn op_writer_group(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> R<i32> {
    let Some((base, camp)) = writer_target(ctx, args.get("--campaign-id"))? else {
        io.warn("[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)");
        return Ok(0);
    };
    guard_writer_ops(
        ctx,
        &base,
        &[
            ("phase_set", args.on("--phase-set")),
            ("cell_set", args.on("--cell-set")),
            ("cell_materialize", args.on("--cell-materialize")),
            ("reentry_decide", args.on("--reentry-decide")),
            ("assign_set", args.on("--assign-set")),
        ],
    )?;
    GROUP.with(|g| {
        *g.borrow_mut() = Some(Group {
            base: base.clone(),
            camp: camp.clone(),
            wrote: Vec::new(),
        })
    });
    let mut current = args.clone();
    let result = (|| -> R<()> {
        for (name, step) in WRITER_STEPS {
            if !current.on(name) {
                continue;
            }
            step(ctx, &current, io)?;
            // `a.node = node_of_cell(base, a.cell)`: the node the phase write derived is the node of every later writer.
            if name == "--phase-set" && !current.on("--node") {
                if let Some(cell) = current.get("--cell").filter(|c| !c.is_empty()).map(str::to_string) {
                    if let Some(node) = node_of_cell(&base, &cell) {
                        current.set("--node", node);
                    }
                }
            }
        }
        Ok(())
    })();
    let group = GROUP.with(|g| g.borrow_mut().take());
    result?;
    let wrote = group.map(|g| g.wrote).unwrap_or_default();
    io.say(&format!("[campaign_init] wrote({camp}): {}", wrote.join(" ")));
    Ok(0)
}

// ── dispatch ─────────────────────────────────────────────────────────────────────────────────

/// What a command line asks for after this module's dispatch.
pub enum Next {
    /// Answered: the exit status (output already written).
    Done(i32),
    /// `--active` (answered by `campaign_read`).
    Active,
    /// `--derive KIND [...]` (answered by `campaign_read`): kind, cell, sweep, context, campaign id.
    Derive {
        kind: String,
        cell: Option<String>,
        sweep: Option<String>,
        context: Option<String>,
        campaign_id: Option<String>,
    },
}

fn print_help(io: &mut Sinks<'_>) {
    let _ = io.out.write_all(crate::campaign_init_help::HELP.as_bytes());
}

/// The original `main` after `parse_args`, in its order: the first operation that is set answers; the writer operations are
/// one run; nothing set prints the help with status 2.
pub fn dispatch(ctx: &Ctx, args: &Args, io: &mut Sinks<'_>) -> Next {
    if args.on("--correct-reference") && !args.on("--evidence-relocate") {
        io.warn("[campaign_init] FAIL --correct-reference 는 --evidence-relocate와 함께만 쓴다");
        return Next::Done(1);
    }
    if args.on("--selftest") {
        return Next::Done(crate::campaign_write_selftest::selftest(io));
    }
    if args.on("--active") {
        return Next::Active;
    }
    if args.on("--derive") {
        let get = |n: &str| args.get(n).map(str::to_string);
        return Next::Derive {
            kind: args.get("--derive").unwrap_or_default().to_string(),
            cell: get("--cell"),
            sweep: get("--sweep"),
            context: get("--context"),
            campaign_id: get("--campaign-id"),
        };
    }
    let ops: [(&str, Step); 22] = [
        ("--resume-brief", op_resume_brief),
        ("--warm-start-library", |ctx, _, io| op_warm_start_library(ctx, io)),
        ("--grounding-check", op_grounding_check),
        ("--lockset-precheck", op_lockset_precheck),
        ("--assigned-cells", op_assigned_cells),
        ("--emit-slice", op_emit_slice),
        ("--escalation-summary", op_escalation_summary),
        ("--write-brief", op_write_brief),
        ("--lifecycle", op_lifecycle),
        ("--suspend", op_suspend),
        ("--resume-campaign", op_resume_campaign),
        ("--abandon", op_abandon),
        ("--answer-query", op_answer_query),
        ("--sub-echo", op_sub_echo),
        ("--sub-unconverged-accept", op_sub_unconverged_accept),
        ("--slice-update", op_slice_update),
        ("", op_writer_group),
        ("--residue-scan", op_residue_scan),
        ("--verify-purge-gate", op_verify_purge_gate),
        ("--close", op_close),
        ("--init", op_init),
        ("-", |_, _, io| {
            print_help(io);
            Ok(2)
        }),
    ];
    for (name, op) in ops {
        let selected = match name {
            "" => any_writer(args),
            "-" => true,
            _ => args.on(name),
        };
        if !selected {
            continue;
        }
        return Next::Done(match op(ctx, args, io) {
            Ok(status) => status,
            Err(Stop::Refuse(text)) => {
                io.warn(&format!("[campaign_init] FAIL {text}"));
                1
            }
            Err(Stop::Fallback) => {
                io.warn("[campaign_init] ERROR 이 입력은 원본에서 처리되지 않는 예외로 끝나는 형태다 — 네이티브는 traceback 을 재현하지 않는다");
                1
            }
        });
    }
    Next::Done(2)
}

/// The command line as `campaign_init.py` takes it: parse (argparse errors, `-h`), then [`dispatch`]. `--active`/`--derive`
/// are returned to the caller.
pub fn run_argv(ctx: &Ctx, argv: &[String], io: &mut Sinks<'_>) -> Next {
    match crate::campaign_args::parse(argv) {
        crate::campaign_args::Parsed::Help => {
            print_help(io);
            Next::Done(0)
        }
        crate::campaign_args::Parsed::Error(message) => {
            let _ = write!(io.err, "{}{}: error: {}\n", crate::campaign_args::usage(), crate::campaign_args::PROG, message);
            Next::Done(2)
        }
        crate::campaign_args::Parsed::Args(args) => dispatch(ctx, &args, io),
    }
}

/// [`run_argv`] for the shapes answered here; `--active`/`--derive` report [`Outcome::Fallback`] (they belong to
/// `campaign_read`).
pub fn run(ctx: &Ctx, argv: &[String], io: &mut Sinks<'_>) -> Outcome {
    match run_argv(ctx, argv, io) {
        Next::Done(status) => Outcome::Done(status),
        Next::Active | Next::Derive { .. } => Outcome::Fallback,
    }
}
