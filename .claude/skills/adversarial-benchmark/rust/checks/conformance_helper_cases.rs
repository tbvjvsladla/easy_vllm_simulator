//! The helpers of the conformance checker in process: the frozen emitter, reader and transport cases of the original
//! (`data/conformance/helper_cases.json`, `transport_cases.json`), the decoding and newline rules of the readers
//! against the interpreter's behaviour, and the early exit of `main` when a source is missing.

#[path = "conformance_common.rs"]
mod common;

use common::*;
use sweep_stop::conformance::{emit, read, read_optional_strict, Raised};
use sweep_stop::conformance_checks::main;
use sweep_stop::conformance_native::native_value;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::{PyDict, PyValue};

fn outcome_record(result: Result<PyValue, Raised>, stdout: Option<&Recorder>) -> PyDict {
    let mut record = PyDict::new();
    match result {
        Ok(value) => {
            record.set("kind", PyValue::str("value"));
            record.set("value", value);
        }
        Err(raised) => {
            record.set("kind", PyValue::str("exception"));
            record.set("type", PyValue::str(raised.failure().class));
            record.set("message", PyValue::Str(raised.failure().message.clone()));
        }
    }
    if let Some(recorder) = stdout {
        record.set("stdout", PyValue::Str(recorder.stdout.clone()));
    }
    record
}

#[test]
fn emitter_cases_match_the_frozen_original() {
    let mut seen = 0;
    for case in list(read_json("helper_cases.json")) {
        let case = dict(&case);
        if text(field(case, "operation")) != "_emit" {
            continue;
        }
        let arguments = dict(field(case, "arguments"));
        let mut console = Recorder::default();
        let result = emit(field(arguments, "results"), &mut console).map(PyValue::Bool);
        let record = outcome_record(result, Some(&console));
        expected_matches(&record, dict(field(case, "expected")))
            .unwrap_or_else(|why| panic!("{}: {why}", text(field(case, "id"))));
        seen += 1;
    }
    assert_eq!(seen, 14, "all frozen emitter cases ran");
}

#[test]
fn reader_cases_match_the_frozen_original() {
    let mut seen = 0;
    for case in list(read_json("helper_cases.json")) {
        let case = dict(&case);
        if text(field(case, "operation")) != "read" {
            continue;
        }
        let tree = Tree::new("read");
        tree.materialize(dict(field(case, "before")));
        let name = text(field(dict(field(case, "arguments")), "name"));
        let result = read(&tree.context(), &PyStr::new(&name))
            .map(|found| found.map_or(PyValue::None, |text| PyValue::str(&text)));
        expected_matches(&outcome_record(result, None), dict(field(case, "expected")))
            .unwrap_or_else(|why| panic!("{}: {why}", text(field(case, "id"))));
        seen += 1;
    }
    assert_eq!(seen, 10, "all frozen reader cases ran");
}

#[test]
fn transport_cases_match_the_frozen_original() {
    let mut seen = 0;
    for case in list(read_json("transport_cases.json")) {
        let case = dict(&case);
        let tree = Tree::new("transport");
        tree.materialize(dict(field(case, "before")));
        let arguments = dict(field(case, "arguments"));
        let result = native_value(
            &tree.context(),
            field(arguments, "operation"),
            field(arguments, "arguments"),
        )
        .map(PyValue::Str);
        expected_matches(&outcome_record(result, None), dict(field(case, "expected")))
            .unwrap_or_else(|why| panic!("{}: {why}", text(field(case, "id"))));
        seen += 1;
    }
    assert_eq!(seen, 10, "all frozen transport cases ran");
}

/// The replacement decoding of `open(..., errors="replace")` for sequences the frozen cases do not cover; the
/// expected texts are what CPython's `bytes.decode("utf-8", "replace")` gives.
#[test]
fn reader_replacement_decoding_follows_the_interpreter() {
    let cases: [(&[u8], &str); 8] = [
        (b"a\xf0\x9f\x98b", "a\u{fffd}b"),
        (b"\xed\xa0\x80", "\u{fffd}\u{fffd}\u{fffd}"),
        (b"\xc0\x80", "\u{fffd}\u{fffd}"),
        (b"\xe0\x80\x80", "\u{fffd}\u{fffd}\u{fffd}"),
        (
            b"\xf8\x88\x80\x80\x80",
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}",
        ),
        (b"\xff\xfe", "\u{fffd}\u{fffd}"),
        (b"\xf0\x9f\x98", "\u{fffd}"),
        (b"a\xe2\x82\r\nb\rc", "a\u{fffd}\nb\nc"),
    ];
    for (bytes, expected) in cases {
        let tree = Tree::new("decode");
        tree.write("data.txt", bytes);
        let found = read(&tree.context(), &PyStr::new("data.txt")).expect("read");
        assert_eq!(found.as_deref(), Some(expected), "{bytes:?}");
    }
}

#[test]
fn reading_a_name_that_is_absolute_ignores_the_scripts_directory() {
    let tree = Tree::new("absolute");
    let other = Tree::new("absolute-target");
    other.write("x.txt", b"elsewhere");
    let path = other.path("x.txt").to_string_lossy().into_owned();
    let found = read(&tree.context(), &PyStr::new(&path)).expect("read");
    assert_eq!(found.as_deref(), Some("elsewhere"));
}

#[test]
fn an_unreadable_file_is_an_os_error_not_none() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new("unreadable");
    tree.write("data.txt", b"x");
    std::fs::set_permissions(
        tree.path("data.txt"),
        std::fs::Permissions::from_mode(0o000),
    )
    .expect("chmod");
    if std::fs::read(tree.path("data.txt")).is_ok() {
        return; // running with privileges that ignore file modes: the case cannot arise
    }
    let error = read(&tree.context(), &PyStr::new("data.txt")).unwrap_err();
    assert_eq!(error.failure().class, "PermissionError");
    assert!(
        error
            .repr()
            .starts_with("PermissionError(13, 'Permission denied')"),
        "{}",
        error.repr()
    );
}

#[test]
fn optional_strict_reads_return_empty_for_os_errors_and_raise_for_bad_utf8() {
    let tree = Tree::new("strict");
    tree.write("good.txt", "한\r\n글".as_bytes());
    tree.write("bom.txt", b"\xef\xbb\xbfx");
    tree.write("bad.txt", b"ok\xffno");
    std::fs::create_dir_all(tree.path("dir")).expect("directory");
    let path = |name: &str| PyStr::new(&tree.path(name).to_string_lossy());
    assert_eq!(read_optional_strict(&path("good.txt")).unwrap(), "한\n글");
    assert_eq!(read_optional_strict(&path("bom.txt")).unwrap(), "\u{feff}x");
    assert_eq!(read_optional_strict(&path("missing.txt")).unwrap(), "");
    assert_eq!(read_optional_strict(&path("dir")).unwrap(), "");
    let error = read_optional_strict(&path("bad.txt")).unwrap_err();
    assert_eq!(error.failure().class, "UnicodeDecodeError");
    assert_eq!(
        error.failure().message.to_utf8().unwrap(),
        "'utf-8' codec can't decode byte 0xff in position 2: invalid start byte"
    );
    assert_eq!(
        error.repr(),
        "UnicodeDecodeError('utf-8', b'ok\\xffno', 2, 3, 'invalid start byte')"
    );
}

#[test]
fn main_stops_after_the_existence_checks_when_a_source_is_missing() {
    let tree = Tree::new("missing-source");
    for name in [
        "sweep_bench.sh",
        "publish_benchmark_record.sh",
        "lite_metrics_native.py",
    ] {
        tree.write(name, b"");
    }
    let mut console = Recorder::default();
    let status = main(&tree.context(), &mut console).expect("main");
    assert_eq!(status, 1);
    assert_eq!(
        console.stdout.to_utf8().unwrap(),
        "PASS 파일 존재: sweep_bench.sh\nFAIL 파일 존재: render_report.sh\nPASS 파일 존재: publish_benchmark_record.sh\nPASS 파일 존재: lite_metrics_native.py\n\nfull ⊇ lite conformance: 3/4 FAIL\n"
    );
}

#[test]
fn a_transport_that_cannot_start_is_reported_with_its_python_arguments() {
    // The launcher of the reporting owner is a directory: `bash <dir>` fails with a status, which is a RuntimeError.
    let tree = Tree::new("launcher-dir");
    std::fs::create_dir_all(tree.path("bench_reporting_api.sh")).expect("directory");
    let error = native_value(
        &tree.context(),
        &PyValue::str("render_report.build_md"),
        &PyValue::Dict(PyDict::new()),
    )
    .unwrap_err();
    assert_eq!(error.failure().class, "RuntimeError");
    assert!(error
        .failure()
        .message
        .to_utf8()
        .unwrap()
        .starts_with("native reporting call failed: "));
}

/// The whole `native_value` against a reporting launcher that closes both outputs and then reads its input to the end
/// (the stdin regression of the first R14 build): the request of more than a pipe-full must reach the child whole,
/// and the call ends as the original does, a `JSONDecodeError` for the empty answer, at the product limit (not at a
/// shortened one).
#[test]
fn native_value_delivers_a_large_request_to_a_child_that_closed_its_outputs() {
    let tree = Tree::new("stdin-after-outputs");
    tree.write(
        "bench_reporting_api.sh",
        b"exec >&- 2>&-\ncat > \"$(dirname \"$0\")/received.bin\"\n",
    );
    let payload = "x".repeat(262_144);
    let mut arguments = PyDict::new();
    arguments.set("payload", PyValue::str(&payload));
    let operation = PyValue::str("closed-output");
    let arguments = PyValue::Dict(arguments);
    let started = std::time::Instant::now();
    let error = native_value(&tree.context(), &operation, &arguments).unwrap_err();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
    assert_eq!(error.failure().class, "JSONDecodeError");
    assert_eq!(
        error.failure().message.to_utf8().unwrap(),
        "Expecting value: line 1 column 1 (char 0)"
    );
    let mut row = PyDict::new();
    row.set("id", PyValue::str("conformance"));
    row.set("operation", operation);
    row.set("arguments", arguments);
    let expected = format!("{}\n", PyValue::Dict(row).dumps_ascii().expect("JSON"));
    let received = std::fs::read(tree.path("received.bin")).expect("received request");
    assert_eq!(
        received.len(),
        expected.len(),
        "the whole request reached the child"
    );
    assert!(received == expected.as_bytes());
}

/// The lite metrics transport has no timeout in the original: its call writes a request larger than a pipe-full,
/// waits for a child that is slow to answer, and returns the answer, with no limit anywhere.
#[test]
fn the_lite_metrics_call_without_a_limit_writes_waits_and_answers() {
    use sweep_stop::conformance_native::LiteMetrics;
    let tree = Tree::new("lite-unbounded");
    tree.write("lite_metrics_native.py", b"");
    tree.write(
        "lite_metrics_api.sh",
        b"cat > \"$(dirname \"$0\")/received.bin\"\nsleep 0.3\nprintf '%s\\n' '{\"id\": \"python-consumer\", \"encoding\": \"json-values-v1\", \"kind\": \"value\", \"value_json\": \"7\"}'\n",
    );
    let lite = LiteMetrics::beside_adapter(&tree.context());
    let started = std::time::Instant::now();
    let answer = lite
        .call("build", &[("raw", &PyValue::str(&"y".repeat(400_000)))])
        .expect("answer");
    assert_eq!(answer, PyValue::int(7));
    assert!(started.elapsed() >= std::time::Duration::from_millis(300));
    let received = std::fs::read(tree.path("received.bin")).expect("received request");
    assert!(
        received.len() > 400_000,
        "the whole request reached the child"
    );
    assert!(received.ends_with(b"\n"));
}

/// `open(path, encoding="utf-8").read()` decodes the whole file in one call, so the position in the
/// `UnicodeDecodeError` text is the position in the file however large the file is (confirmed against the
/// interpreter for 20 000 and 100 000 bytes).
#[test]
fn strict_read_errors_report_whole_file_positions_for_large_files() {
    let tree = Tree::new("strict-large");
    let mut bytes = vec![b'a'; 20_000];
    bytes.push(0xff);
    bytes.extend_from_slice(b"bbb");
    tree.write("large.txt", &bytes);
    let mut truncated = vec![b'a'; 100_000];
    truncated.extend_from_slice(b"\xe2\x82");
    tree.write("truncated.txt", &truncated);
    let message = |name: &str| {
        read_optional_strict(&PyStr::new(&tree.path(name).to_string_lossy()))
            .unwrap_err()
            .failure()
            .message
            .to_utf8()
            .unwrap()
    };
    assert_eq!(
        message("large.txt"),
        "'utf-8' codec can't decode byte 0xff in position 20000: invalid start byte"
    );
    assert_eq!(
        message("truncated.txt"),
        "'utf-8' codec can't decode bytes in position 100000-100001: unexpected end of data"
    );
}
