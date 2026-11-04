//! Runs the built `sweep_stop` binary and checks the command-line contract: stdin/file input, exit codes
//! (0 continue, 3 stop, 2 declaration error, 1 uncaught), stdout bytes, argparse-style options, the
//! `--self-test` transcript, the report subcommands and the JSONL transport.

#[path = "common.rs"]
mod common;

use common::{dict, list, load_json_file, oracle_dir, text_of};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::PyValue;

const NOW: &str = "2026-09-04T09:30:00Z";

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sweep_stop"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sweep_stop");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    Output {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_contract");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir.join(name)
}

fn state_json(cells: &str, remaining: &str) -> String {
    format!(
        "{{\"declared_budget\": {{\"max_cells\": 6, \"wall_clock_budget_s\": 3600, \"consecutive_failure_limit\": 3, \
         \"repeats\": 3, \"repeats_source\": \"declared(x)\", \"declared_by\": \"tester\", \"basis\": \"fixture\"}}, \
         \"cells_planned\": 6, \"started_utc\": \"2026-09-04T09:00:00Z\", \"cells\": {cells}, \"cells_remaining\": {remaining}}}"
    )
}

#[test]
fn stdin_runs_reproduce_the_recorded_python_cli_output() {
    let dir = oracle_dir();
    let reference = list(load_json_file(&dir.join("cli-reference.json")));
    let cases = list(load_json_file(&dir.join("cases.json")));
    let mut compared = 0;
    for entry in &reference {
        let entry = dict(entry);
        let name = text_of(entry.get("case"));
        let want_code: i32 = entry
            .get("exit_code")
            .and_then(PyValue::as_plain_int)
            .unwrap()
            .to_string()
            .parse()
            .unwrap();
        if name == "existing-self-test" {
            let got = run(&["--self-test"], "");
            assert_eq!(got.code, want_code, "self-test exit");
            assert_eq!(
                got.stdout,
                text_of(entry.get("stdout")),
                "self-test transcript is the original's, line for line"
            );
            assert_eq!(got.stderr, text_of(entry.get("stderr")), "self-test stderr");
            compared += 1;
            continue;
        }
        let case = cases
            .iter()
            .map(dict)
            .find(|case| text_of(case.get("id")) == name)
            .expect("case exists");
        let now = text_of(case.get("now_utc"));
        let state = case.get("state").unwrap().dumps_ascii().unwrap();
        let got = run(&["--state", "-", "--now-utc", &now], &state);
        assert_eq!(got.code, want_code, "{name}: exit code");
        assert_eq!(
            got.stdout,
            text_of(entry.get("stdout")),
            "{name}: stdout bytes"
        );
        assert_eq!(got.stderr, text_of(entry.get("stderr")), "{name}: stderr");
        compared += 1;
    }
    assert_eq!(compared, reference.len());
    assert!(
        compared >= 5,
        "reference holds four evaluations and the self-test"
    );
}

#[test]
fn file_input_matches_stdin_and_is_left_untouched() {
    let text = state_json(
        "[{\"cell_key\": \"a\", \"cell_outcome\": \"serve_failed\"}]",
        "[\"b\"]",
    );
    let path = scratch("state.json");
    std::fs::write(&path, &text).unwrap();
    let from_file = run(&["--state", path.to_str().unwrap(), "--now-utc", NOW], "");
    let from_stdin = run(&["--state", "-", "--now-utc", NOW], &text);
    assert_eq!(
        (from_file.code, &from_file.stdout),
        (from_stdin.code, &from_stdin.stdout)
    );
    assert_eq!(from_file.code, 0);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        text,
        "the state file is read only"
    );
}

#[test]
fn stop_exits_three_and_prints_all_reasons_in_order() {
    let cells: Vec<String> = (0..6)
        .map(|_| "{\"cell_key\": \"same\", \"cell_outcome\": \"serve_failed\"}".to_string())
        .collect();
    let got = run(
        &["--state", "-", "--now-utc", "2026-09-04T10:00:00Z"],
        &state_json(&format!("[{}]", cells.join(",")), "[]"),
    );
    assert_eq!(got.code, 3);
    assert!(got.stdout.contains("\"stopped_by\": [\n    \"cells_exhausted\",\n    \"cell_budget_exhausted\",\n    \"wall_clock_exhausted\",\n    \"consecutive_failures\"\n  ]"));
}

#[test]
fn handled_and_unhandled_failures_use_the_original_exit_codes() {
    let missing = scratch("absent.json");
    let _ = std::fs::remove_file(&missing);
    let cases: [(Vec<&str>, &str, i32, &str); 8] = [
        (vec!["--now-utc", NOW], "", 2, "[sweep_stop] ERROR --state 와 --now-utc 는 필수다(--self-test 제외)\n"),
        (vec!["--state", "-"], "", 2, "[sweep_stop] ERROR --state 와 --now-utc 는 필수다(--self-test 제외)\n"),
        (vec!["--state", "-", "--now-utc", NOW], "", 2, "[sweep_stop] ERROR 상태를 읽을 수 없다: Expecting value: line 1 column 1 (char 0)\n"),
        (vec!["--state", "-", "--now-utc", NOW], "[]", 2, "[sweep_stop] ERROR 상태는 JSON 객체여야 한다\n"),
        (vec!["--state", "-", "--now-utc", "garbage"], "{}", 2, "[sweep_stop] ERROR --now-utc 를 시각으로 읽을 수 없다('garbage'): Invalid isoformat string: 'garbage'\n"),
        (vec!["--state", "/definitely/not/here.json", "--now-utc", NOW], "", 2, "[sweep_stop] ERROR 상태를 읽을 수 없다: [Errno 2] No such file or directory: '/definitely/not/here.json'\n"),
        (vec!["--state", "-", "--now-utc", NOW], "{\"x\": 1", 2, "[sweep_stop] ERROR 상태를 읽을 수 없다: Expecting ',' delimiter: line 1 column 8 (char 7)\n"),
        (vec!["--bogus"], "", 2, ""),
    ];
    for (args, stdin, code, stderr) in cases {
        let got = run(&args, stdin);
        assert_eq!(got.code, code, "{args:?}");
        if !stderr.is_empty() {
            assert_eq!(got.stderr, stderr, "{args:?}");
        }
        assert_eq!(got.stdout, "", "{args:?}: nothing is printed on failure");
    }

    let uncaught = state_json("[]", "5");
    let got = run(&["--state", "-", "--now-utc", NOW], &uncaught);
    assert_eq!(
        (got.code, got.stderr.as_str()),
        (1, "TypeError: 'int' object is not iterable\n")
    );
}

#[test]
fn options_follow_argparse_conventions() {
    let text = state_json("[]", "[\"a\"]");
    for args in [
        vec!["--state=-", "--now-utc", NOW],
        vec!["--sta", "-", "--now", NOW],
        vec!["--state", "-", "--now-utc=2026-09-04T09:30:00Z"],
    ] {
        let got = run(&args, &text);
        assert_eq!(got.code, 0, "{args:?}: {}", got.stderr);
    }
    assert_eq!(
        run(&["--s", "-"], &text).code,
        2,
        "--s is ambiguous between --state and --self-test"
    );
    assert_eq!(run(&["--state"], &text).code, 2, "a value is required");
    assert_eq!(
        run(&["extra", "--state", "-", "--now-utc", NOW], &text).code,
        2,
        "positionals are rejected"
    );
    assert_eq!(
        run(&["--self-test", "--state", "-"], "").code,
        0,
        "--self-test wins over missing --now-utc"
    );
    let help = run(&["-h"], "");
    assert_eq!(
        (help.code, help.stdout.starts_with("usage: sweep_stop")),
        (0, true)
    );
}

#[test]
fn report_subcommands_print_the_text_of_the_replaced_snippets() {
    let stop_path = scratch("verdict.stop.json");
    let cells: Vec<String> = (0..6)
        .map(|i| format!("{{\"cell_key\": \"c{i}\", \"cell_outcome\": \"serve_failed\"}}"))
        .collect();
    let verdict = run(
        &["--state", "-", "--now-utc", "2026-09-04T10:00:00Z"],
        &state_json(&format!("[{}]", cells.join(",")), "[]"),
    );
    std::fs::write(&stop_path, &verdict.stdout).unwrap();
    let stop = stop_path.to_str().unwrap();

    let by = run(&["report", "stopped-by", "--stop-json", stop], "");
    assert_eq!(by.stdout, "  stopped_by: ['cells_exhausted', 'cell_budget_exhausted', 'wall_clock_exhausted', 'consecutive_failures']\n");
    let line = run(&["report", "status-line", "--stop-json", stop], "");
    assert_eq!(line.stdout, "[broad_search] stop=True by=['cells_exhausted', 'cell_budget_exhausted', 'wall_clock_exhausted', 'consecutive_failures'] 남은셀=0\n");

    let state_path = scratch("declared.json");
    std::fs::write(&state_path, state_json("[]", "[\"a\"]")).unwrap();
    let repeats = run(
        &[
            "report",
            "declared-repeats",
            "--state",
            state_path.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(repeats.stdout, "3\u{1f}declared(x)\n");

    std::fs::write(&state_path, "{\"declared_budget\": {\"max_cells\": 1}}").unwrap();
    let legacy = run(
        &[
            "report",
            "declared-repeats",
            "--state",
            state_path.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(legacy.stdout, "\u{1f}출처 미기재\n");

    let missing = run(
        &[
            "report",
            "stopped-by",
            "--stop-json",
            "/definitely/not/here.json",
        ],
        "",
    );
    assert_eq!(missing.code, 1);
    assert_eq!(run(&["report", "nonsense"], "").code, 2);
}

#[test]
fn jsonl_transport_answers_every_line() {
    let valid = state_json("[]", "[\"a\"]");
    let input = format!(
        "{{\"id\": 1, \"state\": {valid}, \"now_utc\": \"{NOW}\"}}\n\n{{\"id\": \"bad-now\", \"state\": {valid}, \"now_utc\": null}}\n{{\"id\": 3}}\nnot json\n"
    );
    let got = run(&["evaluate-jsonl"], &input);
    assert_eq!(got.code, 0);
    let lines: Vec<&str> = got.stdout.lines().collect();
    assert_eq!(lines.len(), 4, "one answer per non-blank input line");
    assert!(
        lines[0].starts_with("{\"id\": 1, \"kind\": \"value\", \"value\": {\"schema_version\": 1")
    );
    assert!(lines[0].ends_with("\"exit_code\": 0}"));
    let second = sweep_stop::pyjson::loads(lines[1]).expect("answers are JSON");
    let second = dict(&second);
    assert_eq!(text_of(second.get("id")), "bad-now");
    assert_eq!(text_of(second.get("kind")), "exception");
    assert_eq!(text_of(second.get("type")), "StopEvalError");
    assert_eq!(
        text_of(second.get("message")),
        "--now-utc 는 ISO-8601 UTC 문자열이어야 한다(받은 값: None)"
    );
    assert!(lines[2].contains("\"kind\": \"invalid_input\""));
    assert!(lines[3].contains("\"kind\": \"invalid_input\""));
}
