//! `pii` checks: the module selftest, the public contract other modules call, and the `exempt_log` list form (the bridge
//! cannot observe a caller's list on either side, so its entries are pinned here against the original's literal text).
//! Fixture strings that match the four deploy patterns are assembled from fragments (this file is a tracked deploy file).
use hintlib::pii::{self, Hit};
use hintlib::value::J;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn selftest_passes() {
    assert_eq!(pii::selftest(), Vec::<String>::new());
}

#[test]
fn hit_display_and_render() {
    let h = Hit::new("term", "x", 1);
    assert_eq!(h.to_string(), "term:x");
    assert_eq!(h.r#match, "x");
    let hits: Vec<Hit> = (1..=3).map(|i| Hit::new("term", &"y".repeat(60), i)).collect();
    assert_eq!(pii::render_hits(&hits, 2).unwrap(), format!("1: term:{y}\n  2: term:{y}\n  … 외 1건", y = "y".repeat(48)));
    let located = vec![("a.md".to_string(), Hit::new("term", "z", 4))];
    assert_eq!(pii::render_located_hits(&located, 20).unwrap(), "a.md:4: term:z");
    assert_eq!(pii::render_hits(&[], 0).unwrap(), "");
}

#[test]
fn exempt_log_entries_match_the_original_text() {
    let op = format!("/{}/{}", "mnt", "models");
    let text = format!("- ${{NAS:-{op}}}:/app:ro\n  # 기본값 {op} 로\n");
    let mut log: Vec<J> = Vec::new();
    let hits = pii::scan_text(&text, None, "deploy", &[], true, true, Some(&mut log)).unwrap();
    assert!(hits.is_empty());
    assert_eq!(
        log,
        vec![
            J::Str(format!("shell-default 1: abs-op-path:{op}")),
            J::Str(format!("comment-echo-of-default 2: abs-op-path:{op}")),
        ]
    );

    let td = hintlib::core::TempDir::new("piicase-");
    let root = td.path();
    std::fs::create_dir_all(root.join("c")).unwrap();
    std::fs::write(root.join("c/compose.yaml"), &text).unwrap();
    std::fs::write(root.join("blob.bin"), b"\xff\x00").unwrap();
    let mut tlog: Vec<J> = Vec::new();
    let th = pii::scan_tree(root, None, "deploy", &[], true, true, None, Some(&mut tlog)).unwrap();
    assert!(th.is_empty());
    assert_eq!(
        tlog,
        vec![
            J::Str("binary blob.bin: UTF-8 해독 불가 — 건너뜀".into()),
            J::Str(format!("shell-default c/compose.yaml:1: abs-op-path:{op}")),
            J::Str(format!("comment-echo-of-default c/compose.yaml:2: abs-op-path:{op}")),
        ]
    );
    // shell default off: both lines are hits, nothing is logged.
    let mut off: Vec<J> = Vec::new();
    let th = pii::scan_tree(root, None, "deploy", &[], false, true, Some(&s(&["c"])), Some(&mut off)).unwrap();
    assert_eq!(th.iter().map(|(r, h)| (r.as_str(), h.line)).collect::<Vec<_>>(), vec![("c/compose.yaml", 1), ("c/compose.yaml", 2)]);
    assert!(off.is_empty());
}

#[test]
fn contract_terms_and_substitution() {
    let td = hintlib::core::TempDir::new("piicase-");
    let repo = td.path();
    assert_eq!(pii::load_pii_terms(repo).unwrap(), None);
    assert_eq!(pii::require_terms(repo).unwrap_err().code, "HINT_PII_TERMS_ABSENT");
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude/pii_terms.txt"), "# c\n OpName \n").unwrap();
    assert_eq!(pii::require_terms(repo).unwrap(), s(&["OpName"]));
    assert_eq!(pii::manifest_placeholder("nodes[sub].work_dir").unwrap(), "<manifest.nodes[sub].work_dir>");
    let ten = pii::_ip(&[10, 1, 2, 3]);
    let manifest = hintlib::jd! {"nodes" => vec![hintlib::jd! {"role" => "main", "host" => ten.clone()}]};
    let table = pii::substitution_table(repo, &manifest).unwrap();
    assert!(table.contains(&(ten.clone(), "<node:main>".to_string())));
    assert!(table.contains(&("OpName".to_string(), "<redacted>".to_string())));
    let out = pii::substitute(&format!("a {ten} OpName {ten}9"), &table, true).unwrap();
    assert_eq!(out, "a <node:main> <redacted> <priv-ip>");
    assert_eq!(pii::substitute("x", &[(String::new(), "<a>".into())], true).unwrap_err().code, "HINT_PII_SUBST_TABLE_SHAPE");
}

#[test]
fn python_word_boundary_and_space_semantics() {
    let ten = pii::_ip(&[10, 20, 30, 40]);
    // Python `\b`: a letter (가) or an `N*` digit (²) before the address is a word char → no hit; a combining mark or a
    // connector punctuation is not (Rust `\w` would say the opposite for both).
    let n = |pre: &str| pii::scan_text(&format!("{pre}{ten}"), None, "deploy", &[], false, true, None).unwrap().len();
    assert_eq!(n("가"), 0);
    assert_eq!(n("²"), 0);
    assert_eq!(n("e\u{301}"), 1);
    assert_eq!(n("‿"), 1);
    assert_eq!(n(" "), 1);
    // Python `\s` includes U+001C..U+001F: `§ \x1c10.1.2` is a section anchor.
    let sec = format!("{}{}", "10.", "1.2");
    assert!(pii::scan_text(&format!("§ \u{1c}{sec}"), None, "deploy", &[], false, true, None).unwrap().is_empty());
}
