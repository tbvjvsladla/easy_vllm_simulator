//! The registered checks of the classifier owner (`sim_classify.py`): the saved original results, observed whole, and the fixed
//! text patterns against Python's own `re`.
//!
//! * `reference_cases` runs the 147 saved function calls through the real `recipe_classifier_api` binary (one request per line, in
//!   one process, compared record by record: value or exception, types, key order, nested metadata, streams, `args_after`,
//!   `kwargs_after`) and the 16 saved command lines through the real `sim_classify` binary, each in its own new private directory made
//!   from the saved declarative setup. The inputs of a command line are read only: the tree after equals the tree before. The only
//!   rebasing is the token `@CASE@` (the capture directory), replaced by this run's private directory. A command line's standard
//!   error is compared after the original's Python traceback frames (its own script and the two `json` modules) were removed and
//!   nothing else. The original module is never run: the expected values are the saved observations of the capture.
//! * `pattern_cases` compares the exception patterns the classifier applies to an error text with Python's `re` (the interpreter is
//!   used only as the oracle of that stdlib primitive, never of the original module) over a declared matrix of texts.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::observe::same_value;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("classifier_reference")
        .join(relative)
}

fn load(relative: &str) -> PyValue {
    let bytes = std::fs::read(data(relative)).expect("saved data");
    loads_text(&PyStr::from_surrogateescape(&bytes)).expect("saved JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("missing {key}")),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn items(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not a list: {}", other.repr()),
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn base64_decode(body: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut accumulator, mut bits) = (0u32, 0u32);
    for byte in body.bytes().filter(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    out
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|at| u8::from_str_radix(&text[2 * at..2 * at + 2], 16).expect("hex"))
        .collect()
}

fn replace_bytes(bytes: &[u8], token: &str, with: &[u8]) -> Vec<u8> {
    let token = token.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(token) {
            out.extend_from_slice(with);
            at += token.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

/// Runs `command` with its streams captured.
fn output_of(command: &mut Command) -> std::process::Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run the program")
}

mod reference_cases {
    use super::*;

    #[test]
    fn the_147_saved_function_calls_are_reproduced_whole_by_one_service_process() {
        let cases = load("cases.json");
        let rows = load("results.json");
        assert_eq!((items(&cases).len(), items(&rows).len()), (147, 147));
        let mut input = Vec::new();
        for case in items(&cases) {
            let mut request = agent_runtime_support::pyvalue::PyDict::new();
            request.set("id", member(case, "id").clone());
            request.set(
                "operation",
                PyValue::str(&format!(
                    "sim_classify.{}",
                    text_of(member(case, "function"))
                )),
            );
            request.set("args", member(case, "args").clone());
            request.set("kwargs", member(case, "kwargs").clone());
            input.extend_from_slice(
                PyValue::Dict(request)
                    .dumps_ascii()
                    .expect("a request")
                    .as_bytes(),
            );
            input.push(b'\n');
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_recipe_classifier_api"))
            .arg("evaluate-jsonl")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the service");
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(&input)
                .expect("send");
        }
        let output = child.wait_with_output().expect("the service ends");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let lines: Vec<&[u8]> = output
            .stdout
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), 147);
        let mut wrong: Vec<String> = Vec::new();
        for (line, expected) in lines.iter().zip(items(&rows)) {
            let got = loads_text(&PyStr::from_surrogateescape(line)).expect("a record");
            if !same_value(expected, &got) {
                wrong.push(format!(
                    "{}\n  want {}\n  got  {}",
                    text_of(member(expected, "id")),
                    expected.repr(),
                    got.repr()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of 147 differ:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// The saved standard error of a command line without the original's traceback frames: the header and each frame of the
    /// original script, of the `json` modules or of the frozen `codecs` decoder (the invalid-UTF-8 diagnostic); anything else is refused.
    fn strip_original_frames(saved: &[u8], script: &[u8]) -> Result<Vec<u8>, String> {
        const HEADER: &[u8] = b"Traceback (most recent call last):\n";
        let Some(mut rest) = saved.strip_prefix(HEADER) else {
            return Ok(saved.to_vec());
        };
        let allowed = |file: &[u8]| -> bool {
            file == script
                || file.ends_with(b"/json/__init__.py")
                || file.ends_with(b"/json/decoder.py")
                || file == b"<frozen codecs>"
        };
        let mut frames = 0;
        while rest.starts_with(b"  File \"") {
            let end = rest
                .iter()
                .position(|&byte| byte == b'\n')
                .map_or(rest.len(), |at| at + 1);
            let line = &rest[..end];
            let after = &line[b"  File \"".len()..];
            let Some(quote) = after.iter().position(|&byte| byte == b'"') else {
                return Err("a frame without a file".to_string());
            };
            if !allowed(&after[..quote]) {
                return Err(format!(
                    "a frame outside the original's modules: {}",
                    String::from_utf8_lossy(line)
                ));
            }
            frames += 1;
            rest = &rest[end..];
            while rest.starts_with(b"    ") {
                let end = rest
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(rest.len(), |at| at + 1);
                rest = &rest[end..];
            }
        }
        if frames == 0 || rest.is_empty() || rest.starts_with(b" ") {
            return Err(
                "no frame, no final block, or a final block that starts with a space".to_string(),
            );
        }
        Ok(rest.to_vec())
    }

    fn run_command_line(case: &PyValue) -> Option<String> {
        let id = text_of(member(case, "id"));
        let area =
            PrivateDirectory::create(&format!("classifier-{id}-")).expect("a private directory");
        let root = area.path().join("case");
        std::fs::create_dir(&root).expect("the case root");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o775)).expect("a mode");
        let has_trial = matches!(case, PyValue::Dict(map) if map.contains_key("trial_hex"));
        if has_trial {
            let path = root.join("trial.json");
            std::fs::write(&path, hex_decode(&text_of(member(case, "trial_hex"))))
                .expect("the trial file");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o664))
                .expect("a mode");
        }
        let before = tree(&root);
        let root_bytes = root.as_os_str().as_bytes().to_vec();
        let arguments: Vec<OsString> = items(member(case, "args"))
            .iter()
            .map(|argument| {
                OsString::from_vec(replace_bytes(
                    text_of(argument).as_bytes(),
                    "@CASE@",
                    &root_bytes,
                ))
            })
            .collect();
        let output = output_of(
            Command::new(env!("CARGO_BIN_EXE_sim_classify"))
                .args(&arguments)
                .current_dir(&root)
                .env_clear(),
        );
        let want_out = replace_bytes(
            &base64_decode(&text_of(member(case, "stdout"))),
            "@CASE@",
            &root_bytes,
        );
        let saved_err = base64_decode(&text_of(member(case, "stderr")));
        let script = b"@CASE@/node/.claude/skills/vllm-recipe-explorer/scripts/sim_classify.py";
        let want_err = match strip_original_frames(&saved_err, script) {
            Ok(rest) => replace_bytes(&rest, "@CASE@", &root_bytes),
            Err(problem) => {
                return Some(format!(
                    "{id}: the saved diagnostic is not in the expected shape: {problem}"
                ))
            }
        };
        let wanted_status = match member(case, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        let mut problems: Vec<String> = Vec::new();
        if output.status.code().map(|code| code.to_string()) != Some(wanted_status.clone()) {
            problems.push(format!(
                "status {:?} want {wanted_status}",
                output.status.code()
            ));
        }
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        if output.stderr != want_err {
            problems.push(format!(
                "stderr differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_err),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if tree(&root) != before {
            problems.push("the input tree changed".to_string());
        }
        if problems.is_empty() {
            None
        } else {
            Some(format!("{id} {arguments:?}: {}", problems.join("; ")))
        }
    }

    /// The names, kinds, modes and bytes of the case's tree (depth first, by name).
    fn tree(root: &std::path::Path) -> Vec<(String, u32, Option<Vec<u8>>)> {
        let mut rows = vec![(
            ".".to_string(),
            std::fs::metadata(root).expect("root").permissions().mode() & 0o7777,
            None,
        )];
        let mut names: Vec<PathBuf> = std::fs::read_dir(root)
            .expect("a directory")
            .map(|entry| entry.expect("entry").path())
            .collect();
        names.sort();
        for path in names {
            let info = std::fs::symlink_metadata(&path).expect("an entry");
            assert!(info.file_type().is_file(), "only files are expected");
            rows.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                info.permissions().mode() & 0o7777,
                Some(std::fs::read(&path).expect("bytes")),
            ));
        }
        rows
    }

    #[test]
    fn the_16_saved_command_lines_end_as_the_original_did_and_leave_their_inputs_unchanged() {
        let cases = load("cli.json");
        assert_eq!(items(&cases).len(), 16);
        let failures: Vec<String> = items(&cases).iter().filter_map(run_command_line).collect();
        assert!(
            failures.is_empty(),
            "{} of 16 differ:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_diagnostic_comparator_removes_only_original_frames_and_keeps_the_whole_final_block() {
        let script = b"@CASE@/s/sim_classify.py";
        let saved: &[u8] = b"Traceback (most recent call last):\n  File \"@CASE@/s/sim_classify.py\", line 9, in <module>\n    x()\n    ^^^\n  File \"/usr/lib/python3.12/json/decoder.py\", line 1, in f\n    y\nValueError: a\n    indented second line\nthird\n";
        assert_eq!(
            strip_original_frames(saved, script).expect("stripped"),
            b"ValueError: a\n    indented second line\nthird\n".to_vec()
        );
        assert_eq!(
            strip_original_frames(b"usage: x\n", script).expect("plain"),
            b"usage: x\n".to_vec()
        );
        assert!(strip_original_frames(b"Traceback (most recent call last):\n  File \"/elsewhere/other.py\", line 1, in f\n    x\nE: m\n", script).is_err());
        assert!(strip_original_frames(b"Traceback (most recent call last):\n  File \"@CASE@/s/sim_classify.py\", line 1, in f\n    x\n", script).is_err());
        assert!(
            strip_original_frames(b"Traceback (most recent call last):\nE: m\n", script).is_err()
        );
    }
}

mod pattern_cases {
    use super::*;
    use recipe_runtime::classifier::has_exception;

    /// Python's `re` with the five exception patterns of the classifier, as the original writes them (the module is not imported):
    /// the set of texts, one answer per line.
    const PROBE: &str = r#"
import json, re, sys
EXC = (r"Traceback \(most recent call last\)",
       r"\bERROR\b.*(Error|Exception)",
       r"^\s*\w*(Error|Exception):",
       r"CompilationError",
       r"Engine core initialization failed")
for text in json.load(sys.stdin):
    print('1' if any(re.search(p, text, re.MULTILINE) for p in EXC) else '0')
"#;

    #[test]
    fn the_exception_patterns_agree_with_pythons_re_on_a_declared_matrix_of_texts() {
        let mut texts: Vec<String> = [
            "",
            "x",
            "Traceback (most recent call last)",
            "Traceback (most recent call last",
            "traceback (most recent call last)",
            "ERROR SomeError",
            "ERROR Exception",
            "ERROR",
            "ERROR\nSomeError",
            "ERROR\rSomeError",
            "ERROR\r\nSomeError",
            "xERROR SomeError",
            "ERROR_ SomeError",
            "ERROR\u{345} SomeError",
            "ERROR\u{1c89} SomeError",
            "ERROR\u{0e01} SomeError",
            "ERROR\u{0660} SomeError",
            "ERROR\u{00b2} SomeError",
            "ERROR\u{2160} SomeError",
            "ERROR\u{00a0}SomeError",
            "SomeERROR Error",
            "ERRORS Error",
            "error SomeError",
            "ERROR: Error",
            "ERRORError",
            "ERROR Err",
            "ERROR E",
            "ERROR  \t Error and more",
            "ERROR a\nb Error",
            "a ERROR b\nc Error",
            "ERROR\u{2028}Error",
            "ERROR\u{85}Error",
            "ERROR\u{0b}Error",
            "ERROR\u{0c}Error",
            "ERROR\u{1c}Error",
            "CustomError: failed",
            "  CustomError: failed",
            "\n  CustomException: failed",
            "\n\t\u{2003}CustomError: failed",
            "\u{a0}Error:",
            "\u{3000}\u{2003}Exception:",
            "Error:",
            "Exception:",
            "myerror: failed",
            "my Error: failed",
            "x\nError:",
            "x Error:",
            "x\n\n \n  Exception: y",
            "\r\nError:",
            "\rError:",
            "a\rError:",
            "a\u{2028}Error:",
            "a\u{85}Error:",
            "\u{0345}Error:",
            "\u{1c89}Error:",
            "\u{0660}Error:",
            "\u{2160}Exception:",
            "_Error:",
            "9Error:",
            "Error :",
            "Error;",
            "ValueError: x",
            "KeyError",
            "Err:",
            "CompilationError",
            "xCompilationErrorx",
            "Compilationerror",
            "Engine core initialization failed",
            "engine core initialization failed",
            "Engine core initialization  failed",
            "something else",
            "a\u{ff21}Error:",
            "\u{ff21}\u{ff22}Exception:",
            "\u{1d7ce}Error:",
            "\u{10400}Error:",
            "\u{1f600}Error:",
        ]
        .iter()
        .map(|text| text.to_string())
        .collect();
        // Longer shapes: runs of white space and word characters before the name, over several lines.
        texts.push(format!("{}Error:", " ".repeat(300)));
        texts.push(format!("{}\nError:", "a".repeat(300)));
        texts.push(format!("{}Error:", "\n".repeat(50)));
        texts.push(format!("{} Error:", "a".repeat(300)));
        let request = PyValue::List(texts.iter().map(|text| PyValue::str(text)).collect())
            .dumps_ascii()
            .expect("JSON");
        let mut child = Command::new("python3")
            .args(["-I", "-W", "ignore", "-c", PROBE])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| {
                panic!("NOT_RUN: the interpreter is the oracle of this check: {error}")
            });
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(request.as_bytes())
                .expect("send");
        }
        let output = child.wait_with_output().expect("the interpreter ends");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answers = String::from_utf8(output.stdout).expect("text");
        let rows: Vec<&str> = answers.lines().collect();
        assert_eq!(rows.len(), texts.len());
        let mut wrong = Vec::new();
        for (text, want) in texts.iter().zip(rows) {
            let got = if has_exception(&PyStr::new(text)) {
                "1"
            } else {
                "0"
            };
            if got != want {
                wrong.push(format!("{text:?}: python {want}, ours {got}"));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} texts differ:\n{}",
            wrong.len(),
            texts.len(),
            wrong.join("\n")
        );
        let _ = texts.iter().filter(|text| text.contains('\u{345}')).count();
    }
}

/// The frozen post-quality measurement program on a tiny schedule (2 rounds of 3 calls): diagnostic children of the checks, not
/// official measurements. Its rows are three saved cases of the predeclared pure workload (`api-006` the scalar conversion, `api-024` the
/// checked KV-fit, `api-135` the batch refinement of a passing trial) with the saved whole values as expectations.
mod bench_cases {
    use super::*;
    use agent_runtime_support::pyvalue::PyDict;

    fn case(id: &str) -> (PyValue, PyValue) {
        let cases = load("cases.json");
        let rows = load("results.json");
        let request = items(&cases)
            .iter()
            .find(|row| text_of(member(row, "id")) == id)
            .unwrap_or_else(|| panic!("{id}"))
            .clone();
        let saved = items(&rows)
            .iter()
            .find(|row| text_of(member(row, "id")) == id)
            .unwrap_or_else(|| panic!("{id}"))
            .clone();
        (request, saved)
    }

    fn row(metric: &str, id: &str) -> PyDict {
        let (request, saved) = case(id);
        let mut row = PyDict::new();
        row.set("metric", PyValue::str(metric));
        row.set(
            "operation",
            PyValue::str(&format!(
                "sim_classify.{}",
                text_of(member(&request, "function"))
            )),
        );
        row.set("args", member(&request, "args").clone());
        row.set("kwargs", member(&request, "kwargs").clone());
        row.set("expected", member(&saved, "value").clone());
        row
    }

    fn run(edit: impl Fn(&mut Vec<PyDict>)) -> (Option<i32>, String, String) {
        let area = PrivateDirectory::create("classifier-bench-").expect("a private directory");
        let mut rows = vec![
            row("pure.profile", "api-006"),
            row("pure.kv_fit", "api-024"),
            row("pure.classify", "api-135"),
        ];
        edit(&mut rows);
        let fixture = area.path().join("fixture.json");
        let text = PyValue::List(rows.into_iter().map(PyValue::Dict).collect())
            .dumps_ascii()
            .expect("JSON");
        std::fs::write(&fixture, text).expect("the fixture");
        let output = output_of(
            Command::new(env!("CARGO_BIN_EXE_recipe_classifier_bench"))
                .arg("--fixture")
                .arg(&fixture)
                .args(["--rounds", "2", "--iterations", "3"])
                .env_clear(),
        );
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    #[test]
    fn the_program_reports_every_series_after_the_whole_values_of_all_rows_were_checked() {
        let (code, out, err) = run(|_| {});
        assert_eq!(code, Some(0), "{err}");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4, "{out}");
        assert_eq!(lines[0], "{\"rounds\": 2, \"iterations\": 3}");
        for (line, metric) in
            lines[1..]
                .iter()
                .zip(["pure.profile", "pure.kv_fit", "pure.classify"])
        {
            let report = loads_text(&PyStr::new(line)).expect("a series");
            assert_eq!(text_of(member(&report, "metric")), metric);
            assert_eq!(items(member(&report, "rounds")).len(), 2);
        }
    }

    fn assert_refused(code: Option<i32>, out: &str, err: &str, fragment: &str) {
        assert_eq!(code, Some(1), "{err}");
        assert!(
            out.is_empty(),
            "nothing is timed or reported after a failed check: {out}"
        );
        assert!(err.contains(fragment), "{err}");
    }

    #[test]
    fn a_wrong_whole_value_a_reordered_key_a_changed_type_or_a_failing_call_ends_the_run_with_no_report(
    ) {
        // The last row's value differs: the earlier rows' series are never reported (the report is buffered).
        let (code, out, err) = run(|rows| {
            let mut expected = match rows[1].get("expected") {
                Some(PyValue::Dict(map)) => map.clone(),
                _ => panic!(),
            };
            expected.set("exceeds", PyValue::Bool(true));
            rows[1].set("expected", PyValue::Dict(expected));
        });
        assert_refused(code, &out, &err, "differs from the saved original result");
        // The same keys in another order are another value.
        let (code, out, err) = run(|rows| {
            let expected = match rows[2].get("expected") {
                Some(PyValue::Dict(map)) => map.clone(),
                _ => panic!(),
            };
            let mut reordered = PyDict::new();
            let mut entries: Vec<(PyStr, PyValue)> = expected.iter().cloned().collect();
            entries.swap(0, 1);
            for (key, value) in entries {
                reordered.insert(key, value);
            }
            rows[2].set("expected", PyValue::Dict(reordered));
        });
        assert_refused(code, &out, &err, "differs from the saved original result");
        // An integer is not the float the function returns.
        let (code, out, err) = run(|rows| {
            rows[0].set("expected", PyValue::int(3));
        });
        assert_refused(code, &out, &err, "differs from the saved original result");
        // A call that fails before any measurement.
        let (code, out, err) = run(|rows| {
            rows[2].set(
                "args",
                PyValue::List(vec![PyValue::None, PyValue::int(80), PyValue::Float(0.9)]),
            );
        });
        assert_refused(code, &out, &err, "fails before measurement");
    }
}

/// The product the classifier takes of the caller's budget and margin (`budget_gib * safety_margin`), against Python's `*`: two numbers
/// multiply as numbers of any size (no index conversion is involved), and a string or list is repeated only by an integer or boolean that
/// fits a signed index-sized integer, checked before a valid negative count becomes zero. The operands come from one JSON text read by
/// both sides; the values and messages are those of CPython's operator through one stdlib-only probe (never the original module).
mod product_cases {
    use super::*;
    use recipe_runtime::classifier::product;

    const PROBE: &str = r#"
import json, sys
for left, right in json.load(sys.stdin):
    try:
        print(repr(left * right))
    except Exception as error:
        print('%s: %s' % (type(error).__name__, error))
"#;

    fn operands() -> String {
        let big = "1208925819614629174706176"; // 2 ** 80
        let ten25 = "10000000000000000000000000";
        let ten400 = format!("1{}", "0".repeat(400));
        let rows: Vec<String> = vec![
            format!("[{ten25}, 1]"),
            format!("[{ten25}, -{big}]"),
            format!("[-{big}, {ten25}]"),
            format!("[{big}, {big}]"),
            format!("[{ten25}, 0.9]"),
            format!("[true, {ten25}]"),
            format!("[{ten400}, 1.5]"),
            format!("[1.5, {ten400}]"),
            "[80, 0.9]".to_string(),
            "[true, 3.0]".to_string(),
            "[null, 0.9]".to_string(),
            "[0.9, null]".to_string(),
            "[\"x\", 3]".to_string(),
            "[3, \"ab\"]".to_string(),
            "[\"x\", -5]".to_string(),
            "[-5, \"ab\"]".to_string(),
            "[\"x\", true]".to_string(),
            "[\"x\", false]".to_string(),
            "[true, \"ab\"]".to_string(),
            "[[1, \"a\"], 2]".to_string(),
            "[2, [1, \"a\"]]".to_string(),
            "[[1], -1]".to_string(),
            format!("[\"x\", -{big}]"),
            format!("[\"x\", {big}]"),
            format!("[-{big}, \"x\"]"),
            format!("[{big}, \"x\"]"),
            "[\"x\", 9223372036854775808]".to_string(),
            "[\"x\", -9223372036854775808]".to_string(),
            "[\"x\", -9223372036854775809]".to_string(),
            "[\"\", 9223372036854775807]".to_string(),
            "[\"\", 4611686018427387904]".to_string(),
            "[\"\", -4611686018427387904]".to_string(),
            "[[], 4611686018427387904]".to_string(),
            format!("[[], {big}]"),
            format!("[[], -{big}]"),
            format!("[\"\", {big}]"),
            format!("[\"\", -{big}]"),
            "[\"x\", null]".to_string(),
            "[null, \"x\"]".to_string(),
            "[\"a\", \"b\"]".to_string(),
            "[[1], {}]".to_string(),
            "[{}, [1]]".to_string(),
            "[\"a\", 1.5]".to_string(),
            "[1.5, \"a\"]".to_string(),
            "[[1], [2]]".to_string(),
            "[{}, 2]".to_string(),
            "[null, null]".to_string(),
            // Result length and capacity: each of these is rejected by the length arithmetic before anything is allocated.
            // (A count whose length still fits is never probed: it would ask the interpreter for a real allocation.)
            "[\"ab\", 9223372036854775807]".to_string(),
            "[9223372036854775807, \"ab\"]".to_string(),
            "[[0, 0], 9223372036854775807]".to_string(),
            "[9223372036854775807, [0, 0]]".to_string(),
            "[\"ab\", 4611686018427387904]".to_string(),
            "[[0, 0], 4611686018427387904]".to_string(),
            "[\"abc\", 3074457345618258603]".to_string(),
            "[[0, 1, 2], 3074457345618258603]".to_string(),
            "[true, [0, 0, 0]]".to_string(),
            // The signed bound is checked before emptiness and before a negative count becomes zero.
            "[\"\", 9223372036854775808]".to_string(),
            "[[], 9223372036854775808]".to_string(),
            "[\"ab\", 9223372036854775808]".to_string(),
            "[[0, 0], -9223372036854775809]".to_string(),
            // Valid negative counts and empty sequences stay empty without building anything.
            "[\"ab\", -9223372036854775808]".to_string(),
            "[[0, 0], -9223372036854775808]".to_string(),
            "[[0, 0], -9223372036854775807]".to_string(),
            "[[], 9223372036854775807]".to_string(),
            "[\"ab\", 0]".to_string(),
            "[[0, 0], false]".to_string(),
        ];
        format!("[{}]", rows.join(","))
    }

    #[test]
    fn the_product_of_the_budget_and_the_margin_equals_pythons_for_numbers_and_sequences() {
        let text = operands();
        let mut child = Command::new("python3")
            .args(["-I", "-W", "ignore", "-c", PROBE])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| {
                panic!("NOT_RUN: the interpreter is the oracle of this check: {error}")
            });
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(text.as_bytes())
                .expect("send");
        }
        let output = child.wait_with_output().expect("the interpreter ends");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answers = String::from_utf8(output.stdout).expect("text");
        let pairs = loads_text(&PyStr::new(&text)).expect("the operands");
        let rows: Vec<&str> = answers.lines().collect();
        assert_eq!(rows.len(), items(&pairs).len());
        let mut wrong = Vec::new();
        for (pair, want) in items(&pairs).iter().zip(rows) {
            let two = items(pair);
            let got = match product(&two[0], &two[1]) {
                Ok(value) => value.repr(),
                Err(failure) => format!(
                    "{}: {}",
                    failure.class,
                    failure.message.to_utf8().expect("text")
                ),
            };
            if got != want {
                wrong.push(format!(
                    "{}: python {want}, ours {got}",
                    pair.repr().chars().take(90).collect::<String>()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} differ:\n{}",
            wrong.len(),
            rows_len(&text),
            wrong.join("\n")
        );
    }

    fn rows_len(text: &str) -> usize {
        items(&loads_text(&PyStr::new(text)).expect("operands")).len()
    }

    /// The two confirmed product bugs of the independent review, and the neighbours that must not move, through the real service:
    /// a trial that loaded and passed, whose product is never compared because the profile has no weights.
    #[test]
    fn a_passing_trial_is_classified_none_unless_the_product_itself_raises() {
        let trial = r#"{"load_ok": true, "functional": {"passed": true}}"#;
        let big = "1208925819614629174706176";
        let ten25 = "10000000000000000000000000";
        let none_value = r#"{"failure_class": "none", "adjust_target": null, "note": "load OK·VRAM OK·functional 통과 → 수렴."}"#;
        let overflow = r#""kind": "exception", "type": "OverflowError", "message": "cannot fit 'int' into an index-sized integer""#;
        let too_long = r#""kind": "exception", "type": "OverflowError", "message": "repeated string is too long""#;
        let memory_error = r#""kind": "exception", "type": "MemoryError", "message": """#;
        let type_error = |kind: &str| {
            format!(
                r#""kind": "exception", "type": "TypeError", "message": "can't multiply sequence by non-int of type '{kind}'""#
            )
        };
        let cases: Vec<(String, String, String)> = vec![
            // (budget, margin, the reply members after the id)
            (
                ten25.to_string(),
                "1".to_string(),
                format!(r#""kind": "value", "value": {none_value}"#),
            ),
            (
                ten25.to_string(),
                format!("-{big}"),
                format!(r#""kind": "value", "value": {none_value}"#),
            ),
            ("\"x\"".to_string(), format!("-{big}"), overflow.to_string()),
            ("\"x\"".to_string(), big.to_string(), overflow.to_string()),
            (format!("-{big}"), "\"x\"".to_string(), overflow.to_string()),
            (
                "\"x\"".to_string(),
                "-5".to_string(),
                format!(r#""kind": "value", "value": {none_value}"#),
            ),
            (
                "\"\"".to_string(),
                "4611686018427387904".to_string(),
                format!(r#""kind": "value", "value": {none_value}"#),
            ),
            ("[]".to_string(), format!("-{big}"), overflow.to_string()),
            // Result length and capacity (the short operands only reach the length arithmetic: nothing is allocated).
            (
                "\"ab\"".to_string(),
                "9223372036854775807".to_string(),
                too_long.to_string(),
            ),
            (
                "9223372036854775807".to_string(),
                "\"ab\"".to_string(),
                too_long.to_string(),
            ),
            (
                "[0, 0]".to_string(),
                "9223372036854775807".to_string(),
                memory_error.to_string(),
            ),
            (
                "9223372036854775807".to_string(),
                "[0, 0]".to_string(),
                memory_error.to_string(),
            ),
            (
                "\"ab\"".to_string(),
                "-9223372036854775808".to_string(),
                format!(r#""kind": "value", "value": {none_value}"#),
            ),
            (
                "[0, 0]".to_string(),
                "9223372036854775808".to_string(),
                overflow.to_string(),
            ),
            ("\"x\"".to_string(), "0.9".to_string(), type_error("float")),
            (
                "\"x\"".to_string(),
                "null".to_string(),
                type_error("NoneType"),
            ),
            (
                "null".to_string(),
                "\"x\"".to_string(),
                type_error("NoneType"),
            ),
        ];
        let mut input = String::new();
        for (index, (budget, margin, _)) in cases.iter().enumerate() {
            input.push_str(&format!(
                r#"{{"id": "p{index}", "operation": "sim_classify.classify", "args": [{trial}, {budget}, {margin}, null], "kwargs": {{}}}}"#
            ));
            input.push('\n');
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_recipe_classifier_api"))
            .arg("evaluate-jsonl")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the service");
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(input.as_bytes())
                .expect("send");
        }
        let output = child.wait_with_output().expect("the service ends");
        assert_eq!(output.status.code(), Some(0));
        let replies = String::from_utf8(output.stdout).expect("text");
        let lines: Vec<&str> = replies.lines().collect();
        assert_eq!(lines.len(), cases.len());
        let mut wrong = Vec::new();
        for (index, ((budget, margin, members), line)) in cases.iter().zip(lines).enumerate() {
            let expected = format!(
                r#"{{"id": "p{index}", {members}, "stdout": "", "stderr": "", "args_after": [{trial}, {budget}, {margin}, null], "kwargs_after": {{}}}}"#
            );
            // The reply is written with ASCII escapes: compare the parsed values (the raw text of the request is not the writer's).
            let want = loads_text(&PyStr::new(&expected)).expect("the expected reply");
            let got = loads_text(&PyStr::new(line)).expect("a reply");
            if !same_value(&want, &got) {
                wrong.push(format!(
                    "{index}: want {}\n  got {}",
                    want.repr(),
                    got.repr()
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {} differ:\n{}",
            wrong.len(),
            cases.len(),
            wrong.join("\n")
        );
    }
}
