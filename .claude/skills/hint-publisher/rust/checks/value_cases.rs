//! Porting substrate: `value` (json.dumps byte equality), `pyre` (Python re spellings) and `core.selftest`.
use hintlib::pyre::{Re, M, S};
use hintlib::value::{dumps_default, dumps_pretty_sorted, dumps_tight_sorted, dumps_unicode, loads, J};
use hintlib::{jd, jl};

#[test]
fn dumps_matches_python() {
    // json.dumps({"b": [1, 2.5, None, True], "a": "가\n"}) == '{"b": [1, 2.5, null, true], "a": "\\uac00\\n"}'
    let v = jd! {"b" => jl![1, 2.5, J::Null, true], "a" => "가\n"};
    assert_eq!(dumps_default(&v), "{\"b\": [1, 2.5, null, true], \"a\": \"\\uac00\\n\"}");
    assert_eq!(dumps_unicode(&v), "{\"b\": [1, 2.5, null, true], \"a\": \"가\\n\"}");
    assert_eq!(dumps_pretty_sorted(&v), "{\n  \"a\": \"가\\n\",\n  \"b\": [\n    1,\n    2.5,\n    null,\n    true\n  ]\n}");
    assert_eq!(dumps_tight_sorted(&jd! {"z" => jd!{}, "a" => jl![]}), r#"{"a":[],"z":{}}"#);
    assert_eq!(dumps_default(&J::Float(1e16)), "1e+16");
    assert_eq!(dumps_default(&J::Float(0.1)), "0.1");
    assert_eq!(dumps_default(&J::Str("\u{1F600}".into())), "\"\\ud83d\\ude00\"");
}

#[test]
fn loads_keeps_order_and_last_duplicate() {
    let v = loads(r#"{"b": 1, "a": 2, "b": 3}"#).unwrap();
    assert_eq!(dumps_unicode(&v), r#"{"b": 3, "a": 2}"#);
    assert!(loads("{").is_err());
}

#[test]
fn regex_python_spellings() {
    let r = Re::new(r"^a.c$", 0);
    assert!(r.is_search("abc\n")); // Python `$` matches before a final newline
    assert!(!r.is_full("abc\n"));
    assert!(Re::new(r"a\Z", 0).is_search("xa"));
    assert!(!Re::new(r"a\Z", 0).is_search("xa\n"));
    assert!(Re::new(r"^b$", M).is_search("a\nb\nc"));
    assert!(Re::new(r"a.b", S).is_search("a\nb"));
    assert_eq!(Re::new(r"[]x]+", 0).findall("a]x]b"), vec!["]x]".to_string()]);
    assert_eq!(Re::new(r"(?P<n>\d+)-(?P=n)", 0).is_full("12-12"), true);
    assert!(Re::new(r"x$", 0).is_match("x"));
}

#[test]
fn core_selftest_passes() {
    assert_eq!(hintlib::core::selftest(), Vec::<String>::new());
}
