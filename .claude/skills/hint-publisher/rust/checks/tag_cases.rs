//! `tag` checks (owner: the tag port). Byte parity with the Python original is the `hp_tag` golden stage; these are the
//! Rust-side facts: the ported selftest and the contracts other modules rely on.
use hintlib::core::HintError;
use hintlib::jd;
use hintlib::tag as t;
use hintlib::value::J;

fn fields() -> J {
    jd! {"version" => "2", "tag" => "hint/x/y/z/w", "topology" => "multi TP=2(Ray)", "anchor" => "a".repeat(40),
         "manifest_ref" => "docs/_evidence/fixture.work-manifest.json",
         "bench_ref" => "../benchmark/benchmark_fixture.yaml", "bench_kind" => "certificate"}
}

#[test]
fn selftest_is_green() {
    assert_eq!(t::selftest(), Vec::<String>::new());
}

#[test]
fn footer_roundtrip_and_binding_error_shape() {
    let f = fields();
    let w = t::build_footer(&f).unwrap();
    assert_eq!(t::parse_footer(&w, &[]).unwrap(), f);
    let e: HintError = t::parse_footer("no footer\n", &[]).unwrap_err();
    assert_eq!(e.code, "HINT_EVIDENCE_BINDING_MISSING");
    assert!(t::is_evidence_binding_error(&e));
    let msg = t::annotation("요약", &f).unwrap();
    assert_eq!(t::parse_annotation(&msg).unwrap()["brief"].as_str(), Some("요약"));
}

#[test]
fn problems_error_carries_codes() {
    let e = t::_fail_problems::<()>("HINT_SEAL_REFUSED", "봉인 전 검사", &["B: x".into(), "A: y".into()], "r").unwrap_err();
    assert_eq!(e.message, "봉인 전 검사 2건:\n  B: x\n  A: y");
    assert_eq!(t::problem_codes(e.problems.as_ref().unwrap()).unwrap(), ["A", "B"]);
}

#[test]
fn push_scope_literal_is_fixed() {
    assert_eq!(t::PUSH_SCOPE_ARGS, ["--no-follow-tags", "--no-recurse-submodules"]);
    let a = t::push_credential_args().unwrap();
    assert_eq!(a[..2], ["-c".to_string(), "credential.helper=".to_string()]);
    assert!(!a.iter().any(|x| x.contains("ghp_")));
}
