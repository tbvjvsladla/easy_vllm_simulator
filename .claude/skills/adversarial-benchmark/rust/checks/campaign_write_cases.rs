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
fn abbreviations_and_usage_errors_are_answered_natively() {
    let root = scratch("fallback");
    active(&root);
    let (status, out, _) = call(&root, &["--assigned", "main"]);
    assert_eq!((status, out.as_str()), (Some(0), "c1\n"));
    let (status, out, err) = call(&root, &["--ce", "x"]);
    assert_eq!((status, out.as_str()), (Some(2), ""));
    assert!(err.ends_with("campaign_init.py: error: ambiguous option: --ce could match --cell, --cell-set, --cell-materialize, --cells\n"), "{err}");
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
fn a_missing_validator_file_does_not_matter() {
    let root = scratch("novalidator");
    active(&root);
    std::fs::remove_file(root.join(".claude/skills/terraforming_node/scripts/campaign_template_validator.py")).unwrap();
    let (status, out, _) = call(&root, &["--assigned-cells", "main"]);
    assert_eq!((status, out.as_str()), (Some(0), "c1\n"));
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

#[test]
fn assign_set_refuses_a_cell_that_already_ran_and_a_cell_of_another_node() {
    let root = scratch("assign");
    active(&root);
    put(&root, "campaigns/camp-a/cells/c1/cell.status.json", "{\"cell_outcome\": \"measured\"}");
    let base = ["--assign-set", "main", "--assign-reason", "r", "--assign-evidence", "e", "--assign-approved-by", "a", "--utc", "U"];
    let with = |cells: &'static str| {
        let mut v = base.to_vec();
        v.extend(["--assign-cells", cells]);
        call(&root, &v)
    };
    let (status, _, err) = with("");
    assert_eq!(status, Some(1));
    assert!(err.contains("pending 이 아닌 셀은 빼거나 mode 를 바꿀 수 없다(증거 연결 보존): c1(cell_outcome=measured)"), "{err}");
    let (status, _, err) = with("c1,s1");
    assert_eq!(status, Some(1));
    assert!(err.contains("셀 's1' 은 노드 'sub' 에 배정돼 있다"), "{err}");
    let mut missing = vec!["--assign-set", "main", "--assign-cells", "c1"];
    missing.extend(["--utc", "U"]);
    let (_, _, err) = call(&root, &missing);
    assert!(err.contains("--assign-reason 는 필수다"), "{err}");
}

#[test]
fn assign_cells_modes_are_closed() {
    let root = scratch("assign-mode");
    active(&root);
    let (status, _, err) = call(&root, &["--assign-set", "main", "--assign-cells", "c1:zz", "--assign-reason", "r", "--assign-evidence", "e", "--assign-approved-by", "a", "--utc", "U"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("--assign-cells 의 mode 는 ('AUTO', 'HITL', 'STAY') 중 하나다: 'c1:zz'"), "{err}");
}

#[test]
fn slice_update_accepts_only_a_main_derived_sub_declaration() {
    let root = scratch("sliceupdate");
    active(&root);
    put(&root, "slice.json", "{\"id\": \"camp-a\", \"self_role\": \"main\"}");
    let slice = root.join("slice.json");
    let (status, _, err) = call(&root, &["--slice-update", slice.to_str().unwrap(), "--utc", "U"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("--slice-update 는 메인 파생 선언(self_role=sub · authored_by=main-derived)만 받는다"), "{err}");
    let (status, _, err) = call(&root, &["--slice-update", slice.to_str().unwrap()]);
    assert_eq!(status, Some(1));
    assert!(err.contains("--slice-update 는 --utc 가 필요하다"), "{err}");
}

#[test]
fn escalation_add_without_the_explorer_owner_records_an_undetermined_candidate() {
    let root = scratch("escalation");
    active(&root);
    let (status, out, err) = call(&root, &["--escalation-add", "--cell", "c1", "--utc", "U", "--symptom", "config_arch_unsupported"]);
    assert_eq!(status, Some(0), "{err}");
    assert_eq!(out, "[campaign_init] wrote(camp-a): campaigns/camp-a/escalation_candidates.jsonl\n");
    let line = std::fs::read_to_string(root.join("campaigns/camp-a/escalation_candidates.jsonl")).unwrap();
    assert!(line.contains("\"predicate_1\": null") && line.contains("술어 ① 판정기가 없다("), "{line}");
    let (status, _, err) = call(&root, &["--escalation-add", "--utc", "U"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("--escalation-add 는 --cell 과 --utc 가 필요하다"));
}

#[test]
fn cell_materialize_derives_from_the_serving_yaml_and_stops_at_an_empty_axis() {
    let root = scratch("materialize");
    std::process::Command::new("git").args(["init", "-q", "-b", "single"]).arg(&root).status().unwrap();
    active(&root);
    put(&root, "campaigns/camp-a/campaign.yaml", "{\"id\": \"camp-a\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\", \"topology\": \"single\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}]}}");
    put(&root, "campaigns/_template/cells/_cell/config.yaml", "target_model:\n  path: <<FILL>>\nport: <<FILL>>\nvram_budget_gb: <<FILL>>\n");
    put(&root, "campaigns/_template/cells/_cell/lockset.json", "{\"provenance\": \"<<FILL>>\"}");
    put(&root, "campaigns/_template/cells/_cell/cell.status.json", "{\"cell_id\": \"<<FILL>>\"}");
    put(&root, "output/single/configs/c1.yaml", "model: /models/x\nport: 8001\n");
    let base = ["--cell-materialize", "c1", "--utc", "U"];
    let (status, _, err) = call(&root, &base);
    assert_eq!(status, Some(1));
    assert!(err.contains("채우지 못한 빈칸 ['vram_budget_gb']"), "{err}");
    let mut with = base.to_vec();
    with.extend(["--set", "vram_budget_gb=96"]);
    let (status, out, err) = call(&root, &with);
    assert_eq!(status, Some(0), "{err}");
    assert!(out.ends_with("cells/c1/config.yaml campaigns/camp-a/cells/c1/lockset.json campaigns/camp-a/cells/c1/cell.status.json\n"), "{out}");
    let config = std::fs::read_to_string(root.join("campaigns/camp-a/cells/c1/config.yaml")).unwrap();
    assert!(config.contains("  path: /models/x\nport: 8001\nvram_budget_gb: 96\n"), "{config}");
    let (status, _, err) = call(&root, &with);
    assert_eq!(status, Some(1));
    assert!(err.contains("에 이미 ['config.yaml', 'lockset.json', 'cell.status.json'] 가 있다"), "{err}");
}

#[test]
fn suspension_needs_the_release_declaration_the_naming_ssot_and_a_cooperative_moment() {
    let root = scratch("suspend");
    active(&root);
    let base = ["--suspend", "camp-a", "--utc", "U", "--approved-by", "a"];
    let (status, _, err) = call(&root, &base);
    assert_eq!(status, Some(1));
    assert!(err.contains("보류는 자원을 반납한다 — `--release <config>`(반복)"), "{err}");
    let mut none = base.to_vec();
    none.extend(["--release", "none"]);
    let (status, _, err) = call(&root, &none);
    assert_eq!(status, Some(1));
    assert!(err.contains("문서 명명 SSOT 를 적재하지 못했다: .claude/skills/wiki-desk/src/doc_naming.ts"), "{err}");
    put(&root, "campaigns/camp-a/phases/main/bench.status.json", "{\"state\": \"running\", \"cell_id\": \"c1\"}");
    let (status, _, err) = call(&root, &none);
    assert_eq!(status, Some(1));
    assert!(err.contains("진행 중인 셀이 있다 — c1(main:bench)"), "{err}");
}

#[test]
fn abandonment_asks_for_a_reason_and_a_summary_testlog() {
    let root = scratch("abandon");
    active(&root);
    let (status, _, err) = call(&root, &["--abandon", "camp-a", "--utc", "U", "--approved-by", "a"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("--abandon 은 --reason 이 필요하다"), "{err}");
    let (status, _, err) = call(&root, &["--abandon", "camp-a", "--utc", "U", "--approved-by", "a", "--reason", "r", "--release", "none"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("abandoned 축소 게이트가 닫혀 있다:\n  - 포기 요약 testlog 가 없다"), "{err}");
    put(&root, "docs/testlog/t.md", "x");
    let (status, out, err) = call(&root, &["--abandon", "camp-a", "--utc", "U", "--approved-by", "a", "--reason", "r", "--release", "none", "--summary-testlog", "docs/testlog/t.md"]);
    assert_eq!((status, out.as_str()), (Some(0), "[campaign_init] DRY-RUN dry-run abandon camp-a\n"), "{err}");
}

#[test]
fn resume_refuses_a_campaign_that_is_not_suspended() {
    let root = scratch("resume");
    active(&root);
    let (status, _, err) = call(&root, &["--resume-campaign", "camp-a", "--utc", "U", "--approved-by", "a"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("camp-a 은 active 다 — 재개는 보류 캠페인에만 한다"), "{err}");
}

#[test]
fn a_sub_instance_takes_docs_evidence_only_for_its_own_node_and_cell() {
    let root = scratch("subevidence");
    put(&root, "campaigns/ACTIVE", "camp-s\n");
    put(&root, "campaigns/camp-s/campaign.yaml", "{\"id\": \"camp-s\", \"self_role\": \"sub\", \"nodes\": [{\"node_id\": \"sub\", \"role\": \"sub\"}], \"assignments\": {\"sub\": [{\"cell\": \"s1\"}]}}");
    put(&root, "docs/testlog/t.md", "x");
    let add = |path: &str, cell: &str| call(&root, &["--evidence-add", "--kind", "testlog", "--path", path, "--cell", cell]);
    let (status, _, err) = add("docs/testlog/t.md", "s1");
    assert_eq!(status, Some(0), "{err}");
    let (status, _, err) = add("docs/testlog/t.md", "zz");
    assert_eq!(status, Some(1));
    assert!(err.contains("서브 증거 셀이 이 노드 배정과 다르다: node='sub', cell='zz'"), "{err}");
    let (status, _, err) = add("campaigns/camp-s/campaign.yaml", "s1");
    assert_eq!(status, Some(1));
    assert!(err.contains("서브 evidence claim은 docs 아래의 유효 kind/상대경로여야 한다"), "{err}");
}

#[test]
fn the_native_selftest_holds() {
    let root = scratch("selftest");
    let (status, out, err) = call(&root, &["--selftest"]);
    assert_eq!(status, Some(0), "{out}\n{err}");
    assert!(out.ends_with("[campaign_init] PASS\n") && !out.contains("  FAIL "), "{out}");
}

// ── plan_26111708 B L8 (remedies name the live launchers) · L11 (cell id ↔ serving config bound at declaration) ──────

/// A scratch checkout whose declaration the shipped schema accepts, so the per-cell declaration checks run past the
/// schema. The shipped `campaigns/_template/campaign.schema.json` is read (copied into the scratch), never written.
fn declared(name: &str) -> PathBuf {
    let root = scratch(name);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(4).expect("installed layout has a repository root");
    std::fs::copy(
        repo.join("campaigns/_template/campaign.schema.json"),
        root.join("campaigns/_template/campaign.schema.json"),
    )
    .unwrap();
    // the validator asks for the schema engine's launcher by presence only (the engine itself is in-process)
    put(&root, ".claude/policies/runtime/completion_gate.sh", "#");
    put(&root, "campaigns/ACTIVE", "camp-a\n");
    put(&root, "campaigns/camp-a/campaign.yaml", WELL_FORMED);
    root
}

const WELL_FORMED: &str = "{\"schema_version\": 1, \"id\": \"camp-a\", \"plan_ref\": \"docs/plan/p.md\", \"declared_utc\": \"2026-11-17T00:00:00Z\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\", \"topology\": \"multi\"}, {\"node_id\": \"sub\", \"role\": \"sub\", \"topology\": \"multi\"}], \"matrix\": {\"versions\": [\"0.32.0\"], \"models\": [\"m\"]}, \"assignments\": {\"main\": [{\"cell\": \"nemo35-super-vl-tp2\"}], \"sub\": [{\"cell\": \"s1\"}]}, \"budgets\": {\"smoke_budget_overhead_mib\": 1, \"ready_max_seconds\": 1}, \"control_variables\": {\"model\": \"m\", \"vllm_version\": \"0.32.0\", \"topology\": \"multi\", \"target_gpu\": \"gb10\"}, \"hint_targets\": [], \"topology_sections\": {\"single\": {}, \"multi\": {}}}";

fn assign(root: &Path, node: &str, cells: &str) -> (Option<i32>, String, String) {
    call(root, &["--assign-set", node, "--assign-cells", cells, "--assign-reason", "r", "--assign-evidence", "e", "--assign-approved-by", "a", "--utc", "U"])
}

fn validate(root: &Path, strict: bool) -> Vec<String> {
    let ctx = Ctx { root: root.to_path_buf() };
    sweep_stop::campaign_validator::validate_campaign(&ctx, &root.join("campaigns/camp-a/campaign.yaml"), strict)
        .unwrap_or_else(|_| panic!("the fixture schema is readable"))
}

#[test]
fn l8_grounding_remedy_names_the_campaign_init_launcher() {
    let root = scratch("l8-grounding");
    active(&root);
    let (status, _, err) = call(&root, &["--grounding-check"]);
    assert_eq!(status, Some(4));
    assert!(err.contains("`campaign_init.sh --ground --utc <t>` 로 물어라"), "{err}");
    assert!(!err.contains("campaign_init.py"), "{err}");
}

#[test]
fn l8_sub_reslice_guidance_names_relay_sh_and_sub_cells_need_no_local_config() {
    let root = declared("l8-reslice");
    // an A2A sub (single topology: the multi sub is a cluster worker, not a reslice target)
    put(&root, "campaigns/camp-a/campaign.yaml", &WELL_FORMED.replace("\"multi\"", "\"single\""));
    // sub cells are not observable from the main instance: no serving config is asked for them (L11 ② is the main's own)
    let (status, out, err) = assign(&root, "sub", "s1,s2");
    assert_eq!(status, Some(0), "{out}{err}");
    assert!(err.contains("relay.sh --campaign-control reslice --campaign-id camp-a --node sub"), "{err}");
    assert!(!err.contains("relay.py"), "{err}");
}

#[test]
fn l8_hint_arch_remedy_names_the_campaign_init_launcher() {
    let doc = match sweep_stop::pyjson::loads("{\"hint_targets\": [{\"arch\": \"x\"}]}").ok().map(|v| v.peel().clone()) {
        Some(sweep_stop::pyvalue::PyValue::Dict(d)) => d,
        _ => panic!("fixture is an object"),
    };
    let reasons = sweep_stop::campaign_validator::hint_target_reasons(&doc);
    assert!(reasons.iter().any(|r| r.contains("`campaign_init.sh --hint-approve` 로 적는다")), "{reasons:?}");
    assert!(reasons.iter().all(|r| !r.contains("campaign_init.py")), "{reasons:?}");
}

/// A closed tripwire list: the retired-program commands a remedy may no longer name. Help/usage text (`prog`) is out of
/// scope (golden-pinned; plan_26111708 A4 defers it), so only these command forms are listed.
#[test]
fn l8_no_remedy_in_this_crate_names_a_retired_python_command() {
    const RETIRED: [&str; 7] = [
        "relay.py --campaign-control",
        "campaign_init.py --ground",
        "campaign_init.py --resume-campaign",
        "campaign_init.py --hint-approve",
        "resolve_guidellm.py --record",
        "scan_node.py --emit-sub-manifest",
        "recipe.py --cap",
    ];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut hits = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                for retired in RETIRED {
                    if code.contains(retired) {
                        hits.push(format!("{}:{}: {retired}", path.display(), n + 1));
                    }
                }
            }
        }
    }
    assert!(hits.is_empty(), "{hits:#?}");
}

#[test]
fn l11_assign_set_names_a_cell_id_no_serving_config_carries_and_proceeds() {
    let root = declared("l11-missing");
    put(&root, "output/multi/configs/nemo35-super-vl-tp2.yaml", "model: /m\n");
    put(&root, "output/multi/configs/solar032-nv4-safe-tp2.yaml", "model: /s\n");
    let decl = root.join("campaigns/camp-a/campaign.yaml");
    // P3: the cell was declared under a new id while the triplet it meant is `solar032-nv4-safe-tp2`. An absence is
    // undecided (the triplet may come later), so the change proceeds — but the declaration step names it right away
    // (workflow.md §캠페인: gaps are recorded, only three places block).
    let (status, out, err) = assign(&root, "main", "nemo35-super-vl-tp2,solar2-250b-nv4-tp2");
    assert_eq!(status, Some(0), "{out}{err}");
    assert!(err.contains("경고(기재 · 진행): 셀 'solar2-250b-nv4-tp2' 을 결속할 config 가 없다"), "{err}");
    assert!(err.contains("기대 서빙 config output/multi/configs/solar2-250b-nv4-tp2.yaml 도 셀 입력 campaigns/camp-a/cells/solar2-250b-nv4-tp2/config.yaml 도 없다"), "{err}");
    assert!(std::fs::read_to_string(&decl).unwrap().contains("solar2-250b-nv4-tp2"));
    // the P3 correction (the cell named after the config) is accepted at declaration
    let (status, out, err) = assign(&root, "main", "nemo35-super-vl-tp2,solar032-nv4-safe-tp2");
    assert_eq!(status, Some(0), "{out}{err}");
    assert!(std::fs::read_to_string(&decl).unwrap().contains("solar032-nv4-safe-tp2"));
    let journey = std::fs::read_to_string(root.join("campaigns/camp-a/journey.jsonl")).unwrap();
    assert!(journey.contains("\"added\": [\"solar032-nv4-safe-tp2\"]") && journey.contains("campaign_assignments_revised"), "{journey}");
}

#[test]
fn l11_a_cell_input_binds_a_fresh_cell_and_a_foreign_or_unreadable_one_is_refused() {
    let root = declared("l11-input");
    put(&root, "output/multi/configs/nemo35-super-vl-tp2.yaml", "model: /m\n");
    // explorer flow: the cell input carries the cell's own name before its triplet exists
    put(&root, "campaigns/camp-a/cells/fresh-tp2/config.yaml", "serving:\n  config_name: fresh-tp2\n");
    let (status, out, err) = assign(&root, "main", "nemo35-super-vl-tp2,fresh-tp2");
    assert_eq!(status, Some(0), "{out}{err}");
    // a second change (the declaration now carries assignment_revisions) is still checked for the added cell
    put(&root, "campaigns/camp-a/cells/odd-tp2/config.yaml", "serving:\n  config_name: solar032-nv4-safe-tp2\n");
    let (status, _, err) = assign(&root, "main", "nemo35-super-vl-tp2,fresh-tp2,odd-tp2");
    assert_eq!(status, Some(1));
    assert!(err.contains("셀 'odd-tp2' 의 입력 campaigns/camp-a/cells/odd-tp2/config.yaml 이 serving.config_name='solar032-nv4-safe-tp2' 을 선언한다"), "{err}");
    assert!(err.contains("기대 config 이름 'odd-tp2' (output/multi/configs/odd-tp2.yaml)"), "{err}");
    put(&root, "campaigns/camp-a/cells/bad-tp2/config.yaml", "serving: [\n");
    let (status, _, err) = assign(&root, "main", "nemo35-super-vl-tp2,fresh-tp2,bad-tp2");
    assert_eq!(status, Some(1));
    assert!(err.contains("셀 'bad-tp2' 의 입력 campaigns/camp-a/cells/bad-tp2/config.yaml 을 읽지 못했다"), "{err}");
}

#[test]
fn l11_the_declaration_validator_names_the_expected_config() {
    let root = declared("l11-validator");
    // non-strict: an absence is undecided, not a contradiction — recorded at `--assign-set`, not counted here
    assert!(validate(&root, false).is_empty(), "{:?}", validate(&root, false));
    // strict: the binding sentence stands in for the bare missing-input line (one line per cell)
    let problems = validate(&root, true);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("기대 서빙 config output/multi/configs/nemo35-super-vl-tp2.yaml"), "{problems:?}");
    put(&root, "output/multi/configs/nemo35-super-vl-tp2.yaml", "model: /m\n");
    assert!(validate(&root, false).is_empty(), "a bound, schema-valid declaration passes");
    let problems = validate(&root, true);
    assert_eq!(problems, vec!["배정된 셀 'nemo35-super-vl-tp2' 입력이 없다: campaigns/camp-a/cells/nemo35-super-vl-tp2/config.yaml".to_string()]);
}

#[test]
fn l11_a_sub_instance_is_not_asked_for_configs_before_its_assignment() {
    let root = declared("l11-sub");
    put(
        &root,
        "campaigns/camp-a/campaign.yaml",
        &WELL_FORMED
            .replace("\"schema_version\": 1,", "\"schema_version\": 1, \"self_role\": \"sub\",")
            .replace("\"assignments\": {\"main\": [{\"cell\": \"nemo35-super-vl-tp2\"}], \"sub\": [{\"cell\": \"s1\"}]}", "\"assignments\": {\"sub\": [{\"cell\": \"s1\"}]}"),
    );
    assert!(validate(&root, false).is_empty(), "{:?}", validate(&root, false));
    // ① still holds there: the sub's own cell input may not name another config
    put(&root, "campaigns/camp-a/cells/s1/config.yaml", "serving:\n  config_name: other\n");
    let problems = validate(&root, false);
    assert!(problems.len() == 1 && problems[0].contains("serving.config_name='other'"), "{problems:?}");
}

#[test]
fn l11_materialize_keeps_its_refusals_through_the_shared_path_rule() {
    let root = declared("l11-materialize");
    put(&root, "campaigns/camp-a/campaign.yaml", &WELL_FORMED.replace("nemo35-super-vl-tp2", "solar2-250b-nv4-tp2"));
    let (status, _, err) = call(&root, &["--cell-materialize", "solar2-250b-nv4-tp2", "--utc", "U"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("서빙 yaml output/multi/configs/solar2-250b-nv4-tp2.yaml 부재 — 셀 id = config 이름이어야 한다"), "{err}");
    let root = scratch("l11-materialize-topo");
    active(&root);
    let (status, _, err) = call(&root, &["--cell-materialize", "c1", "--utc", "U"]);
    assert_eq!(status, Some(1));
    assert!(err.contains("노드 'main' 의 topology 를 선언에서 해소하지 못했다: None(추측 ✗)"), "{err}");
}

// ── plan_26111708 B L12 (the standalone campaign-cell sweep record) ─────────────────────────────────────────────────
// The end-to-end run (a real `sweep_bench.sh --campaign-cell` in the sandbox, read back by `--cell-set --sweep-state`,
// the `_bootstrap` no-op, a failed measurement, the multi topology) is `selftest_sweep_repeats` C1–C4; here the writer's
// own place contract.

#[test]
fn l12_campaign_cell_record_takes_only_a_place_derived_from_the_declaration() {
    let root = scratch("l12-place");
    for (state, code, said) in [
        ("output/single/benchlog/sweep_x.json", Some(2), "campaigns/<id>/sweeps/<name>.json 이 아니다"),
        ("campaigns/camp-a/relay/sweep_x.json", Some(2), "campaigns/<id>/sweeps/<name>.json 이 아니다"),
        ("campaigns/_template/sweeps/sweep_x.json", Some(2), "뼈대(_template)다"),
        ("campaigns/_bootstrap/sweeps/sweep_x.json", Some(0), "캠페인 밖(ACTIVE=_bootstrap)"),
    ] {
        let path = root.join(state);
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_bench_shell"))
            .arg("campaign-cell-record")
            .arg(&path)
            .env_clear()
            .env("PATH", "/nonexistent")
            .output()
            .expect("run bench_shell");
        let shown = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert_eq!(out.status.code(), code, "{state}: {shown}");
        assert!(shown.contains(said), "{state}: {shown}");
        assert!(!path.exists(), "{state}: nothing is written outside a campaign's sweeps/");
    }
}

// ── plan_26111708 B (schema): the shipped schema lists what `--assign-set` writes ─────────────────────────────────────

#[test]
fn schema_reads_assignment_revisions_so_the_per_cell_checks_run_after_an_assign_set() {
    let root = declared("schema-revisions");
    put(&root, "output/multi/configs/nemo35-super-vl-tp2.yaml", "model: /m\n");
    put(&root, "output/multi/configs/solar032-nv4-safe-tp2.yaml", "model: /s\n");
    let (status, out, err) = assign(&root, "main", "nemo35-super-vl-tp2,solar032-nv4-safe-tp2");
    assert_eq!(status, Some(0), "{out}{err}");
    let decl = root.join("campaigns/camp-a/campaign.yaml");
    let written = std::fs::read_to_string(&decl).unwrap();
    assert!(written.contains("assignment_revisions"), "{written}");
    // the writer's own output (t0 + rev 1) passes the schema: no SCHEMA_UNKNOWN_PROPERTY, nothing else either
    assert!(validate(&root, false).is_empty(), "{:?}", validate(&root, false));
    // and the validation goes past the schema: a hand-added cell whose input names another config is refused (L11 ①)
    put(&root, "campaigns/camp-a/campaign.yaml", &written.replace("\"cell\": \"solar032-nv4-safe-tp2\"", "\"cell\": \"solar2-250b-nv4-tp2\""));
    put(&root, "campaigns/camp-a/cells/solar2-250b-nv4-tp2/config.yaml", "serving:\n  config_name: solar032-nv4-safe-tp2\n");
    let problems = validate(&root, false);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("serving.config_name='solar032-nv4-safe-tp2' 을 선언한다"), "{problems:?}");
    std::fs::remove_file(root.join("campaigns/camp-a/cells/solar2-250b-nv4-tp2/config.yaml")).unwrap();
    // the schema is no looser than the writer: t0 is the original (reason/approver fixed, no change fields), a change
    // carries node·removed·added·source
    for (from, to, code) in [
        ("\"approved_by\": \"t0\"", "\"approved_by\": \"someone\"", "SCHEMA_CONST_VIOLATION:assignment_revisions[0].approved_by"),
        ("\"source\": \"campaign_init --assign-set\"", "\"origin\": \"campaign_init --assign-set\"", "SCHEMA_MISSING_REQUIRED_KEY:assignment_revisions[1].source"),
        ("\"reason\": \"t0 선언\",", "\"reason\": \"t0 선언\", \"node\": \"main\",", "SCHEMA_UNKNOWN_PROPERTY:assignment_revisions[0].node"),
    ] {
        assert!(written.contains(from), "fixture has {from}: {written}");
        put(&root, "campaigns/camp-a/campaign.yaml", &written.replacen(from, to, 1));
        let problems = validate(&root, false);
        assert!(problems.iter().any(|p| p.contains(code)), "{code}: {problems:?}");
    }
}
