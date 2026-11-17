//! Behaviour of the two model scripts that the saved original results do not pin down by themselves, checked against what the
//! original source says and what Python's own `re`, `json`, `http.client` and number formatting do for the same input (the
//! expected values below were obtained from those language functions, not from the original scripts): the Unicode behaviour
//! of the case-insensitive regular expressions, the HTTP failure texts, the encodings `json.loads(bytes)` detects, repeated JSON
//! keys at every nesting level, floor division across number types, Python's argument-binding order, and the write-in-place of
//! warnings lists supplied as arguments.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_api::answer;
use recipe_runtime::model_files::PrivateDirectory;
use std::path::PathBuf;

use super::support::{member, Fixture};

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn int(value: i64) -> PyValue {
    PyValue::int(value)
}

fn list(items: Vec<PyValue>) -> PyValue {
    PyValue::List(items)
}

fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

/// One request through the JSON-lines service, answered as a record.
fn ask(
    operation: &str,
    args: Vec<PyValue>,
    kwargs: Vec<(&str, PyValue)>,
    base: Option<&str>,
) -> PyValue {
    let mut request = vec![
        ("id", text("t")),
        ("operation", text(operation)),
        ("args", list(args)),
        ("kwargs", object(kwargs)),
    ];
    if let Some(base) = base {
        request.push(("context", object(vec![("hf_api_base", text(base))])));
    }
    let line = object(request).dumps_unicode().expect("a request line");
    answer(&line)
}

fn value_of(record: &PyValue) -> PyValue {
    assert_eq!(member(record, "kind"), &text("value"), "{}", record.repr());
    member(record, "value").clone()
}

fn exception_of(record: &PyValue) -> (String, String) {
    assert_eq!(
        member(record, "kind"),
        &text("exception"),
        "{}",
        record.repr()
    );
    let show = |key: &str| match member(record, key) {
        PyValue::Str(own) => own.to_utf8().expect("text"),
        other => panic!("{}", other.repr()),
    };
    (show("type"), show("message"))
}

/// A scratch directory for one check: created exclusively (a directory that was already there is never reused or removed) and
/// removed at the end by its owner.
struct Dir {
    _directory: PrivateDirectory,
    root: PathBuf,
}

impl Dir {
    fn new(label: &str) -> Dir {
        let directory = PrivateDirectory::create(&format!("model-cases-{label}-"))
            .expect("a private scratch directory");
        let root = directory.path().to_path_buf();
        Dir {
            _directory: directory,
            root,
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> &Self {
        std::fs::write(self.root.join(name), bytes).expect("write a fixture file");
        self
    }

    fn path(&self) -> PyValue {
        text(self.root.to_str().expect("a text path"))
    }

    fn file(&self, name: &str) -> PyValue {
        text(self.root.join(name).to_str().expect("a text path"))
    }
}

fn texts(items: &[&str]) -> PyValue {
    list(items.iter().map(|item| text(item)).collect())
}

// ── regular expressions with Unicode and IGNORECASE ───────────────────────────────────────────

#[test]
fn card_precision_follows_python_re_ignorecase_for_unicode_letters() {
    // (card, keywords sorted, mixed note). `re.I` equates dotless i and dotted capital I with `i`, a Kelvin sign is not a `K` of
    // any keyword here, and `\s` is any Unicode space (also across a line break inside `FP4 +\nFP8`).
    let cases: [(&str, &[&str], Option<&str>); 9] = [
        (
            "FP8 M\u{131}xed fp8 mixed",
            &["FP8 M\u{131}xed", "fp8 mixed"],
            None,
        ),
        ("FP8 M\u{130}xed", &["FP8 M\u{130}xed"], None),
        (
            "INT\u{212a}8 int8 \u{130}NT4",
            &["int8", "\u{130}NT4"],
            None,
        ),
        (
            "gptq AWQ awq W4A16 w8a8",
            &["AWQ", "W4A16", "awq", "gptq", "w8a8"],
            None,
        ),
        ("MXFP4/nvfp4 and FP8", &["FP8", "MXFP4", "nvfp4"], None),
        (
            "FP4\u{2003}+\u{a0}FP8 note experts FP4 more\nsecond FP4 + FP8 expert fp4 line",
            &["FP4", "FP4 + FP8", "FP4\u{2003}+\u{a0}FP8", "fp4"],
            Some("FP4\u{2003}+\u{a0}FP8 note experts FP4 more"),
        ),
        (
            "x FP4 +\nFP8 has experts use fp4 here\nnext",
            &["FP4 +\nFP8", "fp4"],
            Some("FP4 +\nFP8 has experts use fp4 here"),
        ),
        (
            "FP4 + FP8 experts only here\nFP4 later",
            &["FP4", "FP4 + FP8"],
            None,
        ),
        (
            "k\u{131} FP4+FP8 EXPERT \u{130} fp4",
            &["FP4+FP8", "fp4"],
            Some("FP4+FP8 EXPERT \u{130} fp4"),
        ),
    ];
    for (card, keywords, note) in cases {
        let found = value_of(&ask(
            "crosscheck_model_card.card_precision",
            vec![text(card), text("model")],
            vec![],
            None,
        ));
        assert_eq!(member(&found, "keywords"), &texts(keywords), "{card:?}");
        match note {
            Some(note) => assert_eq!(member(&found, "mixed_note"), &text(note), "{card:?}"),
            None => assert!(
                matches!(&found, PyValue::Dict(map) if map.get("mixed_note").is_none()),
                "{card:?}"
            ),
        }
    }
    // The note is cut at 200 characters.
    let long = format!("FP4 + FP8 expert {} FP4", "x".repeat(300));
    let found = value_of(&ask(
        "crosscheck_model_card.card_precision",
        vec![text(&long), text("m")],
        vec![],
        None,
    ));
    match member(&found, "mixed_note") {
        PyValue::Str(note) => assert_eq!(note.len(), 200),
        other => panic!("{}", other.repr()),
    }
}

/// `crosscheck` of a directory holding only a `config.json` and the given README; returns the `checks` list.
fn checks_for_readme(readme: &str) -> Vec<PyValue> {
    let dir = Dir::new("readme");
    dir.write("config.json", b"{}")
        .write("README.md", readme.as_bytes());
    let report = value_of(&ask(
        "crosscheck_model_card.crosscheck",
        vec![dir.path()],
        vec![],
        None,
    ));
    match member(&report, "checks") {
        PyValue::List(items) => items.clone(),
        other => panic!("{}", other.repr()),
    }
}

#[test]
fn special_dependencies_in_a_card_use_word_boundaries_and_case_folding() {
    let flagged = |readme: &str| -> Vec<String> {
        let checks = checks_for_readme(readme);
        match member(&checks[1], "flagged") {
            PyValue::List(entries) => entries
                .iter()
                .map(|entry| match member(entry, "pkg") {
                    PyValue::Str(pkg) => pkg.to_utf8().expect("text"),
                    other => panic!("{}", other.repr()),
                })
                .collect(),
            other => panic!("{}", other.repr()),
        }
    };
    assert_eq!(
        flagged("SGL_\u{212a}ERNEL and sgl-kernel"),
        ["sgl_kernel", "sgl-kernel"]
    );
    assert_eq!(
        flagged("apexes TRITON-based; xformers2 XFORMERS"),
        ["triton", "xformers"]
    );
    assert_eq!(
        flagged("FLASH_ATTN, flashinfer. DeepGEMM"),
        ["deepgemm", "flash_attn", "flashinfer"]
    );
    assert_eq!(
        flagged("Tr\u{130}ton \u{17f}gl_kernel tilelang_ causal_conv1d"),
        ["sgl_kernel", "causal_conv1d", "triton"]
    );
    assert_eq!(flagged("mamba_ssm\u{17f} MAMBA_SSM"), ["mamba_ssm"]);
}

#[test]
fn the_context_and_reasoning_mentions_of_a_card() {
    let context = |readme: &str| member(&checks_for_readme(readme)[2], "card").clone();
    let reasoning = |readme: &str| member(&checks_for_readme(readme)[4], "is_reasoning").clone();
    for (readme, wanted) in [
        ("1 Million tokens", true),
        ("11M", false),
        ("1,048576 ctx", true),
        ("one MILLION", true),
        ("1\u{a0}M", true),
        ("1 Mi", false),
        ("0 1M", true),
    ] {
        let expected = if wanted { text("1M") } else { PyValue::None };
        assert_eq!(context(readme), expected, "{readme:?}");
    }
    for (readme, wanted) in [
        ("Reasoning EFFORT: high", true),
        ("thinking budget", true),
        ("<THINK>", true),
        ("reasoning  mode", false),
        ("reason mode", false),
    ] {
        assert_eq!(reasoning(readme), PyValue::Bool(wanted), "{readme:?}");
    }
}

// ── JSON: encodings, repeated keys ─────────────────────────────────────────────────────────────

fn safetensors_with(payload: &[u8]) -> Vec<u8> {
    let mut bytes = (payload.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn a_header_is_decoded_as_json_loads_decodes_bytes() {
    let dir = Dir::new("encodings");
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(r#"{"a": 1}"#.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    dir.write("utf16.st", &safetensors_with(&utf16));
    dir.write("sig.st", &safetensors_with(b"\xef\xbb\xbf{\"a\": 1}"));
    dir.write("pass.st", &safetensors_with(b"{\"a\": \"\xed\xa0\x80\"}"));
    dir.write("utf32.st", &safetensors_with(b"\x00\x00\x00{\x00\x00\x00}"));
    dir.write("odd.st", &safetensors_with(b"\xff\xfe{\x00}"));
    let read = |name: &str| {
        ask(
            "crosscheck_model_card._read_st_header",
            vec![dir.file(name)],
            vec![],
            None,
        )
    };
    assert_eq!(value_of(&read("utf16.st")), object(vec![("a", int(1))]));
    assert_eq!(value_of(&read("sig.st")), object(vec![("a", int(1))]));
    // An encoded surrogate survives (`surrogatepass`), as a lone surrogate in the string.
    let lone = PyValue::Str(PyStr::from_code_points(vec![0xD800]));
    assert_eq!(value_of(&read("pass.st")), object(vec![("a", lone)]));
    assert_eq!(value_of(&read("utf32.st")), object(vec![]));
    assert_eq!(
        exception_of(&read("odd.st")),
        (
            "UnicodeDecodeError".to_string(),
            "'utf-16-le' codec can't decode byte 0x7d in position 4: truncated data".to_string()
        )
    );
}

#[test]
fn a_repeated_key_is_refused_at_every_nesting_level_and_only_inside_one_object() {
    let message = |index: &str| -> String {
        let dir = Dir::new("duplicates");
        dir.write("a.safetensors", &safetensors_with(b"{}"));
        dir.write("model.safetensors.index.json", index.as_bytes());
        let record = ask(
            "parse_model_config.count_mxfp4_params",
            vec![dir.path()],
            vec![],
            None,
        );
        exception_of(&record).1
    };
    // Inside `metadata` (not the weight map) is still an object the hook sees.
    assert_eq!(
        message(r#"{"weight_map": {"w": "a.safetensors"}, "metadata": {"x": 1, "x": 2}}"#),
        "MXFP4_HEADER_LAYOUT_UNSUPPORTED: index 읽기 실패: MXFP4_HEADER_LAYOUT_UNSUPPORTED: JSON 중복 키: x"
    );
    // The same key in two different objects is fine: the refusal that follows is about the missing tensor, not about a key.
    assert_eq!(
        message(r#"{"weight_map": {"w": "a.safetensors"}, "one": {"x": 1}, "two": {"x": 2}}"#),
        "MXFP4_HEADER_LAYOUT_UNSUPPORTED: index가 선언한 텐서 누락"
    );
}

// ── HTTP ───────────────────────────────────────────────────────────────────────────────────────

fn reason_of(record: &PyValue) -> String {
    let value = value_of(record);
    assert_eq!(
        member(&value, "ok"),
        &PyValue::Bool(false),
        "{}",
        value.repr()
    );
    match member(&value, "reason") {
        PyValue::Str(reason) => reason.to_utf8().expect("text"),
        other => panic!("{}", other.repr()),
    }
}

#[test]
fn http_failures_read_as_urllib_reports_them() {
    let fixture = Fixture::start();
    let lookup = |repo: &str, base: &str, timeout: f64| {
        ask(
            "crosscheck_model_card.fetch_hf_safetensors_total",
            vec![text(repo), PyValue::Float(timeout)],
            vec![],
            Some(base),
        )
    };
    // A port this check owns and nobody listens on: a socket bound to a system-chosen loopback port and never listened on stays
    // open (and the port unavailable to every other check) until after the lookup; the connection is refused.
    let refusing = super::support::RefusingPort::bind();
    let closed = refusing.base();
    assert_eq!(
        reason_of(&lookup("x", &closed, 2.0)),
        format!("네트워크 오류: <urlopen error [Errno 111] Connection refused>({closed}/x)")
    );
    // No answer within the timeout: `urlopen` wraps the socket timeout in a URLError; a stalled body is the bare TimeoutError.
    assert_eq!(
        reason_of(&lookup("stall", &fixture.base, 0.3)),
        format!(
            "네트워크 오류: <urlopen error timed out>({}/stall)",
            fixture.base
        )
    );
    assert_eq!(
        reason_of(&lookup("bodystall", &fixture.base, 0.3)),
        format!("네트워크 오류: timed out({}/bodystall)", fixture.base)
    );
    // A path `http.client` refuses before connecting: control characters are an uncaught InvalidURL, other than ASCII a
    // UnicodeEncodeError that the original reports as a parse failure.
    let record = lookup("a b", &closed, 2.0);
    assert_eq!(
        exception_of(&record),
        (
            "InvalidURL".to_string(),
            "URL can't contain control characters. '/a b' (found at least ' ')".to_string()
        )
    );
    assert_eq!(
        reason_of(&lookup("\u{e9}", &closed, 2.0)),
        "응답 파싱 실패: 'ascii' codec can't encode character '\\xe9' in position 5: ordinal not in range(128)"
    );
    // Strings the client treats by scheme: none (a `ValueError` the original reports as a parse failure), an unknown one, no host,
    // and `ftp` (resolved like the others; its host failure is the resolver's, an empty host is its own message). Texts taken once from
    // Python's `urllib.request.urlopen` (stdlib oracle).
    let fetch = |base: &str| {
        reason_of(&ask(
            "crosscheck_model_card.fetch_hf_safetensors_total",
            vec![text("x"), PyValue::Float(1.0)],
            vec![],
            Some(base),
        ))
    };
    assert_eq!(fetch("h"), "응답 파싱 실패: unknown url type: 'h/x'");
    assert_eq!(fetch("//h"), "응답 파싱 실패: unknown url type: '//h/x'");
    assert_eq!(
        fetch("gopher://example.invalid"),
        "네트워크 오류: <urlopen error unknown url type: gopher>(gopher://example.invalid/x)"
    );
    assert_eq!(
        fetch("foo:bar"),
        "네트워크 오류: <urlopen error unknown url type: foo>(foo:bar/x)"
    );
    assert_eq!(
        fetch("http://"),
        "네트워크 오류: <urlopen error no host given>(http:///x)"
    );
    assert_eq!(
        fetch("http:foo"),
        "네트워크 오류: <urlopen error no host given>(http:foo/x)"
    );
    assert_eq!(
        fetch("ftp://"),
        "네트워크 오류: <urlopen error ftp error: no host given>(ftp:///x)"
    );
    assert_eq!(
        fetch("ftp://example.invalid"),
        "네트워크 오류: <urlopen error [Errno -2] Name or service not known>(ftp://example.invalid/x)"
    );
    assert_eq!(
        fetch("FTP://127.0.0.1:1"),
        "네트워크 오류: <urlopen error [Errno 111] Connection refused>(FTP://127.0.0.1:1/x)"
    );
}

#[test]
fn the_external_note_groups_digits_and_formats_percentages_like_python() {
    let fixture = Fixture::start();
    let check = |local: PyValue| {
        value_of(&ask(
            "crosscheck_model_card.crosscheck_external_vram",
            vec![local, PyValue::None, text("ok")],
            vec![],
            Some(&fixture.base),
        ))
    };
    let float = check(PyValue::Float(1234.5));
    assert_eq!(
        member(&float, "note"),
        &text("로컬 실측(1,234.5) vs 외부(100) 오차 1134.5% 허용범위 초과 — 로컬 파일 재확인 필요")
    );
    assert_eq!(member(&float, "relative_diff"), &PyValue::Float(11.345));
    assert_eq!(member(&float, "verdict"), &text("MISMATCH"));
    let edge = check(int(105));
    assert_eq!(
        member(&edge, "note"),
        &text("로컬 실측(105) vs 외부(100) 오차 5.0% (허용범위 내)")
    );
    assert_eq!(member(&edge, "verdict"), &text("OK"));
    let over = check(int(106));
    assert_eq!(member(&over, "verdict"), &text("MISMATCH"));
    // A text is not a count: the subtraction fails first.
    let record = ask(
        "crosscheck_model_card.crosscheck_external_vram",
        vec![text("many"), PyValue::None, text("ok")],
        vec![],
        Some(&fixture.base),
    );
    assert_eq!(
        exception_of(&record),
        (
            "TypeError".to_string(),
            "unsupported operand type(s) for -: 'str' and 'int'".to_string()
        )
    );
}

// ── dynamic typing of config values ────────────────────────────────────────────────────────────

#[test]
fn head_dim_floors_across_number_types_and_refuses_the_rest() {
    let parse = |config: &str| {
        let dir = Dir::new("config");
        dir.write("config.json", config.as_bytes());
        ask("parse_model_config.parse", vec![dir.path()], vec![], None)
    };
    // 10.5 // 4 is 2.0 (a float floor division); True // 1 is the integer 1.
    let value = value_of(&parse(
        r#"{"hidden_size": 10.5, "num_attention_heads": 4, "num_hidden_layers": 2}"#,
    ));
    assert_eq!(member(&value, "head_dim"), &PyValue::Float(2.0));
    let value = value_of(&parse(
        r#"{"hidden_size": true, "num_attention_heads": 1, "num_hidden_layers": 2}"#,
    ));
    assert_eq!(member(&value, "head_dim"), &int(1));
    let (class, message) = exception_of(&parse(
        r#"{"hidden_size": "x", "num_attention_heads": 4, "num_hidden_layers": 2}"#,
    ));
    assert_eq!(
        (class.as_str(), message.as_str()),
        (
            "TypeError",
            "unsupported operand type(s) for //: 'str' and 'int'"
        )
    );
    let (class, message) = exception_of(&parse(
        r#"{"hidden_size": 8, "num_attention_heads": "2", "num_hidden_layers": 2}"#,
    ));
    assert_eq!(
        (class.as_str(), message.as_str()),
        (
            "TypeError",
            "unsupported operand type(s) for //: 'int' and 'str'"
        )
    );
}

// ── argument binding and the arguments after a call ────────────────────────────────────────────

#[test]
fn binding_errors_precede_the_body_in_cpythons_order() {
    let message = |operation: &str, args: Vec<PyValue>, kwargs: Vec<(&str, PyValue)>| {
        let (class, message) = exception_of(&ask(operation, args, kwargs, None));
        assert_eq!(class, "TypeError");
        message
    };
    let parse = "parse_model_config.parse";
    assert_eq!(
        message(
            parse,
            vec![int(1), int(2), int(3), int(4)],
            vec![("x", int(1))]
        ),
        "parse() got an unexpected keyword argument 'x'"
    );
    assert_eq!(
        message(
            parse,
            vec![int(1), int(2), int(3), int(4)],
            vec![("model_path", int(1))]
        ),
        "parse() got multiple values for argument 'model_path'"
    );
    assert_eq!(
        message(parse, vec![int(1), int(2), int(3), int(4)], vec![]),
        "parse() takes from 1 to 3 positional arguments but 4 were given"
    );
    assert_eq!(
        message(parse, vec![], vec![]),
        "parse() missing 1 required positional argument: 'model_path'"
    );
    assert_eq!(
        message("parse_model_config._native_weight_bytes", vec![], vec![]),
        "_native_weight_bytes() missing 2 required positional arguments: 'host_path' and 'warnings'"
    );
    assert_eq!(
        message(
            "parse_model_config._model_id_from_path",
            vec![int(1), int(2)],
            vec![]
        ),
        "_model_id_from_path() takes 1 positional argument but 2 were given"
    );
    let (class, message) = exception_of(&ask("parse_model_config.nope", vec![], vec![], None));
    assert_eq!(
        (class.as_str(), message.as_str()),
        (
            "AttributeError",
            "module 'parse_model_config' has no attribute 'nope'"
        )
    );
    let (class, message) = exception_of(&ask("other.nope", vec![], vec![], None));
    assert_eq!(
        (class.as_str(), message.as_str()),
        ("ModuleNotFoundError", "No module named 'other'")
    );
}

#[test]
fn a_warnings_list_is_written_in_place_wherever_it_was_supplied() {
    let dir = Dir::new("warnings");
    dir.write("config.json", b"{}");
    // An empty directory: the function appends its "no weight files" warning to the list it was given.
    let positional = ask(
        "parse_model_config._native_weight_bytes",
        vec![dir.path(), texts(&["initial"])],
        vec![],
        None,
    );
    assert_eq!(value_of(&positional), PyValue::None);
    let after = match member(&positional, "args_after") {
        PyValue::List(items) => items.clone(),
        other => panic!("{}", other.repr()),
    };
    assert_eq!(
        after[1],
        texts(&[
            "initial",
            "weight 파일(*.safetensors/*.bin/index.json)을 찾지 못함 → native_weight_bytes=None"
        ])
    );
    // Supplied as a keyword, the list in `kwargs_after` is the one written to, and the positional arguments are untouched.
    let keyword = ask(
        "parse_model_config._native_weight_bytes",
        vec![dir.path()],
        vec![("warnings", texts(&["initial"]))],
        None,
    );
    assert_eq!(
        member(member(&keyword, "kwargs_after"), "warnings"),
        &texts(&[
            "initial",
            "weight 파일(*.safetensors/*.bin/index.json)을 찾지 못함 → native_weight_bytes=None"
        ])
    );
    assert_eq!(member(&keyword, "args_after"), &list(vec![dir.path()]));
    // Something that is not a list has no `append`: found out when the first warning is written, and nothing changes.
    let tuple_like = ask(
        "parse_model_config._native_weight_bytes",
        vec![dir.path(), text("not a list")],
        vec![],
        None,
    );
    assert_eq!(
        exception_of(&tuple_like),
        (
            "AttributeError".to_string(),
            "'str' object has no attribute 'append'".to_string()
        )
    );
    assert_eq!(
        member(&tuple_like, "args_after"),
        &list(vec![dir.path(), text("not a list")])
    );
    // A binding failure never touches the arguments.
    let missing = ask(
        "parse_model_config._native_weight_bytes",
        vec![dir.path()],
        vec![],
        None,
    );
    assert_eq!(member(&missing, "args_after"), &list(vec![dir.path()]));
}

// ── the command lines follow the terminal width ──────────────────────────────────────────────────

#[test]
fn help_and_usage_follow_the_terminal_width_as_argparse_lays_them_out() {
    // Expected texts: Python's own `argparse` (3.12) formatting parsers declared with the two programs' strings, at three widths
    // (`COLUMNS`), saved in `data/model_argparse_oracle.json`; the original scripts are not run.
    let oracle = super::support::load("../model_argparse_oracle.json");
    for (program, key, binary) in [
        (
            "parse_model_config.sh",
            "parse",
            env!("CARGO_BIN_EXE_parse_model_config"),
        ),
        (
            "crosscheck_model_card.sh",
            "cross",
            env!("CARGO_BIN_EXE_crosscheck_model_card"),
        ),
    ] {
        for width in [40, 100, 23] {
            let run = |arguments: &[&str]| {
                std::process::Command::new(binary)
                    .args(arguments)
                    .env_clear()
                    .env("COLUMNS", width.to_string())
                    .stdin(std::process::Stdio::null())
                    .output()
                    .expect("run the program")
            };
            let wanted = |what: &str| match member(&oracle, &format!("{key}-{what}-{width}")) {
                PyValue::Str(own) => own.to_utf8().expect("text"),
                other => panic!("{}", other.repr()),
            };
            let help = run(&["--help"]);
            assert_eq!(help.status.code(), Some(0));
            assert_eq!(
                String::from_utf8_lossy(&help.stdout),
                wanted("help"),
                "{program} at {width}"
            );
            let bad = run(&["--unknown"]);
            assert_eq!(bad.status.code(), Some(2));
            assert_eq!(
                String::from_utf8_lossy(&bad.stderr),
                format!(
                    "{}{program}: error: unrecognized arguments: --unknown\n",
                    wanted("usage")
                ),
                "{program} at {width}"
            );
        }
    }
}

// ── the service's transport ──────────────────────────────────────────────────────────────────────

#[test]
fn the_service_answers_every_line_and_reports_a_failing_stream_with_a_non_zero_status() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let program = env!("CARGO_BIN_EXE_recipe_model_api");
    // One record per request line, in order, blank lines skipped, a bad line answered by a RequestError record.
    let mut child = Command::new(program)
        .arg("evaluate-jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the service");
    let requests = concat!(
        r#"{"id": 1, "operation": "parse_model_config._model_id_from_path", "args": ["/a/b/"]}"#,
        "\n\n",
        "not json\n",
        r#"{"id": 3, "operation": "parse_model_config._model_id_from_path", "args": [], "kwargs": {"x": 1}}"#,
        "\n"
    );
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(requests.as_bytes())
        .expect("send the requests");
    let output = child.wait_with_output().expect("the service ends");
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).expect("ASCII records");
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows.len(), 3, "{text}");
    assert!(rows[0].starts_with(r#"{"id": 1, "kind": "value", "value": "a/b", "stdout": "", "stderr": "", "args_after": ["/a/b/"]"#));
    assert!(rows[1].contains(r#""type": "RequestError""#) && rows[1].contains("not JSON"));
    assert!(
        rows[2].contains("got an unexpected keyword argument 'x'")
            && rows[2].contains(r#""kwargs_after": {"x": 1}"#)
    );
    // A standard output that cannot be written is a diagnostic and status 1, never a quiet success.
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full");
    let mut child = Command::new(program)
        .arg("constants")
        .stdin(Stdio::null())
        .stdout(full)
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the service");
    let mut message = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().expect("stderr"), &mut message)
        .expect("read stderr");
    assert_eq!(child.wait().expect("ends").code(), Some(1));
    assert!(
        message.starts_with("recipe_model_api: cannot write standard output: OSError: [Errno 28]"),
        "{message}"
    );
    // A standard input that cannot be read (a directory) is a diagnostic and status 1 too.
    let directory = std::fs::File::open(std::env::temp_dir()).expect("a directory");
    let output = Command::new(program)
        .arg("evaluate-jsonl")
        .stdin(directory)
        .output()
        .expect("run the service");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with("recipe_model_api: cannot read standard input:"));
    // The constants are one object with the original module constants.
    let output = Command::new(program)
        .arg("constants")
        .output()
        .expect("run the service");
    let text = String::from_utf8(output.stdout).expect("ASCII");
    assert!(
        text.contains(r#""DEFAULT_NAS_HOST_ROOT": "/mnt/models""#)
            && text.contains(r#""HF_API_BASE": "https://huggingface.co/api/models""#)
    );
}
