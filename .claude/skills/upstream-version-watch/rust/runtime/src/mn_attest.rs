//! The two JSON writers `multinode_serve_smoke.sh` ran as inline `python3 - <<PY` (plan_26111615 pass 3): the node parity
//! attestation v2 (`_attest_write`) and the serve proof. Same files (`json.dump(doc, ensure_ascii=False, indent=2,
//! sort_keys=True)` + `\n`), the same stdout line and exit 0 on success; where the original raised (an undecodable file, a
//! ledger field of the wrong type, incomparable patch tuples, a lone surrogate) the status is 1 with one stderr line, and the
//! shell prints its "기록 실패" notice exactly as before. Reached through `inline_json.sh attestation-write|serve-proof`.

use crate::wheelhouse_util::sha256_hex;
use agent_runtime_support::pyjson;
use agent_runtime_support::pysort::{py_eq, sorted_by_key};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::{Component, Path, PathBuf};

type R<T> = Result<T, String>;

fn vs(s: &str) -> PyValue {
    PyValue::str(s)
}
fn opt(s: Option<String>) -> PyValue {
    s.map_or(PyValue::None, |t| vs(&t))
}
fn dict(items: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}
fn get<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(k).map(PyValue::peel),
        _ => None,
    }
}
/// `sort_keys=True`, recursively (inside lists and tuples too).
fn sorted(v: &PyValue) -> PyValue {
    match v.peel() {
        PyValue::Dict(d) => {
            let mut e: Vec<(PyStr, PyValue)> = d.iter().map(|(k, x)| (k.clone(), sorted(x))).collect();
            e.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut out = PyDict::new();
            for (k, x) in e {
                out.insert(k, x);
            }
            PyValue::Dict(out)
        }
        PyValue::List(l) => PyValue::List(l.iter().map(sorted).collect()),
        PyValue::Tuple(l) => PyValue::Tuple(l.iter().map(sorted).collect()),
        o => o.clone(),
    }
}

/// `json.dump(doc, fh, ensure_ascii=False, indent=2, sort_keys=True); fh.write("\n")`.
fn write_doc(out: &str, doc: &PyValue) -> R<()> {
    let text = sorted(doc).dumps_pretty().map_err(|_| "json.dump failed".to_string())?;
    let text = text.to_utf8().ok_or("UnicodeEncodeError: surrogates not allowed")?;
    std::fs::write(out, text + "\n").map_err(|e| format!("{out}: {e}"))
}

/// Python `str.isspace` for one code point (`str.strip()`/`str.split()` separators).
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `read(p)`: `open(p, encoding="utf-8").read().strip() or None`; OSError → None, a decode error is raised.
fn read(p: &str) -> R<Option<String>> {
    let Ok(bytes) = std::fs::read(p) else { return Ok(None) };
    let text = pyjson::decode_file_text(&bytes).map_err(|e| format!("UnicodeDecodeError: {p}: {e:?}"))?;
    let t = text.to_utf8().unwrap_or_default();
    let t = t.trim_matches(py_space);
    Ok(if t.is_empty() { None } else { Some(t.to_string()) })
}

/// `str.splitlines()` (no keepends).
fn splitlines(s: &str) -> Vec<String> {
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
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `os.path.relpath(path)` against the process working directory (lexical, like `posixpath`).
fn relpath(path: &str) -> String {
    fn norm(p: &Path) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for c in p.components() {
            match c {
                Component::Normal(x) => v.push(x.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    v.pop();
                }
                _ => {}
            }
        }
        v
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let p = Path::new(path);
    let abs = if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) };
    let (a, b) = (norm(&cwd), norm(&abs));
    let common = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); a.len() - common];
    parts.extend(b[common..].iter().cloned());
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

/// `(x.get("vllm") or {}).get("git_sha")` — a truthy non-dict `vllm` raised AttributeError in the original.
fn vllm_git_sha(led: &PyValue) -> R<PyValue> {
    match get(led, "vllm") {
        None => Ok(PyValue::None),
        Some(v) if !v.is_truthy() => Ok(PyValue::None),
        Some(v @ PyValue::Dict(_)) => Ok(get(v, "git_sha").cloned().unwrap_or(PyValue::None)),
        Some(v) => Err(format!("AttributeError: '{}' object has no attribute 'get'", v.type_name())),
    }
}

fn identity(led: &PyValue, exempt: &[String]) -> R<Option<String>> {
    if !matches!(led.peel(), PyValue::Dict(_)) {
        return Ok(None);
    }
    let mut args = PyDict::new();
    match get(led, "build_args") {
        None => {}
        Some(v) if !v.is_truthy() => {}
        Some(PyValue::Dict(d)) => {
            for (k, v) in d.iter() {
                if !exempt.iter().any(|x| k.equals(x)) {
                    args.insert(k.clone(), v.clone());
                }
            }
        }
        Some(v) => return Err(format!("AttributeError: '{}' object has no attribute 'items'", v.type_name())),
    }
    let patch_rows: Vec<PyValue> = match get(led, "patches") {
        None => Vec::new(),
        Some(v) if !v.is_truthy() => Vec::new(),
        Some(PyValue::List(l)) | Some(PyValue::Tuple(l)) => l.iter().map(|x| x.peel().clone()).collect(),
        Some(PyValue::Dict(_)) | Some(PyValue::Str(_)) => Vec::new(), // iterates keys / characters: none is a dict
        Some(v) => return Err(format!("TypeError: '{}' object is not iterable", v.type_name())),
    };
    let tuples: Vec<PyValue> = patch_rows
        .iter()
        .filter(|p| matches!(p, PyValue::Dict(_)))
        .map(|p| {
            PyValue::Tuple(["phase", "file", "script_sha256", "result"].iter().map(|k| get(p, k).cloned().unwrap_or(PyValue::None)).collect())
        })
        .collect();
    let pats = sorted_by_key(tuples.clone(), &tuples).map_err(|f| format!("{}: {}", f.class, f.message.to_utf8().unwrap_or_default()))?;
    let body = dict(vec![
        ("dockerfile", get(led, "dockerfile").cloned().unwrap_or(PyValue::None)),
        ("build_args", PyValue::Dict(args)),
        ("patches", PyValue::List(pats)),
        ("vllm_git_sha", vllm_git_sha(led)?),
    ]);
    let text = sorted(&body).dumps_unicode().map_err(|_| "json.dumps failed".to_string())?;
    let text = text.to_utf8().ok_or("UnicodeEncodeError: surrogates not allowed")?;
    Ok(Some(sha256_hex(text.as_bytes())))
}

fn verdict(a: &PyValue, b: &PyValue) -> &'static str {
    if matches!(a, PyValue::None) || matches!(b, PyValue::None) {
        "unobservable"
    } else if py_eq(a, b) {
        "equal"
    } else {
        "differ"
    }
}

fn is_sha40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn attestation(a: &[String]) -> R<String> {
    if a.len() < 9 {
        return Err("ValueError: not enough values to unpack (expected 9)".into());
    }
    let (out, config, image, phase, cap, rows_path, main_id, sub_id) = (&a[0], &a[1], &a[2], &a[3], &a[4], &a[5], &a[7], &a[8]);
    let mut checks: Vec<PyValue> = Vec::new();
    for line in splitlines(&read(rows_path)?.unwrap_or_default()) {
        let mut f: Vec<String> = line.split('\t').map(String::from).collect();
        f.extend(std::iter::repeat(String::new()).take(4));
        checks.push(dict(vec![("axis", vs(&f[0])), ("node", vs(&f[1])), ("observed", vs(&f[2])), ("expected", vs(&f[3])), ("phase", vs("build"))]));
    }
    let mut doc = PyDict::new();
    for (k, v) in [
        ("schema_version", PyValue::int(2)),
        ("kind", vs("multinode_node_parity_attestation")),
        ("config", vs(config)),
        ("topology", vs("multi")),
        ("image_tag", vs(image)),
        ("phase", vs(phase)),
    ] {
        doc.set(k, v);
    }
    doc.set("parity_blocking", PyValue::Bool(false));
    doc.set("parity_note", vs("기록 전용 — differ 를 차단할지는 사람 결정(D-5). image digest 는 판정 축이 아니다(동일 ABI ≠ 동일 이미지)."));
    let mut parity_out: Option<PyValue> = None;
    if phase == "build" {
        doc.set("provenance", vs("measured(docker run in each node image · 이번 실행 --build 대조)"));
    } else {
        doc.set("provenance", vs("measured(docker exec in each running container · host nvidia-smi)"));
        let env = std::env::var("SF_NON_IDENTITY").unwrap_or_default();
        let mut exempt: Vec<String> = env.split(py_space).filter(|x| !x.is_empty()).map(String::from).collect();
        exempt.sort_by(|x, y| x.chars().map(u32::from).cmp(y.chars().map(u32::from)));
        let capf = |role: &str, ext: &str| Path::new(cap).join(format!("{role}.{ext}")).to_string_lossy().into_owned();
        let mut nodes = PyDict::new();
        let mut ledgers: Vec<PyValue> = Vec::new();
        for (role, node_id) in [("main", main_id), ("sub", sub_id)] {
            let raw = read(&capf(role, "ledger.json"))?;
            let (mut ledger, mut status) = (PyValue::None, "unobservable(capture failed or legacy image without /opt/easy-vllm/build_ledger.json)".to_string());
            if let Some(raw) = raw {
                match pyjson::loads(&raw) {
                    Ok(v) => {
                        ledger = v;
                        status = "observed".into();
                    }
                    Err(_) => status = "unobservable(ledger unparsable)".into(),
                }
            }
            if status == "observed" && !matches!(ledger.peel(), PyValue::Dict(_)) {
                ledger = PyValue::None;
                status = "unobservable(ledger is not a JSON object)".into();
            }
            let mut sha = if matches!(ledger.peel(), PyValue::Dict(_)) { vllm_git_sha(&ledger)? } else { PyValue::None };
            let mut sha_src = if sha.is_truthy() { vs("build_ledger.vllm.git_sha") } else { PyValue::None };
            if !sha.is_truthy() {
                if let Some(cand) = read(&capf(role, "vllm_sha"))? {
                    if is_sha40(&cand) {
                        sha = vs(&cand);
                        sha_src = vs("docker exec git -C /workspace/vllm-src rev-parse HEAD");
                    }
                }
            }
            let row = dict(vec![
                ("node_id", vs(node_id)),
                ("container", opt(read(&capf(role, "container"))?)),
                ("image_id", opt(read(&capf(role, "image_id"))?)),
                ("build_ledger", ledger.clone()),
                ("build_ledger_status", vs(&status)),
                ("vllm_sha", sha),
                ("vllm_sha_source", sha_src),
                ("vllm_dist_version", opt(read(&capf(role, "vllm_dist"))?)),
                ("torch", opt(read(&capf(role, "torch"))?)),
                ("driver", opt(read(&capf(role, "driver"))?)),
            ]);
            nodes.set(role, row);
            ledgers.push(ledger);
        }
        let nodes_v = PyValue::Dict(nodes);
        let node_axis = |role: &str, axis: &str| get(get(&nodes_v, role).unwrap(), axis).cloned().unwrap_or(PyValue::None);
        let mut parity = PyDict::new();
        for axis in ["vllm_sha", "torch", "driver"] {
            let (x, y) = (node_axis("main", axis), node_axis("sub", axis));
            parity.set(axis, dict(vec![("verdict", vs(verdict(&x, &y))), ("main", x), ("sub", y)]));
        }
        let (la, lb) = (identity(&ledgers[0], &exempt)?, identity(&ledgers[1], &exempt)?);
        let (va, vb) = (opt(la), opt(lb));
        parity.set(
            "build_ledger",
            dict(vec![
                ("verdict", vs(verdict(&va, &vb))),
                ("main_identity_sha256", va.clone()),
                ("sub_identity_sha256", vb.clone()),
                ("identity_exempt_build_args", PyValue::List(exempt.iter().map(|x| vs(x)).collect())),
                ("compared", vs("dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha")),
            ]),
        );
        let parity_v = PyValue::Dict(parity);
        for axis in ["vllm_sha", "torch", "driver", "build_ledger_identity"] {
            let key = if axis == "build_ledger_identity" { "build_ledger" } else { axis };
            let v = get(get(&parity_v, key).unwrap(), "verdict").cloned().unwrap_or(PyValue::None);
            let vtext = match &v {
                PyValue::Str(s) => s.to_utf8().unwrap_or_default(),
                _ => String::new(),
            };
            if vtext != "equal" && vtext != "differ" {
                continue;
            }
            for (role, other) in [("main", "sub"), ("sub", "main")] {
                let pick = |r: &str| if axis == "build_ledger_identity" { if r == "main" { va.clone() } else { vb.clone() } } else { node_axis(r, axis) };
                checks.push(dict(vec![
                    ("axis", vs(axis)),
                    ("node", vs(role)),
                    ("observed", pick(role)),
                    ("expected", pick(other)),
                    ("expected_source", vs(&format!("parity:{other}"))),
                    ("phase", vs("serve")),
                    ("verdict", v.clone()),
                ]));
            }
        }
        doc.set("nodes", nodes_v);
        parity_out = Some(parity_v.clone());
        doc.set("parity", parity_v);
    }
    doc.set("checks", PyValue::List(checks));
    write_doc(out, &PyValue::Dict(doc))?;
    let mut summary: Vec<String> = Vec::new();
    if let Some(PyValue::Dict(p)) = parity_out.as_ref().map(PyValue::peel) {
        let mut keys: Vec<(String, String)> = p
            .iter()
            .map(|(k, v)| (k.to_utf8().unwrap_or_default(), get(v, "verdict").and_then(|x| if let PyValue::Str(s) = x { s.to_utf8() } else { None }).unwrap_or_default()))
            .collect();
        keys.sort_by(|x, y| x.0.chars().map(u32::from).cmp(y.0.chars().map(u32::from)));
        summary = keys.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    }
    let line = format!("[mn] 노드 정합 attestation v2({phase}) → {} {}", relpath(out), summary.join(" "));
    Ok(line.trim_end_matches(py_space).to_string())
}

/// `inline_json attestation-write <out> <config> <image> <phase> <cap> <rows> <sf> <main_id> <sub_id>` (env `SF_NON_IDENTITY`).
pub fn attestation_write(args: &[String]) -> i32 {
    match attestation(args) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            eprintln!("[attestation-write] {e}");
            1
        }
    }
}

/// `num(v)`: `int(v) if v.isdigit() else None`.
fn num(v: &str) -> PyValue {
    if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
        return PyValue::None;
    }
    let t = v.trim_start_matches('0');
    pyjson::loads(if t.is_empty() { "0" } else { t }).unwrap_or(PyValue::None)
}

/// `inline_json serve-proof <out> <config> <ready> <http> <verdict> <evidence> <clen> <colen> <rlen> <fr> <image> <mc> <att>`.
pub fn serve_proof(a: &[String]) -> i32 {
    if a.len() < 13 {
        eprintln!("[serve-proof] ValueError: not enough values to unpack (expected 13)");
        return 1;
    }
    let (out, config, ready, http, verdict, evidence, clen, colen, rlen, fr, image, mc, att) =
        (&a[0], &a[1], &a[2], &a[3], &a[4], &a[5], &a[6], &a[7], &a[8], &a[9], &a[10], &a[11], &a[12]);
    let content_len = if verdict == "not-attempted" {
        PyValue::None
    } else if verdict == "pass" && evidence == "chat.content" {
        num(clen)
    } else if verdict == "pass" && evidence == "v1.completions" {
        num(colen)
    } else {
        PyValue::int(0)
    };
    let or_none = |s: &str| if s.is_empty() { PyValue::None } else { vs(s) };
    let doc = dict(vec![
        ("schema_version", PyValue::int(1)),
        ("kind", vs("multinode_serve_proof")),
        ("config", vs(config)),
        ("topology", vs("multi")),
        ("ready", PyValue::Bool(ready == "1")),
        ("health_http", num(http)),
        ("inference_verdict", vs(verdict)),
        ("evidence", or_none(evidence)),
        ("content_len", content_len),
        ("chat_content_len", num(clen)),
        ("completion_text_len", num(colen)),
        ("reasoning_len", num(rlen)),
        ("finish_reason", or_none(fr)),
        ("image_tag", or_none(image)),
        ("master_container", or_none(mc)),
        ("attestation_ref", or_none(att)),
        ("provenance", vs("measured(multinode_serve_smoke)")),
    ]);
    match write_doc(out, &doc) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[serve-proof] {e}");
            1
        }
    }
}
