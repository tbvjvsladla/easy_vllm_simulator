//! The saved original results of the two model scripts, observed whole: 182 function calls (`data/model_reference/results.json`:
//! value, exception class and message, streams, and the arguments after the call) and 23 command lines (raw status and bytes).
//! Nothing here runs the original; the saved rows were captured once from the unmodified scripts on a small input tree that is
//! copied into a scratch directory for each run.
//!
//! The data files hold the capture's directory as the token `@REFERENCE@` (the captured paths were rewritten once, mechanically,
//! so that no machine path is stored); every string of an expected row and every argument is given the scratch directory
//! back in its place. That substitution is the only rebasing. The loopback HTTP fixture is the one the capture used: it
//! answers on 127.0.0.1:30067 when that port is free (else on a free port, and then the base URL of the saved context and rows is
//! rebased to it), and the requests it received are compared with the saved ones.

use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::api::render;
use recipe_runtime::model_api::answer;
use recipe_runtime::observe::same_value;
use std::io::Read;
use std::process::{Command, Stdio};

use super::support::{
    data, items, load, member, rebased, text_of, Fixture, Scratch, SAVED_BASE, TOKEN,
};

// ── the 182 function calls ───────────────────────────────────────────────────────────────────

#[test]
fn the_182_saved_function_calls_are_reproduced_whole() {
    let scratch = Scratch::new("api");
    let fixture = Fixture::start();
    let reference = scratch.root.to_string_lossy().into_owned();
    let replacements = [(TOKEN, reference), (SAVED_BASE, fixture.base.clone())];
    let cases = load("cases.json");
    let results = load("results.json");
    let rows = items(member(&results, "rows"));
    let cases = items(&cases);
    assert_eq!((cases.len(), rows.len()), (182, 182));
    let mut failures: Vec<String> = Vec::new();
    for (case, saved) in cases.iter().zip(rows) {
        let id = text_of(member(case, "id"));
        assert_eq!(id, text_of(member(saved, "id")));
        let expected = rebased(saved, &replacements);
        let mut args = member(case, "args").clone();
        if let PyValue::Dict(map) = case {
            if let Some(PyValue::Str(source)) = map.get("args_from_saved_case") {
                let source = source.to_utf8().expect("id");
                let row = rows
                    .iter()
                    .find(|row| text_of(member(row, "id")) == source)
                    .expect("the saved row a case builds on");
                args = PyValue::List(vec![member(row, "value").clone()]);
            }
        }
        let args = rebased(&args, &replacements);
        let kwargs = rebased(member(case, "kwargs"), &replacements);
        let mut request = PyDict::new();
        request.set("id", PyValue::str(&id));
        request.set(
            "operation",
            PyValue::str(&format!(
                "{}.{}",
                text_of(member(case, "owner")),
                text_of(member(case, "function"))
            )),
        );
        request.set("args", args);
        request.set("kwargs", kwargs);
        if let PyValue::Dict(map) = case {
            if let Some(context) = map.get("context") {
                request.set("context", rebased(context, &replacements));
            }
        }
        let line = PyValue::Dict(request)
            .dumps_unicode()
            .expect("a request line");
        let actual = answer(&line);
        if !same_value(&expected, &actual) {
            failures.push(format!(
                "{id}\n  want {}\n  got  {}",
                expected.repr().chars().take(700).collect::<String>(),
                actual.repr().chars().take(700).collect::<String>()
            ));
        }
        render(&actual).expect("every record renders");
    }
    assert!(
        failures.is_empty(),
        "{} of 182 differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // The requests the fixture received are the saved ones, in order.
    let saved: Vec<(String, Option<String>)> = items(&load("http-requests.json"))
        .iter()
        .map(|row| {
            (
                text_of(member(row, "path")),
                Some(text_of(member(row, "accept"))),
            )
        })
        .collect();
    assert_eq!(fixture.received(), saved);
}

// ── the 23 command lines ─────────────────────────────────────────────────────────────────────

fn replace_bytes(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let (from, to) = (from.as_bytes(), to.as_bytes());
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    out
}

#[test]
fn the_23_saved_command_lines_end_as_the_originals_did() {
    let scratch = Scratch::new("cli");
    let reference = scratch.root.to_string_lossy().into_owned();
    let cases = load("cli-cases.json");
    let results = load("cli-results.json");
    let rows = items(member(&results, "rows"));
    let cases = items(&cases);
    assert_eq!((cases.len(), rows.len()), (23, 23));
    let mut failures: Vec<String> = Vec::new();
    for (case, row) in cases.iter().zip(rows) {
        let id = text_of(member(case, "id"));
        let program = match text_of(member(case, "owner")).as_str() {
            "parse_model_config" => env!("CARGO_BIN_EXE_parse_model_config"),
            _ => env!("CARGO_BIN_EXE_crosscheck_model_card"),
        };
        let args: Vec<String> = items(member(case, "args"))
            .iter()
            .map(|arg| text_of(arg).replace(TOKEN, &reference))
            .collect();
        let output = Command::new(program)
            .args(&args)
            .current_dir(scratch.root.join("input"))
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("run the program");
        let expected_status = match member(row, "exit_code") {
            PyValue::Int(number) => number.to_string(),
            other => panic!("{}", other.repr()),
        };
        let want_out = replace_bytes(
            &std::fs::read(data(&format!("cli/{id}/stdout.bin"))).expect("saved stdout"),
            TOKEN,
            &reference,
        );
        let want_err = replace_bytes(
            &std::fs::read(data(&format!("cli/{id}/stderr.bin"))).expect("saved stderr"),
            TOKEN,
            &reference,
        );
        let got_status = output
            .status
            .code()
            .map_or("signal".to_string(), |code| code.to_string());
        let mut problems: Vec<String> = Vec::new();
        if got_status != expected_status {
            problems.push(format!("status {got_status} want {expected_status}"));
        }
        if output.stdout != want_out {
            problems.push(format!(
                "stdout differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_out),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        // An unhandled Python exception's report in the saved original is a traceback; this program writes the complete final
        // `Class: message` block only. The observer removes exactly the original's header and its frames of the frozen source
        // (nothing else) and compares what is left strictly.
        let stderr_ok = match strip_original_traceback(&want_err, reference.as_bytes()) {
            Ok(rest) => output.stderr == rest,
            Err(problem) => {
                panic!("{id}: the saved traceback is not in the expected shape: {problem}")
            }
        };
        if !stderr_ok {
            problems.push(format!(
                "stderr differs:\n--- want\n{}\n--- got\n{}",
                String::from_utf8_lossy(&want_err),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if !problems.is_empty() {
            failures.push(format!("{id} {args:?}: {}", problems.join("; ")));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 23 differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // Read-only data stays read-only: a command line does not write into its input.
    let mut buffer = Vec::new();
    std::fs::File::open(
        scratch
            .root
            .join("input")
            .join("Org-Normal")
            .join("config.json"),
    )
    .and_then(|mut file| file.read_to_end(&mut buffer))
    .expect("the input is still there");
}

/// The saved standard error of a case without the part of Python's traceback this program does not write: the header line and
/// each frame (`  File "<capture>/source/<script>.py", line N, in X`, then its indented source and caret lines) of the two
/// frozen scripts. Output that is not a traceback is returned whole; a traceback with a frame of any other file, or with
/// nothing after the frames, is an `Err` (the observer never guesses what to drop).
pub fn strip_original_traceback(saved: &[u8], capture: &[u8]) -> Result<Vec<u8>, String> {
    const HEADER: &[u8] = b"Traceback (most recent call last):\n";
    let Some(mut rest) = saved.strip_prefix(HEADER) else {
        return Ok(saved.to_vec());
    };
    let frame = |line: &[u8]| -> bool {
        let prefix = [b"  File \"".as_slice(), capture, b"/source/".as_slice()].concat();
        let Some(after) = line.strip_prefix(prefix.as_slice()) else {
            return false;
        };
        [
            &b"parse_model_config.py\", line "[..],
            &b"crosscheck_model_card.py\", line "[..],
        ]
        .iter()
        .any(|script| after.starts_with(script))
    };
    let mut frames = 0usize;
    loop {
        let end = rest
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(rest.len(), |at| at + 1);
        let line = &rest[..end];
        if line.starts_with(b"  File ") {
            if !frame(line.strip_suffix(b"\n").unwrap_or(line)) {
                return Err(format!(
                    "a frame outside the frozen sources: {}",
                    String::from_utf8_lossy(line)
                ));
            }
            frames += 1;
            rest = &rest[end..];
            // its source line and caret marks: indented by four spaces
            while rest.starts_with(b"    ") {
                let end = rest
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(rest.len(), |at| at + 1);
                rest = &rest[end..];
            }
        } else {
            break;
        }
    }
    if frames == 0 || rest.is_empty() || rest.starts_with(b" ") {
        return Err(
            "no frame, no final block, or a final block that starts with a space".to_string(),
        );
    }
    Ok(rest.to_vec())
}

#[test]
fn the_traceback_observer_removes_the_header_and_frozen_frames_and_nothing_else() {
    let saved: &[u8] = b"Traceback (most recent call last):\n  File \"@REFERENCE@/source/parse_model_config.py\", line 9, in <module>\n    raise SystemExit(_main())\n                     ^^^^^^^\n  File \"@REFERENCE@/source/crosscheck_model_card.py\", line 5, in f\n    x = y\nValueError: a\n    indented second line\nthird\n";
    assert_eq!(
        strip_original_traceback(saved, b"@REFERENCE@").expect("a traceback"),
        b"ValueError: a\n    indented second line\nthird\n".to_vec()
    );
    // Not a traceback: returned whole.
    assert_eq!(
        strip_original_traceback(b"usage: x\n", b"@REFERENCE@").expect("plain"),
        b"usage: x\n".to_vec()
    );
    // A frame of another file, or no final block, is refused rather than guessed.
    assert!(strip_original_traceback(b"Traceback (most recent call last):\n  File \"/usr/lib/python3.12/json/__init__.py\", line 1, in f\n    x\nE: m\n", b"@REFERENCE@").is_err());
    assert!(strip_original_traceback(b"Traceback (most recent call last):\n  File \"@REFERENCE@/source/parse_model_config.py\", line 1, in f\n    x\n", b"@REFERENCE@").is_err());
    assert!(strip_original_traceback(
        b"Traceback (most recent call last):\nE: m\n",
        b"@REFERENCE@"
    )
    .is_err());
}
