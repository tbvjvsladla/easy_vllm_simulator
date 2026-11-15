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
    let (status, out, err) = call(&root, &["--evidence-add", "--kind", "plan", "--path", "x.md"]);
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
    put(root, ".claude/skills/terraforming_node/scripts/campaign_template_validator.py", "#");
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
