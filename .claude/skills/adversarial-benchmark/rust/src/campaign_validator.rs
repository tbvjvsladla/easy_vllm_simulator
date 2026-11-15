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
