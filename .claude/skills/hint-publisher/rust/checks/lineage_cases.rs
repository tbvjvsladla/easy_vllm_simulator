//! `lineage` checks (owner: the lineage port). Byte parity with the original is the golden stage `hp_lineage`; these
//! checks cover the module selftest and a few pure helpers without the bridge.
use hintlib::jd;
use hintlib::lineage as l;
use hintlib::value::J;

#[test]
fn lineage_selftest_passes() {
    assert_eq!(l::selftest(), Vec::<String>::new());
}

#[test]
fn slug_pattern_text_is_python_spelling() {
    let p = l::slug_pattern("Zeta2.5-Flash").unwrap();
    assert_eq!(p.pattern, "(?<![0-9a-z])zeta2[-_. ]?5[-_. ]?flash(?![0-9a-z])");
    assert!(p.search("a Zeta2_5 FLASH b"));
    assert!(!p.search("zeta2.50-flash"));
    assert_eq!(l::slug_pattern("--").err().map(|e| e.code).as_deref(), Some("HINT_LINEAGE_FILTER_ABSENT"));
}

#[test]
fn tool_snapshot_shapes() {
    let sha = "a".repeat(40);
    assert_eq!(l::tool_snapshot_rel("run_bench.sh", &sha).unwrap(), "inputs/sources/run_bench.sh@aaaaaaaaaaaa");
    assert_eq!(l::tool_snapshot_rel("../x", &sha).unwrap_err().code, "HINT_LINEAGE_TOOL_SOURCE_SHAPE");
    assert_eq!(l::tool_snapshot_rel("x", "abc").unwrap_err().code, "HINT_LINEAGE_TOOL_SOURCE_SHAPE");
    let bad = jd! {"path" => "tools/x.sh", "origin" => format!("git:{sha}:tools/x.sh")};
    let got = l::_tool_candidate_problem(std::path::Path::new("."), &bad).unwrap().unwrap();
    assert!(got.starts_with("tool-source-shape"));
}

#[test]
fn receiver_summary_limits() {
    let docs: Vec<J> = (0..200).map(|i| jd! {"path" => format!("docs/plan/plan_26010100_{}_{i}.md", "x".repeat(200))}).collect();
    let err = l::receiver_summary(&jd! {"documents" => docs}, &J::Null, &J::Null).unwrap_err();
    assert_eq!(err.code, "HINT_LINEAGE_SUMMARY_TOO_LARGE");
    assert_eq!(l::receiver_summary(&J::List(vec![]), &J::Null, &J::Null).unwrap_err().code, "HINT_LINEAGE_UNREADABLE");
    let ok = l::receiver_summary(&jd! {"documents" => vec![jd!{"path" => "a"}]}, &J::Null, &J::Null).unwrap();
    assert_eq!(ok.get_str("kind"), Some(l::SUMMARY_KIND));
}

#[test]
fn resolve_stem_and_identity() {
    let lin = jd! {"documents" => vec![jd!{"path" => "docs/testlog/testlog_26010600_x.md"}], "evidence_candidates" => J::list()};
    assert_eq!(l::resolve_stem(&lin, "testlog_26010600").unwrap().as_deref(), Some("docs/testlog/testlog_26010600_x.md"));
    assert_eq!(l::resolve_stem(&lin, "plan_26010600").unwrap(), None);
    let a = jd! {"model" => "M", "gpu" => "G", "topology" => "multi", "tp" => 2};
    let b = jd! {"model" => "m", "gpu" => "g", "topology" => "MULTI", "tp" => "2"};
    assert!(l::identity_equal(&a, &b).unwrap());
    assert!(!l::identity_equal(&a, &J::Null).unwrap());
}
