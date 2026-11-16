//! Native `campaign_template_validator.py --acceptance INSTANCE`: the acceptance predicates P4 (the sub instruction went out
//! before the main started), P5 (the sub wrote its own progress) and P6 (every observable assigned cell states where its
//! lockset came from). The other modes of the validator (`--campaign`, `--instance`, `--template`, `--selftest`) stay with
//! the owner program, which the launcher reaches through the explicit compatibility route.
//!
//! Output is the owner's: observation lines first on standard output (`  <note>`), then either `PASS` on standard output or
//! `FAIL (n)` with one `  - <problem>` line each on standard error (status 1).

use crate::campaign_write::{
    assigned_cells, assignments_of, dict_of, py, get_text, list_of, lockset_observable, lockset_provenance_reason, py_repr,
    read_declaration, text_of, tuple_repr, Ctx, Sinks, FILL,
};
use crate::pyjson;
use crate::pyvalue::{PyDict, PyValue};
use std::path::{Path, PathBuf};

const RESERVED_CELL_NAMES: [&str; 2] = ["_cell", "_node"];
// The lockset provenance vocabulary: its single owner is this validator (the retired `campaign_template_validator.py` held it
// as `LOCKSET_PROVENANCE`/`LOCKSET_KNOB_SOURCES`). The explorer (`recipe simulate`) reads these two literal tables from this
// source file, so they stay one literal each, one knob per line.
pub const LOCKSET_PROVENANCE: [&str; 2] = ["explorer-phase2", "hand-authored"];
pub const KNOB_SOURCES: [(&str, &[&str]); 3] = [
    ("batch_source", &["declared-requirement", "kv-fit-measured", "hand-lever"]),
    ("gmu_source", &["target_gmu", "hand"]),
    ("kv_source", &["measured-clamp", "hand"]),
];

fn read_object(path: &Path) -> Option<PyDict> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    match pyjson::loads(&text).ok()? {
        PyValue::Dict(d) => Some(d),
        _ => None,
    }
}

/// `declared_node_ids(camp_dir, role)`.
fn declared_node_ids(camp_dir: &Path, role: Option<&str>) -> Vec<String> {
    let decl = read_declaration(camp_dir);
    let mut out = Vec::new();
    for node in list_of(decl.get("nodes").unwrap_or(&PyValue::None)).into_iter().flatten().filter_map(dict_of) {
        let Some(id) = get_text(node, "node_id") else { continue };
        if id.is_empty() || id.contains(FILL) {
            continue;
        }
        if let Some(role) = role {
            if get_text(node, "role").as_deref() != Some(role) {
                continue;
            }
        }
        out.push(id);
    }
    out
}

fn sorted_json_files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().ends_with(suffix)).unwrap_or(false))
        .collect();
    out.sort();
    out
}

/// `_phase_docs(camp_dir, node)`: `(file name, object)` of every readable `*.status.json`.
fn phase_docs(camp_dir: &Path, node: &str) -> Vec<(String, PyDict)> {
    sorted_json_files(&camp_dir.join("phases").join(node), ".status.json")
        .into_iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            read_object(&p).map(|d| (name, d))
        })
        .collect()
}

fn first_start(doc: &PyDict) -> Option<String> {
    ["first_started_utc", "started_utc"]
        .iter()
        .find_map(|k| get_text(doc, k).filter(|v| !v.is_empty()))
}

fn relay_ledgers(camp_dir: &Path) -> Vec<PyDict> {
    sorted_json_files(&camp_dir.join("relay"), ".json")
        .into_iter()
        .filter(|p| p.file_name().map(|n| n != "pending_hitl.json").unwrap_or(true))
        .filter_map(|p| read_object(&p))
        .collect()
}

fn predicate_p4(camp_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let subs = declared_node_ids(camp_dir, Some("sub"));
    let mains = declared_node_ids(camp_dir, Some("main"));
    if subs.is_empty() || mains.is_empty() {
        return problems;
    }
    let starts: Vec<String> = mains
        .iter()
        .flat_map(|m| phase_docs(camp_dir, m))
        .filter_map(|(_, d)| first_start(&d))
        .collect();
    let Some(main_start) = starts.iter().min().cloned() else {
        return vec![format!(
            "P4: 메인({}) phase 에 시각이 하나도 없다 — 언제 착수했는지 물을 수단이 없다(writer 호출부의 --started-utc 누락)",
            mains.join(", ")
        )];
    };
    let ledgers = relay_ledgers(camp_dir);
    for sub_id in &subs {
        let build: Vec<&PyDict> = ledgers
            .iter()
            .filter(|l| {
                get_text(l, "campaign_node").as_deref() == Some(sub_id.as_str())
                    && get_text(l, "campaign_context_kind").as_deref() == Some("build")
            })
            .collect();
        if build.is_empty() {
            problems.push(format!(
                "P4 {sub_id}: 빌드 context 원장이 없다 — 서브 지시서 선발급의 관측 자리가 비었다(relay 가 campaign_node·campaign_context_kind 를 원장에 적어야 한다)"
            ));
            continue;
        }
        let attempts: Vec<String> = build
            .iter()
            .flat_map(|l| {
                let items = l.get("attempts").filter(|v| v.is_truthy()).and_then(list_of);
                items.into_iter().flatten().filter_map(dict_of).filter_map(|a| get_text(a, "started_utc"))
            })
            .collect();
        let Some(first) = attempts.iter().min() else {
            problems.push(format!(
                "P4 {sub_id}: 빌드 context 는 열렸는데 attempt 가 0 이다 — 원장만 있고 아무것도 나가지 않았다"
            ));
            continue;
        };
        if *first >= main_start {
            problems.push(format!(
                "P4 {sub_id}: 서브 빌드 지시서({first})가 메인 첫 착수({main_start})보다 늦다 — 이것이 '순차로 돌았다'의 관측 가능한 정의다"
            ));
        }
    }
    problems
}

fn predicate_p5(camp_dir: &Path, briefs: &[PathBuf]) -> Vec<String> {
    let mut problems = Vec::new();
    let mains = declared_node_ids(camp_dir, Some("main"));
    for sub_id in declared_node_ids(camp_dir, Some("sub")) {
        let docs = phase_docs(camp_dir, &sub_id);
        if docs.is_empty() {
            continue;
        }
        for (name, doc) in &docs {
            match get_text(doc, "authored_by").filter(|w| !w.is_empty()) {
                None => problems.push(format!(
                    "P5 {sub_id}/{name}: authored_by 가 없다 — 서브 진행표를 누가 적었는지 표시가 없으면 자기저작과 사후 재저작이 구분되지 않는다"
                )),
                Some(who) if mains.contains(&who) => problems.push(format!(
                    "P5 {sub_id}/{name}: authored_by={} — 메인이 서브의 진행표를 적었다(상향 회수는 문서기반이고, 회수분은 서브 저작으로 남는다)",
                    py_repr(&who)
                )),
                Some(_) => {}
            }
        }
        let stamps: Vec<String> = docs.iter().filter_map(|(_, d)| get_text(d, "ended_utc")).collect();
        if stamps.len() >= 2 && stamps.iter().all(|s| *s == stamps[0]) {
            problems.push(format!(
                "P5 {sub_id}: phase {}건의 ended_utc 가 모두 {} 로 같다 — 같은 초에 통째로 저작된 진행표는 관측이 아니라 회고다",
                stamps.len(),
                stamps[0]
            ));
        }
    }
    for brief in briefs {
        let shown = brief.to_string_lossy();
        let text = match std::fs::read(brief).map(String::from_utf8) {
            Ok(Ok(text)) => text,
            _ => continue,
        };
        let doc = match pyjson::loads(&text) {
            Ok(doc) => doc,
            Err(error) => {
                problems.push(format!("P5 brief 파손({shown}): {error}"));
                continue;
            }
        };
        let Some(doc) = dict_of(&doc) else { continue };
        if get_text(doc, "self_role").as_deref() != Some("sub") {
            let role = doc.get("self_role").map(PyValue::repr).unwrap_or_else(|| "None".to_string());
            problems.push(format!(
                "P5 brief {shown}: self_role={role} — 회수된 브리핑은 서브가 자기 인스턴스에서 낸 것이어야 한다"
            ));
        }
        if let Some(author) = get_text(doc, "authored_by") {
            if mains.contains(&author) {
                problems.push(format!("P5 brief {shown}: authored_by={} 가 메인이다", py_repr(&author)));
            }
        }
    }
    problems
}

/// `discover_briefs(camp_dir)`: this campaign's briefs in the standing docs mirror.
fn discover_briefs(ctx: &Ctx, camp_dir: &Path) -> Vec<PathBuf> {
    let mirror = ctx.root.join("sync_staging").join("sub_docs");
    if !mirror.is_dir() {
        return Vec::new();
    }
    let name = camp_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Ok(read) = std::fs::read_dir(mirror.join("logs")) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = read.filter_map(Result::ok).map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter()
        .map(|d| d.join("campaign_brief.json"))
        .filter(|p| p.is_file())
        .filter(|p| read_object(p).and_then(|d| get_text(&d, "campaign_id")).as_deref() == Some(name.as_str()))
        .collect()
}

fn knob_source_reasons(path: &Path) -> Vec<String> {
    let Some(doc) = read_object(path) else { return Vec::new() };
    let mut out = Vec::new();
    for (key, allowed) in KNOB_SOURCES {
        let Some(value) = doc.get(key) else { continue };
        if matches!(value.peel(), PyValue::None) {
            continue;
        }
        let known = text_of(value).map(|t| allowed.contains(&t.as_str())).unwrap_or(false);
        if !known {
            out.push(format!("{key}={} 가 목록 밖이다(허용 {})", value.repr(), tuple_repr(allowed)));
        }
    }
    out
}

/// `p6_observations(camp_dir)`: `(problems, notes)`.
fn p6_observations(ctx: &Ctx, camp_dir: &Path) -> (Vec<String>, Vec<String>) {
    let (mut problems, mut notes) = (Vec::new(), Vec::new());
    let decl = read_declaration(camp_dir);
    if decl.is_empty() {
        return (problems, notes);
    }
    for (node, _) in assignments_of(&decl) {
        let cells: Vec<String> = assigned_cells(&decl, Some(&node))
            .into_iter()
            .filter(|c| !RESERVED_CELL_NAMES.contains(&c.as_str()))
            .collect();
        if !lockset_observable(&decl, Some(&node)) {
            if !cells.is_empty() {
                notes.push(format!(
                    "P6 ⓘ 관측 대상 밖(서브 인스턴스 소관 · 메인은 서브 인스턴스를 읽지 않는다) node={node}: {}",
                    cells.join(", ")
                ));
            }
            continue;
        }
        for cell in cells {
            let lockset = camp_dir.join("cells").join(&cell).join("lockset.json");
            if let Some(why) = lockset_provenance_reason(ctx, &lockset) {
                problems.push(format!("P6 {cell}: {why}"));
                continue;
            }
            for extra in knob_source_reasons(&lockset) {
                notes.push(format!(
                    "P6 ⓘ {cell}: {extra} — 노브 출처 어휘가 갈라지면 대조가 성립하지 않는다(기재 · 차단 ✗)"
                ));
            }
        }
    }
    (problems, notes)
}

// ── P1-P3: the instance predicates (the purge gate reads them) ──────────────────────────────

const SWEEP_TO_CELL: [&str; 7] = [
    "measured", "serve_failed", "build_failed", "void", "pending", "measurement_void", "not_measured",
];

/// `_sweep_cell_records(camp_dir)`: `cell key -> (outcome, source file)`; the record with the later `ended_utc` wins.
fn sweep_cell_records(ctx: &Ctx, camp_dir: &Path) -> Vec<(String, (String, String))> {
    let mut out: Vec<(String, (String, String))> = Vec::new();
    let mut when: Vec<(String, String)> = Vec::new();
    for sweep in sorted_json_files(&camp_dir.join("sweeps"), ".json") {
        let Some(doc) = read_object(&sweep) else { continue };
        let cells = doc.get("cells").filter(|v| v.is_truthy()).and_then(list_of);
        for cell in cells.into_iter().flatten().filter_map(dict_of) {
            let pick = |keys: &[&str]| {
                keys.iter().find_map(|k| cell.get(k).filter(|v| v.is_truthy()).cloned())
            };
            let (Some(key), Some(outcome)) = (
                pick(&["cell_key", "cell_id", "config"]).and_then(|v| text_of(&v)),
                pick(&["cell_outcome", "outcome", "status"]).and_then(|v| text_of(&v)),
            ) else {
                continue;
            };
            let ended = get_text(cell, "ended_utc").unwrap_or_default();
            let known = when.iter().find(|(k, _)| *k == key).map(|(_, w)| w.clone()).unwrap_or_default();
            if out.iter().any(|(k, _)| *k == key) && ended < known {
                continue;
            }
            let value = (outcome, ctx.rel(&sweep));
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = value,
                None => out.push((key.clone(), value)),
            }
            match when.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = ended,
                None => when.push((key, ended)),
            }
        }
    }
    out
}

fn predicate_p1(ctx: &Ctx, camp_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let mut sweeps = sweep_cell_records(ctx, camp_dir);
    sweeps.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, (outcome, src)) in sweeps {
        let status_path = camp_dir.join("cells").join(&key).join("cell.status.json");
        if !SWEEP_TO_CELL.contains(&outcome.as_str()) {
            continue;
        }
        if !status_path.is_file() {
            problems.push(format!(
                "P1 {key}: sweep 은 '{outcome}' 인데 cell.status.json 이 없다 (출처 {src}) — 돌았는데 상태를 아무도 적지 않았다"
            ));
            continue;
        }
        let parsed = std::fs::read(&status_path).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
        let doc = match parsed {
            Some(Ok(doc)) => doc,
            Some(Err(error)) => {
                problems.push(format!("P1 {key}: cell.status.json 파손 — {error}"));
                continue;
            }
            None => {
                problems.push(format!("P1 {key}: cell.status.json 파손 — invalid utf-8"));
                continue;
            }
        };
        let have = dict_of(&doc).and_then(|d| d.get("cell_outcome").cloned()).unwrap_or(PyValue::None);
        if text_of(&have).as_deref() != Some(outcome.as_str()) {
            problems.push(format!(
                "P1 {key}: sweep='{outcome}' vs cell.status='{}' 불일치 (출처 {src}) — 두 자리가 갈라지면 재개가 이미 돈 셀을 다시 돈다",
                have.display_text().to_utf8().unwrap_or_default()
            ));
        }
    }
    problems
}

fn predicate_p2(camp_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let ep = camp_dir.join("evidence_pointers.json");
    if !ep.is_file() {
        return problems;
    }
    let Some(doc) = read_object(&ep) else { return problems };
    let nodes = declared_node_ids(camp_dir, None);
    let multi = nodes.len() > 1;
    let mut bench_nodes: Vec<(String, Vec<String>)> = Vec::new();
    let mut bench_total = 0usize;
    let pointers = doc.get("pointers").filter(|v| v.is_truthy()).and_then(list_of);
    for ptr in pointers.into_iter().flatten().filter_map(dict_of) {
        let kind = ptr.get("kind").map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or_else(|| "None".to_string());
        if kind != "certificate" && kind != "bench_report" {
            continue;
        }
        bench_total += 1;
        let path_text = ptr.get("path").map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or_else(|| "None".to_string());
        let add = |node: String, bench: &mut Vec<(String, Vec<String>)>| match bench.iter_mut().find(|(n, _)| *n == node) {
            Some(slot) => slot.1.push(path_text.clone()),
            None => bench.push((node, vec![path_text.clone()])),
        };
        match get_text(ptr, "node_id").filter(|n| !n.is_empty()) {
            Some(node) => add(node, &mut bench_nodes),
            None if multi => problems.push(format!(
                "P2 노드 태그 없는 벤치 증거: {path_text} — {}노드 캠페인에서는 어느 노드가 잰 것인지 알 수 없다(`--evidence-add --node <id>` 누락)",
                nodes.len()
            )),
            None if nodes.len() == 1 => add(nodes[0].clone(), &mut bench_nodes),
            None => {}
        }
    }
    if multi && bench_total > 0 && bench_nodes.is_empty() {
        problems.push(format!(
            "P2 벤치 증거 {bench_total}건이 **전부** 노드 태그가 없다 — 이 상태의 P2 는 아무 노드도 검사하지 않고 통과한다(공허 통과 금지 · 2026-09-08 재발 방지)"
        ));
    }
    bench_nodes.sort_by(|a, b| a.0.cmp(&b.0));
    for (node, paths) in bench_nodes {
        let (candidates, kind) = if nodes.contains(&node) {
            (vec![node.clone()], "declared")
        } else if node == "cluster" {
            (nodes.clone(), "derived")
        } else {
            (Vec::new(), "unknown")
        };
        if kind == "unknown" {
            let declared = if nodes.is_empty() { "없음".to_string() } else { nodes.join(", ") };
            problems.push(format!(
                "P2 {node}: 벤치 증거 {}건의 노드 태그가 선언 노드({declared})에도 파생 측정축(cluster)에도 없다 — 어느 노드가 잰 것인지 해소할 수 없다(오타 또는 미선언 노드)",
                paths.len()
            ));
            continue;
        }
        let mut seen: Vec<String> = Vec::new();
        let mut reflected = false;
        for cand in &candidates {
            let status_path = camp_dir.join("phases").join(cand).join("bench.status.json");
            if !status_path.is_file() {
                continue;
            }
            let parsed = std::fs::read(&status_path).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
            let st = match parsed {
                Some(Ok(PyValue::Dict(d))) => d,
                Some(Ok(_)) => PyDict::new(),
                Some(Err(error)) => {
                    problems.push(format!("P2 {cand}: bench.status.json 파손 — {error}"));
                    seen.push(cand.clone());
                    continue;
                }
                None => {
                    problems.push(format!("P2 {cand}: bench.status.json 파손 — invalid utf-8"));
                    seen.push(cand.clone());
                    continue;
                }
            };
            let state = st.get("state").cloned().unwrap_or(PyValue::None);
            let proof_ok = dict_of(&st.get("proof").cloned().unwrap_or(PyValue::None)).and_then(|p| p.get("ok").cloned()).unwrap_or(PyValue::None);
            seen.push(format!("{cand}(state={} proof.ok={})", state.repr(), proof_ok.repr()));
            if text_of(&state).as_deref() == Some("done") && matches!(proof_ok.peel(), PyValue::Bool(true)) {
                reflected = true;
                break;
            }
        }
        if reflected {
            continue;
        }
        if seen.is_empty() {
            let where_ = if kind == "declared" {
                format!("phases/{}/bench.status.json", candidates[0])
            } else {
                "선언 노드 어디에도 bench.status.json".to_string()
            };
            problems.push(format!(
                "P2 {node}: 벤치 증거 {}건이 있는데 {where_} 이 없다 — 진행표가 증거보다 낡았다",
                paths.len()
            ));
            continue;
        }
        problems.push(format!(
            "P2 {node}: 벤치 증거가 도착했는데 bench phase 가 그것을 반영하지 않는다 — {} (진행표가 증거보다 낡았다)",
            seen.join("; ")
        ));
    }
    problems
}

fn predicate_p3(camp_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let path = camp_dir.join("campaign.yaml");
    if !path.is_file() {
        return problems;
    }
    let Some(decl) = read_object(&path) else { return problems };
    let nodes = decl.get("nodes").filter(|v| v.is_truthy()).and_then(list_of);
    for node in nodes.into_iter().flatten().filter_map(dict_of) {
        let Some(nid) = get_text(node, "node_id").filter(|n| !n.is_empty() && !n.contains(FILL)) else {
            continue;
        };
        let no_items = assignments_of(&decl)
            .into_iter()
            .find(|(n, _)| *n == nid)
            .map(|(_, items)| list_of(&items).map(|l| l.iter().filter(|x| dict_of(x).is_some()).count()).unwrap_or(0))
            .unwrap_or(0)
            == 0;
        if get_text(node, "topology").as_deref() == Some("multi")
            && get_text(node, "role").as_deref() == Some("sub")
            && no_items
        {
            continue;
        }
        if !camp_dir.join("phases").join(&nid).is_dir() {
            problems.push(format!(
                "P3 {nid}: campaign.yaml 이 선언한 노드인데 phases/{nid}/ 가 없다 — '안 돌았다'와 '돌았는데 아무도 안 적었다'가 구분되지 않는다"
            ));
        }
    }
    problems
}

/// `instance_predicates(camp_dir)`: P1, P2, P3.
pub fn instance_predicates(ctx: &Ctx, camp_dir: &Path) -> Vec<String> {
    let mut out = predicate_p1(ctx, camp_dir);
    out.extend(predicate_p2(camp_dir));
    out.extend(predicate_p3(camp_dir));
    out
}

// ── --campaign / --instance / --template ────────────────────────────────────────────────────

/// A verdict the owner raises as `CampaignContractFailure` (printed as `FAIL <text>`, status 1) or an exception only the
/// owner can render (a schema it cannot read): the caller hands the vector to the owner.
pub enum Failure {
    Contract(String),
    Owner,
}

type V<T> = Result<T, Failure>;

fn die<T>(text: impl Into<String>) -> V<T> {
    Err(Failure::Contract(text.into()))
}

fn find_fill_paths(node: &PyValue, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    match node.peel() {
        PyValue::Dict(d) => {
            for (k, v) in d.iter() {
                let key = k.to_utf8().unwrap_or_default();
                let here = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
                if key.contains(FILL) {
                    out.push(format!("{here} (키)"));
                }
                out.extend(find_fill_paths(v, &here));
            }
        }
        PyValue::List(items) => {
            for (i, v) in items.iter().enumerate() {
                out.extend(find_fill_paths(v, &format!("{path}[{i}]")));
            }
        }
        PyValue::Str(t) if t.to_utf8().map(|t| t.contains(FILL)).unwrap_or(false) => {
            out.push(if path.is_empty() { "$".to_string() } else { path.to_string() });
        }
        _ => {}
    }
    out
}

pub fn fill_rows(text: &str) -> Vec<usize> {
    text.lines().enumerate().filter(|(_, l)| l.contains(FILL)).map(|(i, _)| i + 1).collect()
}

pub fn strip_annotations(doc: &PyDict) -> PyDict {
    let mut out = PyDict::new();
    for (k, v) in doc.iter() {
        if !k.to_utf8().map(|s| s.starts_with('_')).unwrap_or(false) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// `scan_fill_file(path)`: where placeholders remain in one file.
fn scan_fill_file(path: &Path) -> Vec<String> {
    let text = match std::fs::read(path).map(String::from_utf8) {
        Ok(Ok(t)) => t,
        _ => return vec!["(읽기 실패: unreadable)".to_string()],
    };
    let rows = |t: &str| fill_rows(t).into_iter().map(|i| format!("{i}행")).collect::<Vec<_>>();
    if path.extension().map(|e| e == "json").unwrap_or(false) {
        return match pyjson::loads(&text) {
            Err(_) => rows(&text),
            Ok(doc) => {
                let doc = match doc.peel() {
                    PyValue::Dict(d) => PyValue::Dict(strip_annotations(d)),
                    _ => doc,
                };
                find_fill_paths(&doc, "")
            }
        };
    }
    rows(&text)
}

fn contract_value(value: &PyValue) -> Option<agent_runtime_support::pyvalue::PyValue> {
    let text = value.dumps_ascii().ok()?;
    agent_runtime_support::pyjson::loads(&text).ok()
}

/// `_validate_schema(doc, schema, where)`: `where: ('code', 'message')` per violation, as the owner formats the tuple.
fn validate_schema(ctx: &Ctx, doc: &PyValue, schema: &PyValue, where_: &str) -> V<Vec<String>> {
    // The engine is the Rust completion gate (policy-gate); its command surface in the tree is the launcher.
    let engine = ctx.root.join(".claude/policies/runtime/completion_gate.sh");
    if !engine.is_file() {
        return die(format!(
            "스키마 엔진이 없다: {} — 이 트리에는 검증기만 있고 엔진이 오지 않았다. 선언 검증은 메인 단일 창구이며(서브는 --from-slice 로 받은 선언을 소비한다), 이 트리에서 물을 수 있는 것은 --instance 의 P1~P3 이다.",
            ctx.rel(&engine)
        ));
    }
    let (Some(d), Some(s)) = (contract_value(doc), contract_value(schema)) else { return Err(Failure::Owner) };
    let violations = agent_runtime_support::schema::validate_against_schema(&d, &s).map_err(|_| Failure::Owner)?;
    Ok(violations
        .into_iter()
        .map(|(code, message)| format!("{where_}: ({}, {})", py_repr(&code), py_repr(&message)))
        .collect())
}

fn retired_hint_key_reasons(doc: &PyDict) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(items) = doc.get("hint_targets").and_then(list_of) {
        for (i, t) in items.iter().enumerate() {
            if dict_of(t).map(|d| d.contains_key("arch")).unwrap_or(false) {
                out.push(format!(
                    "hint_targets[{i}].arch 는 폐기된 키다 — 태그 이름은 발행 도구가 전량 파생한다(2026-09-21 D8) — 손으로 적은 arch 는 '파생 가능한데 손으로 적은 값'이다. 키를 지우고 승인은 `campaign_init.py --hint-approve` 로 적는다"
                ));
            }
        }
    }
    out
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `validate_campaign(campaign_path, strict_paths)`.
pub fn validate_campaign(ctx: &Ctx, campaign_path: &Path, strict_paths: bool) -> V<Vec<String>> {
    let name = name_of(campaign_path);
    if !campaign_path.is_file() {
        return Ok(vec![format!("campaign 선언 부재: {}", campaign_path.display())]);
    }
    let raw = std::fs::read(campaign_path).ok().and_then(|b| String::from_utf8(b).ok()).ok_or(Failure::Owner)?;
    let doc = match pyjson::loads(&raw) {
        Ok(d) => d,
        Err(error) => {
            let rows = fill_rows(&raw);
            let mut out = vec![format!("{name}: JSON 파손 — {error}")];
            if !rows.is_empty() {
                let shown: Vec<String> = rows.iter().take(10).map(|r| r.to_string()).collect();
                out.push(format!("{name}: 플레이스홀더 {FILL} 가 {}행에 남아 있다(행 [{}])", rows.len(), shown.join(", ")));
            }
            return Ok(out);
        }
    };
    let Some(doc) = dict_of(&doc) else {
        return Ok(vec![format!("{name}: 최상위가 객체가 아니다")]);
    };
    let doc = strip_annotations(doc);
    let doc_value = PyValue::Dict(doc.clone());
    let mut problems: Vec<String> = Vec::new();
    let fills = find_fill_paths(&doc_value, "");
    if !fills.is_empty() {
        problems.push(format!(
            "{name}: 플레이스홀더 {FILL} 가 {}곳에 남아 있다({})",
            fills.len(),
            fills.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    let schema_path = ctx.root.join("campaigns/_template/campaign.schema.json");
    if !schema_path.is_file() {
        return die(format!("campaign schema 부재: {}", schema_path.display()));
    }
    let schema_text = std::fs::read(&schema_path).ok().and_then(|b| String::from_utf8(b).ok()).ok_or(Failure::Owner)?;
    let schema = match pyjson::loads(&schema_text) {
        Ok(s) => s,
        Err(error) => return die(format!("campaign schema 파손({}): {error}", schema_path.display())),
    };
    problems.extend(validate_schema(ctx, &doc_value, &schema, &name)?);
    problems.extend(retired_hint_key_reasons(&doc));
    if !problems.is_empty() {
        return Ok(problems);
    }
    let camp_id = doc.get("id").cloned().unwrap_or(PyValue::None);
    let id_text = text_of(&camp_id);
    if matches!(id_text.as_deref(), Some("_template" | "_bootstrap")) {
        problems.push(format!("예약 id 는 캠페인 이름이 될 수 없다: {} (예약: ('_template', '_bootstrap'))", camp_id.repr()));
    }
    let parent_name = campaign_path.parent().map(name_of).unwrap_or_default();
    if let Some(id) = id_text.as_deref().filter(|i| !i.is_empty()) {
        if parent_name != id && parent_name != "_template" {
            problems.push(format!(
                "선언 id({})와 디렉터리 이름({})이 다르다 — 이름이 곧 증거 연결이다",
                py_repr(id),
                py_repr(&parent_name)
            ));
        }
    }
    let nodes: Vec<PyValue> = list_of(doc.get("nodes").unwrap_or(&PyValue::None)).cloned().unwrap_or_default();
    let node_ids: Vec<PyValue> = nodes.iter().map(|n| dict_of(n).and_then(|d| d.get("node_id").cloned()).unwrap_or(PyValue::None)).collect();
    let mut unique: Vec<&PyValue> = Vec::new();
    for id in &node_ids {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    if unique.len() != nodes.len() {
        problems.push("nodes[].node_id 가 중복이다 — 노드 정체성이 갈리지 않는다".to_string());
    }
    let budgets = doc.get("budgets").and_then(dict_of).cloned().unwrap_or_default();
    for key in ["smoke_budget_overhead_mib", "ready_max_seconds"] {
        let ok = match budgets.get(key).map(PyValue::peel) {
            Some(PyValue::Int(n)) => *n > 0.into(),
            _ => false,
        };
        if !ok {
            problems.push(format!("budgets.{key} 가 선언되지 않았다(0/기본값 금지 — 선언 없으면 무방비)"));
        }
    }
    let cv = doc.get("control_variables").and_then(dict_of).cloned().unwrap_or_default();
    for key in ["model", "vllm_version", "topology", "target_gpu"] {
        let shown = cv.get(key).map(|v| v.display_text().to_utf8().unwrap_or_default()).unwrap_or_default();
        if shown.trim().is_empty() {
            problems.push(format!("control_variables.{key} 공란 — 서브가 저장소 기본값으로 되돌아간다(2026-09-05 실측)"));
        }
    }
    problems.extend(hint_target_reasons(&doc));
    let cells_dir = campaign_path.parent().map(|p| p.join("cells")).unwrap_or_default();
    let assigns = assignments_of(&doc);
    if assigns.is_empty() {
        problems.push("assignments 가 비었다 — 배정이 없으면 어느 노드도 돌 것이 없다".to_string());
    }
    let mut owner: Vec<(String, String)> = Vec::new();
    for (node, items) in &assigns {
        if !node_ids.iter().any(|n| text_of(n).as_deref() == Some(node.as_str())) {
            problems.push(format!("assignments 의 노드 {} 가 nodes[] 에 없다", py_repr(node)));
        }
        let items: Vec<PyValue> = list_of(items).cloned().unwrap_or_default();
        for (i, item) in items.iter().enumerate() {
            let Some(item) = dict_of(item) else { continue };
            let mode = item.get("mode").cloned().unwrap_or_else(|| py("AUTO"));
            if text_of(&mode).as_deref() == Some("STAY") && i != items.len() - 1 {
                problems.push(format!(
                    "assignments[{}][{i}] 의 mode=STAY 가 마지막이 아니다 — STAY 는 벤치 뒤에도 서빙을 유지하므로 뒤에 남은 {}개 셀이 영원히 돌지 않는다",
                    py_repr(node),
                    items.len() - 1 - i
                ));
            }
            let Some(cell) = get_text(item, "cell").filter(|c| !c.is_empty()) else { continue };
            if RESERVED_CELL_NAMES.contains(&cell.as_str()) {
                problems.push(format!("assignments 에 틀 디렉터리 {} 가 들어 있다 — 틀은 실행 대상이 아니다", py_repr(&cell)));
                continue;
            }
            if let Some((_, prev)) = owner.iter().find(|(c, _)| *c == cell) {
                problems.push(format!(
                    "셀 {} 이 노드 {} 와 {} 에 이중 배정됐다 — 셀의 수행 노드는 하나다(증거 태그가 갈린다)",
                    py_repr(&cell),
                    py_repr(prev),
                    py_repr(node)
                ));
                continue;
            }
            owner.push((cell.clone(), node.clone()));
            let cfg = cells_dir.join(&cell).join("config.yaml");
            if !lockset_observable(&doc, Some(node)) {
                continue;
            }
            if strict_paths && !cfg.is_file() {
                problems.push(format!("배정된 셀 {} 입력이 없다: {}", py_repr(&cell), ctx.rel(&cfg)));
                continue;
            }
            if !strict_paths {
                continue;
            }
            for fname in ["config.yaml", "lockset.json"] {
                let f = cells_dir.join(&cell).join(fname);
                if !f.is_file() {
                    continue;
                }
                let where_ = scan_fill_file(&f);
                if !where_.is_empty() {
                    problems.push(format!(
                        "셀 {} 의 {fname} 에 {FILL} 가 {}곳 남아 있다({})",
                        py_repr(&cell),
                        where_.len(),
                        where_.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
        }
    }
    Ok(problems)
}

fn json_children(dir: &Path, pattern_dirs: bool, file: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() == pattern_dirs)
        .map(|p| p.join(file))
        .filter(|p| p.exists())
        .collect();
    out.sort();
    out
}

/// `validate_instance(camp_dir)`.
pub fn validate_instance(ctx: &Ctx, camp_dir: &Path) -> V<Vec<String>> {
    let mut problems = validate_campaign(ctx, &camp_dir.join("campaign.yaml"), true)?;
    let ep = camp_dir.join("evidence_pointers.json");
    if ep.is_file() {
        let where_ = scan_fill_file(&ep);
        if !where_.is_empty() {
            problems.push(format!(
                "evidence_pointers.json 에 {FILL} 가 {}곳 남아 있다({}) — 성장 배열은 빈 목록으로 출발한다",
                where_.len(),
                where_.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    let mut statuses: Vec<PathBuf> = Vec::new();
    if let Ok(read) = std::fs::read_dir(camp_dir.join("phases")) {
        for node_dir in read.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()) {
            statuses.extend(sorted_json_files(&node_dir, ".status.json"));
        }
    }
    statuses.sort();
    for status in statuses {
        let parent = status.parent().map(name_of).unwrap_or_default();
        if RESERVED_CELL_NAMES.contains(&parent.as_str()) {
            continue;
        }
        let fname = name_of(&status);
        let parsed = std::fs::read(&status).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
        let doc = match parsed {
            Some(Ok(d)) => d,
            Some(Err(e)) => {
                problems.push(format!("{fname}: 파손 — {e}"));
                continue;
            }
            None => {
                problems.push(format!("{fname}: 파손 — unreadable"));
                continue;
            }
        };
        let Some(doc) = dict_of(&doc) else { return Err(Failure::Owner) };
        let proof = doc.get("proof").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
        let source_blank = match proof.get("source").filter(|v| v.is_truthy()) {
            Some(v) => v.display_text().to_utf8().unwrap_or_default().trim().is_empty(),
            None => true,
        };
        let ok_true = matches!(proof.get("ok").map(PyValue::peel), Some(PyValue::Bool(true)));
        if ok_true && source_blank {
            problems.push(format!("{parent}/{fname}: proof.ok=true 인데 source 가 비었다 — 출처 없는 판정은 단언이 검증을 대체한 것이다"));
        }
        if get_text(doc, "state").as_deref() == Some("done") && !ok_true {
            problems.push(format!("{parent}/{fname}: state=done 인데 proof.ok 가 참이 아니다"));
        }
    }
    let mut cell_statuses: Vec<PathBuf> = json_children(&camp_dir.join("cells"), true, "cell.status.json");
    cell_statuses.sort();
    for cs in cell_statuses {
        let parent = cs.parent().map(name_of).unwrap_or_default();
        if RESERVED_CELL_NAMES.contains(&parent.as_str()) {
            continue;
        }
        let parsed = std::fs::read(&cs).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
        let doc = match parsed {
            Some(Ok(d)) => d,
            Some(Err(e)) => {
                problems.push(format!("cell.status.json: 파손 — {e}"));
                continue;
            }
            None => {
                problems.push("cell.status.json: 파손 — unreadable".to_string());
                continue;
            }
        };
        let Some(doc) = dict_of(&doc) else { return Err(Failure::Owner) };
        let blank = match doc.get("void_reason_source").filter(|v| v.is_truthy()) {
            Some(v) => v.display_text().to_utf8().unwrap_or_default().trim().is_empty(),
            None => true,
        };
        if get_text(doc, "cell_outcome").as_deref() == Some("void") && blank {
            problems.push(format!("{parent}: void 인데 void_reason_source 가 비었다"));
        }
    }
    problems.extend(instance_predicates(ctx, camp_dir));
    Ok(problems)
}

fn hint_schema_parity(schema_path: &Path) -> Vec<String> {
    let parsed = std::fs::read(schema_path).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
    let doc = match parsed {
        Some(Ok(d)) => d,
        Some(Err(e)) => return vec![format!("뼈대 campaign.schema.json 파손: {e}")],
        None => return vec!["뼈대 campaign.schema.json 파손: unreadable".to_string()],
    };
    let Some(top) = dict_of(&doc) else {
        return vec![format!("뼈대 campaign.schema.json 파손: '{}' object has no attribute 'get'", doc.type_name())];
    };
    let defs = top.get("definitions").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    let target = defs.get("hint_target").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    let approval = defs.get("hint_approval").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    let mut problems = Vec::new();
    let tprops = target.get("properties").filter(|v| v.is_truthy()).and_then(dict_of).cloned().unwrap_or_default();
    if tprops.contains_key("arch") {
        problems.push("뼈대 스키마 hint_target 에 폐기 키 'arch' 가 살아 있다".to_string());
    }
    if !matches!(target.get("additionalProperties").map(PyValue::peel), Some(PyValue::Bool(false))) {
        problems.push("뼈대 스키마 hint_target 의 additionalProperties 가 false 가 아니다 — 폐기 키가 조용히 통과한다".to_string());
    }
    let min_items = tprops.get("cells").filter(|v| v.is_truthy()).and_then(dict_of).and_then(|c| c.get("minItems"));
    if !matches!(min_items.map(PyValue::peel), Some(PyValue::Int(n)) if *n == 1.into()) {
        problems.push("뼈대 스키마 hint_target.cells 의 minItems 가 1 이 아니다 — 빈 목록이 백지 승인이 된다".to_string());
    }
    let strings = |items: &[&str]| PyValue::List(items.iter().map(|s| py(s)).collect());
    let required = approval.get("required").filter(|v| v.is_truthy()).cloned().unwrap_or_else(|| PyValue::List(Vec::new()));
    if required != strings(&APPROVAL_FIELDS) {
        problems.push(format!(
            "뼈대 스키마 hint_approval.required={} 가 검증기 ['approved_by', 'approved_utc', 'source'] 와 다르다 — 안내 사본이 정본에서 갈라졌다",
            approval.get("required").map(PyValue::repr).unwrap_or_else(|| "None".to_string())
        ));
    }
    let enum_value = approval
        .get("properties")
        .filter(|v| v.is_truthy())
        .and_then(dict_of)
        .and_then(|p| p.get("source"))
        .filter(|v| v.is_truthy())
        .and_then(dict_of)
        .and_then(|s| s.get("enum").cloned());
    if enum_value.as_ref() != Some(&strings(&APPROVAL_SOURCES)) {
        problems.push(format!(
            "뼈대 스키마 hint_approval.source.enum={} 가 검증기 ['declaration-popup', 'publish-popup'] 와 다르다 — 안내 사본이 정본에서 갈라졌다",
            enum_value.map(|v| v.repr()).unwrap_or_else(|| "None".to_string())
        ));
    }
    problems
}

/// `validate_template()`.
pub fn validate_template(ctx: &Ctx) -> Vec<String> {
    let template = ctx.root.join("campaigns/_template");
    let mut problems = Vec::new();
    for rel in [
        "campaign.schema.json", "campaign.yaml", "evidence_pointers.json", "residue.json", "cells/_cell/config.yaml",
        "cells/_cell/lockset.json", "cells/_cell/cell.status.json", "phases/_node/build.status.json",
        "phases/_node/serve.status.json", "phases/_node/bench.status.json", "phases/_node/publish.status.json",
        "relay/.gitkeep", "sweeps/.gitkeep",
    ] {
        if !template.join(rel).exists() {
            problems.push(format!("뼈대 결손: campaigns/_template/{rel}"));
        }
    }
    let tpl = template.join("campaign.yaml");
    if tpl.is_file() {
        let text = std::fs::read(&tpl).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
        if fill_rows(&text).is_empty() {
            problems.push("뼈대 campaign.yaml 에 <<FILL>> 이 하나도 없다 — 빈칸이 곧 계약이다".to_string());
        }
    }
    let lock_tpl = template.join("cells/_cell/lockset.json");
    if lock_tpl.is_file() {
        let parsed = std::fs::read(&lock_tpl).ok().and_then(|b| String::from_utf8(b).ok()).map(|t| pyjson::loads(&t));
        match parsed {
            Some(Err(e)) => problems.push(format!("뼈대 lockset.json 파손: {e}")),
            None => problems.push("뼈대 lockset.json 파손: unreadable".to_string()),
            Some(Ok(doc)) => {
                let d = dict_of(&doc);
                if d.and_then(|d| get_text(d, "provenance")).as_deref() != Some(FILL) {
                    problems.push("뼈대 lockset.json 의 provenance 가 빈칸(<<FILL>>)이 아니다 — 출처는 셀마다 사람이나 explorer 가 말해야 하며 뼈대가 기본값을 줄 수 없다".to_string());
                }
                let missing: Vec<String> = KNOB_SOURCES.iter().map(|(k, _)| k.to_string()).filter(|k| !d.map(|d| d.contains_key(k)).unwrap_or(false)).collect();
                if !missing.is_empty() {
                    problems.push(format!("뼈대 lockset.json 에 노브 출처 칸이 없다: {}", py_list_repr_v(&missing)));
                }
                if let Some(d) = d {
                    let mut expect: Vec<(String, Vec<&str>)> = vec![("_provenance_enum".to_string(), vec!["explorer-phase2", "hand-authored"])];
                    for (k, v) in KNOB_SOURCES {
                        expect.push((format!("_{k}_enum"), v.to_vec()));
                    }
                    for (key, allowed) in expect {
                        let want = PyValue::List(allowed.iter().map(|s| py(s)).collect());
                        if d.get(&key) != Some(&want) {
                            problems.push(format!(
                                "뼈대 lockset.json 의 {key}={} 가 검증기 어휘 {} 와 다르다 — 안내 사본이 정본에서 갈라졌다",
                                d.get(&key).map(PyValue::repr).unwrap_or_else(|| "None".to_string()),
                                want.repr()
                            ));
                        }
                    }
                }
            }
        }
    }
    problems.extend(hint_schema_parity(&template.join("campaign.schema.json")));
    if tpl.is_file() {
        let parsed = std::fs::read(&tpl).ok().and_then(|b| String::from_utf8(b).ok()).and_then(|t| pyjson::loads(&t).ok());
        let empty_targets = parsed
            .as_ref()
            .and_then(dict_of)
            .and_then(|d| d.get("hint_targets"))
            .map(|v| matches!(v.peel(), PyValue::List(l) if l.is_empty()))
            .unwrap_or(false);
        if !empty_targets {
            problems.push("뼈대 campaign.yaml 의 hint_targets 가 빈 목록이 아니다 — 예시 승인이 인스턴스로 복사되면 사람이 하지 않은 승인이 기록된다".to_string());
        }
    }
    problems
}

fn py_list_repr_v(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| py_repr(s)).collect::<Vec<_>>().join(", "))
}

/// `instance_notes(camp_dir)`: the non-blocking observation lines.
pub fn instance_notes(camp_dir: &Path) -> Vec<String> {
    let decl = read_declaration(camp_dir);
    let mut out = Vec::new();
    if decl.is_empty() {
        return out;
    }
    for (node, _) in assignments_of(&decl) {
        let cells: Vec<String> = assigned_cells(&decl, Some(&node)).into_iter().filter(|c| !RESERVED_CELL_NAMES.contains(&c.as_str())).collect();
        if !cells.is_empty() && !lockset_observable(&decl, Some(&node)) {
            out.push(format!("ⓘ 서브 자율 저작 · 관측 대상 밖(셀 입력은 서브 인스턴스 소관) node={node}: {}", cells.join(", ")));
        }
    }
    out
}

/// Which checks one command line asks for.
pub struct Request {
    pub campaign: Option<String>,
    pub instance: Option<String>,
    pub template: bool,
    pub acceptance: Option<String>,
}

/// The whole of `main` after argument parsing: the owner's exit status, or `Err` for what only the owner can render.
pub fn run_request(ctx: &Ctx, request: &Request, io: &mut Sinks<'_>) -> Result<i32, ()> {
    let step = || -> V<(Vec<String>, Vec<String>)> {
        let (mut problems, mut notes) = (Vec::new(), Vec::new());
        if request.template || (request.campaign.is_none() && request.instance.is_none() && request.acceptance.is_none()) {
            problems.extend(validate_template(ctx));
        }
        if let Some(c) = &request.campaign {
            let path = PathBuf::from(c);
            problems.extend(validate_campaign(ctx, &path, true)?);
            notes.extend(instance_notes(path.parent().unwrap_or(Path::new(""))));
        }
        if let Some(i) = &request.instance {
            let dir = PathBuf::from(i);
            problems.extend(validate_instance(ctx, &dir)?);
            notes.extend(instance_notes(&dir));
        }
        if let Some(a) = &request.acceptance {
            let dir = PathBuf::from(a);
            let briefs = discover_briefs(ctx, &dir);
            problems.extend(predicate_p4(&dir));
            problems.extend(predicate_p5(&dir, &briefs));
            let (p6, n6) = p6_observations(ctx, &dir);
            problems.extend(p6);
            notes.extend(n6);
        }
        Ok((problems, notes))
    };
    let (problems, notes) = match step() {
        Ok(pair) => pair,
        Err(Failure::Contract(text)) => {
            io.warn(&format!("[campaign_template_validator] FAIL {text}"));
            return Ok(1);
        }
        Err(Failure::Owner) => return Err(()),
    };
    for n in &notes {
        io.say(&format!("  {n}"));
    }
    if !problems.is_empty() {
        io.warn(&format!("[campaign_template_validator] FAIL ({})", problems.len()));
        for p in &problems {
            io.warn(&format!("  - {p}"));
        }
        return Ok(1);
    }
    io.say("[campaign_template_validator] PASS");
    Ok(0)
}

/// `--acceptance INSTANCE`: the owner's exit status.
pub fn acceptance(ctx: &Ctx, instance: &str, io: &mut Sinks<'_>) -> i32 {
    let camp_dir = PathBuf::from(instance);
    let briefs = discover_briefs(ctx, &camp_dir);
    let mut problems = predicate_p4(&camp_dir);
    problems.extend(predicate_p5(&camp_dir, &briefs));
    let (p6, notes) = p6_observations(ctx, &camp_dir);
    problems.extend(p6);
    for note in &notes {
        io.say(&format!("  {note}"));
    }
    if problems.is_empty() {
        io.say("[campaign_template_validator] PASS");
        return 0;
    }
    io.warn(&format!("[campaign_template_validator] FAIL ({})", problems.len()));
    for problem in &problems {
        io.warn(&format!("  - {problem}"));
    }
    1
}

// ── hint publication pre-approval (the shape of `hint_targets`) ─────────────────────────────

const APPROVAL_FIELDS: [&str; 3] = ["approved_by", "approved_utc", "source"];
const APPROVAL_SOURCES: [&str; 2] = ["declaration-popup", "publish-popup"];
pub const APPROVAL_DEFAULT_SOURCE: &str = "declaration-popup";

fn utc_shape(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// `hint_approval_reason(approval)`: why one approval is incomplete (`None` = complete).
pub fn hint_approval_reason(approval: &PyValue) -> Option<String> {
    let Some(approval) = dict_of(approval) else {
        return Some("approval 이 없다 — 사람 승인의 전사가 없는 항목은 사전승인이 아니다".to_string());
    };
    for field in APPROVAL_FIELDS {
        match get_text(approval, field).filter(|v| !v.trim().is_empty()) {
            None => return Some(format!("approval.{field} 가 비었다 — 반쪽 승인은 승인이 아니다")),
            Some(v) if v.contains(FILL) => {
                return Some(format!("approval.{field} 에 {FILL} 이 남아 있다 — 모르는 값을 그럴듯하게 채우지 않는다"))
            }
            Some(_) => {}
        }
    }
    let utc = get_text(approval, "approved_utc").unwrap_or_default();
    if !utc_shape(&utc) {
        return Some(format!(
            "approval.approved_utc {} 가 주입 시각 모양(YYYY-MM-DDTHH:MM:SSZ)이 아니다 — 발행 페이로드는 이 모양만 옮긴다",
            py_repr(&utc)
        ));
    }
    let source = get_text(approval, "source").unwrap_or_default();
    if !APPROVAL_SOURCES.contains(&source.as_str()) {
        return Some(format!(
            "approval.source {} 가 목록 밖이다(허용 ['declaration-popup', 'publish-popup'])",
            py_repr(&source)
        ));
    }
    None
}

/// `hint_target_reasons(doc)`: the defects of `hint_targets` (empty = it holds).
pub fn hint_target_reasons(doc: &PyDict) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let targets_value = doc.get("hint_targets").cloned();
    if let Some(items) = targets_value.as_ref().and_then(list_of) {
        for (i, target) in items.iter().enumerate() {
            if let Some(t) = dict_of(target) {
                if t.contains_key("arch") {
                    problems.push(format!(
                        "hint_targets[{i}].arch 는 폐기된 키다 — 태그 이름은 발행 도구가 전량 파생한다(2026-09-21 D8) — 손으로 적은 arch 는 '파생 가능한데 손으로 적은 값'이다. 키를 지우고 승인은 `campaign_init.py --hint-approve` 로 적는다"
                    ));
                }
            }
        }
    }
    let Some(targets_value) = targets_value.filter(|v| !matches!(v.peel(), PyValue::None)) else {
        return problems;
    };
    let Some(targets) = list_of(&targets_value) else {
        problems.push("hint_targets 가 목록이 아니다".to_string());
        return problems;
    };
    let nodes: Vec<&PyDict> = list_of(doc.get("nodes").filter(|v| v.is_truthy()).unwrap_or(&PyValue::None))
        .into_iter()
        .flatten()
        .filter_map(dict_of)
        .collect();
    if !targets.is_empty() && get_text(doc, "self_role").as_deref() == Some("sub") {
        let own_ids: Vec<PyValue> = nodes.iter().map(|n| n.get("node_id").cloned().unwrap_or(PyValue::None)).collect();
        if nodes.len() != 1 || get_text(nodes[0], "role").as_deref() != Some("sub") {
            problems.push("self_role=sub 인스턴스에 hint_targets 가 있는데 nodes[] 가 자기(role sub) 노드 하나가 아니다 — 파생 선언(emit_slice)은 그 서브 노드 하나만 싣는다".to_string());
        } else if get_text(nodes[0], "topology").as_deref() != Some("single") {
            problems.push(format!(
                "self_role=sub 인스턴스에 hint_targets 가 있다 — topology {} 의 서브는 자체 발행하지 않는다(자체 발행 = ['single'] · 승인은 메인 선언에만)",
                nodes[0].get("topology").map(PyValue::repr).unwrap_or_else(|| "None".to_string())
            ));
        }
        for (i, target) in targets.iter().enumerate() {
            if let Some(t) = dict_of(target) {
                let nid = t.get("node_id").cloned().unwrap_or(PyValue::None);
                if !own_ids.contains(&nid) {
                    problems.push(format!(
                        "self_role=sub 인스턴스의 hint_targets[{i}] 는 다른 노드({}) 몫이다 — 파생 선언은 그 노드 몫만 운반한다",
                        nid.repr()
                    ));
                }
            }
        }
    }
    let node_ids: Vec<PyValue> = nodes.iter().map(|n| n.get("node_id").cloned().unwrap_or(PyValue::None)).collect();
    let mut approved_at: Vec<(String, usize)> = Vec::new();
    for (i, target) in targets.iter().enumerate() {
        let Some(t) = dict_of(target) else {
            problems.push(format!("hint_targets[{i}] 가 객체가 아니다"));
            continue;
        };
        let nid = t.get("node_id").cloned().unwrap_or(PyValue::None);
        if !node_ids.contains(&nid) {
            problems.push(format!("hint_targets[{i}].node_id {} 가 nodes[] 에 없다", nid.repr()));
            continue;
        }
        let cells = t.get("cells").and_then(list_of).filter(|c| !c.is_empty());
        let cells: Vec<PyValue> = match cells {
            Some(c) => c.clone(),
            None => {
                problems.push(format!(
                    "hint_targets[{i}].cells 가 비었다 — 승인할 셀을 명시 열거한다(빈 목록 = 전 셀 해석은 폐기 · 백지 승인 ✗)"
                ));
                Vec::new()
            }
        };
        let nid_text = text_of(&nid).unwrap_or_default();
        let assigned_here = assigned_cells(doc, Some(&nid_text));
        for c in &cells {
            let Some(name) = text_of(c).filter(|n| !n.trim().is_empty() && !n.contains(FILL)) else {
                problems.push(format!("hint_targets[{i}].cells 에 빈 셀 이름 {} 이 있다", c.repr()));
                continue;
            };
            if !assigned_here.contains(&name) {
                problems.push(format!(
                    "hint_targets[{i}].cells 의 {} 이 assignments[{}] 에 없다 — 배정 SSOT 는 assignments 이고 hint_targets 는 파생이다",
                    py_repr(&name),
                    nid.repr()
                ));
            }
            match approved_at.iter().find(|(n, _)| *n == name) {
                Some((_, at)) => problems.push(format!(
                    "hint_targets[{i}].cells 의 {} 은 이미 hint_targets[{at}] 가 승인했다 — 한 셀의 승인은 하나다(어느 전사가 그 셀의 승인인지 갈리지 않는다)",
                    py_repr(&name)
                )),
                None => approved_at.push((name, i)),
            }
        }
        if let Some(why) = hint_approval_reason(t.get("approval").unwrap_or(&PyValue::None)) {
            problems.push(format!("hint_targets[{i}] {why}"));
        }
    }
    problems
}

// ── --selftest ───────────────────────────────────────────────────────────────────────────────

/// `campaign_template_validator --selftest` of the native program: the verdicts of P1-P6, the declaration checks and the
/// schema engine on a scratch checkout. Exit 0 and a final `PASS` mean every check held.
pub fn selftest(io: &mut Sinks<'_>) -> i32 {
    let mut ok = true;
    let mut check = |io: &mut Sinks<'_>, label: &str, cond: bool| {
        let _ = writeln!(io.out, "{}{label}", if cond { "  PASS " } else { "  FAIL " });
        ok &= cond;
    };
    let root = std::env::temp_dir().join(format!("campaign-validator-selftest-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&root);
    let put = |rel: &str, text: &str| {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, text);
    };
    let ctx = Ctx { root: root.clone() };
    let decl = "{\"id\": \"c\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\"}, {\"node_id\": \"sub\", \"role\": \"sub\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}], \"sub\": [{\"cell\": \"s1\"}]}}";
    put("campaigns/c/campaign.yaml", decl);
    let inst = root.join("campaigns/c");

    check(io, "P3: 선언된 노드의 진행표가 없으면 사유로 센다", predicate_p3(&inst).len() == 2);
    put("campaigns/c/phases/main/bench.status.json", "{\"first_started_utc\": \"2026-01-01T01:00:00Z\"}");
    put("campaigns/c/phases/sub/bench.status.json", "{\"authored_by\": \"main\", \"ended_utc\": \"T\"}");
    check(io, "★P3 진행표가 있으면 통과", predicate_p3(&inst).is_empty());
    check(io, "★P5 메인이 서브 진행표를 적었으면 적색", predicate_p5(&inst, &[]).iter().any(|p| p.contains("메인이 서브의 진행표를 적었다")));
    check(io, "P4 서브 빌드 원장이 없으면 적색", predicate_p4(&inst).iter().any(|p| p.contains("빌드 context 원장이 없다")));
    put("campaigns/c/relay/l.json", "{\"campaign_node\": \"sub\", \"campaign_context_kind\": \"build\", \"attempts\": [{\"started_utc\": \"2026-01-01T02:00:00Z\"}]}");
    check(io, "★P4 서브 지시서가 메인 첫 착수보다 늦으면 적색", predicate_p4(&inst).iter().any(|p| p.contains("보다 늦다")));
    put("campaigns/c/relay/l.json", "{\"campaign_node\": \"sub\", \"campaign_context_kind\": \"build\", \"attempts\": [{\"started_utc\": \"2026-01-01T00:30:00Z\"}]}");
    check(io, "P4 서브 지시서가 먼저 나갔으면 통과", predicate_p4(&inst).is_empty());
    let (problems, notes) = p6_observations(&ctx, &inst);
    check(io, "P6 관측 가능한 셀의 lockset 부재는 적색, 서브 셀은 관측 대상 밖 기재", problems.len() == 1 && notes.len() == 1);
    put("campaigns/c/cells/c1/lockset.json", "{\"provenance\": \"hand-authored\", \"gmu_source\": \"odd\"}");
    let (problems, notes) = p6_observations(&ctx, &inst);
    check(io, "★P6 손작성 표시는 통과, 노브 출처 어휘 밖 값은 적색이 아니라 기재", problems.is_empty() && notes.iter().any(|n| n.contains("gmu_source")));

    put("campaigns/c/sweeps/s.json", "{\"cells\": [{\"cell_key\": \"c1\", \"cell_outcome\": \"measured\"}]}");
    check(io, "P1 sweep 은 measured 인데 cell.status 가 없으면 적색", predicate_p1(&ctx, &inst).iter().any(|p| p.contains("cell.status.json 이 없다")));
    put("campaigns/c/cells/c1/cell.status.json", "{\"cell_outcome\": \"pending\"}");
    check(io, "★P1 sweep 과 cell.status 가 갈라지면 적색", predicate_p1(&ctx, &inst).iter().any(|p| p.contains("불일치")));
    put("campaigns/c/evidence_pointers.json", "{\"pointers\": [{\"kind\": \"certificate\", \"path\": \"x\"}]}");
    check(io, "★P2 노드 태그 없는 벤치 증거는 다노드 캠페인에서 적색(공허 통과 ✗)", predicate_p2(&inst).iter().any(|p| p.contains("노드 태그 없는 벤치 증거")));

    let approval = |utc: &str, source: &str| {
        let mut d = crate::pyvalue::PyDict::new();
        d.set("approved_by", py("사람"));
        d.set("approved_utc", py(utc));
        d.set("source", py(source));
        PyValue::Dict(d)
    };
    check(io, "hint 승인: 완결된 승인은 통과", hint_approval_reason(&approval("2026-01-01T00:00:00Z", "declaration-popup")).is_none());
    check(io, "★hint 승인: 주입 시각 모양이 아니면 거부", hint_approval_reason(&approval("T", "declaration-popup")).is_some());
    check(io, "★hint 승인: 출처 어휘 밖이면 거부", hint_approval_reason(&approval("2026-01-01T00:00:00Z", "zz")).is_some());
    check(io, "★hint 승인: 반쪽 승인(approval 없음)은 승인이 아니다", hint_approval_reason(&PyValue::None).is_some());

    let schema = agent_runtime_support::pyjson::loads("{\"type\": \"object\", \"required\": [\"a\"], \"properties\": {\"a\": {\"type\": \"integer\", \"minimum\": 1}}, \"additionalProperties\": false}").unwrap();
    let verdict = |doc: &str| {
        let d = agent_runtime_support::pyjson::loads(doc).unwrap();
        agent_runtime_support::schema::validate_against_schema(&d, &schema).unwrap().into_iter().map(|(c, _)| c).collect::<Vec<_>>()
    };
    check(io, "스키마 엔진: 적합한 문서는 위반 0", verdict("{\"a\": 2}").is_empty());
    check(io, "★스키마 엔진: 필수 키 부재·모르는 키·최소값 위반을 코드로 가른다", verdict("{\"b\": 1}") == ["SCHEMA_MISSING_REQUIRED_KEY:a", "SCHEMA_UNKNOWN_PROPERTY:b"] && verdict("{\"a\": 0}") == ["SCHEMA_MINIMUM_VIOLATION:a"]);
    check(io, "★스키마 엔진: bool 은 integer 가 아니다", verdict("{\"a\": true}") == ["SCHEMA_TYPE_MISMATCH:a"]);

    check(io, "뼈대가 없는 체크아웃은 뼈대 결손으로 적색", validate_template(&ctx).iter().any(|p| p.contains("뼈대 결손: campaigns/_template/campaign.schema.json")));
    check(io, "★선언 부재는 위반 목록(예외 아님)", matches!(validate_campaign(&ctx, &root.join("none/campaign.yaml"), true), Ok(v) if v.len() == 1));

    let _ = std::fs::remove_dir_all(&root);
    let _ = writeln!(io.out, "[campaign_template_validator] {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        1
    }
}
