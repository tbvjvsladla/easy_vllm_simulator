//! The command bodies and their helpers (hint.py `_gate` … `cmd_branch_transition`).
use std::path::{Path, PathBuf};

use super::util::*;
use super::*;
use crate::core::{self, HintError, HR};
use crate::value::{dumps_pretty, dumps_pretty_sorted, dumps_with, py_repr, Dump, Map, J};
use crate::{artifacts, branch, catalog, evidence, lineage, naming, pii, tag, template};
use crate::{jd, jl};

fn js(s: &str) -> J {
    J::Str(s.to_string())
}

fn p_j(p: &Path) -> J {
    J::Str(core::posix(p))
}

fn fail<T>(code: &str, msg: impl Into<String>, remedy: Option<&str>) -> HR<T> {
    core::fail(code, msg, remedy)
}

// ── 게이트 ─────────────────────────────────────────────────────────────────────────────────────
/// 승격 게이트 1회 — 액션은 `HINT_ACTION_FOR_CMD[cmd]` 에서만 읽는다. 실행 불가 두 경우는 결정론 포장 JSON 을 싣는다.
pub fn _gate(repo: &Path, manifest: &J, cmd: &str) -> HR<J> {
    let action = action_for_cmd(cmd).ok_or_else(|| HintError::py("KeyError", py_repr(cmd)))?;
    match evidence::authorize(repo, manifest, action) {
        Err(mut e) if !e.is_py() && _GATE_BARE_CODES.contains(&e.code.as_str()) => {
            e.gate_json = Some(_gate_bare_json(action, &e.code, &e.message, e.exit_code));
            Err(e)
        }
        r => r,
    }
}

/// The deterministic wrapper JSON of a gate that could not run (`allowed: false`).
pub fn _gate_bare_json(action: &str, code: &str, message: &str, exit_code: i32) -> String {
    let mut messages = Map::new();
    messages.insert(code, message);
    core::dumps(&jd! {
        "schema_version" => 1, "mode" => "promotion", "action" => action, "task_class" => J::Null,
        "authorization_state" => J::Null, "allowed" => false, "reason_codes" => jl![code],
        "messages" => J::Dict(messages), "identity" => J::Null,
        "exit_code" => (if exit_code != 0 { exit_code } else { 2 }) as i64,
    })
}

/// D8 권한 비대칭 — push 역할 선언 또는 카탈로그 권위 선언이 있어야 한다.
pub fn _require_central(repo: &Path, action: &str) -> HR<()> {
    if repo.join(core::REL_CENTRAL_FLAG).is_file() || repo.join(core::REL_PUSH_FLAG).is_file() {
        return Ok(());
    }
    let c = core::REL_CENTRAL_FLAG;
    let p = core::REL_PUSH_FLAG;
    fail(
        "HINT_PUSH_CENTRAL_AUTHORITY_ABSENT",
        format!("'{action}' 는 **자기 원격의 색인·배포 권위**를 가진 체크아웃에서만 실행된다 — `{c}` 부재(D8 권한 비대칭 · plan_26082009 · 2026-08-20)."),
        Some(&format!(
            "▸ single 서브(자체 발행 장착)는 push 역할만 선언한다: `{p}`(terraforming_node 설치 절차). \
             ▸ 이 체크아웃이 자기 원격(origin)의 hint 카탈로그를 소유한다면 권위를 선언하라: \
             printf '%s\\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > {c} \
             (비추적 — 각 저장소가 스스로 선언한다 · 선언은 그 원격의 index.json·HINTS.md 정합을 떠안는 책임이다). \
             ▸ 상류에 기여하는 입장이면 `hint.sh continue … --no-push` 로 봉인·로컬 검증까지만 하고 그 태그를 상류에 전달하라\
             (색인·배포는 상류가 한다). ▸ 어느 쪽도 아니면 **우회하지 말고** 어느 쪽인지부터 정하라 — 맨 `git tag -a` + \
             `git push` 로 만든 태그는 증거 바인딩이 없다."
        )),
    )
}

/// The docker runner handed to the evidence/artifacts owners (`Runtime.docker`). The Python passes a callable; this port
/// passes a marker the owners interpret: `{"__docker_runner__": "read-only"}` (`_docker_read_only`: only `image inspect`
/// and `history` run, anything else = docker's own rc 125) or `{"__docker_runner__": "probe-only"}`
/// (`_docker_probe_only`: artifacts `_probe_only_runner` wrapped, declaring `PROBE_ATTR`). `J::Null` = library default.
pub fn docker_runner(read_only: bool) -> J {
    jd! {"__docker_runner__" => if read_only { "read-only" } else { "probe-only" }}
}

/// `_publish_runner(a)`: the publish / refacts docker runner — the image probe is the default; `--docker-read-only` turns
/// it off; `--docker-probe` is the old no-op alias.
pub fn _publish_runner(a: &Args) -> J {
    docker_runner(a.b("docker_read_only"))
}

/// The original `_docker_probe_only.__doc__` (the self-test reads it: the help text and this docstring must say the same).
pub const _DOCKER_PROBE_ONLY_DOC: &str = "publish 의 **기본** 실행기(2026-09-22 S2 round 3 — round 2 에서는 `--docker-probe` 옵트인이었다 · 그 플래그는 이제 no-op 별칭):
    `image inspect`·`history` + **시작하지 않는** 컨테이너의 `create`(`--pull never`
    필수)·`cp`(이 실행기가 만든 컨테이너에서만)·`rm`(이 실행기가 만든 컨테이너만)을 실물로 부르고 `image_probe` 능력을 선언한다
    (artifacts `PROBE_ATTR` — 이미지 안 패치 사본 · 적용 표지를 꺼내 `image-probe(script-sha+marker)` 판정을 낸다). 그 밖(`run` ·
    `start` · `exec` · `build` · `compose` · `pull` …)은 docker 자신의 실패(rc 125)로 돌려준다. 이미지 원장(`/opt/easy-vllm/build_ledger.json`)
    도 artifacts 가 이 능력을 보고 **같은 create+cp** 로 읽으므로(`_observe_ledger`) 이 실행기에서는 원장 관측 ② 가 성립한다 — `docker
    run … cat` 은 부르지 않는다(누가 부르면 거부된다). 2026-09-22 · plan_26092119 S2 round 2 통합 정정: 옛 문구는 \"원장 cat 이 거부되어
    관측 실패로 남는다\" 였다 — artifacts 가 원장 읽기를 create+cp 로 옮긴 뒤로는 사실이 아니었다(도움말도 같이 고쳤다).
    왜: S2 1차 재생은 9개 패치 중 7개를 '적용 미관측' 으로 실었는데 기계 채점기는 이미지 파일로 6개를
    판정했다(관측할 수 있었던 것) — 컨테이너를 **띄우지 않고** 그 관측을 얻는 경로가 필요했다. 2차 재생은 옵트인 플래그를 쓰지 않아
    같은 7개를 다시 `unobserved` 로 실었다 → 기본값으로 올렸다(끄는 것은 `--docker-read-only` · 정책 자체검사의 docker 호출 계약도
    \"시작하지 않는 create · 만든 컨테이너의 cp · rm\" 으로 함께 넓혔다 — runtime_selftest `_hint_docker_calls_non_starting`).
    2026-09-22 round 3 적대 리뷰: 이 함수는 artifacts `_probe_only_runner`(라이브러리 기본 실행기)와 **같은 규칙을 손으로 한 벌 더**
    적고 있었고 이미 갈라져 있었다(여기는 create 에 `--pull never` 만 · 저쪽은 `--network none` 도 요구). 규칙의 소유자는 artifacts
    하나다 — 여기서는 그 실행기를 감싸 evidence 의 호출 꼴(`capture_output` · `text` · `timeout` 키워드)만 맞춘다.";

// ── 경로 · 상태 ───────────────────────────────────────────────────────────────────────────────
pub fn _show(repo: &Path, p: &Path) -> String {
    let rp = core::resolve(p);
    let rr = core::resolve(repo);
    match rp.strip_prefix(&rr) {
        Ok(r) => {
            let s = core::posix(r);
            if s.is_empty() {
                ".".into()
            } else {
                s
            }
        }
        Err(_) => core::posix(&rp),
    }
}

/// The CLI invocation printed in instructions (intended difference: the Rust launcher, run with bash).
pub fn _cli(repo: &Path) -> String {
    if repo.join(core::REL_HINT_CLI).is_file() {
        format!("bash {}", core::REL_HINT_CLI)
    } else {
        format!("bash {}", core::posix(&core::scripts_dir().join("hint.sh")))
    }
}

pub fn _require_fresh_draft(draft: &Path) -> HR<()> {
    if !draft.exists() {
        return Ok(());
    }
    let d = core::posix(draft);
    if !draft.is_dir() {
        return fail("HINT_DRAFT_EXISTS", format!("draft 자리가 디렉터리가 아니다: {d}"), None);
    }
    if draft.join(STATE_NAME).exists() {
        return fail(
            "HINT_DRAFT_EXISTS",
            format!("이미 publish 된 draft 다: {d}"),
            Some("이어서 저작하려면 `hint.sh continue --draft <draft>` · 새로 시작하려면 draft 를 지우고 publish 를 다시 한다."),
        );
    }
    let mut extra: Vec<String> = std::fs::read_dir(draft)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n != "inputs" && n != "payload").collect())
        .unwrap_or_default();
    extra.sort();
    let payload = draft.join("payload");
    let payload_files = if payload.is_dir() { files_under(&payload, &payload).len() } else { 0 };
    if !extra.is_empty() || payload_files > 0 {
        let head: Vec<String> = extra.iter().take(5).cloned().collect();
        return fail(
            "HINT_DRAFT_EXISTS",
            format!("draft 자리가 비어 있지 않다(끊긴 publish 의 잔재): {d} · {} · payload 파일 {payload_files}건", list_repr(&head)),
            Some(
                "잔재를 확인하고 draft 디렉터리를 지운 뒤 publish 를 다시 한다(잔재가 페이로드에 섞이지 않게). 캠페인 셀은 \
                 발행 기록이 첫 시각으로 이미 묶였을 수 있다 — 끊긴 publish 와 **같은** --generated-utc 로 다시 실행한다.",
            ),
        );
    }
    if payload.is_dir() {
        rmtree(&payload)?;
    }
    Ok(())
}

pub fn _load_state(draft: &Path) -> HR<J> {
    let p = draft.join(STATE_NAME);
    let mut st = core::read_json_code(&p, "HINT_DRAFT_STATE_UNREADABLE")?;
    if !st.is_dict() || !eq_int(st.get("schema_version"), STATE_SCHEMA) || !st["tag"].truthy() || !st["topic"].truthy() {
        return fail(
            "HINT_DRAFT_STATE_UNREADABLE",
            format!("state.json 모양이 아니다: {}", core::posix(&p)),
            Some("hint.sh publish 가 만든 draft 에서 실행한다(state.json 은 도구가 쓴다 · 손으로 고치지 않는다)."),
        );
    }
    if let J::Dict(m) = &mut st {
        m.setdefault("steps", J::dict());
    }
    Ok(st)
}

pub fn _save_state(draft: &Path, st: &J) -> HR<()> {
    core::write_json(&draft.join(STATE_NAME), st)
}

pub fn _drafts(repo: &Path) -> HR<Vec<(PathBuf, J)>> {
    let root = repo.join(core::REL_DRAFTS);
    let mut out = Vec::new();
    if root.is_dir() {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        dirs.sort();
        for d in dirs {
            let sp = d.join(STATE_NAME);
            if !sp.exists() {
                continue;
            }
            let Ok(doc) = load_json_typed(&sp) else { continue };
            if doc.is_dict() && eq_int(doc.get("schema_version"), STATE_SCHEMA) {
                out.push((d, doc));
            }
        }
    }
    Ok(out)
}

fn sel_of(st: &J) -> J {
    if st["selectors"].truthy() {
        st["selectors"].clone()
    } else {
        J::dict()
    }
}

fn eq_opt_str(v: &J, s: Option<&str>) -> bool {
    match s {
        None => v.is_null(),
        Some(s) => v.as_str() == Some(s),
    }
}

pub fn _find_draft(repo: &Path, draft: Option<&str>, campaign: Option<&str>, cell: Option<&str>, node: Option<&str>) -> HR<PathBuf> {
    if let Some(dr) = draft.filter(|s| !s.is_empty()) {
        let d = core::resolve(Path::new(dr));
        if !d.join(STATE_NAME).is_file() {
            return fail(
                "HINT_DRAFT_ABSENT",
                format!("state.json 이 없다 — publish 된 draft 가 아니다: {}", core::posix(&d)),
                Some("hint.sh publish 를 먼저 실행한다(출력의 draft 경로를 --draft 로 넘긴다)."),
            );
        }
        return Ok(d);
    }
    let (Some(campaign), Some(cell)) = (campaign.filter(|s| !s.is_empty()), cell.filter(|s| !s.is_empty())) else {
        return fail("HINT_SELECTOR_ABSENT", "continue 대상이 없다 — `--draft DIR` 또는 `--campaign ID --cell CELL`.", None);
    };
    let hits: Vec<PathBuf> = _drafts(repo)?
        .into_iter()
        .filter(|(_, st)| {
            let s = sel_of(st);
            eq_opt_str(&s["campaign"], Some(campaign)) && eq_opt_str(&s["cell"], Some(cell)) && (node.is_none() || eq_opt_str(&s["node"], node))
        })
        .map(|(d, _)| d)
        .collect();
    if hits.is_empty() {
        return fail(
            "HINT_DRAFT_ABSENT",
            format!("{}/ 에 캠페인 {campaign} 셀 {cell} 의 draft 가 없다.", core::REL_DRAFTS),
            Some(&format!("`hint.sh publish --campaign {campaign} --cell {cell} --generated-utc <UTC>` 를 먼저 실행한다.")),
        );
    }
    if hits.len() > 1 {
        let shown: Vec<String> = hits.iter().map(|h| _show(repo, h)).collect();
        return fail("HINT_DRAFT_AMBIGUOUS", format!("draft 가 {}개다: {}", hits.len(), list_repr(&shown)), Some("--node 또는 --draft 로 하나를 고른다."));
    }
    Ok(hits.into_iter().next().unwrap())
}

pub fn _find_draft_for_tag(repo: &Path, tagname: &str) -> HR<Option<PathBuf>> {
    let hits: Vec<PathBuf> = _drafts(repo)?.into_iter().filter(|(_, st)| eq_opt_str(&st["tag"], Some(tagname))).map(|(d, _)| d).collect();
    if hits.len() > 1 {
        return fail("HINT_DRAFT_AMBIGUOUS", format!("태그 {tagname} 의 draft 가 {}개다 — --draft 로 고른다.", hits.len()), None);
    }
    Ok(hits.into_iter().next())
}

pub fn _step(st: &J, name: &str) -> Option<J> {
    let steps = &st["steps"];
    if !steps.truthy() {
        return None;
    }
    steps.get(name).filter(|v| !v.is_null()).cloned()
}

pub fn _mark(draft: &Path, st: &mut J, name: &str, data: J) -> HR<()> {
    if let J::Dict(m) = st {
        let steps = m.setdefault("steps", J::dict());
        steps.set(name, data);
        m.insert("stage", name);
    }
    _save_state(draft, st)
}

// ── 증거 · 이름 ────────────────────────────────────────────────────────────────────────────────
pub fn _evidence(repo: &Path, campaign: Option<&str>, cell: Option<&str>, publication: Option<&str>, node: Option<&str>, docker: &J) -> HR<J> {
    if let Some(p) = publication {
        evidence::from_publication(repo, &js(p), &J::from(node), docker)
    } else {
        super::hooks::from_campaign(repo, &J::from(campaign), &J::from(cell), &J::from(node), docker)
    }
}

pub fn _check_selectors(campaign: Option<&str>, cell: Option<&str>, publication: Option<&str>, replay: bool, require_replay: bool) -> HR<()> {
    let t = |s: Option<&str>| s.map(|x| !x.is_empty()).unwrap_or(false);
    if publication.is_some() {
        if t(campaign) || t(cell) {
            return fail("HINT_SELECTOR_CONFLICT", "--publication 은 --campaign/--cell 과 함께 쓰지 않는다.", None);
        }
        if require_replay && !replay {
            return fail("HINT_REPLAY_FLAG_REQUIRED", "--publication 은 발행 기록 재생이다 — `--replay` 를 명시한다(읽기 전용 경로).", None);
        }
        return Ok(());
    }
    if replay {
        return fail("HINT_SELECTOR_CONFLICT", "--replay 는 --publication 과 함께만 쓴다.", None);
    }
    if !(t(campaign) && t(cell)) {
        return fail("HINT_SELECTOR_ABSENT", "발행 대상이 없다 — `--campaign ID --cell CELL` 또는 `--publication TOPIC --replay`.", None);
    }
    Ok(())
}

/// 이름 충돌 조회(차단 ✗) → (hit | None, 조회 기록).
pub fn _name_hit(repo: &Path, tagname: &str, remote: &str, remote_check: bool, tolerate: bool) -> HR<(Option<String>, String)> {
    if !remote_check {
        return Ok((tag::name_collision(repo, tagname, None)?, "skipped(--no-remote-check · 로컬만 확인)".into()));
    }
    match tag::name_collision(repo, tagname, Some(remote)) {
        Ok(h) => Ok((h, format!("{remote}: 조회함"))),
        Err(e) if tolerate && !e.is_py() && e.code == "HINT_REMOTE_QUERY_FAILED" => {
            Ok((tag::name_collision(repo, tagname, None)?, format!("unreachable({remote} · 로컬만 확인)")))
        }
        Err(e) => Err(e),
    }
}

pub fn _require_name_pii_clean(repo: &Path, tagname: &str) -> HR<()> {
    let terms = pii::load_pii_terms(repo)?.unwrap_or_default();
    let hits = pii::scan_text(tagname, Some(&terms), "deploy", &[], false, true, None)?;
    if !hits.is_empty() {
        let pats: std::collections::BTreeSet<String> = hits.iter().map(|h| h.pattern.clone()).collect();
        return fail(
            "HINT_TAG_NAME_PII",
            format!(
                "파생 이름이 배포 PII 스캔에 걸린다({}) — 이 이름은 커밋·봉인에서 반드시 거부된다: {tagname}. 아무것도 쓰지 않았다.",
                pats.into_iter().collect::<Vec<_>>().join(", ")
            ),
            Some(
                "4마디 vLLM 릴리스(0.10.x.y 등)는 private-ipv4 패턴의 3옥텟 접두 매치에 걸린다 — 스캐너 체계 변경은 사람 결정이다\
                 (P3 · plan_26092119). pii_terms 리터럴이면 그 값이 이름(모델 슬러그 등)에 들어온 경로를 확인한다.",
            ),
        );
    }
    Ok(())
}

pub fn _require_unbound_publication_time(repo: &Path, topic: &str, utc: &str) -> HR<String> {
    let rec = repo.join(core::REL_EVIDENCE_DIR).join(format!("{topic}.json"));
    if !rec.is_file() {
        return Ok(utc.to_string());
    }
    let Ok(doc) = load_json_typed(&rec) else { return Ok(utc.to_string()) };
    let bound = match doc.get("generated_utc") {
        Some(J::Str(b)) if !b.is_empty() && b != utc => b.clone(),
        _ => return Ok(utc.to_string()),
    };
    let scaffolded: Vec<PathBuf> = _drafts(repo)?.into_iter().filter(|(_, st)| eq_opt_str(&st["topic"], Some(topic))).map(|(d, _)| d).collect();
    let ed = core::REL_EVIDENCE_DIR;
    if !scaffolded.is_empty() {
        let shown: Vec<String> = scaffolded.iter().take(3).map(|d| _show(repo, d)).collect();
        return fail(
            "HINT_PUBLICATION_TIME_BOUND",
            format!(
                "셀 발행 기록 `{ed}/{topic}.json` 이 이미 generated_utc={bound} 로 묶여 있고 그 발행의 draft 가 있다({}) — 다른 시각({utc})의 발행은 같은 기록을 다시 묶는다(불변). 아무것도 쓰지 않았다.",
                list_repr(&shown)
            ),
            Some(
                "그 draft 를 이어서 저작한다(`hint.sh continue --draft …` · 사실 재파생은 `hint.sh refacts`). 같은 셀의 새 판은 그 발행을 \
                 끝낸 뒤 새 발행 기록으로 낸다.",
            ),
        );
    }
    core::require_utc(Some(&bound), &format!("{ed}/{topic}.json generated_utc"))?;
    _log(&format!(
        "발행 기록 `{topic}` 이 이미 generated_utc={bound} 로 묶였고 스캐폴드된 draft 가 없다(앞선 publish 가 스캐폴드 전에 멈췄다) — \
         묶인 시각 {bound} 를 채택한다(명시한 --generated-utc {utc} 는 쓰지 않는다 · 발행 기록의 시각은 불변)."
    ));
    Ok(bound)
}

/// 03 부하 곡선의 원천: 바인딩된 bench_report → 묶인 스윕 색인 → 없음.
pub fn _bench_source(repo: &Path, ev: &J) -> Option<String> {
    let rep = &ev["bench_report_path"];
    if rep.truthy() && repo.join(rep.display()).is_file() {
        return Some(rep.display());
    }
    let sw = &ev["sweep"];
    let dir = &sw["dir"];
    if dir.truthy() && repo.join(dir.display()).join("sweep_index.json").is_file() {
        return Some(format!("{}/sweep_index.json", dir.display()));
    }
    None
}

pub fn _with_bench_command(steps: &J, bc: &J) -> J {
    let mut out = if steps.truthy() { steps.clone() } else { J::list() };
    let cmd = match bc.get("command") {
        Some(J::Str(c)) if !py_strip(c).is_empty() => py_strip(c).to_string(),
        _ => return out,
    };
    if !bc.is_dict() {
        return out;
    }
    if let J::List(items) = &mut out {
        for s in items.iter_mut() {
            if !s.is_dict() || s.get_str("phase") != Some("bench") || disp_or_empty(&s["command_source"]).starts_with("reconstructed") {
                continue;
            }
            let mut lines = vec![cmd.clone()];
            let unrec = &bc["unrecorded"];
            if unrec.truthy() {
                let parts: Vec<String> = match unrec {
                    J::Str(t) => t.chars().map(|c| c.to_string()).collect(),
                    J::Dict(m) => m.keys().map(str::to_string).collect(),
                    other => other.items().iter().map(J::display).collect(),
                };
                lines.push(format!("# 결과 JSON 이 들지 않아 재구성하지 못한 인자(관측 불가): {}", parts.join(" · ")));
            }
            if bc["source"].truthy() {
                lines.push(format!("# {}", bc["source"].display()));
            }
            s.set("command", lines.join("\n"));
            s.set("command_source", "reconstructed(bench json)");
        }
    }
    out
}

/// PAYLOAD.build — evidence 관측 + artifacts 선택자.
pub fn _build_doc(ev: &J, art: &J) -> J {
    let b = if ev["build_identity"].truthy() { ev["build_identity"].clone() } else { J::dict() };
    let bsrc = if b["source"].truthy() { b["source"].clone() } else { J::dict() };
    let mut build = Map::new();
    for k in _BUILD_KEYS {
        build.insert(k, b[k].clone());
    }
    let mut src = Map::new();
    for k in _BUILD_KEYS {
        if let Some(v) = bsrc.get(k) {
            src.insert(k, v.clone());
        }
    }
    let sel = if art["build"].truthy() { art["build"].clone() } else { J::dict() };
    let loc = if b["image_digest_local"].is_dict() { Some(b["image_digest_local"].clone()) } else { None };
    if !build.get("image_digest").map(J::truthy).unwrap_or(false) {
        if let Some(loc) = loc.filter(|l| l.truthy()) {
            if loc["same_build"].truthy() && loc["id"].truthy() {
                build.insert("image_digest", loc["id"].clone());
                src.insert("image_digest", loc["source"].clone());
            } else {
                src.insert("image_digest_local_note", loc["source"].clone());
            }
        }
    }
    if art.get_str("plane") == Some("native") {
        build.insert("track", "native");
        src.insert("track", if art["plane_source"].truthy() { art["plane_source"].clone() } else { js("artifacts.plane_of") });
        let ns = if art["native_source"].is_dict() { art["native_source"].clone() } else { J::dict() };
        if sel["dockerfile"].truthy() {
            build.insert("dockerfile", sel["dockerfile"].clone());
            src.insert(
                "dockerfile",
                format!(
                    "원천 이미지 Dockerfile(native 셀은 Docker 로 실행되지 않았다 · 원천 셀 {} · 원천 이미지 {} · artifacts 선택자 {})",
                    ns["cell"].display(),
                    ns["image_tag"].display(),
                    sel["dockerfile_source"].display()
                ),
            );
            build.insert("source_image_track", sel["track"].clone());
            src.insert("source_image_track", format!("artifacts 선택자(원천 셀 {} · {})", ns["cell"].display(), sel["dockerfile_source"].display()));
        } else {
            build.insert("dockerfile", J::Null);
            let why = if ns["why"].truthy() { ns["why"].display() } else { "원천 신호 없음".into() };
            src.insert("dockerfile", format!("원천 이미지 미관측({why}) — Dockerfile 을 싣지 못했다(결손)"));
        }
    } else if sel["dockerfile"].truthy() {
        build.insert("dockerfile", sel["dockerfile"].clone());
        let ds = if sel["dockerfile_source"].truthy() { sel["dockerfile_source"].clone() } else { src.get("dockerfile").cloned().unwrap_or(J::Null) };
        src.insert("dockerfile", ds);
        build.insert("track", sel["track"].clone());
        src.insert("track", format!("artifacts 선택자({})", sel["dockerfile_source"].display()));
    }
    build.insert("source", J::Dict(src));
    J::Dict(build)
}

// ── 승인 · 사실 갱신 ───────────────────────────────────────────────────────────────────────────
pub fn _approval(repo: &Path, st: &J, approved_by: Option<&str>, approved_utc: Option<&str>) -> HR<J> {
    let sel = sel_of(st);
    let t = |s: Option<&str>| s.map(|x| !x.is_empty()).unwrap_or(false);
    if st.get_str("mode") == Some("campaign") {
        if t(approved_by) || t(approved_utc) {
            return fail(
                "HINT_APPROVAL_SOURCE_CONFLICT",
                "캠페인 셀의 승인 원천은 campaign.yaml hint_targets 하나다(O6) — --approved-by/--approved-utc 는 캠페인 밖(재생) 발행용이다.",
                // plan_26111708 B L8: the remedy names the live `campaign_init.sh` launcher.
                Some("캠페인 승인은 `campaign_init.sh --hint-approve` 로 적는다(선언 확인·발행 팝업의 사람 발화 전사)."),
            );
        }
        let ap = evidence::approval_for(repo, &sel["campaign"], &sel["cell"], &sel["node"])?;
        let Some(ap) = ap else {
            return fail(
                "HINT_APPROVAL_ABSENT",
                format!(
                    "캠페인 {} 셀 {} 의 발행 승인이 없다(hint_targets[].approval) — 어느 ref 도 움직이지 않았다.",
                    sel["campaign"].display(),
                    sel["cell"].display()
                ),
                Some(&format!(
                    "사람 Y/N 을 받아(제안 전용 · 무인 자동 승인 ✗) `bash {} --hint-approve --campaign-id {} --node <배정 노드> --cells {} \
                     --approved-by \"<사람 발화 전사>\" --utc <UTC>` 로 적은 뒤 continue 를 다시 실행한다.",
                    core::owner_launcher(std::path::Path::new(""), core::REL_CAMPAIGN_INIT).display(),
                    sel["campaign"].display(),
                    sel["cell"].display()
                )),
            );
        };
        let mut out = Map::new();
        for k in APPROVAL_KEYS {
            out.insert(k, ap[k].clone());
        }
        return Ok(J::Dict(out));
    }
    let text = py_strip(approved_by.unwrap_or("")).to_string();
    if text.is_empty() || !t(approved_utc) {
        return fail(
            "HINT_APPROVAL_ABSENT",
            "캠페인 밖 발행은 사람 승인 전사가 필요하다(--approved-by · --approved-utc) — 어느 ref 도 움직이지 않았다.",
            Some("사람 Y/N 을 받아 그 발화를 전사해 `--approved-by \"<사람 발화 전사>\" --approved-utc <UTC>` 로 넘긴다(제안 전용)."),
        );
    }
    let text = _one_line_utterance(Some(&text), "HINT_APPROVAL_INVALID", "--approved-by")?;
    let utc = core::require_utc(approved_utc, "--approved-utc")?;
    Ok(jd! {"approved_by" => text, "approved_utc" => utc, "source" => "cli:--approved-by"})
}

/// 사람 발화 전사 한 줄 — 빈칸 · 여러 줄 · 자리표시 ✗.
pub fn _one_line_utterance(text: Option<&str>, code: &str, what: &str) -> HR<String> {
    let s = py_strip(text.unwrap_or(""));
    if s.is_empty() || s.contains('\n') || s.contains('\r') || s.starts_with("<<") || s.starts_with("__") || s.ends_with(">>") || s.ends_with("__") {
        return fail(
            code,
            format!("{what} 는 한 줄의 사람 발화 전사다(빈칸 · 자리표시 · 여러 줄 ✗)."),
            Some("사람 Y/N(결정)을 받아 그 발화를 역할과 요지로 전사한다 — 에이전트가 스스로 적지 않는다."),
        );
    }
    Ok(s.to_string())
}

/// `core.require_utc(value, what)` for a JSON value (a non-str truthy value is Python's `TypeError` from `re.match`).
fn require_utc_j(v: &J, what: &str) -> HR<String> {
    if !v.truthy() {
        return core::fail(
            "HINT_TIME_NOT_INJECTED",
            format!("{what} 는 UTC `YYYY-MM-DDTHH:MM:SSZ` 로 주입해야 한다: {}", v.repr()),
            Some("벽시계를 읽지 않는다 — 호출자가 시각을 넘긴다."),
        );
    }
    match v {
        J::Str(s) => core::require_utc(Some(s), what),
        other => Err(py_err("TypeError", format!("expected string or bytes-like object, got '{}'", other.type_name()))),
    }
}

/// `sorted(template.FACTCHECK_CLASSES)` (the class numbers).
fn factcheck_classes() -> Vec<i64> {
    let mut v: Vec<i64> = template::FACTCHECK_CLASSES().as_dict().map(|m| m.keys().filter_map(|k| k.parse().ok()).collect()).unwrap_or_default();
    v.sort_unstable();
    v
}

/// factcheck.json 모양 검사(결정론 · 부수효과 0). 반환 = 결함 목록.
pub fn _factcheck_problems(doc: &J, st: &J) -> HR<Vec<String>> {
    if !doc.is_dict() {
        return Ok(vec!["최상위가 객체가 아니다".into()]);
    }
    let mut bad: Vec<String> = Vec::new();
    if !eq_int(doc.get("schema_version"), FACTCHECK_SCHEMA) {
        bad.push(format!("schema_version 이 {FACTCHECK_SCHEMA} 가 아니다: {}", doc["schema_version"].repr()));
    }
    let mut mine: Vec<String> = Vec::new();
    for k in ["tag", "base_tag"] {
        if st[k].truthy() {
            let s = st[k].display();
            if !mine.contains(&s) {
                mine.push(s);
            }
        }
    }
    mine.sort();
    let tag_in = match &doc["tag"] {
        J::Str(t) => mine.contains(t),
        _ => false,
    };
    if !tag_in {
        bad.push(format!("tag {} ∉ 이 draft 의 이름 {}(다른 draft 의 보고를 옮기지 않는다)", doc["tag"].repr(), list_repr(&mine)));
    }
    let ok_str = |v: &J| matches!(v, J::Str(s) if !py_strip(s).is_empty());
    for k in ["author", "checker"] {
        if !ok_str(&doc[k]) {
            bad.push(format!("{k} 가 비었다(역할 이름으로 적는다 — 사람 이름 ✗)"));
        }
    }
    if ok_str(&doc["author"]) && ok_str(&doc["checker"]) {
        let a = casefold(py_strip(doc["author"].as_str().unwrap()));
        let c = casefold(py_strip(doc["checker"].as_str().unwrap()));
        if a == c {
            bad.push("checker == author — 사실 검증은 저작자가 아닌 Agent 가 한다(독립성)".into());
        }
    }
    match require_utc_j(&doc["checked_utc"], "checked_utc") {
        Ok(_) => {}
        Err(e) if !e.is_py() => bad.push(e.message),
        Err(e) => return Err(e),
    }
    let n = &doc["claims_checked"];
    if !matches!(n, J::Int(i) if *i >= 1) {
        bad.push(format!("claims_checked 는 1 이상의 정수다: {}", n.repr()));
    }
    let want = factcheck_classes();
    let cls = &doc["classes_checked"];
    let cls_ok = match cls {
        J::List(l) => {
            let mut ints: Vec<i64> = l.iter().filter_map(is_int).collect();
            ints.sort_unstable();
            ints == want && l.len() == want.len()
        }
        _ => false,
    };
    let want_repr = format!("[{}]", want.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "));
    if !cls_ok {
        bad.push(format!("classes_checked 는 7부류 전부 {want_repr} 다(부류 = template.FACTCHECK_CLASSES): {}", cls.repr()));
    }
    let maxw = want.iter().copied().max().unwrap_or(0);
    let items: Vec<J> = match &doc["items"] {
        J::List(l) => l.clone(),
        _ => {
            bad.push("items 는 목록이다(지적 0건이면 빈 목록)".into());
            Vec::new()
        }
    };
    let mut seen: Vec<String> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if !it.is_dict() {
            bad.push(format!("items[{i}] 가 객체가 아니다"));
            continue;
        }
        match &it["id"] {
            J::Str(s) if !py_strip(s).is_empty() && !seen.contains(s) => seen.push(s.clone()),
            other => bad.push(format!("items[{i}].id 가 비었거나 중복이다: {}", other.repr())),
        }
        if !FACTCHECK_VERDICTS.iter().any(|v| it["verdict"].as_str() == Some(v)) {
            bad.push(format!("items[{i}].verdict ∈ {}: {}", tuple_repr(&FACTCHECK_VERDICTS), it["verdict"].repr()));
        }
        let c = &it["class"];
        if !matches!(c, J::Int(x) if *x >= 0 && *x <= maxw) {
            bad.push(format!("items[{i}].class 는 0(기타) 또는 부류 번호 1..{maxw} 다: {}", c.repr()));
        }
        for k in ["where", "claim"] {
            if !ok_str(&it[k]) {
                bad.push(format!("items[{i}].{k} 가 비었다"));
            }
        }
        if !FACTCHECK_TARGETS.iter().any(|v| it["target"].as_str() == Some(v)) {
            bad.push(format!(
                "items[{i}].target ∈ {}(산문 = 저작자가 고친다 · fact = 지적만 · 생산자 교정 후 refacts): {}",
                tuple_repr(&FACTCHECK_TARGETS),
                it["target"].repr()
            ));
        }
        if !FACTCHECK_STATUSES.iter().any(|v| it["status"].as_str() == Some(v)) {
            bad.push(format!("items[{i}].status ∈ {}: {}", tuple_repr(&FACTCHECK_STATUSES), it["status"].repr()));
        } else if it.get_str("status") == Some("fixed") && !ok_str(&it["resolution"]) {
            bad.push(format!("items[{i}] 가 fixed 인데 resolution(무엇을 어떻게 고쳤나)이 없다"));
        }
    }
    Ok(bad)
}

/// 발행 전 독립 사실 검증 보고 판정(부수효과 0) → (facts.factcheck 요약, 막을 오류 | None).
pub fn _factcheck_eval(draft: &Path, st: &J) -> HR<(J, Option<HintError>)> {
    let p = draft.join("inputs").join(FACTCHECK_NAME);
    let rel = format!("inputs/{FACTCHECK_NAME}");
    if !p.is_file() {
        return Ok((
            jd! {"status" => "absent", "reason_code" => "HINT_FACTCHECK_ABSENT", "open" => J::list(), "source" => rel.clone()},
            Some(HintError::new(
                "HINT_FACTCHECK_ABSENT",
                format!("발행 전 독립 사실 검증 보고가 없다(draft {rel}) — 린트 0 은 인용 무결성만 본다(주장의 참 ✗)."),
                Some(&format!(
                    "저작자가 아닌 Agent 가 페이로드의 사실 주장을 전부 뽑아 인용 출처와 대조하고(7부류 체크리스트 · SKILL.md §2) \
                     {rel} 로 남긴다 → 저작자가 고친 뒤 lint → continue. 사람이 면제를 결정했으면 \
                     `--factcheck-waiver \"<사람 발화 전사>\"`."
                )),
            )),
        ));
    }
    let probs = match load_json_typed(&p) {
        Err((_, e)) => vec![format!("읽을 수 없다: {e}")],
        Ok(doc) => {
            let probs = _factcheck_problems(&doc, st)?;
            if probs.is_empty() {
                return factcheck_eval_doc(&doc, &rel);
            }
            probs
        }
    };
    let head: Vec<String> = probs.iter().take(8).cloned().collect();
    Ok((
        jd! {"status" => "invalid", "reason_code" => "HINT_FACTCHECK_INVALID", "open" => J::list(), "source" => rel.clone()},
        Some(HintError::new(
            "HINT_FACTCHECK_INVALID",
            format!("{rel} 모양 결함 {}건: {}", probs.len(), head.join(" · ")),
            Some("SKILL.md §2 의 factcheck.json 형식대로 검증 Agent 가 다시 쓴다(저작자가 대신 쓰지 않는다)."),
        )),
    ))
}

fn factcheck_eval_doc(doc: &J, rel: &str) -> HR<(J, Option<HintError>)> {
    let items: Vec<J> = if doc["items"].truthy() { doc["items"].items().to_vec() } else { Vec::new() };
    let open_ids: Vec<J> = items
        .iter()
        .filter(|it| FACTCHECK_VERDICTS.iter().any(|v| it["verdict"].as_str() == Some(v)) && it.get_str("status") != Some("fixed"))
        .map(|it| it["id"].clone())
        .collect();
    let fixed = items.iter().filter(|it| it.get_str("status") == Some("fixed")).count() as i64;
    let mut base = jd! {
        "checker" => py_strip(doc["checker"].as_str().unwrap_or("")), "author" => py_strip(doc["author"].as_str().unwrap_or("")),
        "checked_utc" => doc["checked_utc"].clone(), "claims_checked" => doc["claims_checked"].clone(), "items" => items.len() as i64,
        "fixed" => fixed, "open" => J::List(open_ids.clone()), "source" => rel,
    };
    if !open_ids.is_empty() {
        base.set("status", "open");
        base.set("reason_code", "HINT_FACTCHECK_OPEN");
        let head = J::List(open_ids.iter().take(12).cloned().collect());
        return Ok((
            base,
            Some(HintError::new(
                "HINT_FACTCHECK_OPEN",
                format!("사실 검증의 열린 지적 {}건(오답 · 오도 · 근거 없음 중 status≠fixed): {}", open_ids.len(), head.repr()),
                Some(
                    "저작자가 각 지적을 페이로드에서 고치고(또는 주장을 빼고) 그 항목을 status=fixed + resolution 으로 닫은 뒤 \
                     lint → continue. 이견(disputed)은 열린 채다 — 사람이 결정한다(--factcheck-waiver).",
                ),
            )),
        ));
    }
    base.set("status", "passed");
    base.set("reason_code", J::Null);
    Ok((base, None))
}

/// 판정 + 사람 면제(있으면).
pub fn _factcheck_gate(draft: &Path, st: &J, waiver: Option<&str>, utc: &str) -> HR<(J, Option<HintError>)> {
    let (mut summary, err) = _factcheck_eval(draft, st)?;
    let Some(err) = err else {
        if waiver.map(|w| !w.is_empty()).unwrap_or(false) {
            _log("--factcheck-waiver 는 쓰지 않았다 — 사실 검증 보고가 통과했다.");
        }
        return Ok((summary, None));
    };
    let Some(w) = waiver.filter(|w| !w.is_empty()) else { return Ok((summary, Some(err))) };
    summary.set("status", "waived");
    summary.set("reason_code", err.code.clone());
    summary.set("waiver", w);
    summary.set("waived_utc", utc);
    Ok((summary, None))
}

/// 01 §1.2 소제목 → slots[*].rationale · confidence 재계산.
pub fn _slot_rationales(payload: &Path, slots: &J) -> HR<J> {
    let subs = template::subsections(payload, "01-artifacts", "1.2")?;
    let mut out = if slots.truthy() { slots.clone() } else { J::dict() };
    if let J::Dict(m) = &mut out {
        for (name, row) in m.0.iter_mut() {
            if !row.is_dict() {
                continue;
            }
            let mut keys: Vec<String> = vec![name.clone()];
            if row["files"].truthy() {
                for f in row["files"].items() {
                    keys.push(py_basename(&f.display()));
                }
            }
            let texts: Vec<String> = subs
                .iter()
                .filter(|s| (keys.contains(&s["key"].display()) || keys.contains(&s["title"].display())) && s["text"].truthy())
                .map(|s| s["text"].display())
                .collect();
            let mut rat = if texts.is_empty() { None } else { Some(texts.join(" · ")) };
            if let Some(r) = &rat {
                if r.chars().count() > _RATIONALE_MAX {
                    let head: String = r.chars().take(_RATIONALE_MAX - 1).collect();
                    rat = Some(format!("{}…", py_rstrip(&head)));
                }
            }
            row.set("rationale", J::from(rat));
            let conf = artifacts::slot_confidence(row)?;
            row.set("confidence", conf);
        }
    }
    Ok(out)
}

pub fn _update_payload_json(payload: &Path, fields: &[(&str, J)]) -> HR<()> {
    let p = payload.join(branch::PAYLOAD_JSON);
    let mut doc = core::read_json_code(&p, "HINT_PAYLOAD_JSON_UNREADABLE")?;
    for (k, v) in fields {
        doc.set(*k, v.clone());
    }
    core::write_json(&p, &doc)
}

fn git_head(repo: &Path) -> HR<Option<String>> {
    let h = core::git_nocheck(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?.stdout.trim().to_string();
    Ok(if h.is_empty() { None } else { Some(h) })
}

pub fn _finalize_provenance(repo: &Path, payload: &Path, st: &J) -> HR<()> {
    let p = payload.join(branch::PROVENANCE_JSON);
    let mut prov = core::read_json_code(&p, "HINT_PROVENANCE_UNREADABLE")?;
    let head = git_head(repo)?;
    let sa = st["source_anchor"].clone();
    prov.set("source_anchor", sa.clone());
    prov.set("source_anchor_is_head", if sa.truthy() { J::Bool(sa.as_str().is_some() && sa.as_str() == head.as_deref()) } else { J::Null });
    let br = core::git_nocheck(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let b = br.stdout.trim();
    prov.set("assembly_branch", if br.returncode == 0 && !b.is_empty() { js(b) } else { J::Null });
    core::write_json(&p, &prov)?;
    prov.set("payload_files", branch::payload_files(payload)?);
    core::write_json(&p, &prov)
}

pub fn _require_draft_equals_anchor(repo: &Path, payload: &Path, anchor: &str) -> HR<()> {
    let mut want: Vec<(String, String)> =
        branch::tree_entries(repo, anchor)?.iter().map(|e| (e["path"].display(), e["sha"].display())).collect();
    let rels = branch::payload_files(payload)?;
    let mut got: Vec<(String, String)> = Vec::new();
    if !rels.is_empty() {
        let input: String = rels.iter().map(|r| format!("{}\n", core::posix(&payload.join(r)))).collect();
        let out = core::git_full(repo, &["hash-object", "--no-filters", "--stdin-paths"], true, Some(&input), &[])?.stdout;
        got = rels.iter().cloned().zip(out.split_whitespace().map(str::to_string)).collect();
    }
    // dict semantics: last value per key wins
    let dedup = |v: &mut Vec<(String, String)>| {
        let mut m: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (k, s) in v.drain(..) {
            m.insert(k, s);
        }
        m.into_iter().collect::<Vec<_>>()
    };
    let (got, want) = (dedup(&mut got), dedup(&mut want));
    if got != want {
        let mut diff: Vec<(String, String)> = got.iter().filter(|x| !want.contains(x)).cloned().collect();
        diff.extend(want.iter().filter(|x| !got.contains(x)).cloned());
        diff.sort();
        let paths: Vec<String> = diff.iter().take(10).map(|(p, _)| p.clone()).collect();
        return fail(
            "HINT_DRAFT_CHANGED_AFTER_COMMIT",
            format!("커밋({}) 뒤 draft 페이로드가 바뀌었다: {}", core::head_chars(anchor, 12), list_repr(&paths)),
            Some(
                "hint 브랜치는 앞으로만 간다 — 개정이 필요하면 새 publish(새 이름)로 낸다. 커밋한 내용으로 되돌리려면 \
                 branch.export_tree 로 앵커 트리를 풀어 draft 를 맞춘다.",
            ),
        );
    }
    Ok(())
}

pub fn _manifest_doc(repo: &Path, st: &J) -> HR<J> {
    core::read_json_code(&repo.join(st["manifest"].display()), "HINT_MANIFEST_UNREADABLE")
}

pub fn _require_binding_artifact(man: &J, repo: &Path, before_commit: bool) -> HR<(String, String)> {
    let Some(r) = super::hooks::binding_artifact_path(man, Some(repo))?.filter(|s| !s.is_empty()) else {
        return fail(
            "HINT_CERTIFICATE_BINDING_ABSENT",
            format!(
                "footer bench_ref 로 묶을 계측 산출물(인증서 · bench_report)이 없다(binding_artifact_path = None){}",
                if before_commit { " — 커밋하지 않았다." } else { "." }
            ),
            Some(
                "인증서가 발행되는 판정(explicit ∧ PASS)이면 인증서를, 그 밖(non-PASS · 비발행 권한 PASS · OBSERVATION-ONLY · map_only)이면 bench_report 를 \
                 발행 기록에 바인딩한다(evidence_publisher).",
            ),
        );
    };
    let Some(kind) = tag::bench_kind_of(&js(&r))? else {
        return fail(
            "HINT_CERTIFICATE_BINDING_ABSENT",
            format!("footer bench_ref 의 종류를 이름에서 가를 수 없다(benchmark_*.yaml | bench_report_*.md): {r}"),
            Some("발행 기록의 바인딩이 docs.md 명명 SSOT 의 계측 산출물을 가리키는지 확인한다."),
        );
    };
    Ok((r, kind))
}

// ── 린트 ───────────────────────────────────────────────────────────────────────────────────────
pub fn _subst_table(repo: &Path, st: &J) -> HR<J> {
    let man = evidence::output_manifest(repo, &st["topology"].display())?.unwrap_or(J::Null);
    let rows = pii::substitution_table(repo, &man)?;
    Ok(J::List(rows.into_iter().map(|(a, b)| jl![a, b]).collect()))
}

pub fn _lint(repo: &Path, payload: &Path, draft: &Path, st: &J, sealed: bool, historical_verify: bool) -> HR<Vec<J>> {
    let lin = _lineage_full(draft, &draft.join("payload"))?;
    let facts = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
    let man = if repo.join(st["manifest"].display()).is_file() { _manifest_doc(repo, st)? } else { J::Null };
    let bench = if st["bench_source"].truthy() { Some(PathBuf::from(st["bench_source"].display())) } else { None };
    let subst = _subst_table(repo, st)?;
    template::lint(
        repo,
        payload,
        &lin,
        &subst,
        &man,
        bench.as_deref(),
        &facts,
        sealed,
        historical_verify,
        &J::Null,
        &J::Null,
        &J::Null,
        &J::Null,
        Some(draft),
        &J::Null,
    )
}

/// tag.verify_local 의 서사 린터: 봉인된 git 오브젝트 트리를 임시 디렉터리에 풀어 sealed 린트.
pub struct LintFn {
    pub draft: Option<PathBuf>,
    pub st: Option<J>,
    pub workspace: Option<PathBuf>,
    pub historical_verify: bool,
}

pub fn _lint_fn(draft: Option<&Path>, st: Option<&J>, workspace: Option<&Path>, historical_verify: bool) -> LintFn {
    LintFn { draft: draft.map(Path::to_path_buf), st: st.cloned(), workspace: workspace.map(Path::to_path_buf), historical_verify }
}

impl LintFn {
    pub fn call(&self, repo: &Path, anchor: &str, tagname: &str) -> HR<J> {
        if let Some(found) = super::hooks::lint_findings() {
            return Ok(found); // self-test: `mock.patch.object(hint, "_lint_fn", return_value=injected)`
        }
        let (Some(draft), Some(st)) = (&self.draft, &self.st) else {
            return Ok(jl![jd! {
                "code" => "HINT_LINT_DRAFT_ABSENT",
                "message" => format!("{tagname} 를 봉인한 draft(inputs/template_facts.json)를 찾지 못했다 — 서사 린트를 건너뛰지 않는다 (--draft 로 지정)"),
            }]);
        };
        let td = core::TempDir::new("hint-verify-");
        let dst = td.path().join("payload");
        branch::export_tree(repo, anchor, &dst)?;
        let ws = self.workspace.clone().unwrap_or_else(|| repo.to_path_buf());
        Ok(J::List(_lint(&ws, &dst, draft, st, true, self.historical_verify)?))
    }
}

pub fn _fail_lint(issues: &[J], sealed: bool) -> HR<()> {
    let codes: std::collections::BTreeSet<String> = issues.iter().map(|i| i["code"].display()).collect();
    let lines: Vec<String> =
        issues.iter().take(40).map(|i| format!("{}  {}:{}  {}", i["code"].display(), i["file"].display(), i["line"].display(), i["message"].display())).collect();
    fail(
        "HINT_LINT_FAILED",
        format!(
            "{}린트 {}건({}) — 커밋하지 않았다:\n  {}{}",
            if sealed { "봉인 뒤 " } else { "" },
            issues.len(),
            codes.into_iter().collect::<Vec<_>>().join(", "),
            lines.join("\n  "),
            if issues.len() > 40 { "\n  …" } else { "" }
        ),
        Some("PROMPT 지시대로 저작을 고친 뒤 `hint.sh lint --draft <draft>` 로 확인하고 continue 를 다시 실행한다."),
    )
}

// ── publish ────────────────────────────────────────────────────────────────────────────────────
/// 증거 → 발행기 구동 전의 수집(쓰기 = draft 안뿐 · publish · refacts 공용).
#[allow(clippy::too_many_arguments)]
pub fn _collect(repo: &Path, ev: &mut J, dn: &naming::DerivedName, utc: &str, topic: &str, draft: &Path, payload: &Path, lineage_add: &[String], docker: &J) -> HR<J> {
    let ident = evidence::identity(ev)?;
    let tool_snaps = evidence::tool_snapshots(repo, ev, draft)?;
    let env_observed = evidence::measurement_env_observed(repo, ev)?;
    let att_scope = evidence::attestation_scope(repo, ev)?;
    let records = J::List(evidence::publication_records(repo)?);
    let lin = lineage::derive(
        repo,
        ev,
        ident["base_slug"].as_str().unwrap_or_default(),
        &core::kst_token(utc)?,
        lineage::DEFAULT_DEPTH,
        &J::from(lineage_add),
        &records,
        Some(topic),
        &J::Null,
        None,
        &[],
        "PAYLOAD.identity.base_slug",
    )?;
    let art = super::hooks::collect(repo, ev, payload, docker)?;
    evidence::bind_image_ledger_mut(ev, &art["build_ledger_vllm"])?;
    let bench_src = _bench_source(repo, ev);
    let mut missing: std::collections::BTreeSet<String> = ev["missing"].items().iter().map(J::display).collect();
    for m in art["missing"].items() {
        missing.insert(m.display());
    }
    let missing: Vec<String> = missing.into_iter().collect();
    let mut reasons = Map::new();
    let err = &ev["native_launch_error"];
    if err.truthy() && missing.iter().any(|m| m == "HINT_MISSING_NATIVE_LAUNCH") {
        let man = evidence::output_manifest(repo, &ev["topology"].display())?.unwrap_or(J::Null);
        let table = pii::substitution_table(repo, &man)?;
        reasons.insert("HINT_MISSING_NATIVE_LAUNCH", format!("producer native_launch_error: {}", pii::substitute(&err.display(), &table, true)?));
    }
    let absence = evidence::campaign_absence(ev, Some(repo))?;
    if missing.iter().any(|m| m == "HINT_MISSING_CAMPAIGN_INSTANCE") && absence["reason"].truthy() {
        reasons.insert("HINT_MISSING_CAMPAIGN_INSTANCE", absence["reason"].clone());
    }
    let vocab = naming::load_vocab(repo)?;
    let collected_facts = evidence::naming_facts(repo, ev)?;
    let collected_name = super::hooks::derive_name(&collected_facts, &vocab)?;
    if collected_name.tag != dn.tag {
        return fail(
            "HINT_DERIVED_NAME_MISMATCH",
            format!("이미지 원장 수집 뒤 기본 이름 {} ≠ 수집 전 {}", py_repr(&collected_name.tag), py_repr(&dn.tag)),
            Some("관측된 빌드 입력을 정정하고 이름을 다시 파생한다. 기존 이름에 새 명명 메타만 덮어쓰지 않는다."),
        );
    }
    let mut naming_base = collected_name.to_payload();
    naming_base.set("vllm_observed", evidence::vllm_observed(ev, Some(repo))?);
    Ok(jd! {
        "ident" => ident, "tool_snaps" => J::List(tool_snaps), "env_observed" => J::List(env_observed), "att_scope" => J::from(att_scope),
        "lineage" => lin, "art" => art, "bench_src" => J::from(bench_src), "missing" => J::from(missing), "missing_reasons" => J::Dict(reasons),
        "naming_base" => naming_base, "tail_sources" => evidence::tail_sources(repo, ev)?,
        "tail_candidates" => J::List(naming::tail_candidates(&collected_facts, &vocab)?),
    })
}

/// 수집 조각 + 발행 기록 → (facts, PAYLOAD.json 문서). 쓰기 0.
#[allow(clippy::too_many_arguments)]
pub fn _assemble(repo: &Path, ev: &J, dn: &naming::DerivedName, col: &J, utc: &str, topic: &str, manifest_rel: &str, man: &J) -> HR<(J, J)> {
    let art = &col["art"];
    let ident = &col["ident"];
    let task_class = man["task_class"].clone();
    let qual = evidence::qualification(ev)?;
    let build = _build_doc(ev, art);
    let measurement = evidence::measurement(ev)?;
    let measurement_config = evidence::measurement_config(repo, ev)?;
    let event_timeline = J::List(evidence::event_timeline(repo, ev)?);
    let event_ledger_spans = J::List(evidence::event_ledger_spans(repo, ev)?);
    let event_campaign_boundary = evidence::event_campaign_boundary(repo, ev)?;
    let bench_definition = evidence::bench_definition(repo, ev)?;
    let grade = template::cell_grade(&jd! {"measurement" => measurement.clone(), "measurement_config" => measurement_config.clone(), "task_class" => task_class.clone()})?;
    let dataset_prov = evidence::dataset_provenance(repo, ev)?;
    let bench_md = if col["bench_src"].truthy() && grade != "lite" { Some(template::render_bench_source(repo, &col["bench_src"])?) } else { None };
    let mut camp = jd! {"id" => ev["campaign_id"].clone(), "cell" => ev["cell"].clone(), "node" => ev["node"].clone(), "mode" => ev["mode"].clone()};
    let sealed = evidence::sealed_publication(repo, ev)?;
    let mut prior_approval = J::Null;
    if let Some(sealed) = sealed.filter(|s| s.truthy()) {
        if camp["id"].is_null() && sealed["campaign"].truthy() {
            camp.set("id", sealed["campaign"]["id"].clone());
            camp.set("id_source", format!("봉인 페이로드 {}:PAYLOAD.json campaign.id(재생 전용 대체)", core::head_chars(&sealed["anchor"].display(), 12)));
        }
        if sealed["approval"].truthy() {
            let mut pa = sealed["approval"].clone();
            pa.set("replay_source", sealed["source"].clone());
            prior_approval = pa;
        }
    }
    if camp["id"].is_null() {
        let mcamp = evidence::measurement_campaign(ev)?;
        match mcamp.get_str("status") {
            Some("in-campaign") => {
                camp.set("id", mcamp["id"].clone());
                camp.set("id_source", format!("{}(재생 전용 대체)", mcamp["source"].display()));
            }
            Some("outside") => {
                camp.set("membership", jd! {"status" => "outside", "id" => mcamp["id"].clone(), "source" => mcamp["source"].clone()});
            }
            _ => {}
        }
    }
    let pubj = jd! {"topic" => topic, "manifest_ref" => manifest_rel, "task_class" => task_class.clone()};
    let lin = &col["lineage"];
    let or_list = |v: &J| if v.truthy() { v.clone() } else { J::list() };
    let art_more = [
        ("agent_requests", or_list(&art["agent_requests"])),
        ("build_context_map", or_list(&art["build_context_map"])),
        ("native_source", art["native_source"].clone()),
    ];
    let mut facts = jd! {
        "tag" => dn.tag.clone(), "base_tag" => dn.tag.clone(), "generated_utc" => utc, "naming" => col["naming_base"].clone(),
        "identity" => ident.clone(), "build" => build.clone(),
        "plane" => art["plane"].clone(), "applied_set" => art["applied_set"].clone(), "slots" => art["slots"].clone(),
        "qualification" => qual.clone(), "measurement" => measurement.clone(), "measurement_config" => measurement_config.clone(),
        "missing" => col["missing"].clone(),
        "missing_reasons" => col["missing_reasons"].clone(), "cell_grade" => grade.clone(), "dataset_provenance" => dataset_prov.clone(),
        "lineage_reading_list" => J::List(lineage::reading_list(lin)?), "value_status_candidates" => art["value_status_candidates"].clone(),
        "reproduce_steps" => _with_bench_command(&art["reproduce_steps"], &evidence::bench_command(repo, ev)?),
        "sub_recipe" => art["sub_recipe"].clone(), "bench_section_md" => J::from(bench_md),
        "task_class" => task_class.clone(), "campaign" => camp.clone(), "approval" => J::Null,
        "prior_approval" => prior_approval.clone(), "publication" => pubj.clone(), "event_timeline" => event_timeline,
        "event_ledger_spans" => event_ledger_spans,
        "event_campaign_boundary" => event_campaign_boundary, "bench_definition" => bench_definition.clone(), "factcheck" => J::Null,
        "measurement_env_observed" => col["env_observed"].clone(), "tool_snapshots" => col["tool_snaps"].clone(), "attestation_scope" => col["att_scope"].clone(),
        "tail_candidates" => col["tail_candidates"].clone(), "env_shapes" => or_list(&art["env_shapes"]),
    };
    for (k, v) in &art_more {
        facts.set(*k, v.clone());
    }
    let mut qsub = Map::new();
    for k in ["health_200", "inference_observed", "sources", "method"] {
        qsub.insert(k, qual[k].clone());
    }
    let pointers: Vec<J> = ev["pointers"]
        .items()
        .iter()
        .map(|p| {
            let mut m = Map::new();
            for k in ["kind", "path", "cell_id", "node_id"] {
                m.insert(k, p[k].clone());
            }
            J::Dict(m)
        })
        .collect();
    let mut doc = jd! {
        "schema_version" => PAYLOAD_SCHEMA_VERSION, "format" => branch::PAYLOAD_FORMAT, "tag" => dn.tag.clone(), "generated_utc" => utc,
        "campaign" => camp, "identity" => ident.clone(), "naming" => col["naming_base"].clone(), "plane" => art["plane"].clone(), "build" => build,
        "applied_set" => art["applied_set"].clone(), "slots" => art["slots"].clone(),
        "qualification" => J::Dict(qsub),
        "measurement" => measurement, "measurement_config" => measurement_config, "missing" => col["missing"].clone(),
        "cell_grade" => grade, "dataset_provenance" => dataset_prov,
        "missing_reasons" => col["missing_reasons"].clone(), "approval" => J::Null, "prior_approval" => prior_approval,
        "evidence_pointers" => J::List(pointers),
        "publication" => pubj, "bench_definition" => bench_definition, "factcheck" => J::Null,
        "measurement_env_observed" => col["env_observed"].clone(), "tool_snapshots" => col["tool_snaps"].clone(), "attestation_scope" => col["att_scope"].clone(),
    };
    for (k, v) in &art_more {
        doc.set(*k, v.clone());
    }
    Ok((facts, doc))
}

/// PAYLOAD.json · LINEAGE(전체 · 수신자 요약) · PROVENANCE · 이름 확정 입력.
pub fn _write_payload_meta(draft: &Path, payload: &Path, tag_name: &str, utc: &str, doc: &J, col: &J) -> HR<()> {
    let inputs = draft.join("inputs");
    std::fs::create_dir_all(&inputs).map_err(|e| core::io_err(&e, &inputs))?;
    core::write_json(&inputs.join(lineage::LINEAGE_FULL_NAME), &col["lineage"])?;
    core::write_json(&payload.join("LINEAGE.json"), &lineage::receiver_summary(&col["lineage"], &J::Null, &J::Null)?)?;
    core::write_json(&inputs.join(NAMING_BASE_NAME), &col["naming_base"])?;
    core::write_json(&inputs.join(TAIL_SOURCES_NAME), &col["tail_sources"])?;
    core::write_json(&payload.join(branch::PAYLOAD_JSON), doc)?;
    let mut prov = jd! {
        "schema_version" => PROVENANCE_SCHEMA_VERSION, "tag" => tag_name, "source_anchor" => J::Null,
        "source_anchor_is_head" => J::Null, "assembly_branch" => J::Null, "payload_files" => J::list(), "generated_utc" => utc,
    };
    core::write_json(&payload.join(branch::PROVENANCE_JSON), &prov)?;
    prov.set("payload_files", branch::payload_files(payload)?);
    core::write_json(&payload.join(branch::PROVENANCE_JSON), &prov)
}

/// publish 의 입력.
pub struct PublishArgs<'a> {
    pub campaign: Option<&'a str>,
    pub cell: Option<&'a str>,
    pub publication: Option<&'a str>,
    pub replay: bool,
    pub node: Option<&'a str>,
    pub generated_utc: Option<&'a str>,
    pub lineage_add: &'a [String],
    pub out: Option<&'a str>,
    pub remote: &'a str,
    pub remote_check: bool,
    pub docker: J,
}

/// 셀 1개 → 스캐폴드 → 정지. 반환 = 요약(draft · payload 는 경로 문자열).
pub fn publish(repo: &Path, a: &PublishArgs) -> HR<J> {
    let repo = core::resolve(repo);
    let repo = repo.as_path();
    let mut utc = core::require_utc(a.generated_utc, "--generated-utc")?;
    _check_selectors(a.campaign, a.cell, a.publication, a.replay, true)?;
    let out_dir = a.out.filter(|s| !s.is_empty()).map(|o| core::resolve(Path::new(o)));
    if let Some(o) = &out_dir {
        _require_fresh_draft(o)?;
    }
    let mut ev = _evidence(repo, a.campaign, a.cell, a.publication, a.node, &a.docker)?;
    evidence::qualification(&ev)?;
    artifacts::bind_image_git_release(repo, &mut ev, &a.docker)?;
    let label = evidence::topology_label(&evidence::identity(&ev)?)?;
    let dn = super::hooks::derive_name(&evidence::naming_facts(repo, &ev)?, &naming::load_vocab(repo)?)?;
    tag::check_ref_format(repo, &dn.tag)?;
    _require_name_pii_clean(repo, &dn.tag)?;
    let (base_hit, remote_note) = _name_hit(repo, &dn.tag, a.remote, a.remote_check, a.replay || !a.remote_check)?;
    let topic = evidence::topic_for(&ev)?;
    let is_campaign = ev.get_str("mode") == Some("campaign");
    let draft = out_dir.unwrap_or_else(|| repo.join(core::REL_DRAFTS).join(if is_campaign { topic.clone() } else { format!("replay__{topic}") }));
    _require_fresh_draft(&draft)?;
    let payload = draft.join("payload");
    if is_campaign {
        utc = _require_unbound_publication_time(repo, &topic, &utc)?;
    }
    let col = _collect(repo, &mut ev, &dn, &utc, &topic, &draft, &payload, a.lineage_add, &a.docker)?;
    let (manifest_path, gate) = if is_campaign {
        let mp = super::hooks::drive_publisher(repo, &ev, &js(&topic), &js(&utc), &draft)?;
        _gate(repo, &p_j(&mp), "continue")?;
        (mp, "allowed(hint_finalize 사전 확인 · 부수효과 전)")
    } else {
        let mp = &ev["publication"]["manifest_path"];
        if !mp.truthy() {
            return fail("HINT_PUBLICATION_MANIFEST_ABSENT", format!("발행 기록 {topic} 의 work-manifest 가 없다 — 재생할 승격 기록이 없다."), None);
        }
        let mp = repo.join(mp.display());
        _gate(repo, &p_j(&mp), "continue")?;
        (mp, "allowed(hint_finalize 사전 확인 · 읽기 전용 · 재생)")
    };
    let manifest_rel = core::rel(repo, &manifest_path)?;
    let man = core::read_json_code(&manifest_path, "HINT_MANIFEST_UNREADABLE")?;
    let (facts, doc) = _assemble(repo, &ev, &dn, &col, &utc, &topic, &manifest_rel, &man)?;
    let prompts = template::render_scaffold(repo, &facts, &payload, None)?;
    _write_payload_meta(&draft, &payload, &dn.tag, &utc, &doc, &col)?;
    let head = git_head(repo)?;
    let approval_note = _approval_probe(repo, &ev)?;
    let lin = &col["lineage"];
    let st = jd! {
        "schema_version" => STATE_SCHEMA, "stage" => "scaffolded", "mode" => ev["mode"].clone(),
        "node_arg" => J::from(a.node),
        "selectors" => jd! {"campaign" => ev["campaign_id"].clone(), "cell" => ev["cell"].clone(), "node" => ev["node"].clone(),
                            "publication" => if is_campaign { J::Null } else { js(&topic) }},
        "tag" => dn.tag.clone(), "base_tag" => dn.tag.clone(), "topic" => topic.clone(), "manifest" => manifest_rel.clone(), "topology" => ev["topology"].clone(),
        "topology_label" => label, "generated_utc" => utc.clone(), "bench_source" => col["bench_src"].clone(), "source_anchor" => J::from(head),
        "remote_check" => remote_note.clone(), "base_name_exists" => J::from(base_hit.clone()), "gate_precheck" => gate, "approval_at_publish" => approval_note.clone(),
        "prompts" => prompts.len() as i64, "lineage_documents" => lin["documents"].len() as i64, "lineage_add" => J::from(a.lineage_add),
        "steps" => jd! {"publish" => jd! {"utc" => utc.clone(), "missing" => col["missing"].clone()}},
    };
    _save_state(&draft, &st)?;
    let agent_requests: Vec<J> = col["art"]["agent_requests"].items().iter().filter(|r| r.is_dict()).map(|r| r["path"].clone()).collect();
    let tool_snapshots: Vec<J> = col["tool_snaps"]
        .items()
        .iter()
        .filter(|x| x.is_dict() && x["snapshot_rel"].truthy())
        .map(|x| js(&py_basename(&x["snapshot_rel"].display())))
        .collect();
    Ok(jd! {
        "tag" => dn.tag.clone(), "draft" => p_j(&draft), "payload" => p_j(&payload), "topic" => topic, "mode" => ev["mode"].clone(), "manifest" => manifest_rel,
        "prompts" => J::List(prompts), "reading_list" => J::List(lineage::reading_list(lin)?), "missing" => col["missing"].clone(), "remote_check" => remote_note,
        "gate" => gate, "approval" => approval_note, "files" => J::from(branch::payload_files(&payload)?), "state" => st,
        "applied" => _applied_summary(&col["art"]["applied_set"]), "generated_utc" => utc,
        "tail_candidates" => col["tail_candidates"].clone(), "base_name_exists" => J::from(base_hit),
        "agent_requests" => J::List(agent_requests), "tool_snapshots" => J::List(tool_snapshots),
    })
}

/// publish 출력용 적용 판정 요약(읽기 전용).
pub fn _applied_summary(aset: &J) -> J {
    let empty = J::dict();
    let aset = if aset.is_dict() { aset } else { &empty };
    let patches: Vec<&J> = aset["patches"].items().iter().filter(|x| x.is_dict()).collect();
    let probe = aset["probes"].items().iter().find(|x| x.is_dict() && x.get_str("tier") == Some("image-probe")).map(|x| js(&x["result"].display()));
    let unobserved = patches.iter().filter(|x| disp_or_empty(&x["result"]).starts_with("unobserved")).count() as i64;
    jd! {"total" => patches.len() as i64, "unobserved" => unobserved, "image_probe" => J::from(probe)}
}

/// publish 출력용 승인 상태(읽기 전용).
pub fn _approval_probe(repo: &Path, ev: &J) -> HR<String> {
    if ev.get_str("mode") != Some("campaign") {
        return Ok("캠페인 밖 — continue 에 --approved-by/--approved-utc(사람 발화 전사)".into());
    }
    let ap = match evidence::approval_for(repo, &ev["campaign_id"], &ev["cell"], &ev["node"]) {
        Ok(a) => a,
        Err(e) if !e.is_py() => return Ok(format!("기록 결함 {} — continue 가 막힌다(campaign_init --hint-approve 로 다시 적는다)", e.code)),
        Err(e) => return Err(e),
    };
    match ap {
        None => Ok("없음 — continue 전에 사람 Y/N 을 받아 campaign_init --hint-approve 로 적는다".into()),
        Some(ap) => Ok(format!("있음({} · {})", ap["hitl_source"].display(), ap["approved_utc"].display())),
    }
}

fn pad_right(s: &str, w: usize) -> String {
    format!("{s:<w$}")
}

pub fn _print_publish(repo: &Path, res: &J) {
    let out = |s: String| io::outln(&s);
    let d = _show(repo, Path::new(&res["draft"].display()));
    out(format!("{_OUT} publish — 스캐폴드 완료 · 정지(태그·브랜치 부수효과 0)"));
    out(format!("  기본 이름 {}  (꼬리 전 — continue 가 inputs/tail.json 으로 확정 · 중복이면 -t<YYMMDDHHMM>)", res["tag"].display()));
    if res["base_name_exists"].truthy() {
        out(format!("  ⓘ 기본 이름이 이미 있다({}) — 꼬리가 가르지 않으면 이 발행은 timestamp 새 판이 된다", res["base_name_exists"].display()));
    }
    if res["generated_utc"].truthy() {
        out(format!(
            "  시각     {}{}",
            res["generated_utc"].display(),
            if res.get_str("mode") != Some("campaign") {
                "(재생 draft 의 주입 시각 · 발행 기록의 원래 시각과 무관)"
            } else {
                "(발행 기록에 묶인 값 · 명시와 다르면 위 로그가 채택을 알렸다)"
            }
        ));
    }
    out(format!("  draft    {d}"));
    out(format!("  모드     {} · 토픽 {} · work-manifest {}", res["mode"].display(), res["topic"].display(), res["manifest"].display()));
    out(format!("  원격 충돌 확인 {} · 게이트 {}", res["remote_check"].display(), res["gate"].display()));
    out(format!("  승인     {}", res["approval"].display()));
    let missing: Vec<String> = res["missing"].items().iter().map(J::display).collect();
    out(format!("  결손     {}", if missing.is_empty() { "없음".to_string() } else { missing.join(", ") }));
    let ap = &res["applied"];
    if ap["total"].truthy() {
        let probe = if ap["image_probe"].truthy() { ap["image_probe"].display() } else { "기록 없음".into() };
        out(format!("  적용 판정 빌드 패치 {}개 중 미관측 {}개 · 이미지 탐침 {probe}", ap["total"].display(), ap["unobserved"].display()));
        if ap["unobserved"].truthy() && !disp_or_empty(&ap["image_probe"]).starts_with("done") {
            out("  ⚠ 이미지 탐침이 돌지 않은 채 판정했다 — 탐침(시작하지 않는 컨테이너의 create · cp · rm)은 기본이다: `--docker-read-only` \
                 로 껐다면 빼고 다시 publish 한다(새 --out 또는 draft 를 지운 뒤) · 측정 이미지가 이 호스트에 없으면 탐침할 수 없다\
                 (미관측 그대로 기재 · 이미지를 받거나 다시 지을지는 사람 결정)."
                .into());
        }
    }
    out(format!("  페이로드 파일 {}개", res["files"].len()));
    let cands: Vec<&J> = res["tail_candidates"].items().iter().filter(|c| c.is_dict()).collect();
    if !cands.is_empty() {
        let parts: Vec<String> = cands
            .iter()
            .map(|c| format!("{}({})", if c["token"].truthy() { c["token"].display() } else { c["error"].display() }, c["axis"].display()))
            .collect();
        out(format!("  꼬리 후보(강제 ✗ · 00 §0.9 · 이 셀을 가르는 노브만): {}", parts.join(" · ")));
    }
    for rel in res["agent_requests"].items() {
        out(format!("  Agent 저작 요청 {} — 첫 줄 경고를 지키고 본문을 쓴다(렌더러로 만들 수 없는 자리)", rel.display()));
    }
    let prompts = res["prompts"].items();
    let todo = prompts.iter().filter(|p| !p["optional"].truthy()).count();
    out(format!("\n채워야 할 PROMPT {todo}개(선택 {}개) — `<<AGENT:` 줄을 지시대로 바꾼다:", prompts.len() - todo));
    for p in prompts {
        let mut extra: Vec<String> = Vec::new();
        if p["blocks"].truthy() {
            if let Some(m) = p["blocks"].as_dict() {
                let parts: Vec<String> = m.iter().map(|(k, v)| if v.truthy() { format!("{k}≥{}", v.display()) } else { k.to_string() }).collect();
                extra.push(format!("블록 {}", parts.join(", ")));
            }
        }
        if p["excerpts_required"].truthy() {
            extra.push(format!("발췌 ≥{}", p["excerpts_required"].display()));
        }
        if p["optional"].truthy() {
            extra.push("선택".into());
        }
        let title = p.get("title").map(J::display).unwrap_or_default();
        out(format!(
            "  {} §{} {} — {}{}",
            p["file"].display(),
            p["section"].display(),
            title,
            p["question"].display(),
            if extra.is_empty() { String::new() } else { format!("  [{}]", extra.join(" · ")) }
        ));
    }
    let rl = res["reading_list"].items();
    let total: i64 = rl.iter().map(|x| py_int(&x["bytes"])).sum();
    out(format!("\nLINEAGE 읽기 목록 {}건 · {} B(계층 → 깊이 → 관련도 → 날짜 · 모호 해소 문서는 뒤):", rl.len(), thousands(total)));
    for x in rl.iter().take(60) {
        let kind = if x["kind"].truthy() { x["kind"].display() } else { "?".into() };
        out(format!(
            "  d{} {} {}  ({} B{})",
            x["depth"].display(),
            pad_right(&kind, 10),
            x["path"].display(),
            thousands(py_int(&x["bytes"])),
            if x.get_str("tier") == Some("ambiguous") { " · 모호 해소" } else { "" }
        ));
    }
    if rl.len() > 60 {
        out(format!("  … 외 {}건 — {d}/payload/LINEAGE.json", rl.len() - 60));
    }
    let cli = _cli(repo);
    out("\n발췌는 **치환 후 원문**에서 그대로 옮긴다(자리표시를 추측하지 않는다):".into());
    out(format!("  {cli} excerpt --draft {d} --source <문서 stem|경로|artifacts 파일명|<도구>@<rev12>> [--lines a-b] [--numbered]"));
    out("  (비-마크다운 출처의 발췌 머리 `§L<a>-<b>` 는 --numbered 가 줄마다 붙이는 `L<n>` 번호 그대로)".into());
    let snaps: Vec<String> = res["tool_snapshots"].items().iter().map(J::display).collect();
    out(format!(
        "  측정 도구 원문 스냅샷(발췌 출처 토큰 · 03 §3.4): {}",
        if snaps.is_empty() { "없음 — 도구가 정한 인자는 원문 없이 단정하지 않는다".to_string() } else { snaps.join(" · ") }
    ));
    out("저작 중 확인(부수효과 0):".into());
    out(format!("  {cli} lint --draft {d}"));
    let mut nxt = format!("{cli} continue --draft {d} --generated-utc {_UTC_EXAMPLE}");
    if res.get_str("mode") != Some("campaign") {
        nxt.push_str(" --approved-by \"<사람 발화 전사>\" --approved-utc <UTC>");
    }
    out(format!("다음:\n  {nxt}"));
}

pub fn cmd_publish(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let lineage_add = a.l("lineage_add");
    let (campaign, cell, publication, node, gu, out, remote) =
        (a.s("campaign"), a.s("cell"), a.s("publication"), a.s("node"), a.s("generated_utc"), a.s("out"), a.s("remote").unwrap_or_else(|| "origin".into()));
    let pa = PublishArgs {
        campaign: campaign.as_deref(),
        cell: cell.as_deref(),
        publication: publication.as_deref(),
        replay: a.b("replay"),
        node: node.as_deref(),
        generated_utc: gu.as_deref(),
        lineage_add: &lineage_add,
        out: out.as_deref(),
        remote: &remote,
        remote_check: !a.b("no_remote_check"),
        docker: _publish_runner(a),
    };
    let res = publish(&repo, &pa)?;
    _print_publish(&repo, &res);
    Ok(0)
}

// ── 이름 확정 · 안내 커밋 · 봉인 출처 ───────────────────────────────────────────────────────────
pub fn _read_tail(draft: &Path) -> HR<J> {
    let p = draft.join("inputs").join(TAIL_NAME);
    if !p.is_file() {
        return fail(
            "HINT_TAIL_ABSENT",
            format!("이름 꼬리가 없다(draft inputs/{TAIL_NAME}) — 빈 꼬리도 `[]` 로 명시한다."),
            Some("00-hint.md §0.9 PROMPT 대로 `inputs/tail.json` 을 쓴다(후보 표 참고 · 근거 = 이 셀의 서빙 설정 file · key · value)."),
        );
    }
    let doc = match load_json_typed(&p) {
        Ok(d) => d,
        Err((ty, e)) => return fail("HINT_TAIL_FORMAT", format!("inputs/{TAIL_NAME} 를 JSON 으로 읽을 수 없다: {ty}: {e}"), None),
    };
    if !doc.is_list() {
        return fail("HINT_TAIL_FORMAT", format!("inputs/{TAIL_NAME} 최상위는 행 목록이다(빈 꼬리 = []): {}", doc.type_name()), None);
    }
    Ok(doc)
}

fn read_tail_sources(draft: &Path) -> HR<J> {
    let p = draft.join("inputs").join(TAIL_SOURCES_NAME);
    if p.is_file() {
        core::read_json_code(&p, "HINT_TAIL_SOURCES_UNREADABLE")
    } else {
        Ok(J::dict())
    }
}

/// lint 용 꼬리 판정(부수효과 0).
pub fn _tail_issues(draft: &Path) -> HR<Vec<J>> {
    let rel = format!("inputs/{TAIL_NAME}");
    let tail = match _read_tail(draft) {
        Ok(t) => t,
        Err(e) if !e.is_py() => return Ok(vec![jd! {"code" => e.code, "file" => rel, "line" => 1, "message" => e.message}]),
        Err(e) => return Err(e),
    };
    let src = read_tail_sources(draft)?;
    Ok(naming::validate_tail(&tail, &src)?.into_iter().map(|(c, m)| jd! {"code" => c, "file" => rel.clone(), "line" => 1, "message" => m}).collect())
}

pub fn _confirm_name(repo: &Path, draft: &Path, st: &mut J, payload: &Path, remote: &str, tolerate_remote: bool, git_repo: Option<&Path>) -> HR<J> {
    let tail = _read_tail(draft)?;
    let sources = read_tail_sources(draft)?;
    let probs = naming::validate_tail(&tail, &sources)?;
    if let Some((c0, _)) = probs.first() {
        let lines: Vec<String> = probs.iter().take(12).map(|(c, m)| format!("{c}  {m}")).collect();
        return fail(
            c0,
            format!("이름 꼬리 결함 {}건(inputs/{TAIL_NAME}) — 커밋하지 않았다:\n  {}", probs.len(), lines.join("\n  ")),
            Some("근거는 이 셀의 서빙 설정(inputs/tail_sources.json 에 있는 파일 · 키)에서 값 그대로 옮긴다 · `hint.sh lint` 로 미리 본다."),
        );
    }
    let base = if st["base_tag"].truthy() { st["base_tag"].display() } else { st["tag"].display() };
    let mut name = naming::compose_name(&base, &tail)?;
    let g = git_repo.unwrap_or(repo);
    let (hit, mut note) = _name_hit(g, &name, remote, true, tolerate_remote)?;
    let mut ts: Option<String> = None;
    if hit.is_some() {
        let gu = st["generated_utc"].display();
        ts = Some(naming::timestamp_token(&gu)?);
        name = naming::with_timestamp(&name, &gu)?;
        let (hit2, n2) = _name_hit(g, &name, remote, true, tolerate_remote)?;
        note = n2;
        if let Some(h2) = hit2 {
            return fail(
                "HINT_NAME_COLLISION",
                format!("timestamp 를 붙인 이름도 이미 있다({h2}): {name} — 같은 분에 같은 셀 2건."),
                Some("publish 를 새 --generated-utc 로 다시 실행한다(새 판 · 옛 태그는 교정하지 않는다 · P1)."),
            );
        }
    }
    tag::check_ref_format(g, &name)?;
    _require_name_pii_clean(repo, &name)?;
    let base_doc = core::read_json_code(&draft.join("inputs").join(NAMING_BASE_NAME), "HINT_NAMING_BASE_UNREADABLE")?;
    let naming_doc = naming::apply_tail(&base_doc, &tail, ts.as_deref())?;
    let changed = template::update_facts(repo, payload, &jd! {"tag" => name.clone(), "naming" => naming_doc.clone()}, None)?;
    let pj = payload.join(branch::PAYLOAD_JSON);
    let mut doc = core::read_json_code(&pj, "HINT_PAYLOAD_JSON_UNREADABLE")?;
    if !py_eq(&doc["tag"], &js(&name)) || !doc.get("naming").map(|n| py_eq(n, &naming_doc)).unwrap_or(false) {
        doc.set("tag", name.clone());
        doc.set("naming", naming_doc.clone());
        core::write_json(&pj, &doc)?;
    }
    let pp = payload.join(branch::PROVENANCE_JSON);
    let mut prov = core::read_json_code(&pp, "HINT_PROVENANCE_UNREADABLE")?;
    if !py_eq(&prov["tag"], &js(&name)) {
        prov.set("tag", name.clone());
        core::write_json(&pp, &prov)?;
    }
    let tail_tokens: Vec<J> = tail.items().iter().filter(|r| r.is_dict()).map(|r| r["token"].clone()).collect();
    let rec = jd! {
        "base" => base, "final" => name.clone(), "tail" => J::List(tail_tokens), "timestamp" => J::from(ts.clone()),
        "collision" => J::from(hit.clone()), "remote_check" => note, "facts_changed" => J::from(changed),
    };
    if st["tag"].as_str() != Some(name.as_str()) {
        let extra = match &ts {
            Some(t) => format!(" (기본 이름 · 꼬리 합친 이름이 이미 있어({}) timestamp {t} 를 붙였다)", J::from(hit.clone()).display()),
            None => String::new(),
        };
        _log(&format!("이름 확정: {name}{extra}"));
    }
    st.set("tag", name);
    _mark(draft, st, "name", rec.clone())?;
    Ok(rec)
}

pub fn _resolve_guide(repo: &Path, st: &mut J, draft: &Path, remote: &str, pinned: Option<&str>, no_push: bool) -> HR<J> {
    if pinned.is_some() && !no_push {
        return fail(
            "HINT_GUIDE_PIN_REQUIRES_NO_PUSH",
            "--guide-commit(원격 조회 없는 안내 커밋 지정)은 --no-push(오프라인 재생)와만 쓴다.",
            Some("push 까지 가는 발행은 원격 hint 브랜치의 안내 커밋을 읽어 부모로 삼는다(지정 ✗)."),
        );
    }
    let (sha, fmt) = branch::guide_commit(repo, remote, pinned)?;
    let source =
        if pinned.is_some() { "pinned(--guide-commit · 원격 조회 ✗ · 오프라인 재생)".to_string() } else { format!("{remote}: ls-remote {}(읽기)", core::HINT_BRANCH_REF) };
    let g = jd! {"sha" => sha, "format" => fmt, "source" => source};
    if !st.get("guide").map(|x| py_eq(x, &g)).unwrap_or(false) {
        st.set("guide", g.clone());
        _save_state(draft, st)?;
    }
    Ok(g)
}

pub fn _seal_lineage(repo: &Path, draft: &Path, st: &J, payload: &Path) -> HR<J> {
    let full = _lineage_full(draft, payload)?;
    let cited = template::cited_sources(repo, payload, &full, Some(draft))?;
    let mut paths: Vec<J> = cited["paths"].items().to_vec();
    if st["bench_source"].truthy() {
        paths.push(st["bench_source"].clone());
    }
    let snap = lineage::sealed_sources(repo, &J::List(paths))?;
    let summ = lineage::receiver_summary(&full, &cited["excerpt_counts"], &J::List(snap.clone()))?;
    let p = payload.join("LINEAGE.json");
    if !p.is_file() || !lineage_same(&p, &summ)? {
        core::write_json(&p, &summ)?;
    }
    Ok(jd! {"sealed_sources" => snap.len() as i64, "excerpt_sources" => cited["excerpt_counts"].len() as i64})
}

/// `json.loads(p.read_text()) == json.loads(core.dumps(summary))` (an unreadable file is Python's exception).
fn lineage_same(p: &Path, summary: &J) -> HR<bool> {
    let cur = match load_json_typed(p) {
        Ok(v) => v,
        Err((ty, e)) => return Err(py_err(&ty, e)),
    };
    let want = crate::value::loads(&core::dumps(summary)).map_err(|e| py_err("JSONDecodeError", e))?;
    Ok(py_eq(&cur, &want))
}

pub fn _refresh_lineage_summary(repo: &Path, draft: &Path, payload: &Path) -> HR<bool> {
    let full = _lineage_full(draft, payload)?;
    let cited = template::cited_sources(repo, payload, &full, Some(draft))?;
    let summary = lineage::receiver_summary(&full, &cited["excerpt_counts"], &J::Null)?;
    let path = payload.join("LINEAGE.json");
    if path.is_file() && lineage_same(&path, &summary)? {
        return Ok(false);
    }
    core::write_json(&path, &summary)?;
    Ok(true)
}

/// 린트 · 발췌 · 봉인 스냅샷이 읽는 전체 계보(draft `inputs/LINEAGE.full.json` · v6 draft 는 페이로드 LINEAGE.json).
pub fn _lineage_full(draft: &Path, payload: &Path) -> HR<J> {
    let full = draft.join("inputs").join(lineage::LINEAGE_FULL_NAME);
    if full.is_file() {
        return core::read_json_code(&full, "HINT_LINEAGE_UNREADABLE");
    }
    let doc = core::read_json_code(&payload.join("LINEAGE.json"), "HINT_LINEAGE_UNREADABLE")?;
    if doc.is_dict() && doc.get_str("kind") != Some(lineage::SUMMARY_KIND) {
        return Ok(doc);
    }
    fail(
        "HINT_LINEAGE_UNREADABLE",
        format!("전체 계보가 없다(draft inputs/{}) — 페이로드 LINEAGE.json 은 수신자 요약이다.", lineage::LINEAGE_FULL_NAME),
        Some("`hint.sh refacts --draft <draft>` 로 계보를 다시 파생한다."),
    )
}

/// continue 끝 카탈로그 커밋 → {status: committed|unchanged|skipped-dirty, …}.
pub fn _commit_catalog(repo: &Path, tagname: &str) -> HR<J> {
    let st = core::git_nocheck(repo, &["status", "--porcelain=v1", "-z", "--untracked-files=no"])?;
    if st.returncode != 0 {
        return fail("HINT_CATALOG_COMMIT_FAILED", format!("git status 실패(rc={}): {}", st.returncode, core::tail_chars(st.stderr.trim(), 300)), None);
    }
    let changed: Vec<String> = st.stdout.split('\0').filter(|e| e.chars().count() > 3).map(|e| e.chars().skip(3).collect()).collect();
    let mut others: Vec<String> = changed.iter().filter(|x| !_CATALOG_PATHS.contains(&x.as_str())).cloned().collect();
    others.sort();
    let mut untracked: Vec<String> = Vec::new();
    for x in _CATALOG_PATHS {
        if repo.join(x).is_file() && core::git_nocheck(repo, &["ls-files", "--error-unmatch", "--", x])?.returncode != 0 {
            untracked.push(x.to_string());
        }
    }
    let mut mine: std::collections::BTreeSet<String> = changed.iter().filter(|x| _CATALOG_PATHS.contains(&x.as_str())).cloned().collect();
    mine.extend(untracked);
    let mine: Vec<String> = mine.into_iter().collect();
    if mine.is_empty() {
        return Ok(jd! {"status" => "unchanged"});
    }
    if !others.is_empty() {
        let head: Vec<String> = others.iter().take(5).cloned().collect();
        _log(&format!(
            "⚠ 카탈로그를 커밋하지 않았다 — 추적 파일에 다른 변경 {}건이 있다({}). 카탈로그({})는 워킹트리에 남았다: 다른 변경을 정리한 뒤 두 경로만 커밋한다.",
            others.len(),
            list_repr(&head),
            mine.join(", ")
        ));
        return Ok(jd! {"status" => "skipped-dirty", "others" => J::from(others)});
    }
    let mut add: Vec<&str> = vec!["add", "--"];
    add.extend(mine.iter().map(String::as_str));
    core::git(repo, &add)?;
    let msg = format!("chore(hint): 카탈로그 파생 — {tagname} 발행 반영");
    let mut cm: Vec<&str> = vec!["commit", "-q", "-m", &msg, "--"];
    cm.extend(mine.iter().map(String::as_str));
    let r = core::git_nocheck(repo, &cm)?;
    if r.returncode != 0 {
        let text = if !r.stderr.is_empty() { r.stderr.clone() } else { r.stdout.clone() };
        return fail(
            "HINT_CATALOG_COMMIT_FAILED",
            format!("카탈로그 커밋 실패(rc={} — 훅 · 신원 설정을 확인한다 · 우회 ✗): {}", r.returncode, core::tail_chars(py_strip(&text), 400)),
            Some("원인을 고친 뒤 continue 를 다시 실행한다(카탈로그 파생 · 커밋은 멱등)."),
        );
    }
    let sha = core::git(repo, &["rev-parse", "HEAD"])?.stdout.trim().to_string();
    Ok(jd! {"status" => "committed", "commit" => sha, "paths" => J::from(mine)})
}

pub fn _hint_clone_repo(workspace: &Path, hint_clone: &Path) -> HR<PathBuf> {
    let g = core::resolve(hint_clone);
    if core::git_nocheck(&g, &["rev-parse", "--is-inside-work-tree"]).map(|p| p.returncode).unwrap_or(1) != 0 {
        return fail(
            "HINT_CLONE_NOT_A_REPO",
            format!("--hint-clone {} 이 git 저장소가 아니다.", core::posix(&g)),
            Some("terraforming_node 서브 설치 절차(hint 발행 장착)가 만든 hint 클론 경로를 준다."),
        );
    }
    if g == core::resolve(workspace) {
        return fail("HINT_CLONE_IS_WORKSPACE", "--hint-clone 이 워크스페이스 자신이다 — 별도 저장소여야 한다(SUB_GIT_LOCAL_ONLY).", None);
    }
    Ok(g)
}

// ── continue ───────────────────────────────────────────────────────────────────────────────────
pub struct ContinueArgs<'a> {
    pub generated_utc: Option<&'a str>,
    pub approved_by: Option<&'a str>,
    pub approved_utc: Option<&'a str>,
    pub remote: &'a str,
    pub no_push: bool,
    pub factcheck_waiver: Option<&'a str>,
    pub guide_commit: Option<&'a str>,
    pub docker: J,
}

pub fn continue_(repo: &Path, draft: &Path, a: &ContinueArgs, hint_clone: Option<&Path>) -> HR<J> {
    let Some(hc) = hint_clone else { return _continue(repo, draft, a, None) };
    let g = _hint_clone_repo(repo, hc)?;
    let prev = core::set_aux_root(Some(&core::resolve(repo)));
    let r = _continue(repo, draft, a, Some(&g));
    core::set_aux_root(prev.as_deref());
    r
}

fn _continue(repo: &Path, draft: &Path, a: &ContinueArgs, git_repo: Option<&Path>) -> HR<J> {
    let repo = core::resolve(repo);
    let draft = core::resolve(draft);
    let (repo, draft) = (repo.as_path(), draft.as_path());
    let g = git_repo.unwrap_or(repo);
    let utc_arg = core::require_utc(a.generated_utc, "--generated-utc")?;
    let mut st = _load_state(draft)?;
    let payload = draft.join("payload");
    let topic = st["topic"].display();
    let label = st["topology_label"].display();
    // ① 승인 — 부수효과 0
    let approval = _approval(repo, &st, a.approved_by, a.approved_utc)?;
    let waiver = match a.factcheck_waiver {
        Some(w) => Some(_one_line_utterance(Some(w), "HINT_FACTCHECK_WAIVER_INVALID", "--factcheck-waiver")?),
        None => None,
    };
    if a.guide_commit.is_some() && !a.no_push {
        return fail("HINT_GUIDE_PIN_REQUIRES_NO_PUSH", "--guide-commit(원격 조회 없는 안내 커밋 지정)은 --no-push(오프라인 재생)와만 쓴다.", None);
    }
    if !a.no_push {
        _require_central(repo, "continue(push 포함)")?;
    }
    let commit_step = _step(&st, "commit");
    if let Some(cs) = &commit_step {
        _require_draft_equals_anchor(g, &payload, &cs["anchor"].display())?;
    }
    let utc = if st["commit_utc"].truthy() { st["commit_utc"].display() } else { utc_arg.clone() };
    if st["commit_utc"].truthy() && st["commit_utc"].display() != utc_arg {
        _log(&format!("커밋 시각은 첫 커밋 때 적은 {} 를 다시 쓴다(재개 · 같은 메시지·시각이어야 중복 커밋이 없다).", st["commit_utc"].display()));
    }
    if commit_step.is_none() {
        let facts = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
        let slots_in = if facts["slots"].truthy() { facts["slots"].clone() } else { J::dict() };
        let slots = _slot_rationales(&payload, &slots_in)?;
        let (fc_summary, fc_err) = _factcheck_gate(draft, &st, waiver.as_deref(), &utc_arg)?;
        let changed = template::update_facts(
            repo,
            &payload,
            &jd! {"approval" => approval.clone(), "slots" => slots.clone(), "factcheck" => fc_summary.clone()},
            None,
        )?;
        _update_payload_json(&payload, &[("approval", approval.clone()), ("slots", slots), ("factcheck", fc_summary.clone())])?;
        _mark(draft, &mut st, "approval", jd! {"approval" => approval.clone(), "facts_changed" => J::from(changed)})?;
        template::refresh(&payload)?;
        let issues = _lint(repo, &payload, draft, &st, false, false)?;
        if !issues.is_empty() {
            let codes: std::collections::BTreeSet<String> = issues.iter().map(|i| i["code"].display()).collect();
            _mark(draft, &mut st, "lint", jd! {"ok" => false, "codes" => J::from(codes.into_iter().collect::<Vec<_>>())})?;
            _fail_lint(&issues, false)?;
        }
        _mark(draft, &mut st, "lint", jd! {"ok" => true, "sealed" => false})?;
        let mut fc_step = jd! {"ok" => fc_err.is_none()};
        if let Some(m) = fc_summary.as_dict() {
            for (k, v) in m.iter() {
                if k != "source" {
                    fc_step.set(k, v.clone());
                }
            }
        }
        _mark(draft, &mut st, "factcheck", fc_step)?;
        if let Some(e) = fc_err {
            return Err(e);
        }
        _confirm_name(repo, draft, &mut st, &payload, a.remote, a.no_push, Some(g))?;
        _resolve_guide(g, &mut st, draft, a.remote, a.guide_commit, a.no_push)?;
        template::seal_prompts(&payload, &template::payload_docs(&facts)?)?;
        let issues = _lint(repo, &payload, draft, &st, true, false)?;
        if !issues.is_empty() {
            let codes: std::collections::BTreeSet<String> = issues.iter().map(|i| i["code"].display()).collect();
            _mark(draft, &mut st, "lint", jd! {"ok" => false, "sealed" => true, "codes" => J::from(codes.into_iter().collect::<Vec<_>>())})?;
            _fail_lint(&issues, true)?;
        }
        _mark(draft, &mut st, "lint", jd! {"ok" => true, "sealed" => true})?;
        let sl = _seal_lineage(repo, draft, &st, &payload)?;
        _mark(draft, &mut st, "lineage", sl)?;
    }
    let tagname = st["tag"].display();
    let brief = template::brief(&payload)?;
    let manifest = p_j(&repo.join(st["manifest"].display()));
    // ⑤ 게이트 → 계측 산출물 → 커밋
    _gate(repo, &manifest, "continue")?;
    _require_binding_artifact(&_manifest_doc(repo, &st)?, repo, true)?;
    let anchor = match &commit_step {
        Some(cs) => cs["anchor"].display(),
        None => {
            _finalize_provenance(repo, &payload, &st)?;
            st.set("commit_utc", utc.clone());
            _save_state(draft, &st)?;
            let message = branch::commit_message(&payload, &utc)?;
            let guide = st["guide"]["sha"].display();
            let anchor = super::hooks::commit_payload(g, &payload, &message, &utc, Some(&guide), None)?;
            _mark(draft, &mut st, "commit", jd! {"anchor" => anchor.clone(), "utc" => utc.clone(), "guide" => guide})?;
            anchor
        }
    };
    // ⑥ 승격 목표 → 게이트 재확인
    if _step(&st, "target").is_none() {
        if st.get_str("mode") != Some("campaign") && !draft.join("inputs").join("pii.json").is_file() {
            let sel = sel_of(&st);
            let pubn = if sel["publication"].truthy() { sel["publication"].clone() } else { js(&topic) };
            let ev = evidence::from_publication(repo, &pubn, &sel["node"], &a.docker)?;
            evidence::publisher_inputs(repo, &ev, draft)?;
        }
        evidence::set_promotion_target(repo, &js(&topic), &js(&tagname), &js(&label), &js(&anchor), &js(&utc), Some(draft))?;
        _mark(draft, &mut st, "target", jd! {"anchor" => anchor.clone()})?;
    }
    let man = _manifest_doc(repo, &st)?;
    let probs = evidence::promotion_binding_problems(&man, &tagname, &label, &anchor)?;
    if !probs.is_empty() {
        let parts: Vec<String> = probs.iter().map(|(c, m)| format!("{c}: {m}")).collect();
        return fail(
            "HINT_PROMOTION_BINDING_MISMATCH",
            format!("work-manifest 의 promotion_target 이 봉인 대상과 다르다: {}", parts.join("; ")),
            Some("evidence_publisher set-promotion-target 이 쓴 목표를 확인한다(hint.sh 는 manifest 를 직접 고치지 않는다)."),
        );
    }
    _gate(repo, &manifest, "continue")?;
    let (bench_ref, bench_kind) = _require_binding_artifact(&man, repo, false)?;
    // ⑦ 봉인 → 이 태그 1개 로컬 검증
    let fields = jd! {
        "version" => tag::FOOTER_VERSION, "tag" => tagname.clone(), "topology" => label.clone(), "anchor" => anchor.clone(),
        "manifest_ref" => st["manifest"].clone(), "bench_ref" => bench_ref, "bench_kind" => bench_kind,
    };
    let message = tag::annotation(&brief, &fields)?;
    let obj = match tag::seal(g, &tagname, &anchor, &message, &utc, if a.no_push { None } else { Some(a.remote) }) {
        Ok(o) => o,
        Err(e) => {
            if let Some(p) = &e.problems {
                _mark(draft, &mut st, "seal", jd! {"ok" => false, "codes" => J::from(tag::problem_codes(p)?)})?;
            }
            return Err(e);
        }
    };
    let lf = _lint_fn(Some(draft), Some(&st), Some(repo), false);
    let lint = |r: &Path, an: &str, t: &str| lf.call(r, an, t);
    let mut infos: Vec<J> = Vec::new();
    let vprobs = tag::verify_local(g, &tagname, Some(&lint), Some(&mut infos), Some(repo))?;
    for line in &infos {
        _log(&line.display());
    }
    if !vprobs.is_empty() {
        let codes = tag::problem_codes(&vprobs)?;
        _mark(draft, &mut st, "verify", jd! {"ok" => false, "codes" => J::from(codes.clone()), "infos" => J::List(infos.clone())})?;
        let head: Vec<String> = vprobs.iter().take(30).cloned().collect();
        return fail(
            "HINT_LOCAL_VERIFY_FAILED",
            format!("봉인한 태그의 로컬 검증 {}건({}):\n  {}", vprobs.len(), codes.join(", "), head.join("\n  ")),
            Some("각 code 의 자리를 고친다 — 검증되지 않은 태그는 밀지 않는다(D10)."),
        );
    }
    _mark(draft, &mut st, "verify", jd! {"ok" => true, "tag_object" => obj.clone(), "infos" => J::List(infos.clone())})?;
    let mut result = jd! {
        "tag" => tagname.clone(), "anchor" => anchor.clone(), "tag_object" => obj, "manifest" => st["manifest"].clone(), "pushed" => false,
        "draft" => p_j(draft), "guide" => st["guide"]["sha"].clone(), "infos" => J::List(infos.clone()),
    };
    if a.no_push {
        let mut again = format!("hint.sh continue --draft {} --generated-utc <UTC>", _show(repo, draft));
        if st.get_str("mode") != Some("campaign") {
            again.push_str(" --approved-by \"<같은 사람 발화 전사>\" --approved-utc <UTC>");
        }
        _log(&format!("--no-push — 봉인·로컬 검증까지. 이어서 `{again}` 를 다시 실행하면 push → 원격 SHA 대조 → 카탈로그 → (캠페인 셀) publish 위상까지 잇는다."));
        return Ok(result);
    }
    // ⑧ push → 원격 SHA == 로컬
    _gate(repo, &manifest, "push")?;
    let mut pinfos: Vec<J> = Vec::new();
    let pres = match tag::push_tag(g, a.remote, &tagname, false, Some(&lint), Some(&mut pinfos), Some(repo)) {
        Ok(r) => r,
        Err(e) => {
            if let Some(p) = &e.problems {
                _mark(draft, &mut st, "push", jd! {"ok" => false, "remote" => a.remote, "codes" => J::from(tag::problem_codes(p)?)})?;
            }
            return Err(e);
        }
    };
    let local_obj = tag::read_tag(g, &tagname)?.map(|t| t["object_sha"].display()).unwrap_or_default();
    let remote_obj = tag::remote_tag_object(g, a.remote, &tagname)?;
    if remote_obj.as_deref() != Some(local_obj.as_str()) {
        return fail(
            "HINT_REMOTE_SHA_MISMATCH",
            format!(
                "원격 {} 의 태그 오브젝트 {} ≠ 로컬 {}",
                a.remote,
                core::head_chars(&J::from(remote_obj.clone()).display(), 12),
                core::head_chars(&local_obj, 12)
            ),
            None,
        );
    }
    let prev_push = _step(&st, "push").unwrap_or_else(J::dict);
    let push_utc = if pres.get_str("status") == Some("already-on-remote") && prev_push["ok"].truthy() && prev_push["utc"].truthy() {
        prev_push["utc"].display()
    } else {
        utc_arg.clone()
    };
    _mark(
        draft,
        &mut st,
        "push",
        jd! {"ok" => true, "remote" => a.remote, "status" => pres["status"].clone(), "refspec" => pres["refspec"].clone(),
             "remote_object" => J::from(remote_obj.clone()), "utc" => push_utc.clone(), "infos" => J::List(pinfos.clone())},
    )?;
    // ⑨ 카탈로그 ⑩ 캠페인 publish 위상
    let cc;
    if repo.join(core::REL_CENTRAL_FLAG).is_file() {
        let kst = core::kst_iso(&utc_arg)?;
        catalog::derive(repo, a.remote, &kst, false, true, false)?;
        cc = _commit_catalog(repo, &tagname)?;
        _mark(draft, &mut st, "catalog", jd! {"ok" => true, "remote" => a.remote, "generated_kst" => kst, "commit" => cc.clone()})?;
    } else {
        cc = jd! {"status" => "skipped-push-role-only"};
        _log(&format!(
            "카탈로그 단계 건너뜀 — `{}` 없음(push 역할만). 카탈로그는 메인이 다음 파생 때 원격 태그에서 반영한다 · \
             발행 사실(태그 · 원격 SHA · 승인 출처 · 엔진 리비전)은 서브 testlog 로 보고한다.",
            core::REL_CENTRAL_FLAG
        ));
        _mark(draft, &mut st, "catalog", jd! {"ok" => true, "skipped" => "push-role-only", "remote" => a.remote, "commit" => cc.clone()})?;
    }
    let remote_obj_j = J::from(remote_obj.clone());
    if st.get_str("mode") == Some("campaign") {
        let sel = sel_of(&st);
        evidence::phase_set_publish(repo, &sel["campaign"], &sel["cell"], &sel["node"], &js(&tagname), &remote_obj_j, &js(&push_utc), &J::Null)?;
        _mark(draft, &mut st, "phase", jd! {"ok" => true, "proof_source" => format!("refs/tags/{tagname}@{}", remote_obj_j.display())})?;
    }
    _mark(draft, &mut st, "done", jd! {"ok" => true, "remote_object" => remote_obj_j.clone()})?;
    let mut all_infos = infos;
    all_infos.extend(pinfos);
    result.set("pushed", true);
    result.set("remote", a.remote);
    result.set("remote_object", remote_obj_j);
    result.set("push_status", pres["status"].clone());
    result.set("catalog_commit", cc);
    result.set("infos", J::List(all_infos));
    Ok(result)
}

pub fn cmd_continue(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let draft = _find_draft(&repo, a.s("draft").as_deref(), a.s("campaign").as_deref(), a.s("cell").as_deref(), a.s("node").as_deref())?;
    let (gu, ab, au, fw, gc) = (a.s("generated_utc"), a.s("approved_by"), a.s("approved_utc"), a.s("factcheck_waiver"), a.s("guide_commit"));
    let remote = a.s("remote").unwrap_or_else(|| "origin".into());
    let ca = ContinueArgs {
        generated_utc: gu.as_deref(),
        approved_by: ab.as_deref(),
        approved_utc: au.as_deref(),
        remote: &remote,
        no_push: a.b("no_push"),
        factcheck_waiver: fw.as_deref(),
        guide_commit: gc.as_deref(),
        docker: J::Null,
    };
    let hc = a.s("hint_clone").map(PathBuf::from);
    let res = continue_(&repo, &draft, &ca, hc.as_deref())?;
    io::outln(&format!("{_OUT} continue — {}", res["tag"].display()));
    io::outln(&format!(
        "  페이로드 커밋 {}(부모 = 안내 커밋 {}) · 태그 오브젝트 {}",
        res["anchor"].display(),
        core::head_chars(&res["guide"].display(), 12),
        res["tag_object"].display()
    ));
    for line in res["infos"].items() {
        io::outln(&format!("  {}", line.display()));
    }
    if res["pushed"].truthy() {
        io::outln(&format!("  push {} ({}) · 원격 오브젝트 = 로컬 {}", res["remote"].display(), res["push_status"].display(), res["remote_object"].display()));
        let cc = &res["catalog_commit"];
        if cc.get_str("status") == Some("skipped-push-role-only") {
            io::outln("  카탈로그 단계 건너뜀(push 역할만 — 메인이 다음 파생 때 반영) · 캠페인 publish 위상 기록(캠페인 셀이면)");
        } else {
            io::outln(&format!(
                "  카탈로그 파생 완료(record-missing) · 카탈로그 커밋 {}{} · 캠페인 publish 위상 기록(캠페인 셀이면)",
                cc["status"].display(),
                if cc["commit"].truthy() { format!(" {}", core::head_chars(&cc["commit"].display(), 12)) } else { String::new() }
            ));
        }
    } else {
        io::outln("  push 생략(--no-push) — 봉인·로컬 검증까지 끝났다");
    }
    Ok(0)
}

// ── refacts ────────────────────────────────────────────────────────────────────────────────────
fn fact_blocks_of(payload: &Path, docs: &[String]) -> HR<Vec<(String, J)>> {
    let mut out = Vec::new();
    for n in docs {
        let p = payload.join(n);
        if p.is_file() {
            let text = core::read_text(&p).map_err(|e| py_err("OSError", e))?;
            out.push((n.clone(), template::fact_blocks(&text)?));
        }
    }
    Ok(out)
}

fn slot_intro_of(payload: &Path) -> HR<String> {
    let p = payload.join("01-artifacts.md");
    let text = core::read_text(&p).map_err(|e| py_err("FileNotFoundError", e))?;
    Ok(template::slot_intro(&text)?.1)
}

/// 사실 재파생(같은 generated_utc · 같은 셀 선택자) — 산문 · 꼬리 · 사실 검증 보존.
pub fn refacts(repo: &Path, draft: &Path, docker: &J) -> HR<J> {
    let repo = core::resolve(repo);
    let draft = core::resolve(draft);
    let (repo, draft) = (repo.as_path(), draft.as_path());
    let mut st = _load_state(draft)?;
    if let Some(cs) = _step(&st, "commit") {
        return fail(
            "HINT_DRAFT_ALREADY_COMMITTED",
            format!("이미 페이로드 커밋이 있는 draft 다({}) — 사실을 다시 쓰면 커밋한 바이트와 갈라진다.", core::head_chars(&disp_or_empty(&cs["anchor"]), 12)),
            Some("커밋 뒤 개정은 새 publish(새 판)다."),
        );
    }
    let payload = draft.join("payload");
    if !payload.is_dir() {
        return fail("HINT_DRAFT_PAYLOAD_ABSENT", format!("draft 에 payload/ 가 없다: {}", core::posix(draft)), None);
    }
    let sel = sel_of(&st);
    let node = match st.get("node_arg") {
        Some(v) => v.clone(),
        None => sel["node"].clone(),
    };
    let topic = st["topic"].display();
    let mut ev = if st.get_str("mode") == Some("campaign") {
        super::hooks::from_campaign(repo, &sel["campaign"], &sel["cell"], &node, docker)?
    } else {
        let pubn = if sel["publication"].truthy() { sel["publication"].clone() } else { js(&topic) };
        evidence::from_publication(repo, &pubn, &node, docker)?
    };
    artifacts::bind_image_git_release(repo, &mut ev, docker)?;
    let tf = evidence::topic_for(&ev)?;
    if tf != topic {
        return fail("HINT_REFACTS_TOPIC_MISMATCH", format!("재파생한 발행 토픽 {} ≠ draft 의 {}", py_repr(&tf), py_repr(&topic)), None);
    }
    let utc = st["generated_utc"].display();
    let dn = super::hooks::derive_name(&evidence::naming_facts(repo, &ev)?, &naming::load_vocab(repo)?)?;
    tag::check_ref_format(repo, &dn.tag)?;
    _require_name_pii_clean(repo, &dn.tag)?;
    let intro_before = slot_intro_of(&payload)?;
    let snap = template::load_snapshot(&draft.join("inputs").join(template::FACTS_SNAPSHOT))?;
    let before = fact_blocks_of(&payload, &template::payload_docs(&snap)?)?;
    let mut rebind: Option<J> = None;
    let facts;
    let doc;
    let col;
    let old_files: Vec<String>;
    let new_files: Vec<String>;
    let mut keep: Vec<(String, Vec<u8>)> = Vec::new();
    {
        let td = TempIn::new(draft, ".refacts-")?;
        let tmp = td.0.join("payload");
        let lineage_add: Vec<String> = st["lineage_add"].items().iter().map(J::display).collect();
        col = _collect(repo, &mut ev, &dn, &utc, &topic, draft, &tmp, &lineage_add, docker)?;
        if st.get_str("mode") == Some("campaign") {
            let rb = super::hooks::rebind_campaign_narratives(repo, &ev, &js(&topic), &js(&utc), draft)?;
            // evidence returns the manifest as a Path (`{"__path__": s}` in this port) — read either form.
            let mp = rb["manifest"].get("__path__").cloned().unwrap_or_else(|| rb["manifest"].clone()).display();
            let rel_m = core::rel(repo, Path::new(&mp))?;
            if rel_m != st["manifest"].display() {
                return fail("HINT_REFACTS_MANIFEST_MISMATCH", format!("재생성한 work-manifest {rel_m} ≠ draft 의 {}", st["manifest"].display()), None);
            }
            rebind = Some(rb);
        }
        let man = _manifest_doc(repo, &st)?;
        let (f, d) = _assemble(repo, &ev, &dn, &col, &utc, &topic, &st["manifest"].display(), &man)?;
        facts = f;
        doc = d;
        let old_art = payload.join("artifacts");
        for req in facts["agent_requests"].items() {
            let rel = disp_or_empty(&req["path"]);
            let f = payload.join(&rel);
            if !rel.is_empty() && f.is_file() {
                let bytes = std::fs::read(&f).map_err(|e| core::io_err(&e, &f))?;
                if !String::from_utf8_lossy(&bytes).contains(template::AGENT_MARK) {
                    keep.push((rel, bytes));
                }
            }
        }
        let mut of = if old_art.is_dir() { files_under(&old_art, &payload) } else { Vec::new() };
        of.sort();
        of.dedup();
        old_files = of;
        let mut nf = if tmp.join("artifacts").is_dir() { files_under(&tmp.join("artifacts"), &tmp) } else { Vec::new() };
        nf.sort();
        nf.dedup();
        new_files = nf;
        if old_art.exists() {
            rmtree(&old_art)?;
        }
        if tmp.join("artifacts").is_dir() {
            copy_tree(&tmp.join("artifacts"), &old_art)?;
        }
    }
    for (rel, data) in &keep {
        let p = payload.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| core::io_err(&e, parent))?;
        }
        std::fs::write(&p, data).map_err(|e| core::io_err(&e, &p))?;
    }
    template::write_agent_requests(&payload, &facts["agent_requests"])?;
    template::update_facts(repo, &payload, &facts, None)?;
    _write_payload_meta(draft, &payload, &dn.tag, &utc, &doc, &col)?;
    _refresh_lineage_summary(repo, draft, &payload)?;
    let after = fact_blocks_of(&payload, &template::payload_docs(&facts)?)?;
    let mut changed: Vec<(String, String)> = Vec::new();
    let mut diff: Vec<String> = Vec::new();
    let intro_after = slot_intro_of(&payload)?;
    if intro_before != intro_after {
        changed.push(("01-artifacts.md".into(), "fixed-introduction".into()));
        diff.extend(unified_diff(
            &[intro_before],
            &[intro_after],
            "01-artifacts.md fixed introduction (이전)",
            "01-artifacts.md fixed introduction (재파생)",
            3,
        ));
    }
    let mut after_sorted = after.clone();
    after_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let empty = J::dict();
    for (n, blocks) in &after_sorted {
        let b4 = before.iter().find(|(m, _)| m == n).map(|(_, b)| b).unwrap_or(&empty);
        let mut ids: std::collections::BTreeSet<String> = b4.as_dict().map(|m| m.keys().map(str::to_string).collect()).unwrap_or_default();
        if let Some(m) = blocks.as_dict() {
            ids.extend(m.keys().map(str::to_string));
        }
        for fid in ids {
            let a = b4.get(&fid).map(J::display).unwrap_or_default();
            let b = blocks.get(&fid).map(J::display).unwrap_or_default();
            if a != b {
                changed.push((n.clone(), fid.clone()));
                let al: Vec<String> = a.split('\n').map(str::to_string).collect();
                let bl: Vec<String> = b.split('\n').map(str::to_string).collect();
                diff.extend(unified_diff(&al, &bl, &format!("{n} FACT:{fid} (이전)"), &format!("{n} FACT:{fid} (재파생)"), 1));
            }
        }
    }
    if let Some(m) = st.get_mut("steps").and_then(J::as_dict_mut) {
        for k in _REFACTS_RESET_STEPS {
            m.remove(k);
        }
    }
    if let Some(m) = st.as_dict_mut() {
        m.remove("guide");
    }
    st.set("tag", dn.tag.clone());
    st.set("base_tag", dn.tag.clone());
    st.set("bench_source", col["bench_src"].clone());
    let kept: Vec<String> = {
        let mut k: Vec<String> = keep.iter().map(|(r, _)| r.clone()).collect();
        k.sort();
        k.dedup();
        k
    };
    let added: Vec<String> = new_files.iter().filter(|x| !old_files.contains(x)).cloned().collect();
    let removed: Vec<String> = old_files.iter().filter(|x| !new_files.contains(x) && !kept.contains(x)).cloned().collect();
    let changed_j: Vec<String> = changed.iter().map(|(n, f)| format!("{n}#{f}")).collect();
    _mark(
        draft,
        &mut st,
        "refacts",
        jd! {"utc" => utc, "changed" => J::from(changed_j), "artifacts_added" => J::from(added.clone()), "artifacts_removed" => J::from(removed.clone()),
             "narratives_rebound" => rebind.as_ref().map(|r| r["rebound"].clone()).unwrap_or(J::Null)},
    )?;
    Ok(jd! {
        "changed" => J::List(changed.iter().map(|(n, f)| jl![n.clone(), f.clone()]).collect()), "diff" => diff.join("\n"), "tag" => dn.tag.clone(),
        "rebind" => rebind.map(|r| jd! {"rebound" => r["rebound"].clone(), "manifest" => st["manifest"].clone()}).unwrap_or(J::Null),
        "artifacts" => jd! {"added" => J::from(added), "removed" => J::from(removed), "kept_authored" => J::from(kept)},
    })
}

pub fn cmd_refacts(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let draft = _find_draft(&repo, a.s("draft").as_deref(), None, None, None)?;
    let res = refacts(&repo, &draft, &docker_runner(a.b("docker_read_only")))?;
    let changed: Vec<String> = res["changed"].items().iter().map(|c| format!("{}#{}", c.at(0).display(), c.at(1).display())).collect();
    io::outln(&format!(
        "{_OUT} refacts — {} · 기본 이름 {} · 바뀐 FACT {}개{}",
        _show(&repo, &draft),
        res["tag"].display(),
        changed.len(),
        if changed.is_empty() { String::new() } else { format!(": {}", changed.join(", ")) }
    ));
    let rb = &res["rebind"];
    if !rb.is_null() {
        let rebound = rb["rebound"].items();
        let detail = if !rebound.is_empty() {
            let parts: Vec<String> = rebound
                .iter()
                .map(|r| format!("{} {} → {}", r["kind"].display(), py_basename(&r["from"].display()), py_basename(&r["to"].display())))
                .collect();
            format!(" — {}", parts.join(", "))
        } else {
            "(바인딩 = 캠페인 포인터 · no-op)".into()
        };
        io::outln(&format!("  서사 재바인딩: {}건{detail} · 발행기 입력 · {} 재생성", rebound.len(), rb["manifest"].display()));
    }
    let ar = &res["artifacts"];
    let removed: Vec<String> = ar["removed"].items().iter().map(J::display).collect();
    io::outln(&format!(
        "  산출물: 추가 {} · 제거 {} · 저작본 보존 {}{}",
        ar["added"].len(),
        removed.len(),
        ar["kept_authored"].len(),
        if removed.is_empty() { String::new() } else { format!(" — 제거 {}", list_repr(&removed.iter().take(6).cloned().collect::<Vec<_>>())) }
    ));
    if res["diff"].truthy() {
        io::outln(&res["diff"].display());
    }
    io::outln(&format!(
        "{_OUT} 보존: 저작 산문 · hint-event · inputs/tail.json · inputs/factcheck.json — 사실이 바뀌었으면 산문 · 사실 검증을 다시 본다\
         (continue 가 이름을 다시 확정한다)."
    ));
    Ok(0)
}

// ── lint · excerpt ─────────────────────────────────────────────────────────────────────────────
/// 부수효과 0 — draft 페이로드를 임시 사본에서 refresh 한 뒤 lint.
pub fn lint_draft(repo: &Path, draft: &Path) -> HR<Vec<J>> {
    let st = _load_state(draft)?;
    if !draft.join("payload").is_dir() {
        return fail(
            "HINT_DRAFT_PAYLOAD_ABSENT",
            format!("draft 에 payload/ 가 없다: {}", core::posix(draft)),
            Some("hint.sh publish 가 만든 draft 에서 실행한다(끊긴 publish 면 draft 를 지우고 같은 --generated-utc 로 다시)."),
        );
    }
    let mut issues = {
        let td = core::TempDir::new("hint-lint-");
        let copy = td.path().join("payload");
        copy_tree(&draft.join("payload"), &copy)?;
        template::refresh(&copy)?;
        _lint(repo, &copy, draft, &st, false, false)?
    };
    if _step(&st, "commit").is_none() {
        issues.extend(_tail_issues(draft)?);
    }
    Ok(issues)
}

pub fn cmd_lint(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let draft = core::resolve(Path::new(&a.s("draft").unwrap_or_default()));
    let issues = lint_draft(&repo, &draft)?;
    if a.b("json") {
        io::outln(&dumps_pretty(&J::List(issues.clone())));
    } else {
        for i in &issues {
            io::outln(&format!("{}  {}:{}  {}", i["code"].display(), i["file"].display(), i["line"].display(), i["message"].display()));
        }
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for i in &issues {
            *counts.entry(i["code"].display()).or_default() += 1;
        }
        let head = if issues.is_empty() { "통과(0건)".to_string() } else { format!("{}건", issues.len()) };
        let cs = if counts.is_empty() {
            String::new()
        } else {
            format!(" — {}", counts.iter().map(|(c, n)| format!("{c}×{n}")).collect::<Vec<_>>().join(" · "))
        };
        io::outln(&format!("{_OUT} lint {head}{cs} (파생 블록은 continue 가 refresh 한다 — 여기서는 사본에서 미리 반영해 판정했다 · 보려면 `refresh`)"));
        let (fc, err) = _factcheck_eval(&draft, &_load_state(&draft)?)?;
        io::outln(&format!(
            "{_OUT} 사실 검증 보고 {}{} (린트와 별개 · 저작자가 아닌 Agent 의 inputs/factcheck.json)",
            fc["status"].display(),
            match &err {
                Some(e) => format!(" — continue 는 {} 로 멈춘다", e.code),
                None => " — continue 게이트 통과".into(),
            }
        ));
    }
    Ok(if issues.is_empty() { 0 } else { 1 })
}

/// draft 페이로드의 파생 블록을 다시 쓴다(draft 안 쓰기만).
pub fn refresh_draft(repo: &Path, draft: &Path) -> HR<Vec<String>> {
    let st = _load_state(draft)?;
    if let Some(cs) = _step(&st, "commit") {
        return fail(
            "HINT_DRAFT_ALREADY_COMMITTED",
            format!("이미 hint 브랜치에 커밋한 draft 다({}) — 파생 블록을 다시 쓰면 커밋한 바이트와 갈라진다.", core::head_chars(&disp_or_empty(&cs["anchor"]), 12)),
            Some("커밋 뒤 개정은 새 publish(새 이름)다 — continue 재실행은 커밋한 내용 그대로 잇는다."),
        );
    }
    if !draft.join("payload").is_dir() {
        return fail("HINT_DRAFT_PAYLOAD_ABSENT", format!("draft 에 payload/ 가 없다: {}", core::posix(draft)), None);
    }
    let mut changed = template::refresh(&draft.join("payload"))?;
    if _refresh_lineage_summary(repo, draft, &draft.join("payload"))? {
        changed.push("LINEAGE.json".into());
    }
    Ok(changed)
}

pub fn cmd_refresh(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let draft = _find_draft(&repo, a.s("draft").as_deref(), None, None, None)?;
    let changed = refresh_draft(&repo, &draft)?;
    io::outln(&format!(
        "{_OUT} refresh — {} · {} §0.3 · §0.5 에서 확인한다",
        if changed.is_empty() { "바뀐 파생 블록 없음(이미 최신)".to_string() } else { format!("다시 쓴 파생 블록 {}", changed.join(", ")) },
        _show(&repo, &draft.join("payload").join("00-hint.md"))
    ));
    Ok(0)
}

crate::re_static!(lines_re, r"\s*(\d+)\s*-\s*(\d+)\s*");

fn py_int_digits(s: &str) -> i64 {
    s.chars().fold(0i64, |acc, c| acc.saturating_mul(10).saturating_add(c.to_digit(10).unwrap_or(0) as i64))
}

/// 발췌 저작 도우미: 린터가 대조하는 치환 후 원문(`numbered` = 줄마다 `L<n>\t`).
pub fn excerpt_text(repo: &Path, draft: &Path, source: &str, lines: Option<&str>, numbered: bool) -> HR<String> {
    let st = _load_state(draft)?;
    let payload = draft.join("payload");
    let lin = _lineage_full(draft, &payload)?;
    let subst = _subst_table(repo, &st)?;
    let text = template::substituted_source(repo, &payload, source, &lin, &subst, &J::Null, &J::Null, &J::Null, Some(draft), &J::Null)?;
    let mut rows: Vec<&str> = text.split('\n').collect();
    let mut first = 1i64;
    let lines_given = lines.map(|l| !l.is_empty()).unwrap_or(false);
    if let Some(l) = lines.filter(|l| !l.is_empty()) {
        let m = lines_re().fullmatch(l);
        let (a, b) = match &m {
            Some(c) => (py_int_digits(c.get(1).unwrap().as_str()), py_int_digits(c.get(2).unwrap().as_str())),
            None => (0, 0),
        };
        if m.is_none() || a < 1 || b < a {
            return fail("HINT_EXCERPT_LINES_SHAPE", format!("--lines 는 `a-b`(1 ≤ a ≤ b)다: {}", py_repr(l)), None);
        }
        if b > rows.len() as i64 {
            return fail("HINT_EXCERPT_LINES_OUT_OF_RANGE", format!("--lines {l} 가 출처(정규화 뒤 {}줄)의 범위 밖이다", rows.len()), None);
        }
        first = a;
        rows = rows[(a - 1) as usize..b as usize].to_vec();
    }
    if numbered {
        if !lines_given && rows.last() == Some(&"") {
            rows.pop();
        }
        return Ok(rows.iter().enumerate().map(|(i, r)| format!("L{}\t{r}", first + i as i64)).collect::<Vec<_>>().join("\n"));
    }
    Ok(rows.join("\n"))
}

pub fn cmd_excerpt(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let draft = core::resolve(Path::new(&a.s("draft").unwrap_or_default()));
    let text = excerpt_text(&repo, &draft, &a.s("source").unwrap_or_default(), a.s("lines").as_deref(), a.b("numbered"))?;
    io::out(&format!("{}\n", text.trim_end_matches('\n')));
    Ok(0)
}

// ── name ───────────────────────────────────────────────────────────────────────────────────────
pub fn cmd_name(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let (campaign, cell, publication, node) = (a.s("campaign"), a.s("cell"), a.s("publication"), a.s("node"));
    _check_selectors(campaign.as_deref(), cell.as_deref(), publication.as_deref(), false, false)?;
    let ev = _evidence(&repo, campaign.as_deref(), cell.as_deref(), publication.as_deref(), node.as_deref(), &J::Null)?;
    let nf = evidence::naming_facts(&repo, &ev)?;
    let vocab = naming::load_vocab(&repo)?;
    let dn = super::hooks::derive_name(&nf, &vocab)?;
    let cands = naming::tail_candidates(&nf, &vocab)?;
    if a.b("json") {
        let mut doc = jd! {"tag" => dn.tag.clone(), "base_tag" => dn.tag.clone()};
        if let Some(m) = dn.to_payload().as_dict() {
            for (k, v) in m.iter() {
                doc.set(k, v.clone());
            }
        }
        doc.set("tail_candidates", J::List(cands));
        io::outln(&dumps_pretty_sorted(&doc));
        return Ok(0);
    }
    io::outln(&format!("{}   (기본 이름 — 꼬리 전 · continue 가 inputs/tail.json 으로 꼬리를 확정하고 중복이면 -t<YYMMDDHHMM>)", dn.tag));
    for k in ["vllm", "model", "arch", "recipe"] {
        let s = dn.segment(k).ok_or_else(|| HintError::py("KeyError", py_repr(k)))?;
        io::outln(&format!("  세그먼트 {} {} ← {}", pad_right(k, 7), pad_right(&s.value, 40), s.source));
    }
    let mut axes: Vec<&str> = naming::ARCH_AXES.to_vec();
    axes.push(naming::PLANE_AXIS);
    axes.extend(naming::RECIPE_AXES.iter());
    for k in axes {
        if let Some(x) = dn.axis(k) {
            io::outln(&format!("  축 {} {} ← {}", pad_right(k, 13), pad_right(&x.value, 40), x.source));
        }
    }
    io::outln(&format!("  빌드 입력 {}", dumps_with(&dn.vllm_build_input, Dump { ensure_ascii: false, sort_keys: true, ..Dump::DEFAULT })));
    io::outln("  꼬리 후보(강제 ✗ · 이 셀을 가르는 노브만 · 셋을 v6 순서 그대로 쓰면 거부):");
    for c in &cands {
        let ev_ = &c["evidence"];
        let where_ = if ev_.is_dict() {
            format!("{} · {}={}", ev_["file"].display(), ev_["key"].display(), ev_["value"].display())
        } else {
            "근거 미특정(저작자가 적는다)".into()
        };
        let tok = if c["token"].truthy() { c["token"].display() } else { "—".into() };
        let meaning = if c["meaning"].truthy() {
            c["meaning"].display()
        } else if c["error"].truthy() {
            c["error"].display()
        } else {
            String::new()
        };
        io::outln(&format!("    {} {} {meaning} ← {where_}", pad_right(&c["axis"].display(), 6), pad_right(&tok, 14)));
    }
    Ok(0)
}

// ── verify · push ──────────────────────────────────────────────────────────────────────────────
pub fn _footer_of(repo: &Path, tagname: &str) -> HR<Option<J>> {
    let Some(obj) = tag::read_tag(repo, tagname)? else { return Ok(None) };
    let body = obj["body"].display();
    match tag::parse_annotation(&body) {
        Ok(a) => Ok(Some(a["footer"].clone())),
        Err(e) if !e.is_py() => match tag::parse_footer(&body, &[]) {
            Ok(f) => Ok(Some(f)),
            Err(e) if !e.is_py() => Ok(None),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

pub fn _draft_state_for(repo: &Path, tagname: &str, draft_arg: Option<&str>) -> HR<(Option<PathBuf>, Option<J>)> {
    let d = match draft_arg.filter(|s| !s.is_empty()) {
        Some(s) => Some(core::resolve(Path::new(s))),
        None => _find_draft_for_tag(repo, tagname)?,
    };
    let Some(d) = d else { return Ok((None, None)) };
    let st = _load_state(&d)?;
    if st["tag"].as_str() != Some(tagname) {
        return fail("HINT_DRAFT_TAG_MISMATCH", format!("draft 의 태그 {} ≠ {}", st["tag"].repr(), py_repr(tagname)), None);
    }
    Ok((Some(d), Some(st)))
}

/// 별도 클론의 태그 오브젝트를 검증하고 게이트·출처 권위는 워크스페이스에 둔다.
pub fn verify_tag(repo: &Path, tagname: &str, draft: Option<&str>, infos: Option<&mut Vec<J>>, hint_clone: Option<&Path>) -> HR<Vec<String>> {
    let workspace = core::resolve(repo);
    let objects = match hint_clone {
        Some(hc) => _hint_clone_repo(&workspace, hc)?,
        None => workspace.clone(),
    };
    let prev = if hint_clone.is_some() { Some(core::set_aux_root(Some(&workspace))) } else { None };
    let r = (|| -> HR<Vec<String>> {
        tag::check_ref_format(&objects, tagname)?;
        let Some(footer) = _footer_of(&objects, tagname)? else {
            return tag::verify_local(&objects, tagname, None, infos, None);
        };
        _gate(&workspace, &footer["manifest_ref"], "verify")?;
        let (d, st) = _draft_state_for(&workspace, tagname, draft)?;
        let lf = _lint_fn(d.as_deref(), st.as_ref(), Some(&workspace), true);
        let lint = |r: &Path, an: &str, t: &str| lf.call(r, an, t);
        tag::verify_local(&objects, tagname, Some(&lint), infos, Some(&workspace))
    })();
    if let Some(prev) = prev {
        core::set_aux_root(prev.as_deref());
    }
    r
}

pub fn cmd_verify(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let tagname = a.s("tag").unwrap_or_default();
    let mut infos: Vec<J> = Vec::new();
    let hc = a.s("hint_clone").map(PathBuf::from);
    let probs = verify_tag(&repo, &tagname, a.s("draft").as_deref(), Some(&mut infos), hc.as_deref())?;
    for line in &infos {
        io::outln(&format!("{_OUT} {}", line.display()));
    }
    if !probs.is_empty() {
        io::outln(&format!("{_OUT} verify FAIL {tagname} — {}건 · codes {}", probs.len(), tag::problem_codes(&probs)?.join(", ")));
        for p in &probs {
            io::outln(&format!("  {p}"));
        }
        return Ok(1);
    }
    io::outln(&format!("{_OUT} verify PASS {tagname}(이 태그 1개 · 로컬 봉인 · 서사 린트 포함)"));
    Ok(0)
}

pub fn cmd_push(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    _require_central(&repo, "push")?;
    let tagname = a.s("tag").unwrap_or_default();
    let remote = a.s("remote").unwrap_or_else(|| "origin".into());
    tag::check_ref_format(&repo, &tagname)?;
    let Some(footer) = _footer_of(&repo, &tagname)? else {
        let probs = tag::verify_local(&repo, &tagname, None, None, None)?;
        return fail("HINT_PUSH_UNVERIFIED", format!("footer 를 읽을 수 없는 태그는 밀지 않는다: {}", list_repr(&probs.iter().take(5).cloned().collect::<Vec<_>>())), None);
    };
    _gate(&repo, &footer["manifest_ref"], "push")?;
    let (d, st) = _draft_state_for(&repo, &tagname, a.s("draft").as_deref())?;
    let mut infos: Vec<J> = Vec::new();
    let lf = _lint_fn(d.as_deref(), st.as_ref(), None, false);
    let lint = |r: &Path, an: &str, t: &str| lf.call(r, an, t);
    let res = tag::push_tag(&repo, &remote, &tagname, !a.b("apply"), Some(&lint), Some(&mut infos), None)?;
    let mut out = res.clone();
    out.set("infos", J::List(infos));
    io::outln(&dumps_pretty_sorted(&out));
    if a.b("apply") {
        _log(&format!("단독 push — 카탈로그는 `hint.sh catalog derive --remote {remote} --generated-kst <KST> --record-missing` 로 파생한다(continue 는 스스로 한다)."));
        if let (Some(st), Some(d)) = (&st, &d) {
            if st.get_str("mode") == Some("campaign") {
                let sd = _show(&repo, d);
                _log(&format!(
                    "⚠ 캠페인 셀 draft({sd}) — publish 위상은 기록되지 않았다. `hint.sh continue --draft {sd} --generated-utc <UTC>` 를 실행하면 원격 확인(이미 반영 · 멱등) → 카탈로그 → 위상까지 잇는다."
                ));
            }
        }
    }
    Ok(0)
}

// ── catalog · match ────────────────────────────────────────────────────────────────────────────
pub fn cmd_catalog_derive(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let rc = catalog::derive(
        &repo,
        &a.s("remote").unwrap_or_default(),
        &a.s("generated_kst").unwrap_or_default(),
        a.b("dry_run"),
        a.b("record_missing"),
        a.b("allow_empty"),
    )?;
    Ok(rc as i32)
}

/// `closing_sequence.sh` 의 재파생 실패 원인 분류: `<부재 종료코드> <부재 행 수>` 한 줄.
pub fn cmd_catalog_classify(a: &Args) -> HR<i32> {
    let mut absent = String::new();
    if a.s("rc").as_deref() == Some("0") {
        if let Ok(doc) = load_json_typed(Path::new(&a.s("index").unwrap_or_default())) {
            if let J::Dict(_) = &doc {
                let hints = &doc["hints"];
                let items: Vec<J> = match hints {
                    _ if !hints.truthy() => Vec::new(),
                    J::List(l) => l.clone(),
                    J::Dict(_) | J::Str(_) => Vec::new(),
                    other => return Err(py_err("TypeError", format!("'{}' object is not iterable", other.type_name()))),
                };
                let n = items.iter().filter(|e| e.is_dict() && e.get_str("object") == Some(catalog::OBJECT_ABSENT_LOCAL)).count();
                absent = n.to_string();
            }
        }
    }
    io::outln(&format!("{} {absent}", catalog::EXIT_LOCAL_TAG_OBJECT_MISSING));
    Ok(0)
}

pub fn cmd_match(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), false)?;
    let idx = catalog::load_index(&repo)?;
    let model = a.s("model").unwrap_or_default();
    let res = catalog::r#match(&idx, &a.s("vllm").unwrap_or_default(), &model, a.s("arch").as_deref(), a.b("include_other"))?;
    if a.b("json") {
        io::outln(&dumps_pretty(&J::List(res)));
    } else {
        io::outln(&catalog::format_match(&idx, &J::List(res), &model, a.b("include_other"))?);
    }
    Ok(0)
}

pub fn cmd_branch_transition(a: &Args) -> HR<i32> {
    let repo = core::resolve_repo(a.s("repo").as_deref(), true)?;
    let r = branch::branch_transition(
        &repo,
        &a.s("remote").unwrap_or_default(),
        &a.s("generated_utc").unwrap_or_default(),
        a.s("approved_by").as_deref(),
        a.s("approved_utc").as_deref(),
    )?;
    io::outln(&dumps_pretty_sorted(&r));
    Ok(0)
}
