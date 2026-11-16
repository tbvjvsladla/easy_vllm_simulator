//! `template` checks (owner: the template port). Byte parity with the Python original is the goldens
//! (`.claude/rust/parity/cases/hp_template_*.json`); these are quick invariants per part.
use hintlib::template as t;
use hintlib::value::{loads, J};
use std::path::Path;

// ── t1 ──
#[test]
fn t1_cell_and_code() {
    assert_eq!(t::_cell(&J::Null, t::_MISSING_TEXT).unwrap(), "미관측");
    assert_eq!(t::_cell(&J::from("a|b\nc"), t::_MISSING_TEXT).unwrap(), "a\\|b c");
    assert_eq!(t::_cell(&loads("[\"x|\", null]").unwrap(), "—").unwrap(), "x\\| · —");
    assert_eq!(t::_cell(&loads("{\"b\": 1, \"a\": true}").unwrap(), "").unwrap(), "{\"a\": true, \"b\": 1}");
    assert_eq!(t::_code(&J::from("a`b"), t::_MISSING_TEXT).unwrap(), "`` a`b ``");
    assert_eq!(t::_code(&J::from(""), t::_MISSING_TEXT).unwrap(), "미관측");
}

#[test]
fn t1_lite_predicate() {
    let lite = |s: &str| t::is_lite_cell(&loads(s).unwrap()).unwrap();
    assert!(lite("{\"cell_grade\": \"lite\"}"));
    assert!(!lite("{\"measurement\": {\"benchmark_mode\": \"lite\"}, \"measurement_config\": {\"downgrade_reason\": \"x\"}}"));
    assert!(lite("{\"measurement_config\": {\"bench_mode\": \"lite\"}, \"task_class\": \"hint_map_only\"}"));
    assert!(!lite("{\"measurement_config\": {\"bench_mode\": \"lite\"}}"));
    assert_eq!(t::payload_docs(&loads("{\"cell_grade\": \"lite\"}").unwrap()).unwrap().len(), 3);
}

#[test]
fn t1_renderers_total() {
    let facts = loads("{}").unwrap();
    let ctx = t::_Ctx { repo: Path::new("."), facts: &facts, templates: &[] };
    for (fid, f) in t::FACT_RENDERERS() {
        assert!(f(&ctx).is_ok(), "renderer {fid} failed on empty facts");
    }
    assert!(t::_f_grade(&ctx).unwrap().contains("성능 판정(measurement.verdict) | 미관측 —"));
}

// ── selftest ──
#[test]
fn selftest_is_empty() {
    let bad = t::selftest();
    assert!(bad.is_empty(), "template selftest: {bad:#?}");
}
