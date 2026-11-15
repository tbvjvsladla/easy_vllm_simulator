//! Native `campaign_template_validator.py --acceptance INSTANCE`: the acceptance predicates P4 (the sub instruction went out
//! before the main started), P5 (the sub wrote its own progress) and P6 (every observable assigned cell states where its
//! lockset came from). The other modes of the validator (`--campaign`, `--instance`, `--template`, `--selftest`) stay with
//! the owner program, which the launcher reaches through the explicit compatibility route.
//!
//! Output is the owner's: observation lines first on standard output (`  <note>`), then either `PASS` on standard output or
//! `FAIL (n)` with one `  - <problem>` line each on standard error (status 1).

use crate::campaign_write::{
    assigned_cells, assignments_of, dict_of, get_text, list_of, lockset_observable, lockset_provenance_reason, py_repr,
    read_declaration, text_of, tuple_repr, Ctx, Sinks, FILL,
};
use crate::pyjson;
use crate::pyvalue::{PyDict, PyValue};
use std::path::{Path, PathBuf};

const RESERVED_CELL_NAMES: [&str; 2] = ["_cell", "_node"];
const KNOB_SOURCES: [(&str, &[&str]); 3] = [
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
