//! `catalog` checks (owner: the catalog port). Byte parity with the Python original is the `hp_catalog` golden stage;
//! these are the Rust-side facts: the ported selftest and a few focused contracts other modules rely on.
use hintlib::catalog as c;
use hintlib::value::J;
use hintlib::{jd, jl};

#[test]
fn selftest_is_green() {
    assert_eq!(c::selftest(), Vec::<String>::new());
}

#[test]
fn marker_splice_and_offsets() {
    let doc = format!("가\n{}\n옛\n{}\n끝", c::HINTS_BEGIN, c::HINTS_END);
    let (s, e) = c::locate_derived_block(&doc).unwrap();
    // character offsets (Python string offsets), not bytes
    assert_eq!(s as usize, doc[..doc.find("옛").unwrap()].chars().count());
    assert_eq!(e - s, 2);
    assert_eq!(c::splice_hints_md(&doc, "새\n").unwrap(), doc.replace("옛\n", "새\n"));
    assert_eq!(c::splice_hints_md("x\n", "y").unwrap_err().code, "HINT_CATALOG_MARKER_ABSENT");
}

#[test]
fn cells_and_match() {
    assert_eq!(c::md_cell(&J::from("a|b\n<!-- x")).unwrap(), "a\\|b &lt;!-- x");
    assert_eq!(c::_missing_cell(&jd! {"missing" => J::Null, "object" => c::OBJECT_ABSENT_LOCAL}).unwrap(), c::BENCH_MODE_UNRECEIVED);
    assert_eq!(c::_missing_cell(&jd! {"missing" => jl!["B", "A"]}).unwrap(), "B · A");
    assert_eq!(c::tagger_utc("tagger x <> 0 +0000").unwrap().as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(c::_arch_match("gb10x2-cluster-native", Some("gb10-main")).unwrap(), "hw");
    assert_eq!(c::_vnorm(Some(" v0.29.0 ")).unwrap(), "0.29.0");
    let idx = jd! {"hints" => jl![jd!{"tag" => "hint/0.1/m/a", "model" => "m", "vllm" => "0.1", "arch" => "a"}]};
    let r = c::r#match(&idx, "0.1", "M", None, false).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].get_str("relation"), Some(c::REL_SAME));
    assert_eq!(c::r#match(&idx, " ", "m", None, false).unwrap_err().code, "HINT_CATALOG_MATCH_QUERY_EMPTY");
    assert!(c::format_match(&idx, &J::from(r), "M", false).unwrap().contains("[same-slug] hint/0.1/m/a"));
}
