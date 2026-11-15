//! `render_native_triplet.py`: derives the native (no Docker) triplet of a multi-node cell from its Docker triplet, deterministically.
//! Only the container→host path mapping, the removal of the Docker-plane env keys and the `CONFIG_FILE` / `.sh` rewrite change; every serving knob
//! is carried byte for byte. Exit status: 0 done, 1 drift (`--check`), 2 usage, 3 render refused (fail-closed).
//!
//! The Docker-plane key set is `image_identity ∪ build_tuning` of the compose references (the `slave_forward.candidates` rule: only the compose
//! structure matters, so the Band2 renderer the original consults for the `cluster` / `serve_env` split is not needed) plus the name rule
//! `CONTAINER_NAME` · `*_CONTAINER_NAME` · `COMPOSE_*`. The one check of the original self-test that runs the Python hint publisher
//! (`artifacts.plane_of`) is not part of this program.

use crate::gate_cli::{self, Values};
use recipe_runtime::cliparse::{Kind, Opt, Spec};
use ring::digest::{digest, SHA256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const PROGRAM: &str = "render_native_triplet.py";
const TOOL: &str = "render_native_triplet.py";
const SLAVE_SERVICE: &str = "vllm-slave-serve";
const NEVER_FORWARD: [&str; 1] = ["CONFIG_FILE"];

/// A refused render (`RenderError`, status 3) or a compose-shape violation (`ForwardError`, an unhandled exception: status 1).
pub enum Failure {
    Render(String),
    Forward(String),
}

fn refuse<T>(text: String) -> Result<T, Failure> {
    Err(Failure::Render(text))
}

// ── text helpers with the semantics of the Python original ─────────────────────────────────────────

/// `str.splitlines()`.
fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let sep = matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if sep {
            out.push(&text[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = chars.peek() {
                    chars.next();
                    next = j + 1;
                }
            }
            start = next;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Text-mode reading: invalid bytes replaced, `\r\n` and `\r` become `\n`.
fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Some(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn sha256_text(text: &str) -> String {
    digest(&SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn skip_ws(text: &str, mut at: usize) -> usize {
    while let Some(c) = text[at..].chars().next() {
        if !c.is_whitespace() {
            break;
        }
        at += c.len_utf8();
    }
    at
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut out = vec![0];
    for (i, c) in text.char_indices() {
        if c == '\n' {
            out.push(i + 1);
        }
    }
    out
}

fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |n| at + n)
}

/// `rest` (the remainder of a line) matches `\s*(?:#.*)?$`.
fn comment_or_blank(rest: &str) -> bool {
    let t = rest.trim_start();
    t.is_empty() || t.starts_with('#')
}

/// `re.search(r"(?m)^KEY\s*:\s*(.+?)\s*(?:#.*)?$", text)` → the stripped, unquoted group.
fn yaml_scalar(text: &str, key: &str) -> Option<String> {
    for s in line_starts(text) {
        let Some(rest) = text[s..].strip_prefix(key) else { continue };
        let at = skip_ws(text, text.len() - rest.len());
        if !text[at..].starts_with(':') {
            continue;
        }
        let from = at + 1;
        let value_at = skip_ws(text, from);
        if value_at >= text.len() {
            // the greedy whitespace backtracks: a blank (non-newline) character is itself captured
            if text[from..].chars().any(|c| c != '\n' && c.is_whitespace()) {
                return Some(String::new());
            }
            continue;
        }
        let end = line_end(text, value_at);
        let line = &text[value_at..end];
        let mut cut = None;
        for (k, c) in line.char_indices() {
            let after = k + c.len_utf8();
            if comment_or_blank(&line[after..]) {
                cut = Some(after);
                break;
            }
        }
        let captured = &line[..cut.unwrap_or(line.len())];
        return Some(captured.trim().trim_matches(|c| c == '\'' || c == '"').to_string());
    }
    None
}

// ── env file · compose references ───────────────────────────────────────────────────────────────────

/// `slave_forward.read_env_first`: first match per key, first `=` splits, the raw value kept.
pub(crate) fn read_env_first(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(bytes) = std::fs::read(path).ok() else { return out };
    for raw in splitlines(&String::from_utf8_lossy(&bytes)) {
        if let Some((k, v)) = env_line(raw) {
            out.entry(k.to_string()).or_insert_with(|| v.to_string());
        }
    }
    out
}

fn env_line(raw: &str) -> Option<(&str, &str)> {
    let eq = raw.find('=')?;
    let key = &raw[..eq];
    let mut chars = key.chars();
    let first = chars.next()?;
    if !is_ident_start(first) || !chars.all(is_ident) {
        return None;
    }
    Some((key, &raw[eq + 1..]))
}

fn strip_comment(line: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    for ch in line.chars() {
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            out.push(ch);
        } else if ch == '#' && (prev == ' ' || prev == '\t') {
            break;
        } else {
            out.push(ch);
        }
        prev = ch;
    }
    out.trim_end().to_string()
}

struct Node {
    key: Option<String>,
    value: String,
    children: Vec<usize>,
    indent: i64,
    item: bool,
}

fn parse_node(body: &str) -> Option<(String, String)> {
    let key_len = body
        .char_indices()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '<' | '-')))
        .map_or(body.len(), |(i, _)| i);
    if key_len == 0 {
        return None;
    }
    let rest = body[key_len..].trim_start();
    let rest = rest.strip_prefix(':')?;
    if rest.is_empty() {
        return Some((body[..key_len].to_string(), String::new()));
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((body[..key_len].to_string(), rest.trim_start().to_string()))
}

fn tree(text: &str) -> Vec<Node> {
    let mut nodes = vec![Node { key: None, value: String::new(), children: vec![], indent: -1, item: false }];
    let mut stack = vec![0usize];
    for raw in splitlines(text) {
        let line = strip_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        let indent = (line.chars().count() - line.trim_start_matches(' ').chars().count()) as i64;
        let mut body = line.trim().to_string();
        let item = body == "-" || body.starts_with("- ");
        if item {
            body = body[1..].trim().to_string();
        }
        let parsed = if body.is_empty() { None } else { parse_node(&body) };
        let (key, value) = match parsed {
            Some((k, v)) => (Some(k), v.trim().to_string()),
            None => (None, body.trim().to_string()),
        };
        while nodes[*stack.last().unwrap()].indent >= indent {
            stack.pop();
        }
        let id = nodes.len();
        nodes.push(Node { key, value, children: vec![], indent, item });
        let parent = *stack.last().unwrap();
        nodes[parent].children.push(id);
        stack.push(id);
    }
    nodes
}

fn child(nodes: &[Node], at: Option<usize>, key: &str) -> Option<usize> {
    let at = at?;
    nodes[at].children.iter().copied().find(|&c| nodes[c].key.as_deref() == Some(key) && !nodes[c].item)
}

/// `_REF.finditer`: `${NAME` or `$NAME`, not preceded by `$`; names kept once, in order.
fn refs(all: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in all {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && (i == 0 || chars[i - 1] != '$') {
                let mut j = i + 1;
                if j < chars.len() && chars[j] == '{' {
                    j += 1;
                }
                if j < chars.len() && is_ident_start(chars[j]) {
                    let s = j;
                    while j < chars.len() && is_ident(chars[j]) {
                        j += 1;
                    }
                    let name: String = chars[s..j].iter().collect();
                    if !out.contains(&name) {
                        out.push(name);
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }
    out
}

fn value_of(nodes: &[Node], at: Option<usize>) -> String {
    at.map(|a| nodes[a].value.clone()).unwrap_or_default()
}

/// The Docker-plane keys of the compose file: `image_identity ∪ build_tuning` of `slave_forward.candidates` (CONFIG_FILE never counted).
fn compose_docker_keys(compose: &Path) -> Result<Vec<String>, Failure> {
    let fwd = |code: &str, msg: String| Failure::Forward(format!("{code}: {msg}"));
    let Some(text) = read_text(compose).filter(|_| compose.is_file()) else {
        return Err(fwd("SF_COMPOSE_ABSENT", format!("compose 파일이 없다: {}", compose.display())));
    };
    let nodes = tree(&text);
    let common = nodes[0].children.iter().copied().find(|&c| {
        nodes[c].key.as_deref().is_some_and(|k| k.starts_with("x-")) && nodes[c].value.starts_with('&')
    });
    let Some(common) = common else {
        return Err(fwd("SF_COMPOSE_SHAPE", "공통 앵커 블록(x-…: &name)을 찾지 못했다".into()));
    };
    let anchor = nodes[common].value[1..].trim().to_string();
    let services = child(&nodes, Some(0), "services");
    let Some(slave) = child(&nodes, services, SLAVE_SERVICE) else {
        return Err(fwd("SF_COMPOSE_SHAPE", format!("services.{SLAVE_SERVICE} 가 없다")));
    };
    let merge = child(&nodes, Some(slave), "<<");
    if merge.map(|m| nodes[m].value.clone()) != Some(format!("*{anchor}")) {
        return Err(fwd("SF_COMPOSE_SHAPE", format!("{SLAVE_SERVICE} 가 공통 앵커 *{anchor} 를 병합하지 않는다")));
    }
    let build = child(&nodes, Some(common), "build");
    let image = child(&nodes, Some(common), "image");
    let dockerfile = child(&nodes, build, "dockerfile");
    let selectors = refs(&[value_of(&nodes, image), value_of(&nodes, dockerfile)]);
    let args_node = child(&nodes, build, "args");
    let arg_values: Vec<String> = args_node
        .map(|a| nodes[a].children.iter().map(|&c| nodes[c].value.clone()).collect())
        .unwrap_or_default();
    let build_args = refs(&arg_values);
    let mut keys: Vec<String> = Vec::new();
    for key in selectors.into_iter().chain(build_args) {
        if NEVER_FORWARD.contains(&key.as_str()) || keys.contains(&key) {
            continue;
        }
        keys.push(key);
    }
    Ok(keys)
}

// ── manifest · compose mount readers (shared rules of check_smoke_model) ───────────────────────────

/// `read_app_models_host_token`: the first `- <token>:<root>[:ro] [# …]` line of the compose file.
fn read_app_models_host_token(compose: &Path, root: &str) -> Option<String> {
    let text = read_text(compose)?;
    for line in text.split_inclusive('\n') {
        for (p, _) in line.match_indices('-') {
            let rest = &line[skip_ws(line, p + 1)..];
            let ends: Vec<usize> = rest.char_indices().map(|(i, _)| i).chain([rest.len()]).collect();
            for &k in ends.iter().rev() {
                if k == 0 || rest[..k].chars().any(char::is_whitespace) {
                    continue;
                }
                let Some(tail) = rest[k..].strip_prefix(':').and_then(|t| t.strip_prefix(root)) else { continue };
                if tail_ok(tail) {
                    return Some(rest[..k].to_string());
                }
            }
        }
    }
    None
}

fn tail_ok(tail: &str) -> bool {
    if comment_or_blank(tail) {
        return true;
    }
    if let Some(after) = tail.strip_prefix(':') {
        let n = after.chars().take_while(|c| c.is_ascii_lowercase()).count();
        return (1..=n).any(|m| comment_or_blank(&after[m..]));
    }
    false
}

/// `read_manifest_field`: the first `\s*FIELD\s*:\s*(\S+)` line of `<base>/manifest.yaml`, unquoted.
fn read_manifest_field(base: &Path, field: &str) -> Option<String> {
    let path = base.join("manifest.yaml");
    if !path.is_file() {
        return None;
    }
    let text = read_text(&path)?;
    for line in text.split_inclusive('\n') {
        let Some(rest) = line.trim_start().strip_prefix(field) else { continue };
        let Some(rest) = rest.trim_start().strip_prefix(':') else { continue };
        let rest = rest.trim_start();
        let token: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
        if !token.is_empty() {
            return Some(token.trim_matches(|c| c == '\'' || c == '"').to_string());
        }
    }
    None
}

// ── rendering ───────────────────────────────────────────────────────────────────────────────────────

pub(crate) fn triplet_rel(topology: &str, cell: &str) -> [(&'static str, String); 3] {
    [
        ("yaml", format!("output/{topology}/configs/{cell}.yaml")),
        ("sh", format!("output/{topology}/configs/{cell}.sh")),
        ("env", format!("output/{topology}/envs/.env.{cell}")),
    ]
}

fn safe_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name.chars().count() <= 96
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// `${VAR}` or `${VAR:-default}` (default without `}`), the whole token.
fn mount_var(token: &str) -> Option<String> {
    let inner = token.strip_prefix("${")?.strip_suffix('}')?;
    let name_len = inner.chars().take_while(|&c| is_word(c)).count();
    if name_len == 0 {
        return None;
    }
    let name: String = inner.chars().take(name_len).collect();
    let tail = &inner[name.len()..];
    if tail.is_empty() || (tail.starts_with(":-") && !tail.contains('}')) {
        Some(name)
    } else {
        None
    }
}

fn map_container_path(compose: &Path, base: &Path, value: &str) -> Result<Option<(String, String)>, Failure> {
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() < 3 || !value.starts_with('/') {
        return Ok(None);
    }
    let root = parts[..3].join("/");
    let Some(token) = read_app_models_host_token(compose, &root).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let Some(var) = mount_var(&token) else {
        return refuse(format!("{root} 의 compose 마운트 원천이 VAR 가 아니다({}) — manifest 로 사상할 수 없다", py_repr(&token)));
    };
    let field = var.to_lowercase();
    let Some(host_root) = read_manifest_field(base, &field).filter(|h| !h.is_empty()) else {
        return refuse(format!("manifest.{field} 가 없다 — {root} 를 호스트로 사상할 수 없다(폴백 ✗)"));
    };
    Ok(Some((format!("{}{}", host_root.trim_end_matches('/'), &value[root.len()..]), format!("{root} → manifest.{field}"))))
}

fn py_repr(text: &str) -> String {
    if text.contains('\'') && !text.contains('"') {
        format!("\"{text}\"")
    } else {
        format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

fn py_repr_opt(text: &Option<String>) -> String {
    text.as_ref().map_or("None".to_string(), |t| py_repr(t))
}

/// `(?m)^\s*distributed-executor-backend\s*:\s*ray\b`.
fn has_ray_backend(text: &str) -> bool {
    const KEY: &str = "distributed-executor-backend";
    for s in line_starts(text) {
        let Some(rest) = text[skip_ws(text, s)..].strip_prefix(KEY) else { continue };
        let Some(rest) = rest.trim_start().strip_prefix(':') else { continue };
        if let Some(after) = rest.trim_start().strip_prefix("ray") {
            if !after.chars().next().is_some_and(is_word) {
                return true;
            }
        }
    }
    false
}

/// `(?m)^(model\s*:\s*)(\S+)(.*)$` → (start, end, head, token, tail).
fn find_model(text: &str) -> Option<(usize, usize, String, String, String)> {
    for s in line_starts(text) {
        let Some(rest) = text[s..].strip_prefix("model") else { continue };
        let colon = skip_ws(text, text.len() - rest.len());
        if !text[colon..].starts_with(':') {
            continue;
        }
        let tok_at = skip_ws(text, colon + 1);
        let tok_end = tok_at + text[tok_at..].chars().take_while(|c| !c.is_whitespace()).map(char::len_utf8).sum::<usize>();
        if tok_end == tok_at {
            continue;
        }
        let end = line_end(text, tok_end);
        return Some((s, end, text[s..tok_at].to_string(), text[tok_at..tok_end].to_string(), text[tok_end..end].to_string()));
    }
    None
}

pub type Rendered = BTreeMap<String, String>;

pub fn render(repo: &Path, topology: &str, source_cell: &str, cell: &str) -> Result<Rendered, Failure> {
    if !safe_name(cell) || !safe_name(source_cell) {
        return refuse("셀 이름이 안전 문자 밖이다".into());
    }
    if cell == source_cell {
        return refuse("native 셀 이름은 원본 Docker 셀과 달라야 한다".into());
    }
    let base = repo.join("output").join(topology);
    let src = triplet_rel(topology, source_cell);
    for (k, rel) in &src {
        if !repo.join(rel).is_file() {
            return refuse(format!("원본 Docker 트리플렛 {k} 부재: {rel}"));
        }
    }
    let compose = base.join("docker-compose.yaml");
    if !compose.is_file() {
        return refuse(format!(
            "{} 부재 — 마운트 사상·평면 키 파생의 원천이 없다",
            compose.strip_prefix(repo).unwrap_or(&compose).display()
        ));
    }
    let patch = base.join("configs").join(format!("{source_cell}_patch.py"));
    if patch.exists() {
        return refuse(format!(
            "원본 셀에 런타임 패치({})가 있다 — native 평면 arming 은 미지원(fail-closed · policy:RUNTIME_PATCH_NO_CARRY_FORWARD)",
            patch.file_name().and_then(|n| n.to_str()).unwrap_or("")
        ));
    }
    let rel = |k: &str| src.iter().find(|(n, _)| *n == k).map(|(_, r)| r.clone()).unwrap();
    let y_src = read_text(&repo.join(rel("yaml"))).unwrap_or_default();
    let e_src = read_text(&repo.join(rel("env"))).unwrap_or_default();
    let env = read_env_first(&repo.join(rel("env")));
    let env_get = |k: &str| env.get(k).cloned();
    if env_get("TIKTOKEN_ENABLED").unwrap_or_else(|| "false".into()).trim().to_lowercase() == "true" {
        return refuse("TIKTOKEN_ENABLED=true — native 평면 tiktoken 인코딩 배선은 미지원(fail-closed)".into());
    }
    if !has_ray_backend(&y_src) {
        return refuse("원본 yaml 에 distributed-executor-backend: ray 가 없다(멀티 TP 필수 노브 · serve_runner 게이트와 같다)".into());
    }
    let y_port = yaml_scalar(&y_src, "port");
    let e_port = env_get("SERVING_PORT").unwrap_or_default().trim().to_string();
    if y_port.as_deref().map_or(true, str::is_empty) || y_port.as_deref() != Some(e_port.as_str()) {
        return refuse(format!("yaml port={} ≠ env SERVING_PORT={} — native 는 포트 사상이 없으므로 같아야 한다", py_repr_opt(&y_port), py_repr(&e_port)));
    }
    for need in ["SERVING_MODEL_NAME", "SERVING_PORT", "CONFIG_FILE"] {
        if env_get(need).unwrap_or_default().trim().is_empty() {
            return refuse(format!("원본 env 에 {need} 가 없다"));
        }
    }

    // ① yaml — the `model:` line only
    let Some((start, end, head, token, tail)) = find_model(&y_src) else {
        return refuse("원본 yaml 에 model: 이 없다".into());
    };
    let unquoted = token.trim_matches(|c| c == '\'' || c == '"').to_string();
    let Some((model_host, model_src)) = map_container_path(&compose, &base, &unquoted)? else {
        return refuse(format!("model: {token} 가 compose 마운트 대상이 아니다 — 호스트 경로로 사상할 수 없다"));
    };
    let y_body = format!("{}{}{}{}{}", &y_src[..start], head, model_host, tail, &y_src[end..]);
    let prov = [
        format!("# ⚙ 렌더 산출물 — {TOOL} 가 Docker 트리플렛 {source_cell} 에서 결정론 파생(손저작 ✗ · plan_26092311 N-D3)."),
        format!("#   원본 {} sha256={} · 경로 사상 model: {model_src}", rel("yaml"), &sha256_text(&y_src)[..16]),
        "#   서빙 노브는 원본과 바이트 동일하다 — 고치려면 원본 Docker 트리플렛을 고치고 다시 렌더한다.".to_string(),
    ];
    let yaml_out = format!("# 셀 {cell} — native(Docker 없음) 트리플렛 yaml\n{}\n{y_body}", prov.join("\n"));

    // ② env
    let mut drop = compose_docker_keys(&compose)?;
    for k in env.keys() {
        if k == "CONTAINER_NAME" || k.ends_with("_CONTAINER_NAME") || k.starts_with("COMPOSE_") {
            drop.push(k.clone());
        }
    }
    let rules = ["slave_forward.candidates(compose): image_identity ∪ build_tuning", "이름 규칙: CONTAINER_NAME · *_CONTAINER_NAME · COMPOSE_*"];
    let (mut out_lines, mut dropped, mut remapped): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec![], vec![]);
    for raw in splitlines(&e_src) {
        let Some((key, val)) = env_line(raw) else {
            out_lines.push(raw.to_string());
            continue;
        };
        if drop.iter().any(|d| d == key) {
            dropped.push(key.to_string());
            continue;
        }
        let mut val = val.to_string();
        if key == "CONFIG_FILE" {
            val = cell.to_string();
        } else if val.starts_with('/') {
            let mp = map_container_path(&compose, &base, &val)?;
            if mp.is_none() && val.starts_with("/app/") {
                return refuse(format!("{key}={val} 는 컨테이너 경로인데 compose 마운트가 없다 — 호스트로 사상할 수 없다"));
            }
            if let Some((host, why)) = mp {
                val = host;
                remapped.push(format!("{key}({why})"));
            }
        }
        out_lines.push(format!("{key}={val}"));
    }
    dropped.sort();
    let env_head = [
        format!("# vLLM native 서빙 환경 — {cell}"),
        format!("# ⚙ 렌더 산출물 — {TOOL} 가 Docker 셀 env {} (sha256={}) 에서 파생.", rel("env"), &sha256_text(&e_src)[..16]),
        format!(
            "#   제거한 Docker 평면 키(값 없음 · 규칙: {}): {}",
            rules.join(" / "),
            if dropped.is_empty() { "없음".to_string() } else { dropped.join(" · ") }
        ),
        format!(
            "#   컨테이너 경로 → 호스트 사상: {} · CONFIG_FILE → {cell}",
            if remapped.is_empty() { "없음".to_string() } else { remapped.join(" · ") }
        ),
        "#   실행 평면 = native(이미지 선택자 부재) — 발행기 artifacts._plane 이 이 파일로 평면을 판정한다.".to_string(),
    ];
    let env_all: Vec<String> = env_head.into_iter().chain(out_lines).collect();
    let env_out = format!("{}\n", env_all.join("\n").trim_end_matches('\n'));

    // ③ sh
    let sh_src = read_text(&repo.join(rel("sh"))).unwrap_or_default();
    let sh_lines = [
        "#!/bin/bash".to_string(),
        format!("# {cell} native 서빙 러너 — {TOOL} 가 Docker 트리플렛 {source_cell} 에서 렌더(손저작 ✗)."),
        format!("#   원본 {} sha256={} 의 `vllm serve --config … --served-model-name …` 를", rel("sh"), &sha256_text(&sh_src)[..16]),
        "#   호스트 venv 로 옮긴 것이다. 컨테이너 전용 arming(arm_patch.sh·tiktoken /encodings)은 렌더가 fail-closed 로 막는다.".to_string(),
        "#   기동 주체는 native_multinode_serve.py up 이다 — 셀 env·캐시 격리 env·LD_LIBRARY_PATH·NATIVE_VENV 를 주입해 이 파일을 실행한다.".to_string(),
        "set -euo pipefail".to_string(),
        ": \"${NATIVE_VENV:?NATIVE_VENV(run root 의 venv) 미주입 — native_multinode_serve.py up 이 주입한다}\"".to_string(),
        ": \"${CONFIG_FILE:?CONFIG_FILE 미주입(셀 env)}\"".to_string(),
        ": \"${SERVING_MODEL_NAME:?SERVING_MODEL_NAME 미주입(셀 env)}\"".to_string(),
        "HERE=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")\" && pwd)\"".to_string(),
        "exec \"${NATIVE_VENV}/bin/vllm\" serve --config \"${HERE}/${CONFIG_FILE}.yaml\" \\".to_string(),
        "    --served-model-name \"${SERVING_MODEL_NAME}\"".to_string(),
        String::new(),
    ];
    let sh_out = sh_lines.join("\n");
    let dst = triplet_rel(topology, cell);
    let mut out = Rendered::new();
    for (k, r) in dst {
        out.insert(r, match k {
            "yaml" => yaml_out.clone(),
            "sh" => sh_out.clone(),
            _ => env_out.clone(),
        });
    }
    Ok(out)
}

/// Drift list (empty = the disk equals the render).
pub fn check(repo: &Path, rendered: &Rendered) -> Vec<String> {
    let mut bad = Vec::new();
    for (rel, text) in rendered {
        let p = repo.join(rel);
        let is_link = std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if is_link || !p.is_file() {
            bad.push(format!("{rel}: 부재"));
        } else if read_text(&p).as_deref() != Some(text.as_str()) {
            bad.push(format!("{rel}: 렌더와 다르다(손편집 또는 원본 변경)"));
        }
    }
    bad
}

pub fn apply(repo: &Path, rendered: &Rendered) -> Result<Vec<String>, Failure> {
    use std::os::unix::fs::PermissionsExt;
    let mut wrote = Vec::new();
    for (rel, text) in rendered {
        let p = repo.join(rel);
        if std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            return refuse(format!("{rel} 가 symlink 다 — 쓰지 않는다"));
        }
        if p.is_file() && read_text(&p).as_deref() == Some(text.as_str()) {
            continue;
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Failure::Forward(e.to_string()))?;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let tmp = p.with_file_name(format!("{name}.render-tmp"));
        std::fs::write(&tmp, text.as_bytes()).map_err(|e| Failure::Forward(e.to_string()))?;
        let mode = if rel.ends_with(".sh") { 0o755 } else { 0o644 };
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode)).map_err(|e| Failure::Forward(e.to_string()))?;
        std::fs::rename(&tmp, &p).map_err(|e| Failure::Forward(e.to_string()))?;
        wrote.push(rel.clone());
    }
    Ok(wrote)
}

// ── command line ────────────────────────────────────────────────────────────────────────────────────

fn opt(flags: &'static [&'static str], dest: &'static str, kind: Kind, group: Option<usize>) -> Opt {
    Opt { flags, dest, kind, help: None, required: false, group }
}

pub fn spec() -> Spec {
    Spec {
        prog: PROGRAM,
        description: "render_native_triplet.py — Docker 트리플렛에서 native(Docker 없음) 트리플렛을 **결정론 파생**한다.",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(&["--self-test"], "self_test", Kind::Flag, None),
            opt(&["--source-cell"], "source_cell", Kind::Text, None),
            opt(&["--cell"], "cell", Kind::Text, None),
            opt(&["--topology"], "topology", Kind::Choice(&["multi"]), None),
            opt(&["--repo"], "repo", Kind::Text, None),
            opt(&["--apply"], "apply", Kind::Flag, Some(0)),
            opt(&["--check"], "check", Kind::Flag, Some(0)),
        ],
        group_required: false,
        positional: None,
    }
}

pub fn run(script: Option<&agent_runtime_support::pystr::PyStr>, args: &[agent_runtime_support::pystr::PyStr]) -> i32 {
    let values: Values = match gate_cli::parse_args(&spec(), args) {
        Ok(v) => v,
        Err(status) => return status,
    };
    if gate_cli::flag(&values, "self_test") {
        return self_test();
    }
    let (Some(source), Some(cell)) = (gate_cli::text(&values, "source_cell"), gate_cli::text(&values, "cell")) else {
        gate_cli::write_err("사용: --source-cell <docker 셀> --cell <native 셀> [--apply|--check]\n");
        return 2;
    };
    let topology = gate_cli::text(&values, "topology").unwrap_or_else(|| "multi".into());
    let repo: PathBuf = match gate_cli::text(&values, "repo") {
        Some(r) => std::fs::canonicalize(&r).unwrap_or_else(|_| PathBuf::from(r)),
        None => {
            // the scripts directory sits at <repo>/.claude/skills/upstream-version-watch/scripts
            let scripts = gate_cli::scripts_dir(script, "render_native_triplet");
            scripts.ancestors().nth(4).map(Path::to_path_buf).unwrap_or(scripts)
        }
    };
    match run_render(&repo, &topology, &source, &cell, &values) {
        Ok(status) => status,
        Err(Failure::Render(text)) => {
            gate_cli::write_err(&format!("[render-native] FAIL: {text}\n"));
            3
        }
        Err(Failure::Forward(text)) => {
            gate_cli::write_err(&format!("{text}\n"));
            1
        }
    }
}

fn run_render(repo: &Path, topology: &str, source: &str, cell: &str, values: &Values) -> Result<i32, Failure> {
    let out = render(repo, topology, source, cell)?;
    if gate_cli::flag(values, "check") {
        let drift = check(repo, &out);
        for line in &drift {
            gate_cli::write_out(&format!("[render-native] DRIFT {line}\n"));
        }
        return Ok(if drift.is_empty() { 0 } else { 1 });
    }
    if gate_cli::flag(values, "apply") {
        let wrote = apply(repo, &out)?;
        let text = if wrote.is_empty() { "변경 없음(이미 렌더와 동일)".to_string() } else { format!("쓴 파일: {}", wrote.join(", ")) };
        gate_cli::write_out(&format!("[render-native] {text}\n"));
        return Ok(0);
    }
    for (rel, text) in &out {
        gate_cli::write_out(&format!("[render-native] (dry) {rel} sha256={} {}B\n", &sha256_text(text)[..16], text.chars().count()));
    }
    Ok(0)
}

// ── self-test (the fixture checks of the original, minus the hint-publisher plane judgement) ───────

const FX_COMPOSE: &str = "x-gpu-common: &gpu-common
  image: ${IMAGE_TAG:-easy-vllm:x}
  build:
    context: .
    dockerfile: ${BUILD_DOCKERFILE:-Dockerfile.source-build}
    args:
      VLLM_REF: ${VLLM_REF:-v0.9.0}
      BUILD_JOBS: ${BUILD_JOBS:-16}
  volumes:
    - ${QUANT_MODEL_PATH:-/srv/models}:/app/quant_models:ro
    - ${PLE_MMAP_HOST_PATH:-/srv/models}:/app/ple_mmap:ro
services:
  vllm-master-serve:
    <<: *gpu-common
    container_name: ${MASTER_CONTAINER_NAME:-m}
  vllm-slave-serve:
    <<: *gpu-common
    container_name: ${SLAVE_CONTAINER_NAME:-s}
    environment:
      RAY_PORT: ${RAY_PORT:-6379}
      CONFIG_FILE: ${CONFIG_FILE:-default}
      VLLM_PLE_MMAP: ${VLLM_PLE_MMAP:-0}
      VLLM_PLE_MMAP_DIR: ${VLLM_PLE_MMAP_DIR:-/app/ple_mmap/x}
    env_file:
      - envs/.env.interconnect
      - envs/.env.cluster
";
const FX_YAML: &str = "# 셀 src
model: /app/quant_models/Org/Model-X
host: 0.0.0.0
port: 8080
tensor-parallel-size: 2
distributed-executor-backend: ray
kv-cache-memory-bytes: 21474836480   # 20480MiB/노드
enforce-eager: true
";
const FX_ENV: &str = "# fixture env
COMPOSE_PROJECT_NAME=p
MASTER_CONTAINER_NAME=src-master-container
SLAVE_CONTAINER_NAME=src-slave-container
SERVING_PORT=8080
SERVING_MODEL_NAME=m
CONFIG_FILE=src
TIKTOKEN_ENABLED=false
IMAGE_TAG=easy-vllm:x
BUILD_DOCKERFILE=Dockerfile.source-build
VLLM_REF=v0.9.0
BUILD_JOBS=8
VLLM_PLE_MMAP=1
VLLM_PLE_MMAP_DIR=/app/ple_mmap/q
";

fn fixture(root: &Path) -> PathBuf {
    let base = root.join("output").join("multi");
    let w = |rel: &str, text: &str| {
        let p = base.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    w("docker-compose.yaml", FX_COMPOSE);
    w("manifest.yaml", "quant_model_path: \"/fx/quant\"\nple_mmap_host_path: \"/fx/ple\"\n");
    w("configs/src.yaml", FX_YAML);
    w("configs/src.sh", "#!/bin/bash\nvllm serve --config /app/configs/${CONFIG_FILE}.yaml\n");
    w("envs/.env.src", FX_ENV);
    w("envs/.env.cluster", "RAY_PORT=6379\nMASTER_HOST_IP=198.51.100.1\n");
    root.to_path_buf()
}

fn self_test() -> i32 {
    let mut bad: Vec<String> = Vec::new();
    let mut ck = |name: &str, cond: bool| {
        gate_cli::write_out(&format!("{}{name}\n", if cond { "[PASS] " } else { "[FAIL] " }));
        if !cond {
            bad.push(name.to_string());
        }
    };
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("render-native-{}-{stamp}", std::process::id()));
    let repo = fixture(&dir.join("repo"));
    let ok = |r: Result<Rendered, Failure>| r.ok();
    let out = ok(render(&repo, "multi", "src", "src-native")).unwrap_or_default();
    ck("결정론(두 번 같은 바이트)", Some(&out) == ok(render(&repo, "multi", "src", "src-native")).as_ref() && !out.is_empty());
    let rel = triplet_rel("multi", "src-native");
    let get = |k: &str| out.get(&rel.iter().find(|(n, _)| *n == k).unwrap().1).cloned().unwrap_or_default();
    let (y, e, s) = (get("yaml"), get("env"), get("sh"));
    ck("yaml model: 가 manifest.quant_model_path 로 사상", y.contains("\nmodel: /fx/quant/Org/Model-X\n"));
    let body_src: Vec<&str> = FX_YAML.lines().filter(|l| !l.starts_with("model:") && !l.starts_with('#')).collect();
    let body_out: Vec<&str> = y.lines().filter(|l| !l.starts_with('#') && !l.starts_with("model:")).collect();
    ck("yaml 서빙 노브는 원본과 바이트 동일", body_src == body_out);
    ck("yaml 머리말에 렌더 출처(원본 셀)", y.contains("Docker 트리플렛 src") && y.contains(TOOL));
    let _ = apply(&repo, &out);
    let envp = repo.join(&rel[2].1);
    let keys = read_env_first(&envp);
    ck("★음성대조 — 렌더 env 에 IMAGE_TAG·BUILD_DOCKERFILE 키 없음", !keys.contains_key("IMAGE_TAG") && !keys.contains_key("BUILD_DOCKERFILE"));
    let orig = read_env_first(&repo.join("output/multi/envs/.env.src"));
    ck("★음성대조 — 원본 env 에는 있었다(대조가 눈멀지 않았다)", orig.contains_key("IMAGE_TAG") && orig.contains_key("BUILD_DOCKERFILE"));
    ck(
        "빌드 인자·컨테이너 이름·compose 키 제거",
        !["VLLM_REF", "BUILD_JOBS", "MASTER_CONTAINER_NAME", "SLAVE_CONTAINER_NAME", "COMPOSE_PROJECT_NAME"].iter().any(|k| keys.contains_key(*k)),
    );
    ck(
        "서빙 키 유지 + CONFIG_FILE=native 셀",
        keys.get("SERVING_PORT").map(String::as_str) == Some("8080")
            && keys.get("SERVING_MODEL_NAME").map(String::as_str) == Some("m")
            && keys.get("CONFIG_FILE").map(String::as_str) == Some("src-native")
            && keys.get("VLLM_PLE_MMAP").map(String::as_str) == Some("1"),
    );
    ck("PLE mmap 디렉터리 → manifest.ple_mmap_host_path", keys.get("VLLM_PLE_MMAP_DIR").map(String::as_str) == Some("/fx/ple/q"));
    ck("env 머리말에 렌더 출처", e.contains("Docker 셀 env") && e.contains(TOOL));
    ck(
        "sh = 호스트 venv vllm serve · 설정은 native yaml",
        s.contains("\"${NATIVE_VENV}/bin/vllm\" serve --config \"${HERE}/${CONFIG_FILE}.yaml\"")
            && !s.split("set -euo pipefail").nth(1).unwrap_or("").contains("/app/"),
    );
    ck("--check: 쓴 직후 표류 0", check(&repo, &out).is_empty());
    let mut hand = read_text(&envp).unwrap_or_default();
    hand.push_str("IMAGE_TAG=hand\n");
    let _ = std::fs::write(&envp, hand);
    ck("--check: 손편집(IMAGE_TAG 재삽입)을 표류로 잡는다", check(&repo, &out).iter().any(|b| b.contains("env")));
    let again = apply(&repo, &out).ok();
    ck("apply 멱등(재적용 = 표류분만 쓴다)", again == Some(vec![rel[2].1.clone()]) && apply(&repo, &out).ok() == Some(vec![]));
    let negatives: [(&str, &str, String); 4] = [
        ("manifest 필드 부재 → 폴백 ✗", "output/multi/manifest.yaml", "x: 1\n".into()),
        ("런타임 패치 존재 → 거부", "output/multi/configs/src_patch.py", String::new()),
        ("port ≠ SERVING_PORT → 거부", "output/multi/configs/src.yaml", FX_YAML.replace("port: 8080", "port: 9")),
        ("tiktoken → 거부", "output/multi/envs/.env.src", FX_ENV.replace("TIKTOKEN_ENABLED=false", "TIKTOKEN_ENABLED=true")),
    ];
    for (i, (what, rel_path, text)) in negatives.iter().enumerate() {
        let r2 = fixture(&dir.join(format!("neg{i}")));
        let _ = std::fs::write(r2.join(rel_path), text);
        ck(&format!("★fail-closed — {what}"), matches!(render(&r2, "multi", "src", "src-native"), Err(Failure::Render(_))));
    }
    let _ = std::fs::remove_dir_all(&dir);
    gate_cli::write_out(&format!("[render-native-triplet] {}\n", if bad.is_empty() { "PASS" } else { "FAIL" }));
    if bad.is_empty() { 0 } else { 1 }
}
