//! `slave_forward.py` (non-self-test part): derive the build/serve variables to forward to the multi Ray worker (slave) from the real compose
//! references — the `prefix`, `json` and `default` subcommands, plus the new `non-identity` subcommand (the NON_IDENTITY_BUILD_ARGS list the smoke
//! heredoc formerly imported from the Python module).
//!
//! The Band2 key test (`_band2_owned`) calls `render_dockerfile::env_tier` in process (the Python original imported `render_dockerfile.py`).
//! Port boundary: `--self-test` is not ported (exit 2; run `slave_forward.py --self-test`); argparse's own usage/error text is not reproduced
//! (a malformed command line exits 2 with a short message). The Python module stays for its in-process importers.

use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const GROUPS: [&str; 5] = ["image_identity", "build_tuning", "cluster", "mount", "serve_env"];
pub const NON_IDENTITY_BUILD_ARGS: [&str; 1] = ["BUILD_JOBS"];
const NEVER_FORWARD: [&str; 1] = ["CONFIG_FILE"];
const SLAVE_SERVICE: &str = "vllm-slave-serve";
const SAFE_PATTERN: &str = "[A-Za-z0-9._:/@+,=-]*";
const WHY_DERIVED: &str = "compose 참조에서 자동 파생(기록된 사고 이력 없음 — 새 변수)";

const WHY: &[(&str, &str)] = &[
    ("IMAGE_TAG", "이미지 정체성의 이름 — slave 가 모르면 compose 기본 태그로 빌드·기동한다(plan_26062818 §S2.5 R10)."),
    ("BUILD_DOCKERFILE", "2026-07-24 Solar-Open2: slave build 가 --env-file EFC 만 받아 compose 기본 Dockerfile 로 폴백 → 변종 트랙에서 slave 만 다른 Dockerfile 로 빌드됐다(기본값 콤보에서만 잠복)."),
    ("VLLM_REPO", "포크 핀 좌표 — 빠지면 slave 만 stock 저장소로 빌드된다(plan_26062818 §S2.5 R10)."),
    ("VLLM_REF", "포크/릴리스 ref — 빠지면 slave 가 compose 기본 ref 로 빌드된다(plan_26062818 §S2.5 R10)."),
    ("VLLM_PRETEND_VERSION", "2026-08-02: 비-semver 포크 태그의 setuptools_scm 우회값 — 빠지면 마스터만 빌드되고 slave 는 같은 지점(ValueError)에서 죽는다."),
    ("SM12X_PORT", "2026-08-14(plan_26081418 G-4): build_patches_src 소스 이식 변종 게이트 — 빠지면 마스터만 이식본·slave 는 stock. 부재 = stock(빈 값을 흘리면 compose 기본값을 빈 문자열로 덮는다)."),
    ("SRC_DEPS_AUTHORITY", "2026-08-15(R3 포크 핀): 클론 소스트리 의존 권위 게이트 — 빠지면 마스터만 flashinfer 0.6.17·slave 0.6.16.post3(이 목록의 네 번째 전파 구멍)."),
    ("VLLM_VERSION", "2026-09-05: wheel 트랙 설치 버전 — 빠지면 slave 가 Dockerfile ARG 기본값(0.18.0)으로 빌드돼 같은 IMAGE_TAG 가 노드마다 다른 엔진이 된다(다섯 번째 · compose build-arg 복원과 함께 생긴 구멍)."),
    ("BUILD_JOBS", "2026-08-15: 이미지 정체성은 아니지만 양 노드에 같아야 한다 — slave 만 기본 16 으로 컴파일하면 거기서 OOM(하드다운 #2 가 서브였다)."),
    ("RAY_PORT", "2026-08-14(침묵 누락 3번째): 셀이 head 포트를 바꾸면 slave 는 기본 6379 로 join 을 시도해 nc -z 5분 대기 후 실패한다(클러스터 랑데부 주소의 절반). 과거 멀티 런은 전부 6379(=compose 기본값)라 우연히 일치했다(docs/simlog 26070213·26072500) — 콤보별 포트 분리(plan_26081310 §X2)의 첫 런에서 깨어나는 잠복 결함."),
    ("SLAVE_CONTAINER_NAME", "2026-08-14: 폴백 이름이면 slave 협역 워치독 필터가 매칭 0 — 서브에서만 계층 2층이 조용히 사라진다(devlog_26080212 ⑥ 부류 · 하드다운 #2 는 서브였다). 2026-08-02 에 이미 '엉뚱한 이름으로 떠 있었고 아무도 몰랐다' 로 한 번 드러났다(그때는 마스터측만 교정)."),
    ("MASTER_HOST_IP", "Band2(.env.cluster) 키 — 셀이 덮어쓰면 마스터만 새 값을 본다(head 주소 불일치)."),
    ("SLAVE_HOST_IP", "Band2(.env.cluster) 키 — 셀이 덮어쓰면 slave 자기 IP 가 마스터 판단과 갈린다."),
    ("NAS_MODEL_PATH", "결함#2b(plan_26070119): --env-file 을 쓰면 프로젝트 .env 자동 로드가 꺼져 compose 기본 마운트로 폴백 → 모델 부재(serve 즉사). 셸 env 로 명시 주입한다."),
    ("QUANT_MODEL_PATH", "결함#2b 와 같은 경로 — 양자화 모델 마운트원."),
    ("TIKTOKEN_HOST_PATH", "결함#2b 와 같은 경로 — 오프라인 토크나이저 마운트원."),
    ("PLE_MMAP_HOST_PATH", "2026-09-09(camp-26090918): PLE mmap 로컬 NVMe 스테이징 마운트원 — slave 도 같은 경로에 실재해야 한다."),
    ("VLLM_PLE_MMAP", "2026-09-09(62-qwen4exp-ple-mmap): Ray 워커도 가중치를 올린다 — 마스터만 켜면 slave 는 상주 로드로 OOM/불일치."),
    ("RUNTIME_PATCH_STEM", "2026-10-24(Solar-Open2 자체 이식 플러그인): slave 는 CONFIG_FILE=default 라 model-keyed 런타임 패치를 arming 못 했다 — Ray 워커가 모델 클래스를 import 하는 모델은 slave 가 패치를 먹어야 뜬다. 패치 stem 만 별도 키로 전달(비면 생략 = 종전 동작)."),
    ("VLLM_PLE_MMAP_DIR", "2026-09-09: 워커 로컬 mmap 스테이징 경로 — 마스터와 같은 값이어야 한다."),
];

#[derive(Debug)]
pub struct ForwardError {
    pub code: &'static str,
    pub message: String,
}

fn ferr<T>(code: &'static str, message: String) -> Result<T, ForwardError> {
    Err(ForwardError { code, message })
}

#[derive(Debug, Clone)]
pub struct Forward {
    pub key: String,
    pub value: String,
    pub group: &'static str,
    pub source: &'static str,
    pub reference: String,
}

fn why(key: &str) -> &'static str {
    WHY.iter().find(|(k, _)| *k == key).map_or(WHY_DERIVED, |(_, v)| v)
}

// ── Python text semantics ──────────────────────────────────────────────────────────────────────────

fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
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

/// `repr(str)` of Python for the messages that quote a value.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::new();
    o.push(quote);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == quote => {
                o.push('\\');
                o.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push(quote);
    o
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn safe_value(v: &str) -> bool {
    v.chars().all(|c| c.is_ascii_alphanumeric() || "._:/@+,=-".contains(c))
}

// ── env file ───────────────────────────────────────────────────────────────────────────────────────

/// First match of `KEY=value` per key (the smoke's `val()`); an absent file is an empty map.
fn read_env_first(path: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(path) = path else { return out };
    let p = Path::new(path);
    if !p.is_file() {
        return out;
    }
    let Ok(bytes) = std::fs::read(p) else { return out };
    let text = String::from_utf8_lossy(&bytes);
    for raw in splitlines(&text) {
        let mut it = raw.char_indices();
        match it.next() {
            Some((_, c)) if is_ident_start(c) => {}
            _ => continue,
        }
        let end = raw.char_indices().find(|&(_, c)| !is_ident(c)).map_or(raw.len(), |(i, _)| i);
        if raw[end..].starts_with('=') {
            out.entry(raw[..end].to_string()).or_insert_with(|| raw[end + 1..].to_string());
        }
    }
    out
}

// ── compose parser ─────────────────────────────────────────────────────────────────────────────────

fn strip_comment(line: &str) -> String {
    let (mut out, mut quote, mut prev) = (String::new(), None::<char>, ' ');
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

struct Tree {
    nodes: Vec<Node>,
}

/// `([A-Za-z0-9_.<-]+)\s*:(?:\s+(.*))?` fullmatch → `(key, value)`.
fn parse_node(body: &str) -> Option<(String, String)> {
    let key_end = body.char_indices().find(|&(_, c)| !(c.is_ascii_alphanumeric() || "_.<-".contains(c))).map_or(body.len(), |(i, _)| i);
    if key_end == 0 {
        return None;
    }
    let rest = body[key_end..].trim_start_matches(char::is_whitespace);
    let rest = rest.strip_prefix(':')?;
    if rest.is_empty() {
        return Some((body[..key_end].to_string(), String::new()));
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((body[..key_end].to_string(), rest.trim_start_matches(char::is_whitespace).to_string()))
}

fn build_tree(text: &str) -> Tree {
    let mut nodes = vec![Node { key: None, value: String::new(), children: Vec::new(), indent: -1, item: false }];
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
        let node = match parsed {
            Some((k, v)) => Node { key: Some(k), value: v.trim().to_string(), children: Vec::new(), indent, item },
            None => Node { key: None, value: body.trim().to_string(), children: Vec::new(), indent, item },
        };
        while nodes[*stack.last().unwrap()].indent >= indent {
            stack.pop();
        }
        nodes.push(node);
        let id = nodes.len() - 1;
        let parent = *stack.last().unwrap();
        nodes[parent].children.push(id);
        stack.push(id);
    }
    Tree { nodes }
}

impl Tree {
    fn child(&self, node: Option<usize>, key: &str) -> Option<usize> {
        let node = node?;
        self.nodes[node].children.iter().copied().find(|&c| self.nodes[c].key.as_deref() == Some(key) && !self.nodes[c].item)
    }

    fn texts<'a>(&'a self, node: usize, out: &mut Vec<&'a str>) {
        out.push(&self.nodes[node].value);
        for &c in &self.nodes[node].children {
            self.texts(c, out);
        }
    }

    fn value_of(&self, node: Option<usize>) -> &str {
        node.map_or("", |n| &self.nodes[n].value)
    }
}

/// `(?<!\$)\$(?:\{(IDENT)|(IDENT))` over the texts, first-seen order without duplicates.
fn refs(texts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for text in texts {
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

struct ComposeRefs {
    selectors: Vec<String>,
    build_args: Vec<String>,
    volumes: Vec<String>,
    slave_container: Vec<String>,
    slave_environment: Vec<String>,
    other: Vec<String>,
}

fn compose_refs(compose_path: &str) -> Result<ComposeRefs, ForwardError> {
    let path = Path::new(compose_path);
    if !path.is_file() {
        return ferr("SF_COMPOSE_ABSENT", format!("compose 파일이 없다: {}", path.display()));
    }
    let text = match std::fs::read(path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => return ferr("SF_COMPOSE_ABSENT", format!("compose 파일을 읽지 못했다: {}: {e}", path.display())),
    };
    let tree = build_tree(&text);
    let root = &tree.nodes[0];
    let common = root.children.iter().copied().find(|&c| {
        let n = &tree.nodes[c];
        n.key.as_deref().is_some_and(|k| k.starts_with("x-")) && n.value.starts_with('&')
    });
    let Some(common) = common else {
        return ferr("SF_COMPOSE_SHAPE", "공통 앵커 블록(x-…: &name)을 찾지 못했다".into());
    };
    let anchor = tree.nodes[common].value[1..].trim().to_string();
    let Some(slave) = tree.child(tree.child(Some(0), "services"), SLAVE_SERVICE) else {
        return ferr("SF_COMPOSE_SHAPE", format!("services.{SLAVE_SERVICE} 가 없다"));
    };
    let merge = tree.child(Some(slave), "<<");
    if merge.is_none_or(|m| tree.nodes[m].value != format!("*{anchor}")) {
        return ferr("SF_COMPOSE_SHAPE", format!("{SLAVE_SERVICE} 가 공통 앵커 *{anchor} 를 병합하지 않는다"));
    }
    let common_o = Some(common);
    let build = tree.child(common_o, "build");
    let selectors = refs(&[tree.value_of(tree.child(common_o, "image")), tree.value_of(tree.child(build, "dockerfile"))]);
    let args_node = tree.child(build, "args");
    let arg_values: Vec<&str> = args_node.map_or(Vec::new(), |a| tree.nodes[a].children.iter().map(|&c| tree.nodes[c].value.as_str()).collect());
    let build_args = refs(&arg_values);
    let mut vol_texts: Vec<&str> = Vec::new();
    for n in [tree.child(common_o, "volumes"), tree.child(Some(slave), "volumes")].into_iter().flatten() {
        tree.texts(n, &mut vol_texts);
    }
    let volumes = refs(&vol_texts);
    let slave_container = refs(&[tree.value_of(tree.child(Some(slave), "container_name"))]);
    let mut env_texts: Vec<&str> = Vec::new();
    if let Some(e) = tree.child(Some(slave), "environment") {
        tree.texts(e, &mut env_texts);
    }
    let slave_environment = refs(&env_texts);
    let mut known: HashSet<&String> = HashSet::new();
    for v in [&selectors, &build_args, &volumes, &slave_container, &slave_environment] {
        known.extend(v.iter());
    }
    let mut all: Vec<&str> = Vec::new();
    tree.texts(common, &mut all);
    tree.texts(slave, &mut all);
    let other: Vec<String> = refs(&all).into_iter().filter(|r| !known.contains(r)).collect();
    Ok(ComposeRefs { selectors, build_args, volumes, slave_container, slave_environment, other })
}

fn compose_default(compose_path: &str, key: &str) -> Result<Option<String>, ForwardError> {
    let path = Path::new(compose_path);
    if !path.is_file() {
        return ferr("SF_COMPOSE_ABSENT", format!("compose 파일이 없다: {}", path.display()));
    }
    let text = match std::fs::read(path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => return ferr("SF_COMPOSE_ABSENT", format!("compose 파일을 읽지 못했다: {}: {e}", path.display())),
    };
    for raw in splitlines(&text) {
        let line: Vec<char> = strip_comment(raw).chars().collect();
        let Some(start) = head_end(&line, key) else { continue };
        let mut depth = 1;
        for j in start..line.len() {
            if line[j] == '{' {
                depth += 1;
            } else if line[j] == '}' {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(line[start..j].iter().collect()));
                }
            }
        }
        return ferr("SF_COMPOSE_SHAPE", format!("${{{key}:-…}} 의 괄호가 한 줄 안에서 닫히지 않는다: {}", py_repr(raw.trim())));
    }
    Ok(None)
}

/// End index of the first `(?<!\$)\$\{KEY:?-` in the line.
fn head_end(line: &[char], key: &str) -> Option<usize> {
    let k: Vec<char> = key.chars().collect();
    for i in 0..line.len() {
        if line[i] != '$' || (i > 0 && line[i - 1] == '$') {
            continue;
        }
        let mut j = i + 1;
        if line.get(j) != Some(&'{') {
            continue;
        }
        j += 1;
        if line.len() < j + k.len() || line[j..j + k.len()] != k[..] {
            continue;
        }
        j += k.len();
        if line.get(j) == Some(&':') {
            j += 1;
        }
        if line.get(j) == Some(&'-') {
            return Some(j + 1);
        }
    }
    None
}

// ── Band2 owned keys ───────────────────────────────────────────────────────────────────────────────

fn tier(key: &str) -> Result<(&'static str, Option<String>), ForwardError> {
    crate::render_dockerfile::env_tier(key).map_err(|e| ForwardError {
        code: "SF_ENV_TIER_FAILED",
        message: format!("render_dockerfile.env_tier({}) 실패: {}", py_repr(key), e.message),
    })
}

fn band2_owned(key: &str) -> Result<bool, ForwardError> {
    Ok(tier(key)?.0 != "unknown")
}

pub type Groups = Vec<(&'static str, Vec<String>)>;

pub fn candidates(compose_path: &str, cluster_env: Option<&str>) -> Result<Groups, ForwardError> {
    let refs = compose_refs(compose_path)?;
    let band2_file: HashSet<String> = cluster_env.filter(|c| !c.is_empty()).map(|c| read_env_first(Some(c)).into_keys().collect()).unwrap_or_default();
    let mut out: Groups = GROUPS.iter().map(|g| (*g, Vec::new())).collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut put = |group: &str, key: &str| {
        if NEVER_FORWARD.contains(&key) || seen.contains(key) {
            return;
        }
        seen.insert(key.to_string());
        out.iter_mut().find(|(g, _)| *g == group).unwrap().1.push(key.to_string());
    };
    for key in refs.selectors.iter().chain(refs.build_args.iter()) {
        put(if NON_IDENTITY_BUILD_ARGS.contains(&key.as_str()) { "build_tuning" } else { "image_identity" }, key);
    }
    for key in &refs.volumes {
        put("mount", key);
    }
    for key in &refs.slave_container {
        put("cluster", key);
    }
    for key in &refs.slave_environment {
        let cluster = band2_file.contains(key) || band2_owned(key)?;
        put(if cluster { "cluster" } else { "serve_env" }, key);
    }
    for key in &refs.other {
        put("serve_env", key);
    }
    Ok(out)
}

fn ref_place(group: &str) -> String {
    match group {
        "image_identity" => "x-gpu-common.image|build.dockerfile|build.args".into(),
        "build_tuning" => "x-gpu-common.build.args".into(),
        "mount" => "volumes".into(),
        "cluster" => format!("services.{SLAVE_SERVICE}.container_name|environment(Band2 키)"),
        _ => format!("services.{SLAVE_SERVICE}.environment"),
    }
}

pub fn derive(cell_env: &str, project_env: &str, compose_path: &str, cluster_env: Option<&str>) -> Result<Vec<Forward>, ForwardError> {
    let groups = candidates(compose_path, cluster_env)?;
    let cell = read_env_first(Some(cell_env));
    let project = read_env_first(Some(project_env));
    let mut rows = Vec::new();
    for want in ["image_identity", "build_tuning", "mount", "cluster", "serve_env"] {
        let (group, keys) = groups.iter().find(|(g, _)| *g == want).unwrap();
        for key in keys {
            let (value, source) = if *group == "mount" {
                match project.get(key).filter(|v| !v.is_empty()) {
                    Some(v) => (Some(v), "project_env"),
                    None => (cell.get(key), "cell_env"),
                }
            } else {
                (cell.get(key), "cell_env")
            };
            let Some(value) = value.filter(|v| !v.is_empty()) else { continue };
            if !safe_value(value) {
                return ferr(
                    "SF_UNSAFE_VALUE",
                    format!("{key}={} 는 인용 없는 원격 셸 prefix 에 안전하지 않다(허용 문자 {SAFE_PATTERN}) — {source} 를 고쳐라", py_repr(value)),
                );
            }
            rows.push(Forward { key: key.clone(), value: value.clone(), group, source, reference: ref_place(group) });
        }
    }
    Ok(rows)
}

fn check_groups<'a>(groups: &[&'a str]) -> Result<Vec<&'a str>, ForwardError> {
    let names: Vec<&str> = groups.iter().copied().filter(|g| !g.is_empty()).collect();
    let unknown: Vec<&str> = names.iter().copied().filter(|g| !GROUPS.contains(g)).collect();
    if !unknown.is_empty() || names.is_empty() {
        let shown = if unknown.is_empty() {
            "(없음)".to_string()
        } else {
            format!("[{}]", unknown.iter().map(|g| py_repr(g)).collect::<Vec<_>>().join(", "))
        };
        let allowed = format!("[{}]", GROUPS.iter().map(|g| py_repr(g)).collect::<Vec<_>>().join(", "));
        return ferr("SF_GROUP_UNKNOWN", format!("그룹 {shown} — 허용 {allowed}"));
    }
    Ok(names)
}

pub fn prefix(forwards: &[Forward], groups: &[&str]) -> Result<String, ForwardError> {
    let wanted = check_groups(groups)?;
    Ok(forwards.iter().filter(|f| wanted.contains(&f.group)).map(|f| format!("{}={}", f.key, f.value)).collect::<Vec<_>>().join(" "))
}

fn value_shape(f: &Forward) -> Result<String, ForwardError> {
    if f.group == "mount" {
        return Ok(format!("<manifest.{}>", f.key.to_lowercase()));
    }
    if f.group == "cluster" && band2_owned(&f.key)? {
        let (t, detail) = tier(&f.key)?;
        if t == "env" {
            return Ok(format!("<manifest.{}>", detail.unwrap_or_default()));
        }
    }
    Ok(f.value.clone())
}

// ── JSON (json.dumps(ensure_ascii=False, indent=2, sort_keys=True)) ───────────────────────────────

fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\x08' => o.push_str("\\b"),
            '\x0c' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_objects(rows: &[Vec<(&str, String)>], indent: &str) -> String {
    if rows.is_empty() {
        return "[]".into();
    }
    let mut o = String::from("[\n");
    for (i, row) in rows.iter().enumerate() {
        o.push_str(&format!("{indent}  {{\n"));
        for (j, (k, v)) in row.iter().enumerate() {
            o.push_str(&format!("{indent}    {}: {}{}\n", jstr(k), jstr(v), if j + 1 < row.len() { "," } else { "" }));
        }
        o.push_str(&format!("{indent}  }}{}\n", if i + 1 < rows.len() { "," } else { "" }));
    }
    o.push_str(&format!("{indent}]"));
    o
}

fn json_report(rows: &[Forward]) -> Result<String, ForwardError> {
    let forwards: Vec<Vec<(&str, String)>> = rows
        .iter()
        .map(|f| vec![("group", f.group.to_string()), ("key", f.key.clone()), ("ref", f.reference.clone()), ("source", f.source.to_string()), ("value", f.value.clone())])
        .collect();
    let mut recipe: Vec<Vec<(&str, String)>> = Vec::new();
    for f in rows {
        recipe.push(vec![
            ("compose_ref", f.reference.clone()),
            ("from", f.source.to_string()),
            ("group", f.group.to_string()),
            ("key", f.key.clone()),
            ("value_shape", value_shape(f)?),
            ("why", why(&f.key).to_string()),
        ]);
    }
    Ok(format!(
        "{{\n  \"forwards\": {},\n  \"schema_version\": 1,\n  \"sub_recipe\": {}\n}}",
        json_objects(&forwards, "  "),
        json_objects(&recipe, "  ")
    ))
}

// ── command line ───────────────────────────────────────────────────────────────────────────────────

fn out(text: &str) {
    use std::io::Write;
    let mut so = std::io::stdout().lock();
    let _ = so.write_all(text.as_bytes());
    let _ = so.flush();
}

fn err(text: &str) {
    use std::io::Write;
    let mut se = std::io::stderr().lock();
    let _ = se.write_all(text.as_bytes());
    let _ = se.flush();
}

fn usage_error(msg: &str) -> i32 {
    err(&format!("usage: slave_forward.py [-h] [--self-test] {{prefix,json,default,non-identity}} ...\nslave_forward.py: error: {msg}\n"));
    2
}

struct Opts {
    cell_env: Option<String>,
    project_env: Option<String>,
    compose: Option<String>,
    cluster_env: Option<String>,
    group: Option<String>,
    key: Option<String>,
}

fn parse_opts(cmd: &str, args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { cell_env: None, project_env: None, compose: None, cluster_env: None, group: None, key: None };
    let allowed: &[&str] = match cmd {
        "prefix" => &["--cell-env", "--project-env", "--compose", "--cluster-env", "--group"],
        "json" => &["--cell-env", "--project-env", "--compose", "--cluster-env"],
        "default" => &["--compose", "--key"],
        _ => &[],
    };
    let mut i = 0;
    while i < args.len() {
        let (flag, inline) = match args[i].split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (args[i].clone(), None),
        };
        if !allowed.contains(&flag.as_str()) {
            return Err(format!("unrecognized arguments: {}", args[i]));
        }
        let value = match inline {
            Some(v) => v,
            None => {
                i += 1;
                match args.get(i) {
                    Some(v) => v.clone(),
                    None => return Err(format!("argument {flag}: expected one argument")),
                }
            }
        };
        match flag.as_str() {
            "--cell-env" => o.cell_env = Some(value),
            "--project-env" => o.project_env = Some(value),
            "--compose" => o.compose = Some(value),
            "--cluster-env" => o.cluster_env = Some(value),
            "--group" => o.group = Some(value),
            _ => o.key = Some(value),
        }
        i += 1;
    }
    let need = |v: &Option<String>, n: &str| v.is_some().then_some(()).ok_or_else(|| format!("the following arguments are required: {n}"));
    match cmd {
        "prefix" | "json" => {
            need(&o.cell_env, "--cell-env")?;
            need(&o.project_env, "--project-env")?;
            need(&o.compose, "--compose")?;
            if cmd == "prefix" {
                need(&o.group, "--group")?;
            }
        }
        "default" => {
            need(&o.compose, "--compose")?;
            need(&o.key, "--key")?;
        }
        _ => {}
    }
    Ok(o)
}

pub fn run(args: &[String]) -> i32 {
    if args.first().is_some_and(|a| a == "--self-test") {
        err("[slave-forward] self-test not ported to Rust (use slave_forward.py --self-test)\n");
        return 2;
    }
    if args.is_empty() {
        return usage_error("하위 명령이 필요하다(prefix|json|default) 또는 --self-test");
    }
    let cmd = args[0].as_str();
    if !matches!(cmd, "prefix" | "json" | "default" | "non-identity") {
        return usage_error(&format!("argument cmd: invalid choice: {} (choose from 'prefix', 'json', 'default', 'non-identity')", py_repr(cmd)));
    }
    let o = match parse_opts(cmd, &args[1..]) {
        Ok(o) => o,
        Err(m) => return usage_error(&m),
    };
    let result: Result<i32, ForwardError> = (|| {
        match cmd {
            "non-identity" => {
                let mut names: Vec<&str> = NON_IDENTITY_BUILD_ARGS.to_vec();
                names.sort_unstable();
                for n in names {
                    out(&format!("{n}\n"));
                }
                Ok(0)
            }
            "default" => {
                let key = o.key.as_deref().unwrap();
                match compose_default(o.compose.as_deref().unwrap(), key)? {
                    None => {
                        err(&format!("[slave-forward] {key} 의 compose 기본값이 없다\n"));
                        Ok(1)
                    }
                    Some(v) => {
                        out(&format!("{v}\n"));
                        Ok(0)
                    }
                }
            }
            _ => {
                let rows = derive(o.cell_env.as_deref().unwrap(), o.project_env.as_deref().unwrap(), o.compose.as_deref().unwrap(), o.cluster_env.as_deref())?;
                if cmd == "prefix" {
                    let groups: Vec<&str> = o.group.as_deref().unwrap().split(',').collect();
                    out(&format!("{}\n", prefix(&rows, &groups)?));
                } else {
                    out(&format!("{}\n", json_report(&rows)?));
                }
                Ok(0)
            }
        }
    })();
    match result {
        Ok(status) => status,
        Err(e) => {
            err(&format!("[slave-forward] FAIL({}): {}\n", e.code, e.message));
            2
        }
    }
}
