//! `campaign_write`: modelled writer shapes of `campaign_init.py`, in process, on a scratch checkout under the system
//! temporary directory (a scratch name for this repository: nothing here is tracked).

use std::path::{Path, PathBuf};
use sweep_stop::campaign_write::{run, Args, Ctx, Outcome, Sinks, OPTIONS};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("campaign-write-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("campaigns/_template/cells/_cell")).unwrap();
    std::fs::write(
        dir.join("campaigns/_template/campaign.yaml"),
        "{\n \"_note\": \"x\",\n \"id\": \"<<FILL>>\",\n \"plan_ref\": \"<<FILL>>\"\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("campaigns/_template/evidence_pointers.json"),
        "{\"_note\": \"x\", \"campaign_id\": \"<<FILL>>\"}\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".claude/skills/terraforming_node/scripts")).unwrap();
    std::fs::write(dir.join(".claude/skills/terraforming_node/scripts/campaign_template_validator.py"), "#").unwrap();
    dir
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn call(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let outcome = run(&Ctx { root: root.to_path_buf() }, &argv, &mut Sinks { out: &mut out, err: &mut err });
    let status = match outcome {
        Outcome::Done(status) => Some(status),
        Outcome::Fallback => None,
    };
    (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

const DECL: &str = "{\"id\": \"camp-a\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\"}, {\"node_id\": \"sub\", \"role\": \"sub\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}], \"sub\": [{\"cell\": \"s1\"}]}}";

fn active(root: &Path) {
    put(root, "campaigns/ACTIVE", "camp-a\n");
    put(root, "campaigns/camp-a/campaign.yaml", DECL);
}

#[test]
fn option_table_matches_the_owner_parser_size() {
    assert_eq!(OPTIONS.len(), 108);
    let mut names: Vec<&str> = OPTIONS.iter().map(|(n, _)| *n).collect();
    let sorted = {
        let mut s = names.clone();
        s.sort();
        s
    };
    assert_eq!(names, sorted, "the table stays sorted");
    names.dedup();
    assert_eq!(names.len(), 108);
}

#[test]
fn abbreviations_and_inline_values_are_not_exact() {
    let argv = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(Args::parse(&argv(&["--assigned", "main"])).is_none());
    assert!(Args::parse(&argv(&["--assigned-cells=main"])).is_none());
    assert!(Args::parse(&argv(&["--node", "-x"])).is_none());
    assert!(Args::parse(&argv(&["--node", "main"])).is_some());
}

#[test]
fn phase_set_writes_the_status_file_and_names_it() {
    let root = scratch("phase");
    active(&root);
    let (status, out, err) = call(
        &root,
        &["--phase-set", "bench", "--cell", "s1", "--state", "running", "--started-utc", "T1"],
    );
    assert_eq!((status, err.as_str()), (Some(0), ""));
    assert_eq!(out, "[campaign_init] wrote(camp-a): campaigns/camp-a/phases/sub/bench.status.json\n");
    let text = std::fs::read_to_string(root.join("campaigns/camp-a/phases/sub/bench.status.json")).unwrap();
    assert!(text.contains("\"first_started_utc\": \"T1\"") && text.contains("\"node_id\": \"sub\""));
    assert!(text.starts_with("{\n \"_state_enum\": ["), "sorted keys, indent 1");
}

#[test]
fn proof_ok_without_source_is_refused() {
    let root = scratch("proof");
    active(&root);
    let (status, _, err) = call(&root, &["--phase-set", "bench", "--node", "main", "--state", "done", "--proof-ok"]);
    assert_eq!(status, Some(1));
    assert!(err.starts_with("[campaign_init] FAIL proof.ok=true 에는 --proof-source 가 필수다"));
}

#[test]
fn bootstrap_is_a_noop_with_one_stderr_line() {
    let root = scratch("noop");
    let (status, out, err) = call(&root, &["--phase-set", "bench", "--node", "main", "--state", "running"]);
    assert_eq!(status, Some(0));
    assert_eq!(out, "");
    assert_eq!(err, "[campaign_init] no-op — ACTIVE=_bootstrap (캠페인 밖에서는 상태를 쓰지 않는다)\n");
}

#[test]
fn cell_set_records_gap_and_journey() {
    let root = scratch("cell");
    active(&root);
    let (status, out, _) = call(
        &root,
        &["--cell-set", "c1", "--outcome", "measured", "--next-intent", "next", "--utc", "T2"],
    );
    assert_eq!(status, Some(0));
    assert!(out.ends_with("campaigns/camp-a/cells/c1/cell.status.json campaigns/camp-a/journey.jsonl\n"));
    let cell = std::fs::read_to_string(root.join("campaigns/camp-a/cells/c1/cell.status.json")).unwrap();
    assert!(cell.contains("outcome=measured 인데 측정값이 도착하지 않았다"));
    let journey = std::fs::read_to_string(root.join("campaigns/camp-a/journey.jsonl")).unwrap();
    assert!(journey.starts_with("{\"axis_citation\": null, \"cell_id\": \"c1\""));
}

#[test]
fn shapes_with_other_files_stay_with_the_owner() {
    let root = scratch("fallback");
    active(&root);
    let (status, out, err) = call(&root, &["--escalation-add", "--cell", "c1", "--utc", "U"]);
    assert_eq!((status, out.as_str(), err.as_str()), (None, "", ""));
    let (status, ..) = call(&root, &["--phase-set", "publish", "--node", "main", "--state", "done"]);
    assert_eq!(status, None);
    assert!(!root.join("campaigns/camp-a/cells").exists());
}

#[test]
fn suspended_campaign_refuses_progress_writes() {
    let root = scratch("guard");
    active(&root);
    put(&root, "campaigns/camp-a/journey.jsonl", "{\"kind\": \"campaign_suspended\", \"utc\": \"U\"}\n");
    let (status, _, err) = call(&root, &["--phase-set", "bench", "--node", "main", "--state", "running"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("은 suspended 다 — 진행성 쓰기(--phase-set)를 거부한다"));
}

#[test]
fn init_with_a_topology_other_than_the_checkouts_is_refused() {
    let root = scratch("init");
    let (status, out, _) = call(&root, &["--init", "camp-b", "--plan-ref", "docs/plan/p.md", "--target-topology", "single", "--apply"]);
    // no git checkout here: the topology of the checkout is unknown, the declared target is not the checkout's
    assert_eq!(status, Some(1));
    assert_eq!(out, "");
}

#[test]
fn assigned_cells_lists_in_declaration_order() {
    let root = scratch("assigned");
    active(&root);
    let (status, out, _) = call(&root, &["--assigned-cells", "main"]);
    assert_eq!((status, out.as_str()), (Some(0), "c1\n"));
    let (status, _, err) = call(&root, &["--assigned-cells", "zz"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("assignments['zz'] 가 비었다"));
}

#[test]
fn lockset_precheck_names_the_verdict_on_one_line() {
    let root = scratch("precheck");
    active(&root);
    let (status, out, err) = call(&root, &["--lockset-precheck", "--cell", "c1"]);
    assert_eq!(status, Some(2));
    assert!(out.starts_with("{\"status\": \"refused\", \"campaign_id\": \"camp-a\", \"cell\": \"c1\""), "{out}");
    assert!(err.starts_with("[campaign_init] 셀 출처 precheck 거부 — lockset 부재: campaigns/camp-a/cells/c1/lockset.json"));
    put(&root, "campaigns/camp-a/cells/c1/lockset.json", "{\"provenance\": \"hand-authored\"}");
    let (status, out, _) = call(&root, &["--lockset-precheck", "--cell", "c1"]);
    assert_eq!(status, Some(0));
    assert!(out.contains("\"status\": \"passed\"") && out.contains("\"provenance\": \"hand-authored\""), "{out}");
}

#[test]
fn grounding_check_blocks_only_a_refusal() {
    let root = scratch("grounding");
    active(&root);
    let (status, _, err) = call(&root, &["--grounding-check"]);
    assert_eq!(status, Some(4));
    assert!(err.contains("그라운딩 기록이 없다: campaigns/camp-a/grounding/"));
    put(&root, "campaigns/camp-a/grounding/a.json", "{\"export\": {\"resolution\": {\"status\": \"unresolved\"}}}");
    assert_eq!(call(&root, &["--grounding-check"]).0, Some(0));
    put(&root, "campaigns/camp-a/grounding/b.json", "{\"export\": {\"resolution\": {\"status\": \"refused\"}}}");
    assert_eq!(call(&root, &["--grounding-check"]).0, Some(4));
}

#[test]
fn sweep_record_fills_measurement_and_image() {
    let root = scratch("sweep");
    active(&root);
    put(
        &root,
        "sweep.json",
        "{\"cells\": [{\"cell_key\": \"c1\", \"concurrency_vector\": {\"1\": 19.12}, \"coordinates\": {\"image_digest\": \"sha256:abc\", \"image_tag\": \"t\"}}]}",
    );
    let sweep = root.join("sweep.json");
    let (status, ..) = call(&root, &["--cell-set", "c1", "--outcome", "measured", "--sweep-state", sweep.to_str().unwrap()]);
    assert_eq!(status, Some(0));
    let cell = std::fs::read_to_string(root.join("campaigns/camp-a/cells/c1/cell.status.json")).unwrap();
    assert!(cell.contains("\"decode_tps_conc1\": 19.12") && cell.contains("\"digest\": \"sha256:abc\""), "{cell}");
    let (status, _, err) = call(
        &root,
        &["--cell-set", "c1", "--outcome", "measured", "--sweep-state", sweep.to_str().unwrap(), "--image-digest", "sha256:other"],
    );
    assert_eq!(status, Some(1));
    assert!(err.contains("이미지 좌표가 모순이다"));
}

#[test]
fn reentry_decision_needs_a_proposal() {
    let root = scratch("reentry");
    active(&root);
    let (status, _, err) = call(
        &root,
        &["--reentry-decide", "c1", "--decision", "decline", "--approved-by", "a", "--utc", "U"],
    );
    assert_eq!(status, Some(1));
    assert!(err.contains("셀 c1 에 대기 중인 재발동 제안이 없다(status=None)"));
}

fn closable(root: &Path) {
    active(root);
    put(root, "docs/testlog/t.md", "x");
    put(
        root,
        "campaigns/camp-a/evidence_pointers.json",
        "{\"pointers\": [{\"kind\": \"testlog\", \"path\": \"docs/testlog/t.md\"}]}",
    );
    put(root, "campaigns/camp-a/phases/main/bench.status.json", "{}");
    put(root, "campaigns/camp-a/phases/sub/bench.status.json", "{}");
}

#[test]
fn close_writes_the_event_and_points_active_at_bootstrap() {
    let root = scratch("close");
    closable(&root);
    let (status, out, _) = call(&root, &["--close", "camp-a", "--utc", "T9", "--apply"]);
    assert_eq!(status, Some(0));
    assert_eq!(
        out,
        "[campaign_init] APPLIED closed camp-a → ACTIVE=_bootstrap (인스턴스 보존: campaigns/camp-a)\n"
    );
    assert_eq!(std::fs::read_to_string(root.join("campaigns/ACTIVE")).unwrap(), "_bootstrap\n");
    let journey = std::fs::read_to_string(root.join("campaigns/camp-a/journey.jsonl")).unwrap();
    assert!(journey.contains("\"kind\": \"campaign_closed\"") && journey.contains("\"reason\": \"completed\""));
}

#[test]
fn close_is_refused_while_a_pointer_target_is_missing() {
    let root = scratch("close-gate");
    closable(&root);
    std::fs::remove_file(root.join("docs/testlog/t.md")).unwrap();
    let (status, _, err) = call(&root, &["--close", "camp-a", "--utc", "T9", "--apply"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("종결 선행조건(= purge 게이트)이 닫혀 있다:\n  - camp-a: 증거가 실재하지 않는다 — docs/testlog/t.md (kind=testlog)"));
    assert_eq!(std::fs::read_to_string(root.join("campaigns/ACTIVE")).unwrap(), "camp-a\n");
}

#[test]
fn verify_purge_gate_names_what_keeps_it_closed() {
    let root = scratch("gate");
    closable(&root);
    assert_eq!(call(&root, &["--verify-purge-gate", "camp-a"]).1, "[campaign_init] purge 게이트 열림\n");
    put(&root, "campaigns/camp-a/relay/l.json", "{}");
    let (status, _, err) = call(&root, &["--verify-purge-gate", "camp-a"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("camp-a: relay_summary 포인터가 없다"));
}

#[test]
fn evidence_add_attributes_once_and_refuses_a_rewrite() {
    let root = scratch("evidence");
    active(&root);
    put(&root, "docs/testlog/t.md", "x");
    let add = |cell: &str| {
        call(&root, &["--evidence-add", "--kind", "testlog", "--path", "docs/testlog/t.md", "--cell", cell, "--node", "main"])
    };
    let (status, out, _) = add("c1");
    assert_eq!(status, Some(0));
    assert_eq!(out, "[campaign_init] wrote(camp-a): campaigns/camp-a/evidence_pointers.json\n");
    assert_eq!(add("c1").0, Some(0), "the same attribution is idempotent");
    let (status, _, err) = add("c2");
    assert_eq!(status, Some(1));
    assert!(err.contains("이 포인터의 cell_id 는 이미 'c1' 다 — 'c2' 로 바꾸려면"), "{err}");
}

#[test]
fn freeze_blocks_additions_until_unfreeze() {
    let root = scratch("freeze");
    active(&root);
    put(&root, "docs/testlog/t.md", "x");
    assert_eq!(call(&root, &["--freeze-evidence", "--utc", "T"]).0, Some(1), "no pointer file yet");
    put(&root, "campaigns/camp-a/evidence_pointers.json", "{\"pointers\": []}");
    assert_eq!(call(&root, &["--freeze-evidence", "--utc", "T"]).0, Some(0));
    let (status, _, err) = call(&root, &["--evidence-add", "--kind", "plan", "--path", "docs/testlog/t.md", "--node", "main"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("이 스냅샷은 T 에 동결됐다"), "{err}");
    assert_eq!(call(&root, &["--evidence-add", "--kind", "plan", "--path", "docs/testlog/t.md", "--node", "main", "--unfreeze"]).0, Some(0));
}

#[test]
fn revise_appends_a_revision_after_the_t0_original() {
    let root = scratch("revise");
    active(&root);
    let args = ["--revise", "--revise-set", "x=2", "--revise-reason", "r", "--revise-evidence", "e", "--revise-approved-by", "a", "--utc", "U"];
    assert_eq!(call(&root, &args).0, Some(0));
    let text = std::fs::read_to_string(root.join("campaigns/camp-a/campaign.yaml")).unwrap();
    assert!(text.contains("\"reason\": \"t0 선언\"") && text.contains("\"rev\": 1") && text.contains("\"x\": \"2\""), "{text}");
}

#[test]
fn hint_approve_records_one_event_and_keeps_an_old_approval() {
    let root = scratch("hint");
    active(&root);
    let approve = |cells: &str, utc: &str| {
        call(&root, &["--hint-approve", "--node", "main", "--cells", cells, "--approved-by", "사람", "--utc", utc])
    };
    let (status, out, err) = approve("c1", "2026-11-15T01:00:00Z");
    assert_eq!((status, out.as_str()), (Some(0), "[campaign_init] wrote(camp-a): campaigns/camp-a/campaign.yaml\n"));
    assert!(err.contains("hint 사전승인 hint_targets[0] ← ['c1']"), "{err}");
    let (_, out, err) = approve("c1", "2026-11-16T01:00:00Z");
    assert_eq!(out, "[campaign_init] wrote(camp-a): \n");
    assert!(err.contains("'c1' 은 이미 hint_targets[0] 가 승인했다(declaration-popup · 2026-11-15T01:00:00Z)"), "{err}");
    let (status, _, err) = call(&root, &["--hint-approve", "--node", "main", "--cells", "s1", "--approved-by", "a", "--utc", "2026-11-15T01:00:00Z"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("assignments['main'] 에 없다"));
}

#[test]
fn resume_brief_reads_only_the_instance() {
    let root = scratch("brief");
    active(&root);
    put(&root, "campaigns/camp-a/cells/c1/cell.status.json", "{\"cell_outcome\": \"serve_failed\", \"void_reason\": \"why\"}");
    let (status, out, _) = call(&root, &["--resume-brief"]);
    assert_eq!(status, Some(0));
    assert!(out.starts_with("상태: ACTIVE=camp-a · 보류 0 · 미결 질의 0\n# resume-brief · campaigns/camp-a\n"), "{out}");
    assert!(out.contains("- `c1` [main · AUTO] — serve_failed  (void: why)"), "{out}");
    assert!(out.contains("## 6. 반증된 축 — **재시도 금지**\n- `c1`: why"), "{out}");
    assert!(out.contains("- 다음 pending 셀: 없음(전 셀 종결)") || out.contains("- 다음 pending 셀: s1"), "{out}");
}

#[test]
fn resume_brief_outside_a_campaign_says_so() {
    let root = scratch("brief-boot");
    let (status, out, _) = call(&root, &["--resume-brief"]);
    assert_eq!(status, Some(0));
    assert!(out.ends_with("들지 않는다 — campaigns writer 는 no-op 이고, 재개할 캠페인도 없다.\n"), "{out}");
}

#[test]
fn emit_slice_cuts_one_node_and_names_the_deriver() {
    let root = scratch("slice");
    active(&root);
    let (status, out, _) = call(&root, &["--emit-slice", "sub", "--utc", "T"]);
    assert_eq!(status, Some(0));
    assert!(out.contains("\"authored_by\": \"main-derived\"") && out.contains("\"derived_from\": \"main\""), "{out}");
    assert!(out.contains("\"assignments\": {\n    \"sub\": [") && out.contains("\"hint_targets\": []"), "{out}");
    let (status, _, err) = call(&root, &["--emit-slice", "zz", "--utc", "T"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("nodes[] 에 'zz' 가 없다"));
}

#[test]
fn answer_query_is_idempotent_and_needs_a_suspended_campaign() {
    let root = scratch("answer");
    active(&root);
    put(&root, "campaigns/camp-p/journey.jsonl", "{\"kind\": \"campaign_suspended\", \"utc\": \"U\"}\n");
    let ask = |extra: &[&str]| {
        let mut v = vec!["--answer-query", "camp-p", "--query-decision", "keep", "--utc", "T", "--approved-by", "a"];
        v.extend_from_slice(extra);
        call(&root, &v)
    };
    assert_eq!(ask(&[]).1, "[campaign_init] DRY-RUN answered keep camp-p\n");
    assert_eq!(ask(&["--apply"]).1, "[campaign_init] APPLIED answered keep camp-p\n");
    assert_eq!(
        ask(&["--apply"]).1,
        "[campaign_init] APPLIED no-op — camp-p 의 마지막 전이(0:campaign_suspended)는 이미 답했다(멱등)\n"
    );
    let (status, _, err) = call(&root, &["--answer-query", "camp-a", "--query-decision", "keep", "--utc", "T", "--approved-by", "a"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("camp-a 은 active 다 — 답할 보류 질의가 없다"), "{err}");
}

#[test]
fn lifecycle_prints_the_fold_and_residue_scan_classifies_root_entries() {
    let root = scratch("life");
    active(&root);
    let (status, out, _) = call(&root, &["--lifecycle", "camp-a"]);
    assert_eq!(status, Some(0));
    assert!(out.contains("\"state\": \"active\"") && out.contains("\"source\": \"campaigns/camp-a/journey.jsonl fold\""), "{out}");
    put(&root, ".claude/policies/root_registry.json", "{\"entries\": [{\"name\": \"campaigns\"}], \"tombstones\": [{\"name\": \"old_*\"}]}");
    put(&root, "old_x", "");
    put(&root, "stray", "");
    let (_, out, err) = call(&root, &["--residue-scan"]);
    assert!(out.contains("\"leak\": [\n    \"old_x\"\n  ]") && out.contains("\"unclassified\": ["), "{out}");
    assert!(err.contains("--utc 가 없어 residue.json 에 남기지 않았다"));
}

#[test]
fn a_missing_validator_refuses_an_assignment_question() {
    let root = scratch("novalidator");
    active(&root);
    std::fs::remove_file(root.join(".claude/skills/terraforming_node/scripts/campaign_template_validator.py")).unwrap();
    let (status, _, err) = call(&root, &["--assigned-cells", "main"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("검증기를 찾지 못했다: .claude/skills/terraforming_node/scripts/campaign_template_validator.py — 배정 규약의 소유자가 없다"), "{err}");
}

#[test]
fn import_sub_takes_progress_and_declared_evidence_from_the_mirror() {
    let root = scratch("import");
    active(&root);
    put(&root, "mirror/testlog/t.md", "x");
    put(
        &root,
        "mirror/logs/sub/campaign_brief.json",
        "{\"campaign_id\": \"camp-a\", \"node_id\": \"sub\", \"generated_utc\": \"G\", \"phases\": {\"build\": {\"state\": \"done\", \"proof_ok\": true, \"proof_source\": \"x\"}}, \"cells\": [{\"cell_id\": \"s1\", \"cell_outcome\": \"measured\", \"decode_tps_conc1\": 12}], \"journey_tail\": [], \"evidence_pointers\": [{\"kind\": \"testlog\", \"path\": \"docs/testlog/t.md\", \"node_id\": \"sub\", \"cell_id\": \"s1\"}]}",
    );
    let mirror = root.join("mirror");
    let (status, out, err) = call(&root, &["--import-sub", mirror.to_str().unwrap(), "--utc", "U"]);
    assert_eq!(status, Some(0), "{err}");
    assert!(out.contains("campaigns/camp-a/phases/sub/build.status.json") && out.contains("campaigns/camp-a/cells/s1/cell.status.json"), "{out}");
    let phase = std::fs::read_to_string(root.join("campaigns/camp-a/phases/sub/build.status.json")).unwrap();
    assert!(phase.contains("\"authored_by\": \"sub\"") && phase.contains("서브 회수(brief G)"), "{phase}");
    let cell = std::fs::read_to_string(root.join("campaigns/camp-a/cells/s1/cell.status.json")).unwrap();
    assert!(cell.contains("\"decode_tps_conc1\": 12.0"), "{cell}");
}

#[test]
fn backfill_repairs_a_missing_measurement_from_the_sweep_record() {
    let root = scratch("backfill");
    active(&root);
    put(&root, "campaigns/camp-a/sweeps/s.json", "{\"cells\": [{\"cell_key\": \"c1\", \"concurrency_vector\": {\"1\": 7.5}, \"cell_outcome\": \"measured\"}]}");
    let (status, out, err) = call(&root, &["--backfill-from-docs", "--utc", "U"]);
    assert_eq!(status, Some(0), "{err}");
    assert!(out.contains("campaigns/camp-a/cells/c1/cell.status.json"), "{out}");
    let cell = std::fs::read_to_string(root.join("campaigns/camp-a/cells/c1/cell.status.json")).unwrap();
    assert!(cell.contains("backfill:campaigns/camp-a/sweeps/s.json#cells[cell_key=c1].concurrency_vector.1") && cell.contains("\"cell_outcome\": \"measured\""), "{cell}");
}

#[test]
fn init_from_a_derived_slice_keeps_the_slice_declaration_and_its_parent() {
    let root = scratch("fromslice");
    std::process::Command::new("git").args(["init", "-q", "-b", "single"]).arg(&root).status().unwrap();
    put(&root, "slice.json", "{\"id\": \"camp-s\", \"plan_ref\": \"docs/plan/m.md\", \"self_role\": \"sub\", \"_lifecycle_parent\": \"camp-p\", \"_note\": 1, \"control_variables\": {\"topology\": \"single\"}}");
    let slice = root.join("slice.json");
    let args = ["--init", "camp-s", "--plan-ref", "ignored", "--target-topology", "single", "--from-slice", slice.to_str().unwrap(), "--apply", "--utc", "U", "--approved-by", "a"];
    let (status, out, err) = call(&root, &args);
    assert_eq!(status, Some(0), "{err}");
    assert!(out.ends_with("init=['campaigns/camp-s'] parent=camp-p\n"), "{out}");
    let decl = std::fs::read_to_string(root.join("campaigns/camp-s/campaign.yaml")).unwrap();
    assert!(decl.contains("\"plan_ref\": \"docs/plan/m.md\"") && !decl.contains("_note"), "{decl}");
    let journey = std::fs::read_to_string(root.join("campaigns/camp-s/journey.jsonl")).unwrap();
    assert!(journey.contains("\"parent\": \"camp-p\""), "{journey}");
}

#[test]
fn grounding_without_the_library_tool_records_an_honest_gap() {
    let root = scratch("ground");
    active(&root);
    put(&root, "campaigns/camp-a/campaign.yaml", "{\"id\": \"camp-a\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}]}, \"control_variables\": {\"topology\": \"single\", \"model\": \"M\"}}");
    let (status, out, err) = call(&root, &["--ground", "--utc", "2026-11-15T01:00:00Z"]);
    assert_eq!(status, Some(0), "{err}");
    assert_eq!(out, "[campaign_init] wrote(camp-a): campaigns/camp-a/grounding/20261115T010000Z.json\n");
    assert!(err.contains("입고 건너뜀 — 사서 도구 부재(.claude/skills/wiki-desk/src/cli.ts) · 판정=unresolved 참조=0건"), "{err}");
    assert_eq!(call(&root, &["--grounding-check"]).0, Some(0));
    let (_, out, _) = call(&root, &["--warm-start-library"]);
    assert!(out.starts_with("[campaign_init] 입고 건너뜀"), "{out}");
}
