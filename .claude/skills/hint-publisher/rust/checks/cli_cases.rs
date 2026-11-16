//! `cli` checks (owner: the cli port): the parts the goldens cannot reach — the intended CLI-path difference, the gate
//! wrapper JSON of an un-runnable gate, `_build_doc` (the Python takes a `CellEvidence` dataclass the bridge cannot build),
//! the narrative-linter closure without a draft, and the `--self-test` placeholder.
use std::path::Path;

use hintlib::cli;
use hintlib::value::{loads, J};
use hintlib::{jd, jl};

#[test]
fn cli_path_is_the_rust_launcher() {
    let td = hintlib::core::TempDir::new("cli-check-");
    let repo = td.path();
    // absent launcher in this repo → the skill's own launcher (absolute) · present → the repo-relative launcher
    assert!(cli::_cli(repo).starts_with("bash /") && cli::_cli(repo).ends_with("/scripts/hint.sh"));
    let l = repo.join(hintlib::core::REL_HINT_CLI);
    std::fs::create_dir_all(l.parent().unwrap()).unwrap();
    std::fs::write(&l, "#!/bin/bash\n").unwrap();
    assert_eq!(cli::_cli(repo), "bash .claude/skills/hint-publisher/scripts/hint.sh");
}

#[test]
fn gate_bare_json_shape() {
    let s = cli::_gate_bare_json("hint_finalize", "HINT_GATE_SUBPROCESS_UNAVAILABLE", "없다", 0);
    let want = "{\n  \"action\": \"hint_finalize\",\n  \"allowed\": false,\n  \"authorization_state\": null,\n  \"exit_code\": 2,\n  \"identity\": null,\n  \"messages\": {\n    \"HINT_GATE_SUBPROCESS_UNAVAILABLE\": \"없다\"\n  },\n  \"mode\": \"promotion\",\n  \"reason_codes\": [\n    \"HINT_GATE_SUBPROCESS_UNAVAILABLE\"\n  ],\n  \"schema_version\": 1,\n  \"task_class\": null\n}\n";
    assert_eq!(s, want);
    assert!(cli::_gate_bare_json("a", "C", "m", 3).contains("\"exit_code\": 3"));
    assert_eq!(cli::action_for_cmd("continue"), Some("hint_finalize"));
    assert_eq!(cli::action_for_cmd("publish"), None);
}

#[test]
fn build_doc_selector_and_local_digest() {
    let ev = jd! {"build_identity" => jd! {"image_digest" => J::Null, "track" => "source",
        "image_digest_local" => jd! {"id" => "sha256:aa", "same_build" => true, "source" => "inspect"},
        "source" => jd! {"track" => "env", "zz" => 1}}};
    let b = cli::_build_doc(&ev, &J::dict());
    assert_eq!(b["image_digest"].as_str(), Some("sha256:aa"));
    assert_eq!(b["source"], jd! {"track" => "env", "image_digest" => "inspect"});
    let keys: Vec<&str> = b.as_dict().unwrap().keys().collect();
    assert_eq!(keys.len(), cli::_BUILD_KEYS.len() + 1);
    assert_eq!(keys.last(), Some(&"source"));
    // 측정 뒤 빌드 → 값 비움 · 출처 note
    let ev2 = jd! {"build_identity" => jd! {"image_digest_local" => jd! {"id" => "x", "same_build" => false, "source" => "after"}}};
    let art = jd! {"build" => jd! {"dockerfile" => "Dockerfile.x", "dockerfile_source" => "ledger", "track" => "wheel"}};
    let b2 = cli::_build_doc(&ev2, &art);
    assert!(b2["image_digest"].is_null());
    assert_eq!(b2["source"]["image_digest_local_note"].as_str(), Some("after"));
    assert_eq!(b2["dockerfile"].as_str(), Some("Dockerfile.x"));
    assert_eq!(b2["track"].as_str(), Some("wheel"));
    assert_eq!(b2["source"]["track"].as_str(), Some("artifacts 선택자(ledger)"));
    // native 셀 · 원천 이미지 레시피 있음 / 없음
    let art3 = jd! {"plane" => "native", "build" => jd! {"dockerfile" => "D", "track" => "t", "dockerfile_source" => "s"},
                    "native_source" => jd! {"cell" => "c0", "image_tag" => "img"}};
    let b3 = cli::_build_doc(&jd! {"build_identity" => J::Null}, &art3);
    assert_eq!(b3["track"].as_str(), Some("native"));
    assert_eq!(b3["source"]["track"].as_str(), Some("artifacts.plane_of"));
    assert_eq!(
        b3["source"]["dockerfile"].as_str(),
        Some("원천 이미지 Dockerfile(native 셀은 Docker 로 실행되지 않았다 · 원천 셀 c0 · 원천 이미지 img · artifacts 선택자 s)")
    );
    assert_eq!(b3["source_image_track"].as_str(), Some("t"));
    let b4 = cli::_build_doc(&J::dict(), &jd! {"plane" => "native", "plane_source" => "proof", "native_source" => "x"});
    assert!(b4["dockerfile"].is_null());
    assert_eq!(b4["source"]["dockerfile"].as_str(), Some("원천 이미지 미관측(원천 신호 없음) — Dockerfile 을 싣지 못했다(결손)"));
    assert_eq!(b4["source"]["track"].as_str(), Some("proof"));
}

#[test]
fn lint_fn_without_draft_reports_absence() {
    let lf = cli::_lint_fn(None, None, None, false);
    let r = lf.call(Path::new("."), "abc", "hint/x").unwrap();
    assert_eq!(r[0]["code"].as_str(), Some("HINT_LINT_DRAFT_ABSENT"));
    assert!(r[0]["message"].as_str().unwrap().starts_with("hint/x 를 봉인한 draft"));
}

#[test]
fn docker_runner_markers() {
    assert_eq!(cli::docker_runner(true), jd! {"__docker_runner__" => "read-only"});
    assert_eq!(cli::docker_runner(false), jd! {"__docker_runner__" => "probe-only"});
}

#[test]
fn factcheck_gate_waiver_and_eval() {
    let td = hintlib::core::TempDir::new("cli-fc-");
    let d = td.path();
    let st = jd! {"tag" => "hint/a/b/c/d"};
    let (s, e) = cli::_factcheck_gate(d, &st, None, "2026-01-02T06:00:00Z").unwrap();
    assert_eq!(s["status"].as_str(), Some("absent"));
    assert_eq!(e.unwrap().code, "HINT_FACTCHECK_ABSENT");
    let (s, e) = cli::_factcheck_gate(d, &st, Some("사람: 면제"), "2026-01-02T06:00:00Z").unwrap();
    assert!(e.is_none());
    let keys: Vec<&str> = s.as_dict().unwrap().keys().collect();
    assert_eq!(keys, vec!["status", "reason_code", "open", "source", "waiver", "waived_utc"]);
    assert_eq!(s["reason_code"].as_str(), Some("HINT_FACTCHECK_ABSENT"));
    std::fs::create_dir_all(d.join("inputs")).unwrap();
    let doc = jd! {"schema_version" => 1, "tag" => "hint/a/b/c/d", "author" => "w", "checker" => "c", "checked_utc" => "2026-01-02T06:00:00Z",
        "claims_checked" => 2, "classes_checked" => jl![1, 2, 3, 4, 5, 6, 7],
        "items" => jl![jd! {"id" => "f1", "where" => "w", "claim" => "c", "verdict" => "wrong", "class" => 1, "target" => "prose", "status" => "open"}]};
    std::fs::write(d.join("inputs/factcheck.json"), hintlib::core::dumps(&doc)).unwrap();
    let (s, e) = cli::_factcheck_eval(d, &st).unwrap();
    assert_eq!(s["status"].as_str(), Some("open"));
    assert_eq!(s["open"], jl!["f1"]);
    let e = e.unwrap();
    assert_eq!(e.code, "HINT_FACTCHECK_OPEN");
    assert!(e.message.ends_with("['f1']"));
    std::fs::write(d.join("inputs/factcheck.json"), "{bad").unwrap();
    let (s, e) = cli::_factcheck_eval(d, &st).unwrap();
    assert_eq!(s["status"].as_str(), Some("invalid"));
    assert!(e.unwrap().message.starts_with("inputs/factcheck.json 모양 결함 1건: 읽을 수 없다: "));
    assert!(loads("{}").is_ok());
}

#[test]
fn self_test_is_a_placeholder() {
    assert_eq!(cli::self_test(), 1);
}
