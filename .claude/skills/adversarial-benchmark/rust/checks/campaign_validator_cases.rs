//! `campaign_validator::acceptance`: the P4-P6 verdicts on a scratch instance under the system temporary directory.

use std::path::{Path, PathBuf};
use sweep_stop::campaign_validator::acceptance;
use sweep_stop::campaign_write::{Ctx, Sinks};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("campaign-validator-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn verdict(root: &Path, instance: &str) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let instance = root.join(instance);
    let status = acceptance(
        &Ctx { root: root.to_path_buf() },
        instance.to_str().unwrap(),
        &mut Sinks { out: &mut out, err: &mut err },
    );
    (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

const DECL: &str = "{\"id\": \"c\", \"nodes\": [{\"node_id\": \"main\", \"role\": \"main\"}, {\"node_id\": \"sub\", \"role\": \"sub\"}], \"assignments\": {\"main\": [{\"cell\": \"c1\"}], \"sub\": [{\"cell\": \"s1\"}]}}";

#[test]
fn missing_instance_passes_without_a_declaration() {
    let root = scratch("missing");
    assert_eq!(verdict(&root, "campaigns/none"), (0, "[campaign_template_validator] PASS\n".to_string(), String::new()));
}

#[test]
fn absent_lockset_is_red_and_the_sub_cell_is_a_note() {
    let root = scratch("lockset");
    put(&root, "campaigns/c/campaign.yaml", DECL);
    let (status, out, err) = verdict(&root, "campaigns/c");
    assert_eq!(status, 1);
    assert!(out.starts_with("  P6 ⓘ 관측 대상 밖"), "{out}");
    assert!(err.starts_with("[campaign_template_validator] FAIL (2)\n"), "{err}");
    assert!(err.contains("P4: 메인(main) phase 에 시각이 하나도 없다"));
    assert!(err.contains("P6 c1: lockset 부재: campaigns/c/cells/c1/lockset.json"));
}

#[test]
fn sub_phase_written_by_the_main_is_red() {
    let root = scratch("p5");
    put(&root, "campaigns/c/campaign.yaml", DECL);
    put(&root, "campaigns/c/phases/sub/a.status.json", "{\"authored_by\": \"main\", \"ended_utc\": \"T\"}");
    put(&root, "campaigns/c/phases/sub/b.status.json", "{\"authored_by\": \"sub\", \"ended_utc\": \"T\"}");
    put(&root, "campaigns/c/cells/c1/lockset.json", "{\"provenance\": \"hand-authored\", \"gmu_source\": \"odd\"}");
    let (status, out, err) = verdict(&root, "campaigns/c");
    assert_eq!(status, 1);
    assert!(out.contains("P6 ⓘ c1: gmu_source='odd' 가 목록 밖이다(허용 ('target_gmu', 'hand'))"), "{out}");
    assert!(err.contains("P5 sub/a.status.json: authored_by='main' — 메인이 서브의 진행표를 적었다"));
    assert!(err.contains("P5 sub: phase 2건의 ended_utc 가 모두 T 로 같다"));
}

#[test]
fn an_empty_checkout_has_no_template() {
    let root = scratch("template");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let request = sweep_stop::campaign_validator::Request { campaign: None, instance: None, template: true, acceptance: None };
    let status = sweep_stop::campaign_validator::run_request(
        &Ctx { root: root.clone() },
        &request,
        &mut Sinks { out: &mut out, err: &mut err },
    )
    .unwrap();
    let err = String::from_utf8(err).unwrap();
    assert_eq!(status, 1);
    assert!(err.contains("뼈대 결손: campaigns/_template/campaign.schema.json") && err.contains("뼈대 결손: campaigns/_template/sweeps/.gitkeep"), "{err}");
}

#[test]
fn a_campaign_without_the_schema_engine_names_what_is_missing() {
    let root = scratch("engine");
    put(&root, "campaigns/c/campaign.yaml", "{\"id\": \"c\"}");
    put(&root, "campaigns/_template/campaign.schema.json", "{}");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let request = sweep_stop::campaign_validator::Request {
        campaign: Some(root.join("campaigns/c/campaign.yaml").to_string_lossy().into_owned()),
        instance: None,
        template: false,
        acceptance: None,
    };
    let status = sweep_stop::campaign_validator::run_request(&Ctx { root }, &request, &mut Sinks { out: &mut out, err: &mut err }).unwrap();
    assert_eq!(status, 1);
    assert!(String::from_utf8(err).unwrap().contains("FAIL 스키마 엔진이 없다: .claude/policies/runtime/completion_gate.py"));
}
