//! The launch record's PII substitution and deploy scan: the part of `hintlib/pii.py` that `native_multinode_serve.py::_write_launch` uses
//! (`substitution_table`, `substitute`, `scan_text(profile="deploy")`, `load_pii_terms`) and `launch_substitution_table`. Only these functions are
//! ported (the rest of hintlib stays Python). The four generic patterns run on the Python-`re` engine of this crate, so spans match `re`.
//! Known narrowing: manifest dict keys that are not strings are skipped (a YAML manifest does not carry such keys).

use super::*;
use crate::py_regex_engine::{compile, CompiledPattern, PatternSource};
use std::sync::OnceLock;

pub const PH_REPO: &str = "<repo>";
pub const PH_HOME: &str = "<home>";
pub const PH_PRIV_IP: &str = "<priv-ip>";
pub const PH_HOST: &str = "<host>";
pub const PH_ABS_PATH: &str = "<abs-path>";
pub const PH_REDACTED: &str = "<redacted>";
pub const PH_RUN_ROOT_BASE: &str = "<campaign.native_run_root_base>";
pub const REL_PII_TERMS: &str = ".claude/pii_terms.txt";

/// `GENERIC_PII` in the dict order of `pii.py`.
const GENERIC: [(&str, &str); 4] = [
    (
        "private-ipv4",
        r"\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}\.\d{1,3}|(?<!==)\b(?:192\.168\.|10\.\d{1,3}\.|172\.(?:1[6-9]|2\d|3[01])\.)\d{1,3}",
    ),
    ("email", r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    ("abs-op-path", r"/(?:mnt|home)/[A-Za-z0-9._/-]+"),
    ("spark-host", r"spark-[0-9a-f]{3,}"),
];
const RESIDUAL: [(&str, &str); 4] = [("private-ipv4", PH_PRIV_IP), ("email", PH_REDACTED), ("abs-op-path", PH_ABS_PATH), ("spark-host", PH_HOST)];
const NIC_KEY_TOKENS: [&str; 10] = ["iface", "ifaces", "ifname", "interface", "interfaces", "nic", "nics", "netdev", "hca", "hcas"];
const NIC_LABEL_SHARED: &str = "cluster";
// priorities of `pii.py` (lower wins)
const P_REPO: u8 = 0;
const P_NODE: u8 = 1;
const P_NIC: u8 = 2;
const P_FIELD: u8 = 3;
const P_HOME: u8 = 4;
const P_GENERIC: u8 = 5;
const P_TERM: u8 = 6;

fn prio_name(p: u8) -> &'static str {
    match p {
        P_REPO => "repo",
        P_NODE => "manifest.nodes",
        P_NIC => "manifest NIC 필드",
        P_FIELD => "manifest 경로 필드",
        P_HOME => "home",
        P_GENERIC => "manifest 값",
        _ => "pii_terms",
    }
}

fn patterns() -> R<&'static Vec<CompiledPattern>> {
    static P: OnceLock<Result<Vec<CompiledPattern>, String>> = OnceLock::new();
    let r = P.get_or_init(|| {
        let mut v = Vec::new();
        for (name, src) in GENERIC {
            match compile(PatternSource::Text(&PyStr::new(src))) {
                Ok(p) => v.push(p),
                Err(e) => return Err(format!("PII 패턴 {name} 컴파일 불가: {e:?}")),
            }
        }
        Ok(v)
    });
    match r {
        Ok(v) => Ok(v),
        Err(m) => serve(m.clone()),
    }
}

/// Spans (code points) of generic pattern `idx` in `text`.
fn spans(idx: usize, text: &[char]) -> R<Vec<(usize, usize)>> {
    let s: String = text.iter().collect();
    let mut it = patterns()?[idx].finditer_text(&PyStr::new(&s)).map_err(|e| Err::Serve(format!("PII 스캔 불가: {e:?}")))?;
    let mut out = Vec::new();
    while let Some(sp) = it.next_span().map_err(|e| Err::Serve(format!("PII 스캔 불가: {e:?}")))? {
        out.push(sp);
    }
    Ok(out)
}

fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `_is_section_anchored`: the text between the line start and the match ends with `§\s*` or is `#{1,6}\s+` as a whole.
fn section_anchored(text: &[char], start: usize) -> bool {
    let ls = text[..start].iter().rposition(|&c| c == '\n').map_or(0, |i| i + 1);
    let prefix = &text[ls..start];
    let end = prefix.iter().rposition(|&c| !is_py_space(c)).map_or(0, |i| i + 1);
    if end > 0 && prefix[end - 1] == '§' {
        return true;
    }
    let hashes = prefix.iter().take_while(|&&c| c == '#').count();
    (1..=6).contains(&hashes) && prefix.len() > hashes && prefix[hashes..].iter().all(|&c| is_py_space(c))
}

fn count_lines(text: &[char], upto: usize) -> usize {
    text[..upto].iter().filter(|&&c| c == '\n').count() + 1
}

/// `load_pii_terms`: `None` when the file is absent; a file that cannot be read is an error.
pub fn load_pii_terms(repo: &Path) -> R<Option<Vec<String>>> {
    let p = repo.join(REL_PII_TERMS);
    if !p.exists() {
        return Ok(None);
    }
    if !p.is_file() {
        return serve(format!("HINT_PII_TERMS_UNREADABLE: {REL_PII_TERMS} 가 파일이 아니다"));
    }
    let raw = std::fs::read(&p).map_err(|e| Err::Serve(format!("HINT_PII_TERMS_UNREADABLE: {REL_PII_TERMS} 를 읽을 수 없다: {e}")))?;
    let raw = String::from_utf8(raw).map_err(|_| Err::Serve(format!("HINT_PII_TERMS_UNREADABLE: {REL_PII_TERMS} 를 읽을 수 없다: UnicodeDecodeError")))?;
    let mut terms = Vec::new();
    for line in py_splitlines(&raw) {
        let s = line.trim();
        if !s.is_empty() && !s.starts_with('#') {
            terms.push(s.to_string());
        }
    }
    Ok(Some(terms))
}

/// `str.splitlines()` (the line boundaries Python recognises).
fn py_splitlines(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn dedupe(terms: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for t in terms {
        if !t.is_empty() && seen.insert(t.clone()) {
            out.push(t.clone());
        }
    }
    out
}

/// `scan_text(text, terms, profile="deploy")`: `(pattern, match, line)` for every literal and generic hit (anchored section numbers exempt).
pub fn scan_deploy(text: &str, terms: Option<&[String]>) -> R<Vec<(String, String, usize)>> {
    let chars: Vec<char> = text.chars().collect();
    let mut found: Vec<(usize, usize, (String, String, usize))> = Vec::new();
    let lits = dedupe(terms.unwrap_or(&[]));
    for (order, t) in lits.iter().enumerate() {
        let tc: Vec<char> = t.chars().collect();
        let mut i = 0;
        while i + tc.len() <= chars.len() {
            if chars[i..i + tc.len()] == tc[..] {
                found.push((i, order, ("term".into(), t.clone(), count_lines(&chars, i))));
                i += tc.len();
            } else {
                i += 1;
            }
        }
    }
    let base = lits.len();
    for (idx, (name, _)) in GENERIC.iter().enumerate() {
        for (s, e) in spans(idx, &chars)? {
            if *name == "private-ipv4" && section_anchored(&chars, s) {
                continue;
            }
            found.push((s, base + idx, (name.to_string(), chars[s..e].iter().collect(), count_lines(&chars, s))));
        }
    }
    found.sort_by_key(|f| (f.0, f.1));
    Ok(found.into_iter().map(|f| f.2).collect())
}

// ── substitution table ──────────────────────────────────────────────────────────────────────────

fn is_ipv4(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 4 && p.iter().all(|o| (1..=3).contains(&o.len()) && o.bytes().all(|b| b.is_ascii_digit()) && o.parse::<u32>().is_ok_and(|n| n <= 255))
}

/// `GENERIC_PII["private-ipv4"].fullmatch(v)` for a string that is already a dotted quad.
fn is_private_ipv4(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    if p.len() != 4 {
        return false;
    }
    match p[0] {
        "192" => p[1] == "168",
        "10" => true,
        "172" => matches!(p[1], "16" | "17" | "18" | "19" | "20" | "21" | "22" | "23" | "24" | "25" | "26" | "27" | "28" | "29" | "30" | "31"),
        _ => false,
    }
}

fn is_spark_host(v: &str) -> bool {
    v.strip_prefix("spark-").is_some_and(|r| r.chars().count() >= 3 && r.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

fn is_host_key(key: &str) -> bool {
    let k = key.to_lowercase();
    matches!(k.as_str(), "host" | "hostname" | "ssh_host") || k.ends_with("_host") || k.ends_with("_hostname")
}

fn is_hostname(v: &str) -> bool {
    let b = v.as_bytes();
    !b.is_empty()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
        && b[0].is_ascii_alphanumeric()
        && b[b.len() - 1].is_ascii_alphanumeric()
}

fn is_user(v: &str) -> bool {
    !v.is_empty() && v.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn is_abs_path(v: &str) -> bool {
    if !v.starts_with('/') || v.starts_with("//") || v.chars().any(is_py_space) {
        return false;
    }
    v.split('/').filter(|p| !p.is_empty()).count() >= 2
}

fn home_of(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0].is_empty() && parts[1] == "home" && is_user(parts[2]) {
        return Some(format!("/home/{}", parts[2]));
    }
    None
}

fn is_role(v: &str) -> bool {
    let b = v.as_bytes();
    !b.is_empty() && b[0].is_ascii_lowercase() && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

fn node_labels(nodes: &[&PyValue]) -> Vec<String> {
    let ok: Vec<Option<String>> = nodes
        .iter()
        .map(|n| as_str(dget(n, "role")).filter(|r| is_role(r)))
        .collect();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in ok.iter().flatten() {
        *counts.entry(r.clone()).or_insert(0) += 1;
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut labels = Vec::new();
    for (i, r) in ok.iter().enumerate() {
        match r {
            None => labels.push(i.to_string()),
            Some(r) if counts[r] == 1 => labels.push(r.clone()),
            Some(r) => {
                let n = seen.entry(r.clone()).or_insert(0);
                *n += 1;
                labels.push(format!("{r}{n}"));
            }
        }
    }
    labels
}

fn sorted_entries(d: &PyDict) -> Vec<(String, &PyValue)> {
    let mut e: Vec<(String, &PyValue)> = d.iter().filter_map(|(k, v)| k.to_utf8().map(|k| (k, v.peel()))).collect();
    e.sort_by(|a, b| a.0.cmp(&b.0));
    e
}

fn walk_strings(obj: &PyValue, prefix: &str, top: bool, out: &mut Vec<(String, String)>) {
    match obj.peel() {
        PyValue::Dict(d) => {
            for (key, v) in sorted_entries(d) {
                let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                if key == "nodes" && prefix.is_empty() && top {
                    if let PyValue::List(l) = v.peel() {
                        let refs: Vec<&PyValue> = l.iter().map(PyValue::peel).collect();
                        let labels = node_labels(&refs);
                        for (i, item) in refs.iter().enumerate() {
                            walk_strings(item, &format!("nodes[{}]", labels[i]), false, out);
                        }
                        continue;
                    }
                }
                walk_strings(v, &path, false, out);
            }
        }
        PyValue::List(l) => {
            for (i, item) in l.iter().enumerate() {
                walk_strings(item, &format!("{prefix}[{i}]"), false, out);
            }
        }
        PyValue::Str(s) => {
            if let Some(s) = s.to_utf8() {
                out.push((prefix.to_string(), s));
            }
        }
        _ => {}
    }
}

fn is_nic_key(key: &str) -> bool {
    let mut base = key.to_string();
    if base.ends_with(']') {
        if let Some(i) = base.rfind('[') {
            if !base[i + 1..base.len() - 1].contains(']') {
                base.truncate(i);
            }
        }
    }
    base.to_lowercase().split('_').any(|t| NIC_KEY_TOKENS.contains(&t))
}

fn nic_values(items: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for it in items {
        for part in it.split(',') {
            let v = part.trim().trim_start_matches(['^', '=']).trim();
            let b = v.as_bytes();
            let shape = (2..=32).contains(&b.len())
                && b[0].is_ascii_alphabetic()
                && b[1..].iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'-'));
            if shape && b.iter().any(u8::is_ascii_digit) && !is_ipv4(v) {
                out.push(v.to_string());
            }
        }
    }
    out
}

fn walk_nic(obj: &PyValue, key: &str, out: &mut Vec<String>) {
    match obj.peel() {
        PyValue::Dict(d) => {
            for (k, v) in sorted_entries(d) {
                if k == "nodes" && key.is_empty() {
                    continue;
                }
                let strs: Option<Vec<String>> = match v.peel() {
                    PyValue::Str(s) => s.to_utf8().map(|s| vec![s]),
                    PyValue::List(l) => l.iter().map(|x| if let PyValue::Str(s) = x.peel() { s.to_utf8() } else { None }).collect(),
                    _ => None,
                };
                match strs {
                    Some(items) if is_nic_key(&k) => out.extend(nic_values(&items.iter().map(String::as_str).collect::<Vec<_>>())),
                    _ => walk_nic(v, &k, out),
                }
            }
        }
        PyValue::List(l) => {
            for item in l {
                walk_nic(item, key, out);
            }
        }
        _ => {}
    }
}

fn abspath(p: &Path) -> String {
    let joined = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut parts: Vec<String> = Vec::new();
    for c in joined.to_string_lossy().split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x.to_string()),
        }
    }
    format!("/{}", parts.join("/"))
}

/// `substitution_table(repo, manifest)`: `[(literal, placeholder)]`, longest literal first.
pub fn substitution_table(repo: &Path, manifest: Option<&PyValue>) -> R<Vec<(String, String)>> {
    let mut best: BTreeMap<String, (u8, String)> = BTreeMap::new();
    let mut add = |prio: u8, literal: &str, ph: &str| {
        if literal.is_empty() {
            return;
        }
        let better = match best.get(literal) {
            None => true,
            Some((p, h)) => (prio, ph) < (*p, h.as_str()),
        };
        if better {
            best.insert(literal.to_string(), (prio, ph.to_string()));
        }
    };
    let mut forms = vec![abspath(repo)];
    let resolved = std::fs::canonicalize(repo).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| abspath(repo));
    if !forms.contains(&resolved) {
        forms.push(resolved);
    }
    for rp in &forms {
        if is_abs_path(rp) {
            add(P_REPO, rp.trim_end_matches('/'), PH_REPO);
        }
        if let Some(h) = home_of(rp) {
            add(P_HOME, &h, PH_HOME);
        }
    }
    if let Some(man) = manifest {
        let PyValue::Dict(md) = man.peel() else {
            return serve("HINT_PII_MANIFEST_SHAPE: manifest 는 dict 여야 한다");
        };
        let nodes_v = md.get("nodes").map(PyValue::peel);
        let nodes: Vec<&PyValue> = match nodes_v {
            None | Some(PyValue::None) => Vec::new(),
            Some(PyValue::List(l)) => l.iter().map(PyValue::peel).collect(),
            Some(v) if !v.is_truthy() => Vec::new(),
            Some(_) => return serve("HINT_PII_MANIFEST_SHAPE: manifest.nodes 는 목록이어야 한다"),
        };
        let labels = node_labels(&nodes);
        let mut nic_owner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            let PyValue::Dict(nd) = node else {
                return serve(format!("HINT_PII_MANIFEST_SHAPE: manifest.nodes[{i}] 가 mapping 이 아니다"));
            };
            for (key, val) in sorted_entries(nd) {
                let PyValue::Str(s) = val.peel() else { continue };
                let Some(s) = s.to_utf8() else { continue };
                let v = s.trim();
                if v == "localhost" || v == "0.0.0.0" || v.starts_with("127.") {
                    continue;
                }
                if is_ipv4(v) || (is_host_key(&key) && v.chars().count() >= 3 && is_hostname(v)) {
                    add(P_NODE, v, &format!("<node:{}>", labels[i]));
                }
                if key == "ssh_user" && is_user(v) {
                    add(P_HOME, &format!("/home/{v}"), PH_HOME);
                }
            }
            let mut nn = Vec::new();
            walk_nic(node, "node", &mut nn);
            for nic in nn {
                nic_owner.entry(nic).or_default().insert(labels[i].clone());
            }
        }
        let mut top = Vec::new();
        walk_nic(man, "", &mut top);
        for nic in top {
            nic_owner.entry(nic).or_default().insert(NIC_LABEL_SHARED.into());
        }
        for (nic, owners) in &nic_owner {
            let label = if owners.len() == 1 { owners.iter().next().unwrap().as_str() } else { NIC_LABEL_SHARED };
            add(P_NIC, nic, &format!("<nic:{label}>"));
        }
        let mut strs = Vec::new();
        walk_strings(man, "", true, &mut strs);
        for (field, val) in strs {
            let v = val.trim();
            if is_abs_path(v) {
                add(P_FIELD, v.trim_end_matches('/'), &format!("<manifest.{field}>"));
            } else if is_ipv4(v) && is_private_ipv4(v) {
                add(P_GENERIC, v, PH_PRIV_IP);
            } else if is_spark_host(v) {
                add(P_GENERIC, v, PH_HOST);
            }
        }
    }
    for t in dedupe(&load_pii_terms(repo)?.unwrap_or_default()) {
        add(P_TERM, &t, PH_REDACTED);
    }
    let mut placeholders: BTreeSet<String> = best.values().map(|(_, h)| h.clone()).collect();
    for (_, h) in RESIDUAL {
        placeholders.insert(h.to_string());
    }
    for h in [PH_REPO, PH_HOME, PH_PRIV_IP, PH_HOST, PH_ABS_PATH, PH_REDACTED] {
        placeholders.insert(h.to_string());
    }
    for (lit, (prio, _)) in &best {
        if let Some(clash) = placeholders.iter().find(|ph| ph.contains(lit.as_str())) {
            return serve(format!(
                "HINT_PII_SUBST_COLLISION: {} 리터럴(길이 {})이 치환 표지 {clash} 안에 들어 있다 — 치환이 멱등이 아니게 되고 배포 스캔이 치환된 본문을 다시 잡는다",
                prio_name(*prio),
                lit.chars().count()
            ));
        }
    }
    let mut rows: Vec<(String, String)> = best.into_iter().map(|(l, (_, p))| (l, p)).collect();
    rows.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()).then_with(|| a.0.cmp(&b.0)));
    Ok(rows)
}

/// `launch_substitution_table`: the manifest table plus the campaign run-root rows (the run-root rows win for the same literal).
pub fn launch_substitution_table(c: &Ctx) -> R<Vec<(String, String)>> {
    let man_path = c.repo.join("output").join(TOPOLOGY).join("manifest.yaml");
    let bytes = std::fs::read(&man_path).map_err(|e| Err::Serve(format!("{}: {e}", man_path.display())))?;
    let man = agent_runtime_support::pyyaml::safe_load(&bytes, &PyStr::new(&man_path.to_string_lossy()))
        .map_err(|_| Err::Serve(format!("yaml 해석 실패: {}", man_path.display())))?;
    let man = if man.is_truthy() { man } else { PyValue::Dict(PyDict::new()) };
    let mut extra: BTreeMap<String, String> = BTreeMap::new();
    extra.insert(c.base.trim_end_matches('/').to_string(), PH_RUN_ROOT_BASE.to_string());
    for spec in [&c.main, &c.sub] {
        extra.entry(spec.root.trim_end_matches('/').to_string()).or_insert_with(|| format!("{PH_RUN_ROOT_BASE}/{}", c.run_id));
        let short_ph = format!("{SHORT_BASE}/{}", c.run_id);
        if spec.short.trim_end_matches('/') != short_ph {
            extra.entry(spec.short.trim_end_matches('/').to_string()).or_insert(short_ph);
        }
    }
    let mut rows: Vec<(String, String)> = substitution_table(&c.repo, Some(&man))?.into_iter().filter(|(l, _)| !extra.contains_key(l)).collect();
    rows.extend(extra);
    Ok(rows)
}

// ── substitute ──────────────────────────────────────────────────────────────────────────────────

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The boundary rule of `_literal_regex`, decided by the literal's shape.
fn matches_at(text: &[char], i: usize, lit: &[char], ph: &str, lit_s: &str) -> bool {
    if i + lit.len() > text.len() || text[i..i + lit.len()] != lit[..] {
        return false;
    }
    let prev = if i == 0 { None } else { Some(text[i - 1]) };
    let next = text.get(i + lit.len()).copied();
    if ph.starts_with("<nic:") {
        return !prev.is_some_and(word) && !next.is_some_and(word);
    }
    if is_ipv4(lit_s) {
        let after_dot_digit = next == Some('.') && text.get(i + lit.len() + 1).is_some_and(char::is_ascii_digit);
        return !prev.is_some_and(|p| p.is_ascii_digit() || p == '.') && !next.is_some_and(|n| n.is_ascii_digit()) && !after_dot_digit;
    }
    if lit_s.starts_with('/') {
        return !next.is_some_and(|n| n.is_ascii_alphanumeric() || matches!(n, '.' | '_' | '-'));
    }
    true
}

/// `substitute(text, table)` with the residual four-pattern mask.
pub fn substitute(text: &str, table: &[(String, String)]) -> R<String> {
    let mut repl: BTreeMap<&str, &str> = BTreeMap::new();
    for (l, p) in table {
        if l.is_empty() {
            return serve("HINT_PII_SUBST_TABLE_SHAPE: 치환표 행은 (비어 있지 않은 리터럴, 표지) 문자열 쌍이다");
        }
        if let Some(prev) = repl.insert(l, p) {
            if prev != p {
                return serve(format!("HINT_PII_SUBST_TABLE_SHAPE: 같은 리터럴(길이 {})에 표지가 둘이다", l.chars().count()));
            }
        }
    }
    let mut order: Vec<(&str, &str, Vec<char>)> = repl.iter().map(|(l, p)| (*l, *p, l.chars().collect())).collect();
    order.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then_with(|| a.0.cmp(b.0)));
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some((_, ph, lc)) = order.iter().find(|(l, p, lc)| matches_at(&chars, i, lc, p, l)) {
            out.extend(ph.chars());
            i += lc.len();
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    let mut cur = out;
    for (idx, (name, ph)) in RESIDUAL.iter().enumerate() {
        let mut next: Vec<char> = Vec::with_capacity(cur.len());
        let mut last = 0;
        for (s, e) in spans(idx, &cur)? {
            if *name == "private-ipv4" && section_anchored(&cur, s) {
                continue;
            }
            next.extend(&cur[last..s]);
            next.extend(ph.chars());
            last = e;
        }
        next.extend(&cur[last..]);
        cur = next;
    }
    Ok(cur.into_iter().collect())
}
