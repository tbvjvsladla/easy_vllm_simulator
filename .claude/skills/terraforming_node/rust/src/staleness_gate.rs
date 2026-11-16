//! Port of `staleness_gate.py` — the deterministic trigger of the terraforming conditional preflight (manifest · HW · age
//! axes). Same command line, exit codes (0 FRESH · 4 preflight required · 2 usage/observed invalid), JSON output
//! (`sort_keys=True, indent=2`) and self-test lines as the original. The wall clock is never read (`--now` is injected).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::{decimal_digit_value, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml;
use role_contract::py_eq;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_PREFLIGHT_REQUIRED: i32 = 4;

pub const REASON_MANIFEST_ABSENT: &str = "MANIFEST_ABSENT";
pub const REASON_FLAG_ABSENT: &str = "FLAG_ABSENT";
pub const REASON_HW_DRIFT: &str = "HW_DRIFT";
pub const REASON_ATTESTATION_STALE: &str = "ATTESTATION_STALE";
pub const REASON_MANIFEST_INVALID: &str = "MANIFEST_INVALID";
pub const REASON_OBSERVED_INVALID: &str = "OBSERVED_INVALID";
pub const REASON_FRESH: &str = "FRESH";

const HW_FIELDS: [&str; 6] = ["topology", "cpu_arch", "cuda_version", "gpus_per_node", "gpu_model", "driver_version"];
const INTERCONNECT_FIELDS: [&str; 6] =
    ["type", "hca_devices", "gid_index", "socket_iface", "bandwidth_gbps", "platform_preset"];
const NODE_FIELDS: [&str; 2] = ["role", "host"];
const NODE_COMPARE_FIELDS: [&str; 3] = ["role", "host", "driver_version"];
pub const DEFAULT_MAX_AGE_DAYS: i64 = 90;
const IC_NOTE: &str =
    "single-node: interconnect 6필드는 HW 비교 대상 제외(분산서빙 미사용 · scan_node.evaluate_gate 와 동일 의미론)";

// ── small Python-semantics helpers ───────────────────────────────────────────────────────────

fn get<'a>(d: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    match d.peel() {
        PyValue::Dict(x) => x.get(k).map(|v| v.peel()),
        _ => None,
    }
}
fn has(d: &PyValue, k: &str) -> bool {
    matches!(d.peel(), PyValue::Dict(x) if x.contains_key(k))
}
fn as_str(v: Option<&PyValue>) -> Option<String> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => Some(crate::py::text(s)),
        _ => None,
    }
}
/// `isinstance(v, int) and not isinstance(v, bool)` → its sign/magnitude as i128 (saturating for huge values).
fn plain_int(v: Option<&PyValue>) -> Option<i128> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Int(n)) => {
            let t = n.to_string();
            Some(t.parse::<i128>().unwrap_or(if t.starts_with('-') { i128::MIN } else { i128::MAX }))
        }
        _ => None,
    }
}
fn is_none(v: Option<&PyValue>) -> bool {
    matches!(v.map(|x| x.peel()), Some(PyValue::None) | None)
}
fn nonblank(v: Option<&PyValue>) -> bool {
    matches!(v.map(|x| x.peel()), Some(PyValue::Str(s)) if !s.strip().is_empty())
}
fn ascii_all(s: &str, ok: impl Fn(char) -> bool) -> bool {
    !s.is_empty() && s.chars().all(ok)
}
fn token_ok(s: &str) -> bool {
    ascii_all(s, |c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
}
fn cuda_ok(s: &str) -> bool {
    (2..=4).contains(&s.chars().count()) && s.chars().all(|c| c.is_ascii_digit())
}
fn preset_ok(s: &str) -> bool {
    let mut it = s.chars();
    matches!(it.next(), Some(c) if c.is_ascii_alphanumeric())
        && it.all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}
fn label_ok(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    let an = |x: char| x.is_ascii_alphanumeric();
    match c.len() {
        0 => false,
        1 => an(c[0]),
        n if n <= 63 => an(c[0]) && an(c[n - 1]) && c[1..n - 1].iter().all(|&x| an(x) || x == '-'),
        _ => false,
    }
}

/// `ipaddress.ip_address(value)` succeeds (IPv4 dotted quad without leading zeros, IPv6 with an optional `%scope`).
fn ip_ok(s: &str) -> bool {
    if s.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    let (addr, scope) = match s.split_once('%') {
        Some((a, z)) => (a, Some(z)),
        None => (s, None),
    };
    if scope.is_some_and(|z| z.is_empty() || z.contains('%')) {
        return false;
    }
    addr.parse::<std::net::Ipv6Addr>().is_ok()
}

pub fn valid_host(v: &PyValue) -> bool {
    let PyValue::Str(p) = v.peel() else { return false };
    let s = crate::py::text(p);
    if s.is_empty() || p.len() > 253 {
        return false;
    }
    if ip_ok(&s) {
        return true;
    }
    let body = s.strip_suffix('.').unwrap_or(&s);
    body.split('.').all(label_ok)
}

/// A date as (year, month, day).
pub type Date = (i64, u32, u32);

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

fn ordinal(d: Date) -> i64 {
    // days since 0001-01-01 (proleptic Gregorian), like `date.toordinal()` minus one
    let (y, m, day) = d;
    let yy = y - 1;
    let mut n = yy * 365 + yy / 4 - yy / 100 + yy / 400;
    for mm in 1..m {
        n += days_in_month(y, mm) as i64;
    }
    n + day as i64 - 1
}

/// `parse_stamp(raw)`: exact `YYYYMMDDHH` or `YYYY-MM-DD` (Unicode decimal digits, as Python `\d`) → date, else None.
pub fn parse_stamp(raw: Option<&PyValue>) -> Option<Date> {
    let PyValue::Str(s) = raw?.peel() else { return None };
    let pts = s.code_points();
    let digits = |r: std::ops::Range<usize>| -> Option<i64> {
        let mut n = 0i64;
        for &c in &pts[r] {
            n = n * 10 + decimal_digit_value(c)? as i64;
        }
        Some(n)
    };
    let (y, m, d) = if pts.len() == 10 && pts[4] == '-' as u32 && pts[7] == '-' as u32 {
        (digits(0..4)?, digits(5..7)?, digits(8..10)?)
    } else if pts.len() == 10 {
        let h = digits(8..10)?;
        let v = (digits(0..4)?, digits(4..6)?, digits(6..8)?);
        if h > 23 {
            return None;
        }
        v
    } else {
        return None;
    };
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m as u32) as i64 {
        return None;
    }
    Some((y, m as u32, d as u32))
}

// ── validation ───────────────────────────────────────────────────────────────────────────────

pub fn validate_document(doc: &PyValue, kind: &str, expected_topology: Option<&str>) -> Vec<String> {
    if !matches!(doc.peel(), PyValue::Dict(_)) {
        return vec!["$".into()];
    }
    let mut inv: Vec<String> = vec![];
    let tkey = if kind == "manifest" { "topology" } else { "topology_declared" };
    let topology = if has(doc, tkey) { get(doc, tkey) } else { get(doc, "topology") };
    let topo = as_str(topology);
    if !matches!(topo.as_deref(), Some("single") | Some("multi")) {
        inv.push(tkey.into());
    }
    if kind == "manifest" {
        if let Some(e) = expected_topology {
            if topo.as_deref() != Some(e) {
                inv.push("topology".into());
            }
        }
    }
    for f in ["cpu_arch", "cuda_version", "gpu_model"] {
        if !nonblank(get(doc, f)) {
            inv.push(f.into());
        }
    }
    if let Some(a) = as_str(get(doc, "cpu_arch")) {
        if !["aarch64", "x86_64", "amd64"].contains(&a.as_str()) {
            inv.push("cpu_arch".into());
        }
    }
    if let Some(c) = as_str(get(doc, "cuda_version")) {
        if !cuda_ok(&c) {
            inv.push("cuda_version".into());
        }
    }
    if !plain_int(get(doc, "gpus_per_node")).is_some_and(|n| n >= 1) {
        inv.push("gpus_per_node".into());
    }
    if kind == "manifest" {
        match get(doc, "terraforming") {
            Some(t @ PyValue::Dict(_)) => {
                if parse_stamp(get(t, "scanned_at")).is_none() {
                    inv.push("terraforming.scanned_at".into());
                }
            }
            _ => inv.push("terraforming".into()),
        }
    }
    match get(doc, "interconnect") {
        Some(ic @ PyValue::Dict(_)) => {
            let itype = as_str(get(ic, "type"));
            if !matches!(itype.as_deref(), Some("generic-ethernet") | Some("RoCE v2")) {
                inv.push("interconnect.type".into());
            }
            let hcas = get(ic, "hca_devices");
            let hca_list = match hcas {
                Some(PyValue::List(l)) => Some(l),
                _ => None,
            };
            if hca_list.is_none()
                || hca_list.unwrap().iter().any(|x| !matches!(x.peel(), PyValue::Str(s) if token_ok(&crate::py::text(s))))
            {
                inv.push("interconnect.hca_devices".into());
            }
            let gid = get(ic, "gid_index");
            let scalar_ok = is_none(gid) || plain_int(gid).is_some_and(|n| n >= 0);
            let list_ok = match gid {
                Some(PyValue::List(l)) if l.len() >= 2 => {
                    let ints: Option<Vec<i128>> =
                        l.iter().map(|x| plain_int(Some(x)).filter(|n| *n >= 0)).collect();
                    ints.is_some_and(|v| v.windows(2).all(|w| w[0] < w[1]))
                }
                _ => false,
            };
            if !has(ic, "gid_index") || !(scalar_ok || list_ok) {
                inv.push("interconnect.gid_index".into());
            }
            let sock = get(ic, "socket_iface");
            if !has(ic, "socket_iface")
                || (!is_none(sock) && !as_str(sock).is_some_and(|s| token_ok(&s)))
            {
                inv.push("interconnect.socket_iface".into());
            }
            let bw = get(ic, "bandwidth_gbps");
            let bw_ok = match bw {
                None | Some(PyValue::None) => true,
                Some(PyValue::Int(_)) => plain_int(bw).is_some_and(|n| n >= 0),
                Some(PyValue::Float(x)) => x.is_finite() && *x >= 0.0,
                _ => false,
            };
            if !has(ic, "bandwidth_gbps") || !bw_ok {
                inv.push("interconnect.bandwidth_gbps".into());
            }
            let preset = get(ic, "platform_preset");
            if !has(ic, "platform_preset") || (!is_none(preset) && !as_str(preset).is_some_and(|s| preset_ok(&s))) {
                inv.push("interconnect.platform_preset".into());
            }
            if itype.as_deref() == Some("RoCE v2") {
                if !hca_list.is_some_and(|l| !l.is_empty()) {
                    inv.push("interconnect.hca_devices".into());
                }
                if is_none(gid) {
                    inv.push("interconnect.gid_index".into());
                }
                if is_none(sock) {
                    inv.push("interconnect.socket_iface".into());
                }
            }
        }
        _ => inv.push("interconnect".into()),
    }
    if has(doc, "nodes") {
        match get(doc, "nodes") {
            Some(PyValue::List(nodes)) => {
                for (i, node) in nodes.iter().enumerate() {
                    if !matches!(node.peel(), PyValue::Dict(_)) {
                        inv.push(format!("nodes[{i}]"));
                        continue;
                    }
                    for f in NODE_FIELDS {
                        if !nonblank(get(node, f)) {
                            inv.push(format!("nodes[{i}].{f}"));
                        }
                    }
                    if let Some(r) = as_str(get(node, "role")) {
                        if r != "main" && r != "sub" {
                            inv.push(format!("nodes[{i}].role"));
                        }
                    }
                    if let Some(h @ PyValue::Str(_)) = get(node, "host") {
                        if !valid_host(h) {
                            inv.push(format!("nodes[{i}].host"));
                        }
                    }
                }
            }
            _ => inv.push("nodes".into()),
        }
    }
    let roles = |doc: &PyValue| -> Vec<Option<String>> {
        match get(doc, "nodes") {
            Some(PyValue::List(n)) => n
                .iter()
                .filter(|x| matches!(x.peel(), PyValue::Dict(_)))
                .map(|x| as_str(get(x, "role")))
                .collect(),
            _ => vec![],
        }
    };
    let count = |r: &[Option<String>], w: &str| r.iter().filter(|x| x.as_deref() == Some(w)).count();
    let top = as_str(get(doc, "topology"));
    if kind == "manifest" && top.as_deref() == Some("single") {
        if let Some(PyValue::List(n)) = get(doc, "nodes") {
            if !n.is_empty() && count(&roles(doc), "main") != 1 {
                inv.push("nodes.roster".into());
            }
        }
    }
    if kind == "manifest" && top.as_deref() == Some("multi") {
        let r = roles(doc);
        if count(&r, "main") != 1 || count(&r, "sub") < 1 {
            inv.push("nodes.roster".into());
        }
    }
    inv.sort();
    inv.dedup();
    inv
}

fn strs(v: &[String]) -> PyValue {
    PyValue::List(v.iter().map(|s| PyValue::str(s)).collect())
}

/// The output dict (keys already in `sort_keys` order).
struct Out {
    preflight_required: bool,
    reasons: Vec<String>,
    manifest_path: String,
    hw_check: String,
    drift_fields: Vec<String>,
    age_check: String,
    age_days: Option<i64>,
    max_age_days: i64,
    invalid_fields: Option<Vec<String>>,
    notes: Vec<String>,
}

impl Out {
    fn value(&self) -> PyValue {
        let mut d = PyDict::new();
        d.set("age_check", PyValue::str(&self.age_check));
        d.set("age_days", self.age_days.map_or(PyValue::None, PyValue::int));
        d.set("drift_fields", strs(&self.drift_fields));
        d.set("hw_check", PyValue::str(&self.hw_check));
        if let Some(i) = &self.invalid_fields {
            d.set("invalid_fields", strs(i));
        }
        d.set("manifest_path", PyValue::str(&self.manifest_path));
        d.set("max_age_days", PyValue::int(self.max_age_days));
        d.set("notes", strs(&self.notes));
        d.set("preflight_required", PyValue::Bool(self.preflight_required));
        d.set("reasons", strs(&self.reasons));
        PyValue::Dict(d)
    }
    fn has_reason(&self, r: &str) -> bool {
        self.reasons.iter().any(|x| x == r)
    }
}

fn invalid_result(reason: &str, mut fields: Vec<String>, path: &str, max_age_days: i64) -> Out {
    fields.sort();
    Out {
        preflight_required: true,
        reasons: vec![reason.into()],
        manifest_path: path.into(),
        hw_check: "blocked:invalid-input".into(),
        drift_fields: vec![],
        age_check: "blocked:invalid-input".into(),
        age_days: None,
        max_age_days,
        invalid_fields: Some(fields),
        notes: vec![],
    }
}

// ── canonical HW facts / comparison ──────────────────────────────────────────────────────────

#[derive(Clone)]
enum Canon {
    S(String),
    L(Vec<String>),
    T(Vec<PyValue>),
}

impl PartialEq for Canon {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (Canon::S(a), Canon::S(b)) => a == b,
            (Canon::L(a), Canon::L(b)) => a == b,
            (Canon::T(a), Canon::T(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| py_eq(x, y)),
            _ => false,
        }
    }
}

/// `_scalar(value)`.
fn scalar(v: Option<&PyValue>) -> String {
    match v.map(|x| x.peel()) {
        None | Some(PyValue::None) => String::new(),
        Some(PyValue::Bool(b)) => if *b { "true" } else { "false" }.into(),
        Some(x) => crate::py::text(&x.display_text().strip()),
    }
}

struct Facts {
    top: Vec<(String, String)>,
    ic: Vec<(&'static str, Canon)>,
    nodes_present: bool,
    nodes: Vec<Vec<String>>,
}

impl Facts {
    fn get(&self, f: &str) -> &str {
        &self.top.iter().find(|(k, _)| k == f).unwrap().1
    }
}

fn hw_facts(doc: &PyValue) -> Facts {
    let mut top = vec![];
    for f in HW_FIELDS {
        let mut v = get(doc, f);
        if f == "topology" && (is_none(v) || matches!(v, Some(PyValue::Str(s)) if s.is_empty())) {
            v = get(doc, "topology_declared");
        }
        top.push((f.to_string(), scalar(v)));
    }
    let ic_doc = get(doc, "interconnect").filter(|x| x.is_truthy());
    let mut ic = vec![];
    for f in INTERCONNECT_FIELDS {
        let v = ic_doc.and_then(|d| get(d, f));
        let c = if f == "hca_devices" {
            let mut items: Vec<String> = match v {
                Some(PyValue::List(l)) => l.iter().map(|x| scalar(Some(x))).collect(),
                _ => vec![],
            };
            items.sort();
            Canon::L(items)
        } else if let (true, Some(PyValue::List(l))) = (f == "gid_index", v) {
            Canon::T(l.clone())
        } else {
            Canon::S(scalar(v))
        };
        ic.push((f, c));
    }
    let nodes = match get(doc, "nodes") {
        Some(PyValue::List(l)) => l
            .iter()
            .map(|n| NODE_COMPARE_FIELDS.iter().map(|f| scalar(get(n, f))).collect())
            .collect(),
        _ => vec![],
    };
    Facts { top, ic, nodes_present: has(doc, "nodes"), nodes }
}

fn interconnect_compared(a: &Facts, o: &Facts) -> bool {
    !(a.get("topology") == "single" && o.get("topology") == "single")
}

fn diff_fields(a: &Facts, o: &Facts) -> Vec<String> {
    let mut out = vec![];
    for f in HW_FIELDS {
        if a.get(f) != o.get(f) {
            out.push(f.to_string());
        }
    }
    if interconnect_compared(a, o) {
        for (i, f) in INTERCONNECT_FIELDS.iter().enumerate() {
            if a.ic[i].1 != o.ic[i].1 {
                out.push(format!("interconnect.{f}"));
            }
        }
    }
    if a.nodes_present && o.nodes_present {
        if a.nodes.len() != o.nodes.len() {
            out.push("nodes.count".into());
        } else {
            for (idx, (an, on)) in a.nodes.iter().zip(&o.nodes).enumerate() {
                for (j, f) in NODE_COMPARE_FIELDS.iter().enumerate() {
                    if an[j] != on[j] {
                        out.push(format!("nodes[{idx}].{f}"));
                    }
                }
            }
        }
    }
    out.sort();
    out
}

// ── verdict ──────────────────────────────────────────────────────────────────────────────────

fn evaluate(
    manifest: Option<&PyValue>,
    observed: Option<&PyValue>,
    now: Option<Date>,
    max_age_days: i64,
    manifest_path: &str,
    expected_topology: Option<&str>,
) -> Out {
    if let Some(m) = manifest {
        let inv = validate_document(m, "manifest", expected_topology);
        if !inv.is_empty() {
            return invalid_result(REASON_MANIFEST_INVALID, inv, manifest_path, max_age_days);
        }
    }
    if let Some(o) = observed {
        let inv = validate_document(o, "observed", None);
        if !inv.is_empty() {
            return invalid_result(REASON_OBSERVED_INVALID, inv, manifest_path, max_age_days);
        }
    }
    let terra = |m: &PyValue| get(m, "terraforming").filter(|x| x.is_truthy()).cloned();
    if let (Some(m), Some(n)) = (manifest, now) {
        if let Some(s) = parse_stamp(terra(m).as_ref().and_then(|t| get(t, "scanned_at"))) {
            if ordinal(s) > ordinal(n) {
                return invalid_result(
                    REASON_MANIFEST_INVALID,
                    vec!["terraforming.scanned_at.future".into()],
                    manifest_path,
                    max_age_days,
                );
            }
        }
    }
    let mut reasons: Vec<String> = vec![];
    let mut notes: Vec<String> = vec![];
    let mut drift = vec![];
    let mut age_days = None;
    let mut age_check = "skipped:no-manifest".to_string();
    let mut hw_check = "skipped:no-manifest".to_string();
    match manifest {
        None => reasons.push(REASON_MANIFEST_ABSENT.into()),
        Some(m) => {
            let t = terra(m).unwrap_or(PyValue::Dict(PyDict::new()));
            let is_true = |k: &str| matches!(get(&t, k), Some(PyValue::Bool(true)));
            if !is_true("complete") || !is_true("branch_verified") {
                reasons.push(REASON_FLAG_ABSENT.into());
            }
            match observed {
                None => hw_check = "skipped:no-observed".into(),
                Some(o) => {
                    let (af, of) = (hw_facts(m), hw_facts(o));
                    drift = diff_fields(&af, &of);
                    hw_check = "evaluated".into();
                    if !interconnect_compared(&af, &of) {
                        notes.push(IC_NOTE.into());
                    }
                    if !drift.is_empty() {
                        reasons.push(REASON_HW_DRIFT.into());
                    }
                }
            }
            let scanned = parse_stamp(get(&t, "scanned_at"));
            match (now, scanned) {
                (None, _) => age_check = "skipped:no-now".into(),
                (Some(_), None) => {
                    age_check = "skipped:unparsable-scanned-at".into();
                    notes.push("terraforming.scanned_at 해석 불가 — age 축 미평가(값 날조 ✗)".into());
                }
                (Some(n), Some(s)) => {
                    let d = ordinal(n) - ordinal(s);
                    age_days = Some(d);
                    age_check = "evaluated".into();
                    if d < 0 {
                        notes.push("scanned_at 이 --now 보다 미래 — age 축 미적용".into());
                    } else if d > max_age_days {
                        reasons.push(REASON_ATTESTATION_STALE.into());
                    }
                }
            }
        }
    }
    if reasons.is_empty() {
        reasons.push(REASON_FRESH.into());
    }
    Out {
        preflight_required: reasons != [REASON_FRESH],
        reasons,
        manifest_path: manifest_path.into(),
        hw_check,
        drift_fields: drift,
        age_check,
        age_days,
        max_age_days,
        invalid_fields: None,
        notes,
    }
}

// ── self-test (same cases and lines as the original) ─────────────────────────────────────────

fn j(t: &str) -> PyValue {
    pyjson::loads(t).expect("fixture json")
}
fn with(base: &PyValue, pairs: &[(&str, PyValue)]) -> PyValue {
    let PyValue::Dict(d) = base.clone() else { unreachable!() };
    let mut d = d;
    for (k, v) in pairs {
        d.set(k, v.clone());
    }
    PyValue::Dict(d)
}
fn without(base: &PyValue, key: &str) -> PyValue {
    let PyValue::Dict(d) = base else { unreachable!() };
    let mut n = PyDict::new();
    for (k, v) in d.iter() {
        if !k.equals(key) {
            n.insert(k.clone(), v.clone());
        }
    }
    PyValue::Dict(n)
}
fn plist(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| PyStr::new(s).repr()).collect::<Vec<_>>().join(", "))
}
fn ev(m: Option<&PyValue>, o: Option<&PyValue>, n: Option<Date>) -> Out {
    evaluate(m, o, n, DEFAULT_MAX_AGE_DAYS, "", None)
}
fn json_sorted(o: &Out) -> String {
    crate::py::dumps_unicode(&o.value())
}

fn self_test() -> i32 {
    let mut cases: Vec<(String, bool, String)> = vec![];
    let mut chk = |n: &str, ok: bool, d: String| cases.push((n.to_string(), ok, d));
    let base = j(r#"{"topology": "single", "cpu_arch": "aarch64", "cuda_version": "132", "gpus_per_node": 1,
        "gpu_model": "NVIDIA GB10", "interconnect": {"type": "generic-ethernet", "hca_devices": [], "gid_index": null,
        "socket_iface": null, "bandwidth_gbps": null, "platform_preset": null}, "nodes": [],
        "terraforming": {"complete": true, "branch_verified": true, "scanned_at": "2026072000"}}"#);
    let mut od = PyDict::new();
    for k in ["topology", "cpu_arch", "cuda_version", "gpus_per_node", "gpu_model", "interconnect", "nodes"] {
        od.set(k, get(&base, k).unwrap().clone());
    }
    let observed = PyValue::Dict(od);
    let refd: Date = (2026, 7, 25);
    let r = Some(refd);
    let fresh = ev(Some(&base), Some(&observed), r);
    chk("fresh → preflight 불요", fresh.reasons == [REASON_FRESH] && !fresh.preflight_required, plist(&fresh.reasons));
    chk("manifest 부재 → MANIFEST_ABSENT", ev(None, Some(&observed), r).reasons == [REASON_MANIFEST_ABSENT], String::new());
    let no_flag = with(&base, &[("terraforming", j(r#"{"complete": false, "branch_verified": true, "scanned_at": "2026072000"}"#))]);
    chk("Flag 미발급 → FLAG_ABSENT", ev(Some(&no_flag), Some(&observed), r).has_reason(REASON_FLAG_ABSENT), String::new());
    let drifted = ev(Some(&base), Some(&with(&observed, &[("gpus_per_node", PyValue::int(2))])), r);
    chk("HW 드리프트 → HW_DRIFT + 필드명 보고",
        drifted.has_reason(REASON_HW_DRIFT) && drifted.drift_fields == ["gpus_per_node"], plist(&drifted.drift_fields));
    let roce = j(r#"{"type": "RoCE v2", "hca_devices": ["roceP2p1s0f1", "rocep1s0f1"], "gid_index": 3,
        "socket_iface": "enp1s0f1np1", "bandwidth_gbps": null, "platform_preset": null}"#);
    let single_ic = ev(Some(&base), Some(&with(&observed, &[("interconnect", roce.clone())])), r);
    chk("single + interconnect 발산(미사용 선언 ↔ 실측 RoCE) → HW_DRIFT 미발생",
        single_ic.reasons == [REASON_FRESH] && single_ic.drift_fields.is_empty() && single_ic.hw_check == "evaluated"
            && single_ic.notes.iter().any(|n| n.contains("interconnect")),
        format!("{} drift={} notes={}", plist(&single_ic.reasons), plist(&single_ic.drift_fields), plist(&single_ic.notes)));
    let plus = ev(Some(&base), Some(&with(&observed, &[("interconnect", roce.clone()), ("gpus_per_node", PyValue::int(2))])), r);
    chk("single interconnect 제외가 다른 HW 축을 가리지 않음",
        plus.has_reason(REASON_HW_DRIFT) && plus.drift_fields == ["gpus_per_node"], plist(&plus.drift_fields));
    let m = ev(Some(&base), Some(&observed), r);
    chk("single + interconnect 일치 → 제외 note 는 그대로 표기(침묵 아님)",
        m.notes.iter().any(|n| n.contains("interconnect")), plist(&m.notes));
    chk("attestation 만료 → ATTESTATION_STALE",
        ev(Some(&base), Some(&observed), Some((2027, 7, 25))).has_reason(REASON_ATTESTATION_STALE), String::new());
    let sk = ev(Some(&base), None, None);
    chk("축 생략은 음성정직 표기", sk.hw_check == "skipped:no-observed" && sk.age_check == "skipped:no-now", String::new());
    let unk = ev(Some(&with(&base, &[("terraforming", j(r#"{"complete": true, "branch_verified": true, "scanned_at": "언젠가"}"#))])),
        Some(&observed), r);
    chk("불명 scanned_at 은 fail-closed",
        unk.reasons == [REASON_MANIFEST_INVALID] && unk.preflight_required
            && unk.invalid_fields.as_ref().is_some_and(|f| f.iter().any(|x| x == "terraforming.scanned_at")),
        String::new());
    let roster = j(r#"[{"role": "main", "host": "192.0.2.10"}, {"role": "sub", "host": "192.0.2.11"}]"#);
    let obs_nn = without(&observed, "nodes");
    let inv_of = |o: &Out| o.invalid_fields.clone().unwrap_or_default();
    let inv_disp = |o: &Out| o.invalid_fields.as_ref().map_or("None".to_string(), |f| plist(f));
    let active = ev(Some(&with(&base, &[("nodes", roster.clone())])), Some(&obs_nn), r);
    chk("single + sub-control 로스터 → 유효(HW·age 축 도달)",
        active.reasons == [REASON_FRESH] && active.age_check == "evaluated" && active.hw_check == "evaluated",
        format!("{} hw={} age={}", plist(&active.reasons), active.hw_check, active.age_check));
    chk("single dormant(nodes:[]) → 여전히 유효", ev(Some(&base), Some(&observed), r).reasons == [REASON_FRESH], String::new());
    let main_only = ev(Some(&with(&base, &[("nodes", j(r#"[{"role": "main", "host": "192.0.2.10"}]"#))])), Some(&obs_nn), r);
    chk("★single main 단독 로스터(서브 미등록) → 유효 · HW·age 축 도달",
        main_only.reasons == [REASON_FRESH] && main_only.hw_check == "evaluated" && main_only.age_check == "evaluated",
        format!("{} hw={} age={} inv={}", plist(&main_only.reasons), main_only.hw_check, main_only.age_check, inv_disp(&main_only)));
    let no_main = ev(Some(&with(&base, &[("nodes", j(r#"[{"role": "sub", "host": "192.0.2.11"}]"#))])), Some(&obs_nn), r);
    chk("single 로스터에 main 부재 → nodes.roster fail-closed",
        no_main.reasons == [REASON_MANIFEST_INVALID] && inv_of(&no_main).iter().any(|x| x == "nodes.roster"),
        inv_disp(&no_main));
    let two = ev(Some(&with(&base, &[("nodes", j(r#"[{"role": "main", "host": "192.0.2.10"}, {"role": "main", "host": "192.0.2.11"}]"#))])),
        Some(&obs_nn), r);
    chk("single 로스터 main 중복 → nodes.roster fail-closed", inv_of(&two).iter().any(|x| x == "nodes.roster"), inv_disp(&two));
    let multi_ok = ev(Some(&with(&base, &[("topology", PyValue::str("multi")), ("nodes", roster.clone())])), None, r);
    chk("multi 정상 로스터 → 유효(규칙 불변)", multi_ok.reasons == [REASON_FRESH], plist(&multi_ok.reasons));
    let multi_no_sub = ev(Some(&with(&base, &[("topology", PyValue::str("multi")),
        ("nodes", j(r#"[{"role": "main", "host": "192.0.2.10"}]"#))])), None, r);
    chk("multi 로스터에 sub 부재 → fail-closed(완화되지 않음)",
        inv_of(&multi_no_sub).iter().any(|x| x == "nodes.roster"), inv_disp(&multi_no_sub));
    let ma = with(&base, &[("topology", PyValue::str("multi")), ("nodes", roster.clone())]);
    let mo = with(&obs_nn, &[("topology", PyValue::str("multi"))]);
    let mic = ev(Some(&ma), Some(&with(&mo, &[("interconnect", roce.clone())])), r);
    let mut want = vec!["interconnect.gid_index", "interconnect.hca_devices", "interconnect.socket_iface", "interconnect.type"];
    want.sort();
    let mut got: Vec<&str> = mic.drift_fields.iter().map(|s| s.as_str()).collect();
    got.sort();
    got.dedup();
    chk("multi + interconnect 발산 → 여전히 HW_DRIFT(4필드 보고)",
        mic.has_reason(REASON_HW_DRIFT) && got == want && !mic.notes.iter().any(|n| n.contains("interconnect")),
        format!("{} notes={}", plist(&mic.drift_fields), plist(&mic.notes)));
    chk("multi + interconnect 일치 → FRESH", ev(Some(&ma), Some(&mo), r).reasons == [REASON_FRESH], String::new());
    let mixed = ev(Some(&base), Some(&with(&observed, &[("topology", PyValue::str("multi")), ("interconnect", roce.clone())])), r);
    chk("topology 갈림(single 선언 ↔ multi 관측) → 제외 미적용·fail-closed",
        mixed.has_reason(REASON_HW_DRIFT) && mixed.drift_fields.iter().any(|x| x == "topology")
            && mixed.drift_fields.iter().any(|x| x == "interconnect.type"),
        plist(&mixed.drift_fields));
    let drv_att = with(&base, &[("driver_version", PyValue::str("580.159.03"))]);
    let drv_top = ev(Some(&drv_att), Some(&with(&observed, &[("driver_version", PyValue::str("580.173.02"))])), r);
    chk("top-level driver bump → HW_DRIFT + 필드명 보고",
        drv_top.has_reason(REASON_HW_DRIFT) && drv_top.drift_fields == ["driver_version"], plist(&drv_top.drift_fields));
    let drv_same = ev(Some(&drv_att), Some(&with(&observed, &[("driver_version", PyValue::str("580.159.03"))])), r);
    chk("driver 일치 → 드리프트 아님(비교가 실제로 물린다의 음성 대조)", drv_same.reasons == [REASON_FRESH], plist(&drv_same.reasons));
    let rdrv = j(r#"[{"role": "main", "host": "192.0.2.10", "driver_version": "580.159.03"},
        {"role": "sub", "host": "192.0.2.11", "driver_version": "580.159.03"}]"#);
    let rbump = j(r#"[{"role": "main", "host": "192.0.2.10", "driver_version": "580.159.03"},
        {"role": "sub", "host": "192.0.2.11", "driver_version": "580.173.02"}]"#);
    let nd = ev(Some(&with(&base, &[("nodes", rdrv.clone())])), Some(&with(&observed, &[("nodes", rbump)])), r);
    chk("서브 driver bump → nodes[1].driver_version HW_DRIFT",
        nd.has_reason(REASON_HW_DRIFT) && nd.drift_fields == ["nodes[1].driver_version"], plist(&nd.drift_fields));
    let one = ev(Some(&with(&base, &[("nodes", rdrv)])), Some(&with(&observed, &[("nodes", roster)])), r);
    chk("한쪽만 driver_version 보유 → 무드리프트가 아니라 드리프트(침묵 폴드 금지)",
        one.drift_fields == ["nodes[0].driver_version", "nodes[1].driver_version"], plist(&one.drift_fields));
    let twice: Vec<String> = (0..2).map(|_| json_sorted(&ev(Some(&base), Some(&observed), r))).collect();
    chk("결정론(동일 입력 → 동일 출력)", twice[0] == twice[1], String::new());
    let fut = ev(Some(&base), Some(&observed), Some((2026, 1, 1)));
    chk("미래 scanned_at 은 fail-closed",
        fut.reasons == [REASON_MANIFEST_INVALID] && inv_of(&fut).iter().any(|x| x == "terraforming.scanned_at.future"),
        String::new());
    let failed = cases.iter().filter(|c| !c.1).count();
    for (name, ok, detail) in &cases {
        let tail = if detail.is_empty() { String::new() } else { format!(" — {detail}") };
        println!("{} {name}{tail}", if *ok { "PASS" } else { "FAIL" });
    }
    println!("--- {}/{} PASS", cases.len() - failed, cases.len());
    if failed == 0 { EXIT_OK } else { 1 }
}

// ── CLI ──────────────────────────────────────────────────────────────────────────────────────

pub static SPEC: Spec = Spec {
    prog: "staleness_gate.py",
    usage: include_str!("../help/staleness_gate.usage"),
    help: include_str!("../help/staleness_gate.help"),
    args: &[
        HELP,
        store(&["--topology"]).choices(&["single", "multi"]),
        store(&["--repo"]),
        store(&["--observed"]),
        store(&["--now"]),
        store(&["--max-age-days"]).int(),
        flag(&["--json"]),
        flag(&["--self-test"]),
    ],
    subs: None,
};

/// `os.path.join(a, b)` for a relative `b`.
fn pjoin(a: &str, b: &str) -> String {
    if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

fn print_json(o: &Out) {
    println!("{}", crate::py::dumps_pretty(&o.value()));
}

pub fn main(args: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if a.flag(7) {
        return self_test();
    }
    let Some(topology) = a.text(1) else {
        eprintln!("usage: --topology single|multi 필요(브랜치 추론 ✗ — 토폴로지는 manifest 통로 선택자)");
        return EXIT_USAGE;
    };
    let max_age = a.int(5).unwrap_or(DEFAULT_MAX_AGE_DAYS);
    let mut refd = None;
    if let Some(now) = a.text(4).filter(|s| !s.is_empty()) {
        refd = parse_stamp(Some(&PyValue::str(now)));
        if refd.is_none() {
            eprintln!("usage: --now 는 YYYY-MM-DD 또는 YYYYMMDDHH");
            return EXIT_USAGE;
        }
    }
    let repo = a.text(2).unwrap_or(".");
    let path = pjoin(&pjoin(&pjoin(repo, "output"), topology), "manifest.yaml");
    let mut manifest: Option<PyValue> = None;
    if std::path::Path::new(&path).is_file() {
        let loaded = match std::fs::read(&path) {
            Ok(b) => pyyaml::safe_load(&b, &PyStr::new(&path)).map_err(|e| crate::py::text(&e.message)),
            Err(e) => Err(crate::pathfmt::os_error(&e, &path)),
        };
        manifest = Some(match loaded {
            Ok(PyValue::None) => PyValue::List(vec![]),
            Ok(v) => v,
            Err(m) => {
                eprintln!("manifest 판독 실패: {m}");
                PyValue::List(vec![])
            }
        });
    }
    let mut observed: Option<PyValue> = None;
    if let Some(op) = a.text(3).filter(|s| !s.is_empty()) {
        if !std::path::Path::new(op).is_file() {
            let mut r = invalid_result(REASON_OBSERVED_INVALID, vec!["$".into()], &path, max_age);
            r.notes = vec![format!("observed 경로가 regular file이 아님: {op}")];
            print_json(&r);
            return EXIT_USAGE;
        }
        let loaded = match std::fs::read(op) {
            Ok(b) => pyjson::decode_file_text(&b).and_then(|t| pyjson::loads_text(&t)).map_err(|e| e.to_string()),
            Err(e) => Err(crate::pathfmt::os_error(&e, op)),
        };
        match loaded {
            Ok(PyValue::None) => observed = Some(PyValue::List(vec![])),
            Ok(v) => observed = Some(v),
            Err(m) => {
                let mut r = invalid_result(REASON_OBSERVED_INVALID, vec!["$".into()], &path, max_age);
                r.notes = vec![format!("observed JSON 판독 실패: {m}")];
                print_json(&r);
                return EXIT_USAGE;
            }
        }
    }
    let r = evaluate(manifest.as_ref(), observed.as_ref(), refd, max_age, &path, Some(topology));
    print_json(&r);
    if r.has_reason(REASON_OBSERVED_INVALID) {
        return EXIT_USAGE;
    }
    if r.preflight_required {
        EXIT_PREFLIGHT_REQUIRED
    } else {
        EXIT_OK
    }
}
