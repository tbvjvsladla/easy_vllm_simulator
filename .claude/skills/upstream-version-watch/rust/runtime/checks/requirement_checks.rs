//! Checks of the Requirement constructor projection (R35 `requirement-main-projection-packaging24`): every one of the 224 saved original results
//! (112 inputs under each of the hash profiles `PYTHONHASHSEED=0` and `17`) through the public service with exact typed fields, the behaviours
//! the saved corpus does not name but the pinned sources decide, the protocol refusals and the normal program.
//!
//! The saved controls are read through the environment variable `R35_REQUIREMENT_REFERENCE` (the directory that holds `cases.json` and
//! `seed<N>-requirement-<NNN>/owner-record.json`); the production code contains no path, label or expected value. These checks start no Python.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::PathBuf;
use upstream_runtime::api::render;
use upstream_runtime::requirement::service::{answer, evaluate_jsonl, OPERATION};

const SEEDS: [u32; 2] = [0, 17];

fn reference() -> PathBuf {
    PathBuf::from(std::env::var("R35_REQUIREMENT_REFERENCE").expect(
        "R35_REQUIREMENT_REFERENCE must name the saved reference directory (cases.json, seed0-requirement-001 ...)",
    ))
}

fn read(path: PathBuf) -> PyValue {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    loads(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dict(value: &PyValue) -> &PyDict {
    match value {
        PyValue::Dict(map) => map,
        other => panic!("not an object: {}", other.repr()),
    }
}

fn list(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not an array: {}", other.repr()),
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn shown(value: &PyValue) -> String {
    render(value).expect("renderable")
}

fn request_line(id: &str, input_dto: &PyValue, seed: u32) -> PyStr {
    let line = format!(
        r#"{{"id": {}, "operation": "{OPERATION}", "input_dto": {}, "hash_seed": {seed}}}"#,
        shown(&PyValue::str(id)),
        shown(input_dto)
    );
    PyStr::new(&line)
}

fn answer_for(id: &str, input_dto: &PyValue, seed: u32) -> PyValue {
    answer(&request_line(id, input_dto, seed))
}

fn str_dto(text: &str) -> PyValue {
    PyValue::List(vec![PyValue::str("str"), PyValue::str(text)])
}

fn field<'a>(record: &'a PyValue, name: &str) -> &'a PyValue {
    dict(record)
        .get(name)
        .unwrap_or_else(|| panic!("record has no {name}: {}", shown(record)))
}

/// The `stderr` text of a record.
fn stderr_of(record: &PyValue) -> String {
    text_of(field(record, "stderr"))
}

/// The members of the typed projection dict as `(name, typed value)` pairs.
fn projection_members(record: &PyValue) -> Vec<(String, PyValue)> {
    let PyValue::List(parts) = field(record, "projection") else {
        panic!("projection is a typed array")
    };
    list(&parts[1])
        .iter()
        .map(|pair| {
            let pair = list(pair);
            let PyValue::List(key) = &pair[0] else {
                panic!("typed key")
            };
            (text_of(&key[1]), pair[1].clone())
        })
        .collect()
}

fn is_accepted(record: &PyValue) -> bool {
    let members = projection_members(record);
    let (_, flag) = members
        .iter()
        .find(|(name, _)| name == "accepted")
        .expect("accepted");
    let PyValue::List(parts) = flag else {
        panic!("typed bool")
    };
    matches!(parts[1], PyValue::Bool(true))
}

fn refusal_kind(record: &PyValue) -> String {
    text_of(field(field(record, "refusal"), "kind"))
}

#[test]
fn all_224_saved_originals_match_field_for_field() {
    let root = reference();
    let cases = read(root.join("cases.json"));
    assert_eq!(list(&cases).len(), 112, "the saved corpus has 112 inputs");
    let mut compared = 0;
    for case in list(&cases) {
        let id = text_of(field(case, "id"));
        let number = id.strip_prefix("requirement-").expect("case id");
        for seed in SEEDS {
            let saved =
                read(root.join(format!("seed{seed}-requirement-{number}/owner-record.json")));
            let record = answer_for(&id, field(case, "input_dto"), seed);
            for name in [
                "projection",
                "input_before",
                "input_after",
                "stdout",
                "stderr",
            ] {
                assert_eq!(
                    shown(field(&record, name)),
                    shown(field(&saved, name)),
                    "{id} seed {seed}: {name}"
                );
            }
            assert_eq!(shown(field(&record, "id")), shown(&PyValue::str(&id)));
            assert_eq!(
                text_of(field(&record, "scope")),
                "requirement-main-projection-packaging24"
            );
            assert_eq!(
                dict(&record).len(),
                7,
                "{id}: exactly the seven business fields"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 224);
}

#[test]
fn the_seed_changes_only_the_iteration_order_of_a_set_with_several_specifiers() {
    let input = str_dto("torch>=1,>2,~=3.1,<5,<=4");
    let first = answer_for("order", &input, 0);
    let second = answer_for("order", &input, 17);
    assert_ne!(
        shown(field(&first, "projection")),
        shown(field(&second, "projection"))
    );
    let text = |record: &PyValue| {
        let members = projection_members(record);
        shown(
            &members
                .iter()
                .find(|(name, _)| name == "specifier_text")
                .expect("text")
                .1,
        )
    };
    assert_eq!(
        text(&first),
        text(&second),
        "str(specifier) is sorted, so the seed cannot change it"
    );
}

#[test]
fn a_warning_printed_before_a_rejection_is_kept() {
    let record = answer_for("w", &str_dto(r#"torch; os_name == "a\q" garbage"#), 0);
    assert!(!is_accepted(&record));
    assert_eq!(
        stderr_of(&record),
        "<unknown>:1: SyntaxWarning: invalid escape sequence '\\q'\n"
    );
}

#[test]
fn each_literal_with_an_invalid_escape_warns_once_for_its_first_one() {
    let record = answer_for(
        "w2",
        &str_dto(r#"torch; os_name == "\q\z" and sys_platform == "\y" or extra == "\400""#),
        17,
    );
    assert!(is_accepted(&record));
    assert_eq!(
        stderr_of(&record),
        "<unknown>:1: SyntaxWarning: invalid escape sequence '\\q'\n\
         <unknown>:1: SyntaxWarning: invalid escape sequence '\\y'\n\
         <unknown>:1: SyntaxWarning: invalid octal escape sequence '\\400'\n"
    );
}

#[test]
fn a_rejected_literal_prints_no_warning_of_its_own() {
    let record = answer_for("w3", &str_dto(r#"torch; os_name == "\q\x4""#), 0);
    assert!(!is_accepted(&record));
    assert_eq!(stderr_of(&record), "");
}

#[test]
fn a_backslash_before_a_non_ascii_character_is_not_an_invalid_escape() {
    let record = answer_for("w4", &str_dto("torch; os_name == \"\\\u{e9}\""), 0);
    assert!(is_accepted(&record));
    assert_eq!(stderr_of(&record), "");
}

#[test]
fn a_line_continuation_in_a_marker_string_is_accepted_silently() {
    let record = answer_for("w5", &str_dto("torch; os_name == \"a\\\nb\""), 0);
    assert!(is_accepted(&record));
    assert_eq!(stderr_of(&record), "");
}

#[test]
fn a_python_keyword_variant_of_the_unicode_letters_in_a_version_is_matched_like_re() {
    // `\u{17F}` (long s) is equated with `s` and `\u{130}` lowers to `i` by re.IGNORECASE: both spell a post/preview release.
    for text in ["torch==1.0.po\u{17F}t1", "torch==1.0.prev\u{130}ew1"] {
        let record = answer_for("u", &str_dto(text), 0);
        assert!(is_accepted(&record), "{text}");
    }
}

#[test]
fn deep_marker_nesting_is_refused_not_rejected() {
    let deep = format!(
        "torch; {}os_name == \"x\"{}",
        "(".repeat(101),
        ")".repeat(101)
    );
    let record = answer_for("deep", &str_dto(&deep), 0);
    assert_eq!(
        refusal_kind(&record),
        "marker_nesting_beyond_recursion_limit"
    );
    assert!(dict(&record).get("projection").is_none());
    let ok = format!(
        "torch; {}os_name == \"x\"{}",
        "(".repeat(100),
        ")".repeat(100)
    );
    assert!(is_accepted(&answer_for("deep-ok", &str_dto(&ok), 0)));
}

#[test]
fn an_integer_over_the_conversion_limit_rejects_the_requirement() {
    let digits = "9".repeat(4301);
    let record = answer_for("big", &str_dto(&format!("torch=={digits}")), 0);
    assert!(!is_accepted(&record));
    let fine = "9".repeat(4300);
    assert!(is_accepted(&answer_for(
        "big-ok",
        &str_dto(&format!("torch=={fine}")),
        0
    )));
}

#[test]
fn a_non_str_input_is_rejected_unchanged() {
    let input = PyValue::List(vec![PyValue::str("int"), PyValue::str("1")]);
    let record = answer_for("n", &input, 17);
    assert!(!is_accepted(&record));
    assert_eq!(
        shown(field(&record, "input_before")),
        shown(field(&record, "input_after"))
    );
    assert_eq!(shown(field(&record, "input_before")), shown(&input));
}

#[test]
fn protocol_problems_are_refusals_and_never_a_rejection() {
    let good = shown(&str_dto("torch"));
    let cases = [
        ("not json".to_string(), "malformed_request"),
        ("[1]".to_string(), "malformed_request"),
        (r#"{"operation": "x"}"#.to_string(), "malformed_request"),
        (
            format!(r#"{{"id": 1, "operation": "other", "input_dto": {good}, "hash_seed": 0}}"#),
            "unsupported_operation",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": 1}}"#
            ),
            "unsupported_hash_seed",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": "0"}}"#
            ),
            "unsupported_hash_seed",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": true}}"#
            ),
            "unsupported_hash_seed",
        ),
        (
            format!(r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": {good}}}"#),
            "malformed_request",
        ),
        (
            format!(r#"{{"id": 1, "operation": "{OPERATION}", "hash_seed": 0}}"#),
            "malformed_request",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": ["str"], "hash_seed": 0}}"#
            ),
            "malformed_input_dto",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "{OPERATION}", "input_dto": {good}, "hash_seed": 0, "extra": 1}}"#
            ),
            "malformed_request",
        ),
    ];
    for (line, kind) in cases {
        let record = answer(&PyStr::new(&line));
        assert_eq!(refusal_kind(&record), kind, "{line}");
        assert!(dict(&record).get("projection").is_none(), "{line}");
    }
}

#[test]
fn the_jsonl_program_answers_every_non_blank_line_in_order() {
    let lines = format!(
        "{}\n\n{}\n",
        request_line("a", &str_dto("torch==2"), 0)
            .to_utf8()
            .unwrap(),
        request_line("b", &str_dto("torch=="), 17)
            .to_utf8()
            .unwrap()
    );
    let out = evaluate_jsonl(lines.as_bytes()).expect("evaluated");
    let rows: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0].starts_with(r#"{"id": "a""#) || rows[0].starts_with(r#"{"id":"a""#),
        "{}",
        rows[0]
    );
    assert!(rows[1].contains(r#""b""#));
}

#[test]
fn a_two_part_set_is_read_anywhere_in_the_input_and_written_back_unchanged() {
    let set = r#"["set", [["int", "1"], ["str", "a"]]]"#;
    let input = loads(&format!(r#"["list", [{set}, ["tuple", [{set}]]]]"#)).unwrap();
    let record = answer_for("sets", &input, 0);
    assert!(!is_accepted(&record));
    assert_eq!(shown(field(&record, "input_before")), shown(&input));
    assert_eq!(shown(field(&record, "input_after")), shown(&input));
}

#[test]
fn a_very_long_input_is_bounded_by_its_length_not_by_the_callers_stack() {
    let release = "1.".repeat(60_000);
    let record = answer_for("long-release", &str_dto(&format!("torch=={release}1")), 0);
    assert!(is_accepted(&record));
    let chain = format!(
        "torch; os_name == \"x\"{}",
        " or os_name == \"x\"".repeat(20_000)
    );
    assert!(is_accepted(&answer_for(
        "long-marker",
        &str_dto(&chain),
        17
    )));
    let specifiers = (0..3_000)
        .map(|n| format!("!={n}"))
        .collect::<Vec<_>>()
        .join(",");
    let record = answer_for(
        "many-specifiers",
        &str_dto(&format!("torch{specifiers}")),
        17,
    );
    assert!(is_accepted(&record));
}
