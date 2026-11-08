//! Checks of the default `tomllib.loads(text)` component (R35 `tomllib-default-loads-cpython3123`): every one of the 68 saved original results
//! through the library and through the JSON-lines request path, then behaviours the saved corpus does not name but the pinned CPython 3.12.3 sources
//! decide (each expectation below is derived from reading `tomllib/_parser.py`, `_re.py` and `Objects/longobject.c`; no original was run), the
//! protocol refusals and the named unsupported domain.
//!
//! The saved results are read through the environment variable `R35_TOML_REFERENCE` (the directory that holds `cases.json` and
//! `seed0-toml-<NNN>/owner-record.json`); the production code contains no path, label or expected value. These checks start no Python.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::path::PathBuf;
use upstream_runtime::api::render;
use upstream_runtime::tomllib::projection::{observe_loads, SCOPE};
use upstream_runtime::tomllib::service::{answer, evaluate_jsonl, OPERATION};
use upstream_runtime::tomllib::{
    loads as toml_loads, ExceptionClass, LoadsFailure, TomlValue, UnsupportedDomain,
};

fn reference() -> PathBuf {
    PathBuf::from(std::env::var("R35_TOML_REFERENCE").expect(
        "R35_TOML_REFERENCE must name the saved reference directory (cases.json, seed0-toml-001 ...)",
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

fn field<'a>(record: &'a PyValue, name: &str) -> &'a PyValue {
    dict(record)
        .get(name)
        .unwrap_or_else(|| panic!("record has no {name}: {}", shown(record)))
}

fn str_dto(text: &str) -> PyValue {
    PyValue::List(vec![PyValue::str("str"), PyValue::str(text)])
}

fn request_line(id: &str, input_dto: &PyValue) -> PyStr {
    PyStr::new(&format!(
        r#"{{"id": {}, "operation": "{OPERATION}", "input_dto": {}}}"#,
        shown(&PyValue::str(id)),
        shown(input_dto)
    ))
}

fn answer_for(id: &str, input_dto: &PyValue) -> PyValue {
    answer(&request_line(id, input_dto))
}

/// The projection of the record the service answers for `source`.
fn projection_of(source: &str) -> PyValue {
    let record = answer_for("probe", &str_dto(source));
    field(&record, "projection").clone()
}

/// The typed value of a successful parse, rendered; the test fails when the parse raised.
fn value_of(source: &str) -> String {
    let projection = projection_of(source);
    assert_eq!(
        text_of(field(&projection, "kind")),
        "value",
        "{source:?}: {}",
        shown(&projection)
    );
    shown(field(&projection, "value"))
}

/// `expected` is the typed value in compact JSON; spacing does not matter.
fn expect_value(source: &str, expected: &str) {
    let expected = shown(&loads(expected).expect("expected JSON"));
    assert_eq!(value_of(source), expected, "{source:?}");
}

/// The rows `(class, message, suppress_context, relation)` of the exception the parse raised.
fn chain_of(source: &str) -> Vec<(String, String, bool, Option<String>)> {
    let projection = projection_of(source);
    assert_eq!(
        text_of(field(&projection, "kind")),
        "exception",
        "{source:?}: {}",
        shown(&projection)
    );
    list(field(&projection, "exception_chain"))
        .iter()
        .map(|row| {
            let relation = match field(row, "relation") {
                PyValue::None => None,
                other => Some(text_of(other)),
            };
            let PyValue::Bool(suppress) = field(row, "suppress_context") else {
                panic!("suppress_context is a bool")
            };
            (
                text_of(field(row, "type")),
                text_of(field(row, "message")),
                *suppress,
                relation,
            )
        })
        .collect()
}

fn expect_error(source: &str, class: &str, message: &str) {
    let rows = chain_of(source);
    assert_eq!(rows[0].0, class, "{source:?}");
    assert_eq!(rows[0].1, message, "{source:?}");
}

fn all_cases() -> Vec<PyValue> {
    list(&read(reference().join("cases.json"))).to_vec()
}

fn saved(id: &str) -> PyValue {
    let number = id.strip_prefix("toml-").expect("case id");
    read(reference().join(format!("seed0-toml-{number}/owner-record.json")))
}

const RECORD_MEMBERS: [&str; 7] = [
    "id",
    "scope",
    "projection",
    "input_before",
    "input_after",
    "stdout",
    "stderr",
];

#[test]
fn all_68_saved_results_match_through_the_json_lines_request_path() {
    let cases = all_cases();
    assert_eq!(cases.len(), 68);
    let (mut values, mut exceptions) = (0, 0);
    for case in &cases {
        let id = text_of(field(case, "id"));
        let record = answer_for(&id, field(case, "input_dto"));
        let original = saved(&id);
        let names: Vec<String> = dict(&record)
            .iter()
            .map(|(k, _)| k.to_utf8().unwrap())
            .collect();
        assert_eq!(
            names, RECORD_MEMBERS,
            "{id}: the business record has exactly seven members"
        );
        assert_eq!(text_of(field(&record, "scope")), SCOPE);
        for name in [
            "id",
            "projection",
            "input_before",
            "input_after",
            "stdout",
            "stderr",
        ] {
            assert_eq!(
                shown(field(&record, name)),
                shown(field(&original, name)),
                "{id}: {name}"
            );
        }
        match text_of(field(field(&record, "projection"), "kind")).as_str() {
            "value" => values += 1,
            _ => exceptions += 1,
        }
    }
    assert_eq!((values, exceptions), (41, 27));
}

#[test]
fn all_68_saved_results_match_through_the_library_call() {
    for case in all_cases() {
        let id = text_of(field(&case, "id"));
        let PyValue::List(parts) = field(&case, "input_dto") else {
            panic!("dto")
        };
        let PyValue::Str(source) = &parts[1] else {
            panic!("text")
        };
        // The library call itself, not the request path.
        let projection = observe_loads(source).expect("modelled");
        assert_eq!(
            shown(&projection),
            shown(field(&saved(&id), "projection")),
            "{id}"
        );
        // the typed value the library returns is the one the projection shows
        let expected_kind = text_of(field(field(&saved(&id), "projection"), "kind"));
        match toml_loads(source.code_points()) {
            Ok(_) => assert_eq!(expected_kind, "value", "{id}"),
            Err(LoadsFailure::Raised(raised)) => {
                assert_eq!(expected_kind, "exception", "{id}");
                assert_eq!(
                    raised.message,
                    text_of(field(field(&saved(&id), "projection"), "message")),
                    "{id}"
                );
            }
            Err(LoadsFailure::Unsupported(domain)) => panic!("{id}: {domain:?}"),
        }
    }
}

#[test]
fn the_json_lines_program_function_answers_every_case_in_order() {
    let cases = all_cases();
    let mut input = String::new();
    for case in &cases {
        input.push_str(
            &request_line(&text_of(field(case, "id")), field(case, "input_dto"))
                .to_utf8()
                .unwrap(),
        );
        input.push_str("\n\n");
    }
    let out = evaluate_jsonl(input.as_bytes()).expect("rendered");
    let lines: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(
        lines.len(),
        68,
        "one record per request, blank lines skipped"
    );
    for (line, case) in lines.iter().zip(&cases) {
        let record = loads(line).expect("record");
        let id = text_of(field(case, "id"));
        assert_eq!(text_of(field(&record, "id")), id);
        assert_eq!(
            shown(field(&record, "projection")),
            shown(field(&saved(&id), "projection")),
            "{id}"
        );
    }
}

// ---- behaviours decided by the pinned sources ---------------------------------------------------------------------------------------------

#[test]
fn hexadecimal_octal_and_binary_integers_have_no_decimal_digit_limit() {
    let source = format!("a=0x{}\n", "f".repeat(4005));
    let projection = projection_of(&source);
    let text = shown(&projection);
    assert!(text.contains(&format!("0x{}", "f".repeat(4005))), "bighex");
    expect_value(
        "a=0xf_f\nb=0o7_7\nc=0b1_0\nd=-0\ne=+0\n",
        r#"["dict",[[["str","a"],["int_hex","0xff"]],[["str","b"],["int_hex","0x3f"]],[["str","c"],["int_hex","0x2"]],[["str","d"],["int_hex","0x0"]],[["str","e"],["int_hex","0x0"]]]]"#,
    );
}

#[test]
fn the_decimal_digit_limit_counts_digits_not_underscores_and_not_the_sign() {
    let at_limit = format!("a=-{}\n", "9".repeat(4300));
    assert!(value_of(&at_limit).contains("-0x"));
    let over_limit = format!("a={}\n", "9".repeat(4301));
    let rows = chain_of(&over_limit);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "ValueError");
    assert_eq!(
        rows[0].1,
        "Exceeds the limit (4300 digits) for integer string conversion: value has 4301 digits; use sys.set_int_max_str_digits() to increase the limit"
    );
    assert!(!rows[0].2 && rows[0].3.is_none());
    let projection = projection_of(&over_limit);
    assert_eq!(text_of(field(&projection, "type")), "ValueError");
    assert_eq!(text_of(field(&projection, "module")), "builtins");
    // 4300 digits written with underscores between them are still 4300 digits
    let underscored = format!("a={}9\n", "9_".repeat(4299));
    assert!(value_of(&underscored).contains("0x"));
    let underscored_over = format!("a={}9\n", "9_".repeat(4300));
    assert_eq!(chain_of(&underscored_over)[0].0, "ValueError");
    // the limit is raised inside an array too, and nothing after it is read
    assert_eq!(
        chain_of(&format!("a=[{}]\nb=\n", "9".repeat(4301)))[0].0,
        "ValueError"
    );
}

#[test]
fn floats_keep_their_exact_bits_including_signed_zero_and_signed_nan() {
    expect_value(
        "a=-0.0\nb=+0.0\nc=nan\nd=+nan\ne=-nan\nf=inf\ng=-inf\n",
        r#"["dict",[[["str","a"],["float","-0.0","8000000000000000"]],[["str","b"],["float","0.0","0000000000000000"]],[["str","c"],["float","nan","7ff8000000000000"]],[["str","d"],["float","nan","7ff8000000000000"]],[["str","e"],["float","nan","fff8000000000000"]],[["str","f"],["float","inf","7ff0000000000000"]],[["str","g"],["float","-inf","fff0000000000000"]]]]"#,
    );
    expect_value(
        "a=1_0.5e1_0\nb=1e400\nc=1e-400\nd=-1e-400\ne=1E+2\nf=5e99999999999999999999\ng=0e99999999999999999999\nh=1.5\n",
        r#"["dict",[[["str","a"],["float","105000000000.0","4238727cda000000"]],[["str","b"],["float","inf","7ff0000000000000"]],[["str","c"],["float","0.0","0000000000000000"]],[["str","d"],["float","-0.0","8000000000000000"]],[["str","e"],["float","100.0","4059000000000000"]],[["str","f"],["float","inf","7ff0000000000000"]],[["str","g"],["float","0.0","0000000000000000"]],[["str","h"],["float","1.5","3ff8000000000000"]]]]"#,
    );
}

#[test]
fn a_float_with_a_dangling_marker_stops_before_it() {
    // `1.` and `1e` are not float parts: the integer ends first and the rest is not a statement end
    expect_error(
        "a=1.\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 4)",
    );
    expect_error(
        "a=1e\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 4)",
    );
    expect_error(
        "a=0x\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 4)",
    );
    expect_error(
        "a=1_\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 4)",
    );
    expect_error(
        "a=+inf1\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 7)",
    );
}

#[test]
fn temporal_values_keep_their_kind_offset_and_truncated_fraction() {
    expect_value(
        "a=2024-02-29\nb=0001-01-01\nc=07:32:00.1\nd=07:32:00.0000009\ne=07:32:00.000001999\nf=1979-05-27T07:32:00-00:30\ng=1979-05-27t07:32:00-00:00\nh=1979-05-27 07:32:00Z\ni=1979-05-27T07:32:00+23:59\n",
        r#"["dict",[[["str","a"],["date","2024-02-29"]],[["str","b"],["date","0001-01-01"]],[["str","c"],["time","07:32:00.100000"]],[["str","d"],["time","07:32:00"]],[["str","e"],["time","07:32:00.000001"]],[["str","f"],["datetime","1979-05-27T07:32:00-00:30"]],[["str","g"],["datetime","1979-05-27T07:32:00+00:00"]],[["str","h"],["datetime","1979-05-27T07:32:00+00:00"]],[["str","i"],["datetime","1979-05-27T07:32:00+23:59"]]]]"#,
    );
    // 2000 is a leap year, 1900 and 2023 are not, year 0000 does not exist
    expect_value(
        "d=2000-02-29\n",
        r#"["dict",[[["str","d"],["date","2000-02-29"]]]]"#,
    );
    for (source, cause) in [
        ("d=1900-02-29\n", "day is out of range for month"),
        ("d=2023-04-31\n", "day is out of range for month"),
        ("d=0000-01-01\n", "year 0 is out of range"),
        ("d=0000-01-01T01:02:03\n", "year 0 is out of range"),
    ] {
        let rows = chain_of(source);
        assert_eq!(rows.len(), 2, "{source:?}");
        assert_eq!(
            rows[0],
            (
                "TOMLDecodeError".to_string(),
                "Invalid date or datetime (at line 1, column 3)".to_string(),
                true,
                Some("cause".to_string())
            ),
            "{source:?}"
        );
        assert_eq!(
            rows[1],
            ("ValueError".to_string(), cause.to_string(), false, None),
            "{source:?}"
        );
    }
    // a date whose time part is not a time is a date followed by garbage
    expect_error(
        "d=1979-05-27T25:00:00\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 13)",
    );
    expect_error(
        "d=1979-13-27\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 7)",
    );
}

#[test]
fn error_columns_count_characters_not_bytes_and_follow_crlf_normalisation() {
    expect_error(
        "\"\u{d55c}\u{ae00}\"=1 ?\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 8)",
    );
    expect_error(
        "\"\u{1F600}\"=1 ?\n",
        "TOMLDecodeError",
        "Expected newline or end of document after a statement (at line 1, column 7)",
    );
    expect_error(
        "a=1\r\nb=\r\n",
        "TOMLDecodeError",
        "Invalid value (at line 2, column 3)",
    );
    expect_error(
        "a=1\n\u{ac00}\u{ac00}=1 ?\n",
        "TOMLDecodeError",
        "Invalid statement (at line 2, column 1)",
    );
    expect_error(
        "a=[1,\n2 3]\n",
        "TOMLDecodeError",
        "Unclosed array (at line 2, column 3)",
    );
    expect_error(
        "a=1\nb\n",
        "TOMLDecodeError",
        "Expected '=' after a key in a key/value pair (at line 2, column 2)",
    );
    expect_error(
        "a=1\nb",
        "TOMLDecodeError",
        "Expected '=' after a key in a key/value pair (at end of document)",
    );
}

#[test]
fn key_tuples_in_messages_use_the_interpreters_repr() {
    expect_error(
        "[\"it's\"]\n[\"it's\"]\n",
        "TOMLDecodeError",
        "Cannot declare (\"it's\",) twice (at line 2, column 8)",
    );
    expect_error(
        "[\"\u{d55c}\u{ae00}\".x]\n[\"\u{d55c}\u{ae00}\".x]\n",
        "TOMLDecodeError",
        "Cannot declare ('\u{d55c}\u{ae00}', 'x') twice (at line 2, column 8)",
    );
    expect_error(
        "[\"a\\u200bb\"]\n[\"a\\u200bb\"]\n",
        "TOMLDecodeError",
        "Cannot declare ('a\\u200bb',) twice (at line 2, column 12)",
    );
}

#[test]
fn strings_follow_the_escape_newline_and_quote_rules() {
    expect_value(
        "a=\"\"\"\r\nx\r\ny\"\"\"\nb=\"\"\"q\"\"\"\"\"\nc='''r'''''\nd=\"\"\"\\\n\n   z\"\"\"\ne=\"\"\"a \\   \n  b\"\"\"\nf=\"\\u0000\\U0001F600\"\ng=\"\"\"\"\"\"\nh=''\n",
        r#"["dict",[[["str","a"],["str","x\ny"]],[["str","b"],["str","q\"\""]],[["str","c"],["str","r''"]],[["str","d"],["str","z"]],[["str","e"],["str","a b"]],[["str","f"],["str","\u0000😀"]],[["str","g"],["str",""]],[["str","h"],["str",""]]]]"#,
    );
    expect_error(
        "s=\"\"\"a\\ b\"\"\"\n",
        "TOMLDecodeError",
        "Unescaped '\\' in a string (at line 1, column 9)",
    );
    expect_error(
        "s=\"a\\u12\"\n",
        "TOMLDecodeError",
        "Invalid hex value (at line 1, column 7)",
    );
    expect_error(
        "s=\"a\\",
        "TOMLDecodeError",
        "Unescaped '\\' in a string (at end of document)",
    );
    expect_error(
        "s='abc",
        "TOMLDecodeError",
        "Expected \"'\" (at end of document)",
    );
    expect_error(
        "s='''abc",
        "TOMLDecodeError",
        "Expected \"'''\" (at end of document)",
    );
    expect_error(
        "s=\"abc",
        "TOMLDecodeError",
        "Unterminated string (at end of document)",
    );
    expect_error(
        "s='a\nb'\n",
        "TOMLDecodeError",
        "Found invalid character '\\n' (at line 1, column 5)",
    );
    expect_error(
        "s=\"a\u{7f}\"\n",
        "TOMLDecodeError",
        "Illegal character '\\x7f' (at line 1, column 5)",
    );
    expect_error(
        "s=\"\"\"a\rb\"\"\"\n",
        "TOMLDecodeError",
        "Illegal character '\\r' (at line 1, column 7)",
    );
}

#[test]
fn from_none_raises_hide_their_context_and_plain_raises_have_none() {
    let rows = chain_of("s='abc");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].2 && rows[0].3.is_none(), "raise ... from None");
    let rows = chain_of("a=1\na=2\n");
    assert!(!rows[0].2 && rows[0].3.is_none(), "a plain raise");
    let rows = chain_of("a=1\n[a]\n");
    assert!(rows[0].2, "Cannot overwrite a value from None");
    assert_eq!(rows[0].1, "Cannot overwrite a value (at line 2, column 3)");
}

#[test]
fn comments_reject_control_characters_and_bare_carriage_returns() {
    expect_value(
        "# fine \u{d55c}\u{ae00}\t\na=1 # trailing\n",
        r#"["dict",[[["str","a"],["int_hex","0x1"]]]]"#,
    );
    expect_error(
        "a=1 # bad\u{1}\n",
        "TOMLDecodeError",
        "Found invalid character '\\x01' (at line 1, column 10)",
    );
    expect_error(
        "# a\rb\n",
        "TOMLDecodeError",
        "Found invalid character '\\r' (at line 1, column 4)",
    );
    expect_error(
        "a=1 # bad\u{7f}",
        "TOMLDecodeError",
        "Found invalid character '\\x7f' (at line 1, column 10)",
    );
}

#[test]
fn tables_arrays_and_inline_tables_follow_the_flag_rules() {
    expect_value(
        "z=1\nb.y=2\nb.x=3\n[t]\n[[arr]]\nk=1\n[arr.sub]\nj=2\n[[arr]]\nk=3\n[[arr]]\n[t.u]\n",
        r#"["dict",[[["str","z"],["int_hex","0x1"]],[["str","b"],["dict",[[["str","y"],["int_hex","0x2"]],[["str","x"],["int_hex","0x3"]]]]],[["str","t"],["dict",[[["str","u"],["dict",[]]]]]],[["str","arr"],["list",[["dict",[[["str","k"],["int_hex","0x1"]],[["str","sub"],["dict",[[["str","j"],["int_hex","0x2"]]]]]]],["dict",[[["str","k"],["int_hex","0x3"]]]],["dict",[]]]]]]]"#,
    );
    // `[[a]]` frees the namespaces of the previous item, so the same sub-table can be declared again
    expect_value(
        "[[a]]\n[a.b]\n[[a]]\n[a.b]\n",
        r#"["dict",[[["str","a"],["list",[["dict",[[["str","b"],["dict",[]]]]],["dict",[[["str","b"],["dict",[]]]]]]]]]]"#,
    );
    expect_value(
        "a . b = 1\n\"c.d\".e = 2\n",
        r#"["dict",[[["str","a"],["dict",[[["str","b"],["int_hex","0x1"]]]]],[["str","c.d"],["dict",[[["str","e"],["int_hex","0x2"]]]]]]]"#,
    );
    expect_value(
        "a={x.y=1,x.z=2}\n",
        r#"["dict",[[["str","a"],["dict",[[["str","x"],["dict",[[["str","y"],["int_hex","0x1"]],[["str","z"],["int_hex","0x2"]]]]]]]]]]"#,
    );
    expect_error(
        "a={x=1,x=2}\n",
        "TOMLDecodeError",
        "Duplicate inline table key 'x' (at line 1, column 11)",
    );
    expect_error(
        "a={x={y=1},x.z=2}\n",
        "TOMLDecodeError",
        "Cannot mutate immutable namespace ('x', 'z') (at line 1, column 17)",
    );
    expect_error(
        "a={x=1,x.y=2}\n",
        "TOMLDecodeError",
        "Cannot overwrite a value (at line 1, column 13)",
    );
    expect_error(
        "a.b=1\n[a]\n",
        "TOMLDecodeError",
        "Cannot declare ('a',) twice (at line 2, column 3)",
    );
    expect_error(
        "[a]\nb.c=1\nb.c.d=2\n",
        "TOMLDecodeError",
        "Cannot overwrite a value (at line 3, column 8)",
    );
    expect_error(
        "[a.b]\n[a]\n[a]\n",
        "TOMLDecodeError",
        "Cannot declare ('a',) twice (at line 3, column 3)",
    );
    expect_error(
        "[a\n",
        "TOMLDecodeError",
        "Expected ']' at the end of a table declaration (at line 1, column 3)",
    );
    expect_error(
        "[[a]\n",
        "TOMLDecodeError",
        "Expected ']]' at the end of an array declaration (at line 1, column 4)",
    );
    expect_error(
        "a=[1]\n[[a]]\n",
        "TOMLDecodeError",
        "Cannot mutate immutable namespace ('a',) (at line 2, column 4)",
    );
    expect_error(
        "a=[1]\n[a.b]\n",
        "TOMLDecodeError",
        "Cannot declare ('a', 'b') twice (at line 2, column 5)",
    );
    expect_error(
        "a=1\n[[a]]\n",
        "TOMLDecodeError",
        "Cannot overwrite a value (at line 2, column 4)",
    );
}

#[test]
fn tables_keep_insertion_order_and_the_library_exposes_typed_values() {
    let source = PyStr::new("z=1\na=\"x\"\nm=[true,2.5]\n");
    let table = toml_loads(source.code_points()).expect("parses");
    let keys: Vec<String> = table
        .iter()
        .map(|(key, _)| key.iter().filter_map(|&c| char::from_u32(c)).collect())
        .collect();
    assert_eq!(keys, ["z", "a", "m"]);
    assert!(
        matches!(table.get(&[u32::from(b'm')]), Some(TomlValue::Array(items)) if items.len() == 2)
    );
}

#[test]
fn a_lone_surrogate_in_the_text_is_an_ordinary_character() {
    let dto = loads(r#"["str", "s=\"a\ud800b\"\n"]"#).expect("dto");
    let record = answer_for("surrogate", &dto);
    assert!(shown(field(&record, "projection")).contains("a\\ud800b"));
    assert!(shown(field(&record, "input_before")).contains("\\ud800"));
}

#[test]
fn nesting_up_to_the_modelled_depth_parses_and_beyond_it_is_a_named_refusal() {
    let nest = |depth: usize| format!("a={}{}\n", "[".repeat(depth), "]".repeat(depth));
    assert!(value_of(&nest(100)).contains("list"));
    let inline = |depth: usize| format!("a={}1{}\n", "{x=".repeat(depth), "}".repeat(depth));
    assert!(value_of(&inline(60)).contains("dict"));
    for source in [nest(600), inline(400)] {
        let source = PyStr::new(&source);
        match toml_loads(source.code_points()) {
            Err(LoadsFailure::Unsupported(domain)) => {
                assert_eq!(domain, UnsupportedDomain::NestingBeyondRecursionLimit)
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(observe_loads(&source).is_err());
        let record = answer_for("deep", &str_dto(&source.to_utf8().unwrap()));
        assert!(dict(&record).get("projection").is_none());
        assert_eq!(
            text_of(field(field(&record, "refusal"), "kind")),
            "nesting_beyond_recursion_limit"
        );
    }
    // an error before the deep part is still the parser's own error, as in the original's order of evaluation
    let late = format!("b=\n{}", nest(600));
    expect_error(
        &late,
        "TOMLDecodeError",
        "Invalid value (at line 1, column 3)",
    );
}

#[test]
fn the_exception_classes_report_their_modules_and_single_argument() {
    let projection = projection_of("a=\n");
    assert_eq!(text_of(field(&projection, "type")), "TOMLDecodeError");
    assert_eq!(text_of(field(&projection, "module")), "tomllib");
    assert_eq!(
        shown(field(&projection, "args")),
        shown(&loads(r#"["tuple", [["str", "Invalid value (at line 1, column 3)"]]]"#).unwrap())
    );
    assert_eq!(ExceptionClass::ValueError.module(), "builtins");
}

// ---- protocol ---------------------------------------------------------------------------------------------------------------------------

fn kind_of(record: &PyValue) -> String {
    match dict(record).get("refusal") {
        Some(refusal) => text_of(field(refusal, "kind")),
        None => "business".to_string(),
    }
}

#[test]
fn protocol_problems_are_distinct_refusals_without_a_projection() {
    let good = r#"["str", "a=1\n"]"#;
    let cases = [
        ("this is not json".to_string(), "malformed_request"),
        ("[1]".to_string(), "malformed_request"),
        (
            r#"{"operation": "tomllib.loads.default", "input_dto": ["str", ""]}"#.to_string(),
            "malformed_request",
        ),
        (
            format!(
                r#"{{"id": 1, "operation": "tomllib.loads.default", "input_dto": {good}, "hash_seed": 0}}"#
            ),
            "malformed_request",
        ),
        (
            r#"{"id": 1, "operation": "tomllib.loads.default"}"#.to_string(),
            "malformed_request",
        ),
        (
            format!(r#"{{"id": 1, "operation": 5, "input_dto": {good}}}"#),
            "malformed_request",
        ),
        (
            format!(r#"{{"id": 1, "operation": "other", "input_dto": {good}}}"#),
            "unsupported_operation",
        ),
        (
            r#"{"id": 1, "operation": "tomllib.loads.default", "input_dto": ["nonsense"]}"#
                .to_string(),
            "malformed_input_dto",
        ),
        (
            r#"{"id": 1, "operation": "tomllib.loads.default", "input_dto": ["int", "5"]}"#
                .to_string(),
            "non_text_input",
        ),
        (
            r#"{"id": 1, "operation": "tomllib.loads.default", "input_dto": ["bytes", "6131"]}"#
                .to_string(),
            "non_text_input",
        ),
        (
            r#"{"id": 1, "operation": "tomllib.loads.default", "input_dto": ["none"]}"#.to_string(),
            "non_text_input",
        ),
        (
            format!(r#"{{"id": 1, "operation": "tomllib.loads.default", "input_dto": {good}}}"#),
            "business",
        ),
    ];
    for (line, expected) in cases {
        let record = answer(&PyStr::new(&line));
        assert_eq!(kind_of(&record), expected, "{line}");
        if expected != "business" {
            assert!(dict(&record).get("projection").is_none(), "{line}");
            assert_eq!(text_of(field(&record, "scope")), SCOPE);
        }
    }
}
