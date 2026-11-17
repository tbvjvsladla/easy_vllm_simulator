//! `inline_json` — the small JSON readers that the entry-point shells (`sync_branches.sh`, `sync_to_sub.sh`,
//! `multinode_serve_smoke.sh`, `single_serve_down.sh`) used to run as inline `python3 -c` / `python3 - <<PY`.
//! Each subcommand keeps the stdout bytes and exit status of the heredoc it replaced (failure → the same fallback text).
//! Delivered to the sub by the main node like `sub_runner`; no Python and no cargo is needed where it runs.
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::{Read, Write};
use std::process::ExitCode;

fn out_bytes(s: &PyStr) -> Vec<u8> {
    match s.to_utf8() {
        Some(t) => t.into_bytes(),
        None => s.to_backslashreplace_bytes(),
    }
}

fn emit(text: &str) {
    let mut o = std::io::stdout();
    let _ = o.write_all(text.as_bytes());
    let _ = o.flush();
}

fn emit_py(s: &PyStr, newline: bool) {
    let mut o = std::io::stdout();
    let _ = o.write_all(&out_bytes(s));
    if newline {
        let _ = o.write_all(b"\n");
    }
    let _ = o.flush();
}

fn read_stdin_json() -> Option<PyValue> {
    let mut b = Vec::new();
    std::io::stdin().read_to_end(&mut b).ok()?;
    pyjson::loads_text(&pyjson::decode_stdin_text(&b)).ok()
}

fn read_file_json(path: &str) -> Option<PyValue> {
    let b = std::fs::read(path).ok()?;
    let t = pyjson::decode_file_text(&b).ok()?;
    pyjson::loads_text(&t).ok()
}

/// `d.get(key)` on a dict; `None` when `d` is not a dict (Python would raise AttributeError).
fn get<'a>(d: &'a PyValue, key: &str) -> Option<Option<&'a PyValue>> {
    match d.peel() {
        PyValue::Dict(m) => Some(m.get(key).map(|v| v.peel())),
        _ => None,
    }
}

fn truthy(v: Option<&PyValue>) -> Option<&PyValue> {
    v.filter(|x| x.is_truthy())
}

/// `",".join(v)` — only a list/tuple of str (or a str, joined per character) can be joined.
fn join_strs(v: &PyValue, sep: &str) -> Option<String> {
    match v.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => {
            let mut parts = Vec::new();
            for i in items {
                match i.peel() {
                    PyValue::Str(s) => parts.push(String::from_utf8(out_bytes(s)).ok()?),
                    _ => return None,
                }
            }
            Some(parts.join(sep))
        }
        PyValue::Str(s) => {
            let t = s.to_utf8()?;
            Some(t.chars().map(String::from).collect::<Vec<_>>().join(sep))
        }
        _ => None,
    }
}

fn str_of(v: &PyValue) -> String {
    String::from_utf8_lossy(&out_bytes(&v.display_text())).into_owned()
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn cmd_allowed() -> i32 {
    let ok = read_stdin_json().map_or(false, |d| matches!(get(&d, "allowed"), Some(Some(PyValue::Bool(true)))));
    emit(if ok { "true\n" } else { "false\n" });
    0
}

fn gate_unreadable(action: &str, code: &str, msg: &str, mode: Option<&str>) {
    let mode = mode.map_or("null".to_string(), json_str);
    emit(&format!(
        "{{\n  \"action\": {},\n  \"allowed\": false,\n  \"authorization_state\": null,\n  \"exit_code\": 2,\n  \"identity\": null,\n  \"messages\": {{\n    {}: {}\n  }},\n  \"mode\": {},\n  \"reason_codes\": [\n    {}\n  ],\n  \"schema_version\": 1,\n  \"task_class\": null\n}}\n",
        json_str(action), json_str(code), json_str(msg), mode, json_str(code)
    ));
}

fn cmd_gate_unreadable_branches(a: &[String]) -> i32 {
    if a.len() != 3 {
        eprintln!("usage: inline_json gate-unreadable-branches MODE MANIFEST EXIT");
        return 2;
    }
    let mode = if a[0] == "experimental" || a[0] == "promotion" { Some(a[0].as_str()) } else { None };
    let msg = format!("completion_gate.sh authorize (exit={}) did not produce parseable JSON stdout", a[2]);
    gate_unreadable("sync_branches", "SYNC_BRANCHES_GATE_OUTPUT_UNREADABLE", &msg, mode);
    0
}

fn cmd_gate_unreadable_sub() -> i32 {
    gate_unreadable("sync_to_sub", "SYNC_TO_SUB_GATE_OUTPUT_UNREADABLE", "completion gate produced no parseable JSON", None);
    0
}

fn cmd_divergence_summary() -> i32 {
    const FALLBACK: &str = "unreadable\t-\t-\t-\t0";
    let Some(d0) = read_stdin_json() else {
        emit("unreadable\t-\t-\t-\n");
        return 0;
    };
    let d = if d0.is_truthy() { d0 } else { PyValue::Dict(Default::default()) };
    let line = (|| -> Option<String> {
        let status = or_str(get(&d, "status")?, "unknown")?;
        let basis = or_str(get(&d, "basis")?, "-")?;
        let base = or_str(get(&d, "base")?, "-")?;
        let base12: String = base.chars().take(12).collect();
        let sp = truthy(get(&d, "sync_point")?);
        let spd = sp.cloned().unwrap_or_else(|| PyValue::Dict(Default::default()));
        let sps = or_str(get(&spd, "status")?, "-")?;
        let n = match truthy(get(&d, "absorbed")?) {
            None => 0,
            Some(PyValue::List(l)) | Some(PyValue::Tuple(l)) => l.len(),
            Some(PyValue::Dict(m)) => m.len(),
            Some(PyValue::Str(s)) => s.len(),
            _ => return None,
        };
        Some(format!("{status}\t{basis}\t{base12}\t{sps}\t{n}"))
    })();
    emit(&format!("{}\n", line.as_deref().unwrap_or(FALLBACK)));
    0
}

/// A value that Python would have to use as `str` (a later `"\t".join` / slice): only str works.
fn or_str(v: Option<&PyValue>, default: &str) -> Option<String> {
    match truthy(v) {
        None => Some(default.to_string()),
        Some(x) => str_text(x),
    }
}

fn str_text(v: &PyValue) -> Option<String> {
    match v.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}

fn cmd_parity_summary() -> i32 {
    let Some(d) = read_stdin_json() else { return 1 };
    let res = (|| -> Option<String> {
        let verdict = or_str(get(&d, "verdict")?, "-")?;
        let topo = or_str(get(&d, "topology")?, "-")?;
        let codes = match truthy(get(&d, "reason_codes")?) {
            Some(v) => join_strs(v, ",")?,
            None => String::new(),
        };
        let codes = if codes.is_empty() { "-".to_string() } else { codes };
        let judged = match truthy(get(&d, "legs_judged")?) {
            Some(v) => join_strs(v, ",")?,
            None => String::new(),
        };
        let checked = match get(&d, "legs_checked")? {
            Some(v) => str_of(v),
            None => "None".to_string(),
        };
        let mut out = format!("{verdict}\t{topo}\t{codes}\t{judged}({checked}/4)\n");
        match truthy(get(&d, "reasons")?) {
            None => {}
            Some(PyValue::List(l)) | Some(PyValue::Tuple(l)) => {
                for r in l {
                    out.push_str(&str_of(r));
                    out.push('\n');
                }
            }
            _ => return None,
        }
        Some(out)
    })();
    match res {
        Some(t) => {
            emit(&t);
            0
        }
        None => 1,
    }
}

fn cmd_delivery_source() -> i32 {
    let src = read_stdin_json().and_then(|d| {
        let dp = truthy(get(&d, "delivery_plane")?);
        let dpd = dp.cloned().unwrap_or_else(|| PyValue::Dict(Default::default()));
        let s = truthy(get(&dpd, "source")?)?;
        Some(str_of(s))
    });
    emit(&format!("{}\n", src.as_deref().unwrap_or("unknown")));
    0
}

fn cmd_propagation_fields() -> i32 {
    let Some(d) = read_stdin_json() else { return 0 };
    let Some(pa) = get(&d, "propagation_scope") else { return 1 };
    let Some(pa) = truthy(pa) else { return 0 };
    let line = (|| -> Option<String> {
        let field = |k: &str| -> Option<String> {
            match get(pa, k)?? {
                PyValue::Str(s) => s.to_utf8(),
                _ => None,
            }
        };
        let planes = join_strs(get(pa, "planes")??, ",")?;
        Some([field("status")?, field("topology")?, field("node_id")?, field("ssh_host")?, field("work_dir")?, planes].join("\t"))
    })();
    if let Some(l) = line {
        emit(&format!("{l}\n"));
    }
    0
}

fn py_len_stripped(v: Option<&PyValue>) -> Option<(usize, PyStr)> {
    match truthy(v) {
        None => Some((0, PyStr::new(""))),
        Some(PyValue::Str(s)) => {
            let t = s.strip();
            Some((t.len(), t))
        }
        _ => None,
    }
}

fn cmd_chat_verdict(a: &[String]) -> i32 {
    let res = (|| -> Option<String> {
        let d = read_file_json(a.first()?)?;
        let choices = get(&d, "choices")??;
        let c = match choices {
            PyValue::List(l) => l.first()?.peel(),
            _ => return None,
        };
        let m0 = truthy(get(c, "message")?);
        let m = m0.cloned().unwrap_or_else(|| PyValue::Dict(Default::default()));
        let (clen, _) = py_len_stripped(get(&m, "content")?)?;
        let r = truthy(get(&m, "reasoning")?).or(get(&m, "reasoning_content")?);
        let (rlen, _) = py_len_stripped(r)?;
        let v = if clen > 0 { "content" } else if rlen > 0 { "reasoning-only" } else { "empty" };
        let fr = match get(c, "finish_reason")? {
            Some(x) => str_of(x),
            None => "None".to_string(),
        };
        Some(format!("{v} {clen} {rlen} {fr}"))
    })();
    emit(&format!("{}\n", res.as_deref().unwrap_or("unparsable 0 0 none")));
    0
}

fn cmd_completions_len(a: &[String]) -> i32 {
    let n = (|| -> Option<usize> {
        let d = read_file_json(a.first()?)?;
        let c = match get(&d, "choices")?? {
            PyValue::List(l) => l.first()?.peel(),
            _ => return None,
        };
        py_len_stripped(get(c, "text")?).map(|x| x.0)
    })();
    emit(&format!("{}\n", n.unwrap_or(0)));
    0
}

fn os_error_text(e: &std::io::Error, path: &str) -> String {
    let t = e.to_string();
    let base = t.split(" (os error").next().unwrap_or(&t).to_string();
    match e.raw_os_error() {
        Some(n) => format!("[Errno {n}] {base}: '{path}'"),
        None => base,
    }
}

fn cmd_campaign_budget(a: &[String]) -> i32 {
    if a.len() != 2 {
        eprintln!("usage: inline_json campaign-budget REPO KEY");
        return 2;
    }
    let (repo, key) = (&a[0], &a[1]);
    let Ok(raw) = std::fs::read(format!("{repo}/campaigns/ACTIVE")) else { return 0 };
    let Ok(camp_t) = pyjson::decode_file_text(&raw) else { return 1 };
    let camp = out_bytes(&camp_t.strip());
    let camp = String::from_utf8_lossy(&camp).into_owned();
    if camp.is_empty() || camp == "_bootstrap" {
        return 0;
    }
    let path = format!("{repo}/campaigns/{camp}/campaign.yaml");
    let fail = |why: String| {
        eprintln!("[mn] ⚠ 활성 캠페인 선언을 읽지 못했다({path}: {why}) — 선언값 없이 진행(아래 게이트가 판정)");
        0
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return fail(os_error_text(&e, &path)),
    };
    let doc = match pyjson::decode_file_text(&bytes) {
        Ok(t) => match pyjson::loads_text(&t) {
            Ok(v) => v,
            Err(e) => return fail(e.to_string()),
        },
        Err(e) => return fail(e.to_string()),
    };
    let Some(b) = get(&doc, "budgets") else { return 1 };
    let bd = b.filter(|x| x.is_truthy()).cloned().unwrap_or_else(|| PyValue::Dict(Default::default()));
    let Some(v) = get(&bd, key) else { return 1 };
    if let Some(PyValue::Int(n)) = v {
        if n.sign() == num_bigint::Sign::Plus {
            emit(&format!("{n}\n"));
        }
    }
    0
}

fn cmd_open_sessions(a: &[String]) -> i32 {
    let Some(node_dir) = a.first() else {
        eprintln!("usage: inline_json open-sessions NODE_DIR");
        return 2;
    };
    let dir = format!("{node_dir}/sessions");
    let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(&dir)
        .map(|rd| rd.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    use std::os::unix::ffi::OsStrExt;
    names.retain(|n| {
        let b = n.as_bytes();
        !b.starts_with(b".") && b.ends_with(b".json")
    });
    names.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    for n in names {
        let path = std::path::Path::new(&dir).join(&n);
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(t) = pyjson::decode_file_text(&bytes) else { continue };
        let Ok(doc) = pyjson::loads_text(&t) else { continue };
        let Some(stopped) = get(&doc, "stopped_utc") else { return 1 };
        if matches!(stopped, None | Some(PyValue::None)) {
            let id = truthy(get(&doc, "session_id").flatten());
            match id {
                Some(v) => emit_py(&v.display_text(), true),
                None => {
                    let stem = n.as_bytes();
                    let stem = &stem[..stem.len() - 5];
                    let mut o = std::io::stdout();
                    let _ = o.write_all(stem);
                    let _ = o.write_all(b"\n");
                }
            }
        }
    }
    0
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.to_string() } else { format!("{s}{}", " ".repeat(w - n)) }
}

fn pystr_val(s: &str) -> PyValue {
    PyValue::str(s)
}

/// `container_inventory.sh` body: env IMG_RAW / PS_RAW / REPO / TOPO / FILTER / FMT in, the text or JSON inventory out.
fn cmd_inventory() -> i32 {
    use std::collections::BTreeMap;
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let (repo, topo, filt, fmt) = (env("REPO"), env("TOPO"), env("FILTER"), env("FMT"));
    let (img_raw, ps_raw) = (env("IMG_RAW"), env("PS_RAW"));
    let topos: Vec<&str> = if topo == "both" { vec!["single", "multi"] } else { vec![topo.as_str()] };
    let mut refs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &topos {
        let d = format!("{repo}/output/{t}/envs");
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names {
            if !name.starts_with(".env.") { continue; }
            let Ok(bytes) = std::fs::read(format!("{d}/{name}")) else { continue };
            for line in String::from_utf8_lossy(&bytes).split('\n') {
                if let Some(rest) = line.strip_prefix("IMAGE_TAG=") {
                    refs.entry(rest.trim().to_string()).or_default().push(format!("{t}/{name}"));
                    break;
                }
            }
        }
    }
    struct Img { r: String, iid: String, digest: Option<String>, created: String, size: String, by: Vec<String> }
    let mut images: Vec<Img> = Vec::new();
    for line in img_raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 5 { continue; }
        let (r, iid, digest, created, size) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
        if !filt.is_empty() && !r.contains(&filt) { continue; }
        let mut by = refs.get(r).cloned().unwrap_or_default();
        by.sort();
        images.push(Img {
            r: r.into(), iid: iid.into(),
            digest: if digest == "<none>" || digest.is_empty() { None } else { Some(digest.into()) },
            created: created.into(), size: size.into(), by,
        });
    }
    images.sort_by(|a, b| a.r.cmp(&b.r));
    struct Run { name: String, image: String, status: String, ports: String, cand: bool }
    let mut running: Vec<Run> = Vec::new();
    for line in ps_raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 4 { continue; }
        let low = parts[0].to_lowercase();
        running.push(Run {
            name: parts[0].into(), image: parts[1].into(), status: parts[2].into(), ports: parts[3].into(),
            cand: low.contains("serv") || low.contains("vllm"),
        });
    }
    let have: std::collections::HashSet<&str> = images.iter().map(|i| i.r.as_str()).collect();
    let missing: Vec<(&String, &Vec<String>)> = refs.iter().filter(|(t, _)| !have.contains(t.as_str())).collect();

    if fmt == "json" {
        let list = |v: &[String]| PyValue::List(v.iter().map(|x| pystr_val(x)).collect());
        let mut o = agent_runtime_support::pyvalue::PyDict::new();
        o.set("schema_version", PyValue::int(1));
        o.set("topologies", PyValue::List(topos.iter().map(|x| pystr_val(x)).collect()));
        o.set("filter", pystr_val(&filt));
        let mut imgs = Vec::new();
        for i in &images {
            let mut e = agent_runtime_support::pyvalue::PyDict::new();
            e.set("ref", pystr_val(&i.r));
            e.set("image_id", pystr_val(&i.iid));
            e.set("digest", i.digest.as_deref().map_or(PyValue::None, pystr_val));
            e.set("digest_note", pystr_val(if i.digest.is_none() { "local-build(no registry identity)" } else { "registry" }));
            e.set("created", pystr_val(&i.created));
            e.set("size_reported", pystr_val(&i.size));
            e.set("size_note", pystr_val("노드 간 비교 무효(storage driver 차이) — 신원은 digest 가 답한다"));
            e.set("referenced_by", list(&i.by));
            e.set("referenced", PyValue::Bool(!i.by.is_empty()));
            imgs.push(PyValue::Dict(e));
        }
        o.set("images", PyValue::List(imgs));
        let mut runs = Vec::new();
        for r in &running {
            let mut e = agent_runtime_support::pyvalue::PyDict::new();
            e.set("name", pystr_val(&r.name));
            e.set("image", pystr_val(&r.image));
            e.set("status", pystr_val(&r.status));
            e.set("ports", pystr_val(&r.ports));
            e.set("is_serve_candidate", PyValue::Bool(r.cand));
            runs.push(PyValue::Dict(e));
        }
        o.set("running", PyValue::List(runs));
        let mut miss = Vec::new();
        for (t, srcs) in &missing {
            let mut e = agent_runtime_support::pyvalue::PyDict::new();
            e.set("image_tag", pystr_val(t));
            e.set("referenced_by", list(srcs));
            miss.push(PyValue::Dict(e));
        }
        o.set("referenced_but_absent", PyValue::List(miss));
        match PyValue::Dict(o).dumps_pretty() {
            Ok(t) => emit_py(&t, true),
            Err(_) => return 1,
        }
        return 0;
    }
    let mut out = String::new();
    out.push_str(&format!("[inventory] 빌드된 이미지 (filter={filt})\n"));
    for e in &images {
        let mark = if e.by.is_empty() { "미참조" } else { "사용중" };
        out.push_str(&format!("  {} {}  {} {}\n", pad(&e.r, 52), e.iid, pad(mark, 8), e.created));
        if !e.by.is_empty() {
            out.push_str(&format!("      ← {}\n", e.by.join(", ")));
        }
    }
    if images.is_empty() { out.push_str("  (없음)\n"); }
    out.push_str("[inventory] 서빙 중 컨테이너\n");
    for c in &running {
        out.push_str(&format!("  {} {} {}\n", pad(&c.name, 46), pad(&c.image, 52), c.status));
    }
    if running.is_empty() { out.push_str("  (없음)\n"); }
    if !missing.is_empty() {
        out.push_str("[inventory] ⚠ envfile 이 가리키는데 이미지가 없다(기동 시 실패한다):\n");
        for (t, srcs) in &missing {
            out.push_str(&format!("  {} ← {}\n", pad(t, 52), srcs.join(", ")));
        }
    }
    out.push_str("[inventory] 크기는 싣지 않는다 — storage driver 가 다르면 같은 내용도 다른 값이 나온다(신원=digest).\n");
    emit(&out);
    0
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("allowed") => cmd_allowed(),
        Some("gate-unreadable-branches") => cmd_gate_unreadable_branches(rest),
        Some("gate-unreadable-sub") => cmd_gate_unreadable_sub(),
        Some("divergence-summary") => cmd_divergence_summary(),
        Some("parity-summary") => cmd_parity_summary(),
        Some("delivery-source") => cmd_delivery_source(),
        Some("propagation-fields") => cmd_propagation_fields(),
        Some("chat-verdict") => cmd_chat_verdict(rest),
        Some("completions-len") => cmd_completions_len(rest),
        Some("campaign-budget") => cmd_campaign_budget(rest),
        Some("open-sessions") => cmd_open_sessions(rest),
        Some("inventory") => cmd_inventory(),
        Some("attestation-write") => upstream_runtime::mn_attest::attestation_write(rest),
        Some("serve-proof") => upstream_runtime::mn_attest::serve_proof(rest),
        _ => {
            eprintln!("usage: inline_json {{allowed|gate-unreadable-branches|gate-unreadable-sub|divergence-summary|parity-summary|delivery-source|propagation-fields|chat-verdict|completions-len|campaign-budget|open-sessions|inventory|attestation-write|serve-proof}} ...");
            2
        }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
