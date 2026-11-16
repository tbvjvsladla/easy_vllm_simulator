//! `agent_card_contract.py` — Agent Card structural/capability metadata validator.
//!
//! Endpoint authentication belongs to the Terraform-provisioned OpenSSH transport. The card advertises interfaces,
//! capabilities, skills and the derived node-role extension; it is neither a credential nor an authorization token.
//! Exit codes: 0 OK · 2 usage · 5 contract violation (1 = an uncaught Python exception of the original).

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::py;
use agent_runtime_support::pydyn::dict_key;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{DictKey, PyDict, PyValue};

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_CONTRACT: i32 = 5;

const CARD_REQUIRED: [&str; 8] = [
    "name",
    "description",
    "supportedInterfaces",
    "version",
    "capabilities",
    "defaultInputModes",
    "defaultOutputModes",
    "skills",
];
const CARD_OPTIONAL: [&str; 5] = [
    "provider",
    "documentationUrl",
    "securitySchemes",
    "securityRequirements",
    "iconUrl",
];
const INTERFACE_REQUIRED: [&str; 3] = ["url", "protocolBinding", "protocolVersion"];
const SKILL_REQUIRED: [&str; 6] = [
    "id",
    "name",
    "description",
    "tags",
    "inputModes",
    "outputModes",
];
const SKILL_EXTRA: [&str; 3] = ["examples", "securityRequirements", "extendedDocumentation"];
const CAPABILITY_FIELDS: [&str; 4] = [
    "streaming",
    "pushNotifications",
    "extendedAgentCard",
    "extensions",
];
const EXTENSION_FIELDS: [&str; 4] = ["uri", "description", "required", "params"];
const PROVIDER_REQUIRED: [&str; 2] = ["url", "organization"];
const STANDARD_BINDINGS: [&str; 3] = ["JSONRPC", "GRPC", "HTTP+JSON"];
pub const NODE_ROLE_EXT_URI: &str = "urn:easy-vllm:ext:node-role:v1";
pub const NODE_ROLE_PARAMS: [&str; 9] = [
    "topology",
    "sub_mode",
    "sub_mode_source",
    "rank",
    "rank_source",
    "identity_authority",
    "delivery_plane",
    "tool_plane",
    "tool_plane_source",
];
pub const EXPECTED_SKILL_IDS: [&str; 6] =
    ["inspect", "config", "build", "serve", "bench", "publish"];

pub use crate::raise::Raised;

fn dict(v: Option<&PyValue>) -> Option<&PyDict> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Dict(d)) => Some(d),
        _ => None,
    }
}
fn as_str(v: Option<&PyValue>) -> Option<&PyStr> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => Some(s),
        _ => None,
    }
}
fn is_bool(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::Bool(_))
}

/// Python `repr(sorted(keys))` for a list of str keys.
fn keys_repr(keys: &[PyStr]) -> String {
    let mut k: Vec<&PyStr> = keys.iter().collect();
    k.sort_by(|a, b| a.code_points().cmp(b.code_points()));
    format!(
        "[{}]",
        k.iter().map(|x| x.repr()).collect::<Vec<_>>().join(", ")
    )
}

fn unknown_keys(d: &PyDict, allowed: &[&str]) -> Vec<PyStr> {
    d.iter()
        .filter(|(k, _)| !allowed.iter().any(|a| k.equals(a)))
        .map(|(k, _)| k.clone())
        .collect()
}

/// `urllib.parse.urlparse(value)` reduced to the original's question: `scheme and (netloc or path)`.
pub fn is_uri(value: Option<&PyValue>) -> Result<bool, Raised> {
    let Some(s) = as_str(value) else {
        return Ok(false);
    };
    if s.is_empty() {
        return Ok(false);
    }
    let mut url: Vec<char> = py::text(s)
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();
    let lead = url.iter().take_while(|&&c| (c as u32) <= 0x20).count();
    url.drain(..lead);
    let mut scheme = String::new();
    if let Some(i) = url.iter().position(|&c| c == ':') {
        if i > 0
            && url[0].is_ascii_alphabetic()
            && url[..i]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            scheme = url[..i].iter().collect::<String>().to_ascii_lowercase();
            url.drain(..=i);
        }
    }
    let mut netloc = String::new();
    if url.len() >= 2 && url[0] == '/' && url[1] == '/' {
        let rest = &url[2..];
        let end = rest
            .iter()
            .position(|c| matches!(c, '/' | '?' | '#'))
            .unwrap_or(rest.len());
        netloc = rest[..end].iter().collect();
        url = rest[end..].to_vec();
        let (o, c) = (netloc.contains('['), netloc.contains(']'));
        if o != c {
            return Err(Raised("ValueError: Invalid IPv6 URL".into()));
        }
        if o && c {
            check_bracketed_netloc(&netloc)?;
        }
    }
    if let Some(i) = url.iter().position(|&c| c == '#') {
        url.truncate(i);
    }
    if let Some(i) = url.iter().position(|&c| c == '?') {
        url.truncate(i);
    }
    const USES_PARAMS: [&str; 16] = [
        "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
        "sip", "sips", "mms", "sftp", "tel",
    ];
    if USES_PARAMS.contains(&scheme.as_str()) && url.contains(&';') {
        let i = match url.iter().rposition(|&c| c == '/') {
            Some(slash) => url[slash..]
                .iter()
                .position(|&c| c == ';')
                .map(|k| k + slash),
            None => url.iter().position(|&c| c == ';'),
        };
        if let Some(i) = i {
            url.truncate(i);
        }
    }
    Ok(!scheme.is_empty() && (!netloc.is_empty() || !url.is_empty()))
}

fn check_bracketed_netloc(netloc: &str) -> Result<(), Raised> {
    let hp = netloc.rsplit_once('@').map_or(netloc, |(_, h)| h);
    let host = match hp.split_once('[') {
        Some((before, bracketed)) => {
            if !before.is_empty() {
                return Err(Raised("ValueError: Invalid IPv6 URL".into()));
            }
            let (h, port) = bracketed.split_once(']').unwrap_or((bracketed, ""));
            if !port.is_empty() && !port.starts_with(':') {
                return Err(Raised("ValueError: Invalid IPv6 URL".into()));
            }
            h.to_string()
        }
        None => hp.split_once(':').map_or(hp, |(h, _)| h).to_string(),
    };
    if let Some(rest) = host.strip_prefix('v') {
        let ok = rest.split_once('.').is_some_and(|(hex, tail)| {
            !hex.is_empty()
                && hex.chars().all(|c| c.is_ascii_hexdigit())
                && !tail.is_empty()
                && !tail.contains('\n')
        });
        if !ok {
            return Err(Raised("ValueError: IPvFuture address is invalid".into()));
        }
        return Ok(());
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return Err(Raised(
            "ValueError: An IPv4 address cannot be in brackets".into(),
        ));
    }
    let bare = host.split_once('%').map_or(host.as_str(), |(a, _)| a);
    if bare.parse::<std::net::Ipv6Addr>().is_err() {
        return Err(Raised(format!(
            "ValueError: {} does not appear to be an IPv4 or IPv6 address",
            PyStr::new(&host).repr()
        )));
    }
    Ok(())
}

fn no_floats(v: &PyValue, path: &str) -> Option<String> {
    match v.peel() {
        PyValue::Float(_) => Some(format!(
            "{path}: float 금지(결정론 metadata는 정수/문자열로 표현)"
        )),
        PyValue::Dict(d) => d
            .iter()
            .find_map(|(k, c)| no_floats(c, &format!("{path}.{}", py::text(k)))),
        PyValue::List(l) => l
            .iter()
            .enumerate()
            .find_map(|(i, c)| no_floats(c, &format!("{path}[{i}]"))),
        _ => None,
    }
}

fn hash_all(values: &[PyValue]) -> Result<Vec<DictKey>, Raised> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            dict_key(v, i + 1).map_err(|f| Raised(format!("{}: {}", f.class, py::text(&f.message))))
        })
        .collect()
}

fn distinct(keys: &[DictKey]) -> usize {
    let mut seen: Vec<&DictKey> = vec![];
    for k in keys {
        if !seen.iter().any(|s| same_key(s, k)) {
            seen.push(k);
        }
    }
    seen.len()
}

/// `validate_card(card)`.
pub fn validate_card(card: &PyValue) -> Result<Vec<String>, Raised> {
    let mut v: Vec<String> = vec![];
    let PyValue::Dict(card) = card.peel() else {
        return Ok(vec!["카드가 JSON 객체가 아니다".into()]);
    };
    if let Some(m) = no_floats(&PyValue::Dict(card.clone()), "$") {
        v.push(m);
    }
    let all: Vec<&str> = CARD_REQUIRED
        .iter()
        .chain(CARD_OPTIONAL.iter())
        .copied()
        .collect();
    let unknown = unknown_keys(card, &all);
    if !unknown.is_empty() {
        v.push(format!(
            "표준 밖 최상위 키 {} — 고유 항목은 capabilities.extensions[] 에만 둔다",
            keys_repr(&unknown)
        ));
    }
    for k in CARD_REQUIRED {
        if !card.contains_key(k) {
            v.push(format!("필수 필드 누락: {k}"));
        }
    }
    for k in ["name", "description", "version"] {
        if let Some(x) = card.get(k) {
            let bad = match x.peel() {
                PyValue::Str(s) => k != "description" && s.strip().is_empty(),
                _ => true,
            };
            if bad {
                v.push(format!("{k}: 비어 있지 않은 문자열이어야 한다"));
            }
        }
    }
    if let Some(interfaces) = card
        .get("supportedInterfaces")
        .filter(|x| !matches!(x.peel(), PyValue::None))
    {
        match interfaces.peel() {
            PyValue::List(l) if !l.is_empty() => {
                for (i, iface) in l.iter().enumerate() {
                    let PyValue::Dict(iface) = iface.peel() else {
                        v.push(format!("supportedInterfaces[{i}]: 객체가 아니다"));
                        continue;
                    };
                    for k in INTERFACE_REQUIRED {
                        let ok = as_str(iface.get(k)).is_some_and(|s| !s.is_empty());
                        if !ok {
                            v.push(format!("supportedInterfaces[{i}].{k}: 필수 문자열"));
                        }
                    }
                    let bad = unknown_keys(
                        iface,
                        &["url", "protocolBinding", "protocolVersion", "tenant"],
                    );
                    if !bad.is_empty() {
                        v.push(format!(
                            "supportedInterfaces[{i}]: 표준 밖 키 {}",
                            keys_repr(&bad)
                        ));
                    }
                    if let Some(b) = as_str(iface.get("protocolBinding")) {
                        if !STANDARD_BINDINGS.iter().any(|s| b.equals(s))
                            && !is_uri(iface.get("protocolBinding"))?
                        {
                            v.push(format!("supportedInterfaces[{i}].protocolBinding={}: 표준 binding 또는 URI여야 한다", b.repr()));
                        }
                    }
                }
            }
            _ => v.push("supportedInterfaces: 비어 있지 않은 배열".into()),
        }
    }
    for k in ["defaultInputModes", "defaultOutputModes"] {
        if let Some(x) = card.get(k).filter(|x| !matches!(x.peel(), PyValue::None)) {
            let ok = match x.peel() {
                PyValue::List(l) => {
                    !l.is_empty()
                        && l.iter()
                            .all(|i| matches!(i.peel(), PyValue::Str(s) if !s.is_empty()))
                }
                _ => false,
            };
            if !ok {
                v.push(format!("{k}: 비어 있지 않은 문자열 배열"));
            }
        }
    }
    if let Some(caps) = card
        .get("capabilities")
        .filter(|x| !matches!(x.peel(), PyValue::None))
    {
        match caps.peel() {
            PyValue::Dict(caps) => validate_capabilities(caps, &mut v)?,
            _ => v.push("capabilities: 객체여야 한다".into()),
        }
    }
    if let Some(skills) = card
        .get("skills")
        .filter(|x| !matches!(x.peel(), PyValue::None))
    {
        match skills.peel() {
            PyValue::List(l) if !l.is_empty() => {
                let mut ids = vec![];
                for (i, skill) in l.iter().enumerate() {
                    let PyValue::Dict(skill) = skill.peel() else {
                        v.push(format!("skills[{i}]: 객체가 아니다"));
                        continue;
                    };
                    for k in SKILL_REQUIRED {
                        if !skill.contains_key(k) {
                            v.push(format!("skills[{i}].{k}: 필수"));
                        }
                    }
                    let allowed: Vec<&str> = SKILL_REQUIRED
                        .iter()
                        .chain(SKILL_EXTRA.iter())
                        .copied()
                        .collect();
                    let bad = unknown_keys(skill, &allowed);
                    if !bad.is_empty() {
                        v.push(format!("skills[{i}]: 표준 밖 키 {}", keys_repr(&bad)));
                    }
                    if let Some(t) = skill.get("tags") {
                        if !matches!(t.peel(), PyValue::List(l) if !l.is_empty()) {
                            v.push(format!("skills[{i}].tags: 비어 있지 않은 배열"));
                        }
                    }
                    ids.push(skill.get("id").cloned().unwrap_or(PyValue::None));
                }
                let keys = hash_all(&ids)?;
                if distinct(&keys) != ids.len() {
                    v.push("skills[].id 중복".into());
                }
            }
            _ => v.push("skills: 비어 있지 않은 배열".into()),
        }
    }
    if let Some(p) = card
        .get("provider")
        .filter(|x| !matches!(x.peel(), PyValue::None))
    {
        let ok = match p.peel() {
            PyValue::Dict(d) => PROVIDER_REQUIRED.iter().all(|k| as_str(d.get(k)).is_some()),
            _ => false,
        };
        if !ok {
            v.push("provider: {url, organization} 둘 다 필수 문자열".into());
        }
    }
    Ok(v)
}

fn validate_capabilities(caps: &PyDict, v: &mut Vec<String>) -> Result<(), Raised> {
    let bad = unknown_keys(caps, &CAPABILITY_FIELDS);
    if !bad.is_empty() {
        v.push(format!("capabilities: 표준 밖 키 {}", keys_repr(&bad)));
    }
    for k in ["streaming", "pushNotifications", "extendedAgentCard"] {
        if let Some(x) = caps.get(k) {
            if !is_bool(x) {
                v.push(format!("capabilities.{k}: bool"));
            }
        }
    }
    let empty = vec![];
    let extensions: &Vec<PyValue> = match caps.get("extensions") {
        None => &empty,
        Some(x) => match x.peel() {
            PyValue::List(l) => l,
            _ => {
                v.push("capabilities.extensions: 배열".into());
                &empty
            }
        },
    };
    let mut role: Vec<&PyDict> = vec![];
    for (i, ext) in extensions.iter().enumerate() {
        let PyValue::Dict(ext) = ext.peel() else {
            v.push(format!("capabilities.extensions[{i}]: 객체가 아니다"));
            continue;
        };
        let bad = unknown_keys(ext, &EXTENSION_FIELDS);
        if !bad.is_empty() {
            v.push(format!(
                "capabilities.extensions[{i}]: 표준 밖 키 {}",
                keys_repr(&bad)
            ));
        }
        if !is_uri(ext.get("uri"))? {
            v.push(format!("capabilities.extensions[{i}].uri: URI여야 한다"));
        }
        if let Some(r) = ext.get("required") {
            if !is_bool(r) {
                v.push(format!("capabilities.extensions[{i}].required: bool"));
            }
        }
        if let Some(p) = ext.get("params") {
            if dict(Some(p)).is_none() {
                v.push(format!("capabilities.extensions[{i}].params: 객체"));
            }
        }
        if as_str(ext.get("uri")).is_some_and(|u| u.equals(NODE_ROLE_EXT_URI)) {
            role.push(ext);
        }
    }
    if role.len() != 1 {
        v.push(format!(
            "노드 역할 확장 {NODE_ROLE_EXT_URI} 는 정확히 1개여야 한다(현재 {})",
            role.len()
        ));
        return Ok(());
    }
    let ext = role[0];
    let empty_params = PyDict::new();
    // `extension.get("params") or {}`: a falsy value is an empty mapping; a truthy non-dict is used like the original.
    let pv = ext.get("params");
    let params: &PyDict = match pv.map(|x| x.peel()) {
        Some(PyValue::Dict(d)) => d,
        Some(x) if x.is_truthy() => {
            not_mapping(x, v)?;
            if !matches!(
                ext.get("required").map(|x| x.peel()),
                Some(PyValue::Bool(true))
            ) {
                v.push("node-role 확장은 required: true".into());
            }
            return Ok(());
        }
        _ => &empty_params,
    };
    let missing: Vec<String> = NODE_ROLE_PARAMS
        .iter()
        .filter(|k| !params.contains_key(k))
        .map(|k| PyStr::new(k).repr())
        .collect();
    let extra = unknown_keys(params, &NODE_ROLE_PARAMS);
    if !missing.is_empty() {
        v.push(format!(
            "node-role 확장 params 누락: [{}]",
            missing.join(", ")
        ));
    }
    if !extra.is_empty() {
        v.push(format!(
            "node-role 확장 params 표준 밖 키: {}",
            keys_repr(&extra)
        ));
    }
    if let Some(r) = params.get("rank") {
        if !matches!(r.peel(), PyValue::None | PyValue::Int(_) | PyValue::Bool(_)) {
            v.push("node-role.rank: 정수 또는 null(single)".into());
        }
    }
    if let Some(t) = params.get("tool_plane") {
        let ok = matches!(t.peel(), PyValue::List(l) if l.iter().all(|i| matches!(i.peel(), PyValue::Str(_))));
        if !ok {
            v.push("node-role.tool_plane: 문자열 배열".into());
        }
    }
    for k in [
        "topology",
        "sub_mode",
        "sub_mode_source",
        "rank_source",
        "identity_authority",
        "delivery_plane",
        "tool_plane_source",
    ] {
        if let Some(x) = params.get(k) {
            if !matches!(x.peel(), PyValue::Str(s) if !s.is_empty()) {
                v.push(format!("node-role.{k}: 비어 있지 않은 문자열"));
            }
        }
    }
    if !matches!(
        ext.get("required").map(|x| x.peel()),
        Some(PyValue::Bool(true))
    ) {
        v.push("node-role 확장은 required: true".into());
    }
    Ok(())
}

/// The original with a truthy non-dict `params`: membership/iteration of a list or str, then the exception its
/// `params["rank"]`-style subscripts (or `in` on a non-container) raise.
#[allow(clippy::type_complexity)]
fn not_mapping(x: &PyValue, v: &mut Vec<String>) -> Result<(), Raised> {
    let (members, items): (Box<dyn Fn(&str) -> bool>, Vec<PyStr>) =
        match x {
            PyValue::List(l) => {
                let mut items = vec![];
                for i in l {
                    match i.peel() {
                        PyValue::Str(s) => items.push(s.clone()),
                        _ => return Err(Raised(
                            "TypeError: '<' not supported between instances (mixed params list)"
                                .into(),
                        )),
                    }
                }
                let owned = items.clone();
                (
                    Box::new(move |k: &str| owned.iter().any(|s| s.equals(k))),
                    items,
                )
            }
            PyValue::Str(s) => {
                let t = py::text(s);
                let chars = s
                    .code_points()
                    .iter()
                    .map(|&p| PyStr::from_code_points(vec![p]))
                    .collect();
                (Box::new(move |k: &str| t.contains(k)), chars)
            }
            other => {
                return Err(Raised(format!(
                    "TypeError: argument of type '{}' is not iterable",
                    other.type_name()
                )))
            }
        };
    let missing: Vec<String> = NODE_ROLE_PARAMS
        .iter()
        .filter(|k| !members(k))
        .map(|k| PyStr::new(k).repr())
        .collect();
    let extra: Vec<PyStr> = items
        .into_iter()
        .filter(|s| !NODE_ROLE_PARAMS.iter().any(|k| s.equals(k)))
        .collect();
    if !missing.is_empty() {
        v.push(format!(
            "node-role 확장 params 누락: [{}]",
            missing.join(", ")
        ));
    }
    if !extra.is_empty() {
        v.push(format!(
            "node-role 확장 params 표준 밖 키: {}",
            keys_repr(&extra)
        ));
    }
    let subscript = match x {
        PyValue::Str(_) => "TypeError: string indices must be integers, not 'str'",
        _ => "TypeError: list indices must be integers or slices, not str",
    };
    for k in [
        "rank",
        "tool_plane",
        "topology",
        "sub_mode",
        "sub_mode_source",
        "rank_source",
        "identity_authority",
        "delivery_plane",
        "tool_plane_source",
    ] {
        if members(k) {
            return Err(Raised(subscript.into()));
        }
    }
    Ok(())
}

/// `require_skills(card)`.
pub fn require_skills(card: &PyValue) -> Result<Vec<String>, Raised> {
    let PyValue::Dict(card) = card.peel() else {
        return Err(Raised(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            card.type_name()
        )));
    };
    let skills = card.get("skills").cloned().unwrap_or(PyValue::None);
    let items: Vec<PyValue> = if !skills.is_truthy() {
        vec![]
    } else {
        match skills.peel() {
            PyValue::List(l) => l.clone(),
            PyValue::Dict(d) => d.iter().map(|(k, _)| PyValue::Str(k.clone())).collect(),
            PyValue::Str(s) => s
                .code_points()
                .iter()
                .map(|&p| PyValue::Str(PyStr::from_code_points(vec![p])))
                .collect(),
            other => {
                return Err(Raised(format!(
                    "TypeError: '{}' object is not iterable",
                    other.type_name()
                )))
            }
        }
    };
    let ids: Vec<PyValue> = items
        .iter()
        .filter_map(|s| dict(Some(s)).map(|d| d.get("id").cloned().unwrap_or(PyValue::None)))
        .collect();
    let have = hash_all(&ids)?;
    let mut out = vec![];
    for k in EXPECTED_SKILL_IDS {
        let key = dict_key(&PyValue::str(k), 0).expect("str is hashable");
        if !have.iter().any(|h| same_key(h, &key)) {
            out.push(format!("skill 누락: {k}"));
        }
    }
    Ok(out)
}

pub fn fixture_card() -> PyValue {
    let text = format!(
        r#"{{"name": "fixture", "description": "fixture", "version": "2.0.0",
 "supportedInterfaces": [{{"url": "ssh://sub", "protocolBinding": "urn:test:ssh", "protocolVersion": "1.0"}}],
 "capabilities": {{"streaming": false, "extensions": [{{"uri": "{NODE_ROLE_EXT_URI}", "required": true,
   "params": {{"topology": "single", "sub_mode": "a2a-agent", "sub_mode_source": "derived-from-topology", "rank": null,
     "rank_source": "not-applicable:single", "identity_authority": "ssh-public-key", "delivery_plane": "dormant",
     "tool_plane": [], "tool_plane_source": "fixture"}}}}]}},
 "defaultInputModes": ["text/plain"], "defaultOutputModes": ["application/json"],
 "skills": [{}]}}"#,
        EXPECTED_SKILL_IDS
            .iter()
            .map(|k| format!(r#"{{"id": "{k}", "name": "{k}", "description": "{k}", "tags": ["{k}"], "inputModes": ["text/plain"], "outputModes": ["application/json"]}}"#))
            .collect::<Vec<_>>()
            .join(", ")
    );
    pyjson::loads(&text).expect("fixture card is JSON")
}

fn with<F: FnOnce(&mut PyDict)>(card: &PyValue, f: F) -> PyValue {
    let PyValue::Dict(mut d) = card.clone() else {
        unreachable!()
    };
    f(&mut d);
    PyValue::Dict(d)
}

fn self_test() -> i32 {
    let mut ok = true;
    let mut check = |name: &str, cond: bool| {
        println!("  [{}] {}", if cond { "PASS" } else { "FAIL" }, name);
        ok &= cond;
    };
    let vc = |c: &PyValue| validate_card(c).unwrap_or_default();
    let any = |c: &PyValue, needle: &str| vc(c).iter().any(|m| m.contains(needle));
    let card = fixture_card();
    check(
        "metadata card contract",
        vc(&card).is_empty() && require_skills(&card).is_ok_and(|r| r.is_empty()),
    );
    let bad = with(&card, |d| {
        d.set(
            "node_identity",
            pyjson::loads(r#"{"role": "sub"}"#).unwrap(),
        )
    });
    check(
        "unknown top-level identity field rejected",
        any(&bad, "표준 밖 최상위"),
    );
    let bad =
        pyjson::loads(&py::dumps_unicode(&card).replace("urn:test:ssh", "ssh-claude-p")).unwrap();
    check("non-URI custom binding rejected", any(&bad, "URI"));
    let bad =
        pyjson::loads(&py::dumps_unicode(&card).replace(r#""tool_plane": [], "#, "")).unwrap();
    check("missing role capability rejected", any(&bad, "params 누락"));
    let bad = with(&card, |d| d.set("version", PyValue::Float(1.5)));
    check("float rejected", any(&bad, "float"));
    let bad = with(&card, |d| {
        if let Some(PyValue::List(l)) = d.get_mut("skills") {
            l.truncate(4);
        }
    });
    check(
        "missing skills reported",
        require_skills(&bad).unwrap_or_default()
            == vec![
                "skill 누락: bench".to_string(),
                "skill 누락: publish".to_string(),
            ],
    );
    let bad = with(&card, |d| {
        d.set(
            "signatures",
            pyjson::loads(r#"[{"protected": "x", "signature": "y"}]"#).unwrap(),
        )
    });
    check("credential field rejected", any(&bad, "표준 밖 최상위"));
    println!("self-test: {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        1
    }
}

static VALIDATE: Spec = Spec {
    prog: "agent_card_contract.py validate",
    usage: include_str!("../help/agent_card_contract.validate.usage"),
    help: include_str!("../help/agent_card_contract.validate.help"),
    args: &[
        HELP,
        store(&["--card"]).required(),
        flag(&["--require-skills"]),
    ],
    subs: None,
};

pub static SPEC: Spec = Spec {
    prog: "agent_card_contract.py",
    usage: include_str!("../help/agent_card_contract.usage"),
    help: include_str!("../help/agent_card_contract.help"),
    args: &[HELP, flag(&["--self-test"])],
    subs: Some(("cmd", &[("validate", &VALIDATE)])),
};

/// `json.load(open(path, encoding="utf-8"))`; `Err` = the traceback's last line.
pub fn load_card(path: &str) -> Result<PyValue, Raised> {
    let bytes = std::fs::read(path).map_err(|e| Raised(crate::pathfmt::os_error(&e, path)))?;
    let text =
        pyjson::decode_file_text(&bytes).map_err(|e| Raised(format!("UnicodeDecodeError: {e}")))?;
    pyjson::loads_text_objects(&text).map_err(|e| match e {
        pyjson::LoadError::Decode { .. } => Raised(format!("json.decoder.JSONDecodeError: {e}")),
        pyjson::LoadError::TooDeep { .. } => Raised(format!("RecursionError: {e}")),
        _ => Raised(format!("ValueError: {e}")),
    })
}

pub fn main(args: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if a.flag(1) {
        return self_test();
    }
    let Some((_, sub)) = &a.sub else {
        argp::print_help_stdout(&SPEC);
        return EXIT_USAGE;
    };
    let path = sub.text(1).unwrap_or("");
    let run = || -> Result<i32, Raised> {
        let card = load_card(path)?;
        let mut violations = validate_card(&card)?;
        violations.extend(require_skills(&card)?);
        if !violations.is_empty() {
            eprintln!("[agent-card] 계약 위반:\n  - {}", violations.join("\n  - "));
            return Ok(EXIT_CONTRACT);
        }
        let PyValue::Dict(d) = card.peel() else {
            unreachable!()
        };
        let skills = match d.get("skills").map(|x| x.peel()) {
            Some(PyValue::List(l)) => l.len(),
            _ => 0,
        };
        println!(
            "[agent-card] OK: metadata fields {} · skills {} · unsigned",
            d.len(),
            skills
        );
        Ok(EXIT_OK)
    };
    match run() {
        Ok(c) => c,
        Err(r) => crate::raise::exit(&r),
    }
}

/// Python key equality of two hashable values (`1`, `1.0` and `True` are one key; a NaN only equals the same object).
fn same_key(a: &DictKey, b: &DictKey) -> bool {
    match (a, b) {
        (DictKey::Str(x), DictKey::Str(y)) => x == y,
        (DictKey::Other(x), DictKey::Other(y)) => x.identity == y.identity,
        _ => false,
    }
}
