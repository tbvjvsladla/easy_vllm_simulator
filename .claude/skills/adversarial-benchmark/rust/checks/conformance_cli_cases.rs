//! `conformance_full_superset_lite` as a program, against a private installation copied from the installed sources
//! whose launchers run the owner binaries built from this crate: the passing run byte for byte, the ignored arguments, and
//! the failing controls of the frozen original (a missing source, a gate that comes late, a publication that always
//! publishes, a reporting owner that breaks the protocol, a host-dead D1 verdict whose dict detail ends the
//! report with the original's `TypeError`).

#[path = "conformance_common.rs"]
mod common;

use common::*;
use std::path::Path;
use std::process::{Command, Output};
use sweep_stop::conformance::find_position;
use sweep_stop::pyjson::{decode_file_text, loads_text};
use sweep_stop::pyvalue::PyValue;

const CLI: &str = env!("CARGO_BIN_EXE_conformance_full_superset_lite");

fn launch(program: &str, arguments: &[&str], scripts: Option<&Path>) -> Output {
    let mut command = Command::new(program);
    with_overrides(
        &mut command,
        env!("CARGO_BIN_EXE_bench_reporting_api"),
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    );
    command
        .args(arguments)
        .env_remove("CONFORMANCE_SCRIPTS_DIR");
    if let Some(dir) = scripts {
        command.env("CONFORMANCE_SCRIPTS_DIR", dir);
    }
    output_retrying_busy(&mut command)
}

/// Runs the command, retrying while the executable is still "busy" (`ETXTBSY`). A binary that a check has just
/// created can be held open for writing by a child another thread has forked and not yet exec'd; the hold lasts
/// until that exec, so a bounded wait is the correct response, not a masked failure.
fn output_retrying_busy(command: &mut Command) -> Output {
    const EXECUTABLE_FILE_BUSY: i32 = 26;
    for _ in 0..100 {
        match command.output() {
            Err(error) if error.raw_os_error() == Some(EXECUTABLE_FILE_BUSY) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other.expect("run"),
        }
    }
    panic!("the executable stayed busy for two seconds");
}

fn cli(arguments: &[&str], scripts: &Path) -> Output {
    launch(CLI, arguments, Some(scripts))
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8")
}

fn stderr_last_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .last()
        .unwrap_or("")
        .to_string()
}

fn install(label: &str) -> (Tree, std::path::PathBuf) {
    let tree = Tree::new(label);
    let scripts = install_copy(&tree);
    (tree, scripts)
}

#[test]
fn the_normal_run_prints_all_forty_one_checks_byte_for_byte() {
    let (_tree, scripts) = install("normal");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_of(&output), read_text("normal_stdout.txt"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn arguments_are_ignored_help_included() {
    let (_tree, scripts) = install("ignored");
    for arguments in [
        &["--ignored", "value"][..],
        &["--help"],
        &["-h"],
        &["selftest"],
    ] {
        let output = cli(arguments, &scripts);
        assert_eq!(output.status.code(), Some(0), "{arguments:?}");
        assert_eq!(
            stdout_of(&output),
            read_text("normal_stdout.txt"),
            "{arguments:?}"
        );
        assert!(output.stderr.is_empty(), "{arguments:?}");
    }
}

#[test]
fn a_missing_source_fails_the_existence_check_and_stops() {
    let (_tree, scripts) = install("missing-report");
    std::fs::remove_file(scripts.join("render_report.sh")).expect("remove");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stdout_of(&output),
        "PASS 파일 존재: sweep_bench.sh\nFAIL 파일 존재: render_report.sh\nPASS 파일 존재: publish_benchmark_record.sh\nPASS 파일 존재: lite_metrics_native.py\n\nfull ⊇ lite conformance: 3/4 FAIL\n"
    );
}

#[test]
fn a_gate_after_the_level_call_reports_the_character_positions() {
    let (_tree, scripts) = install("late-gate");
    let original = std::fs::read_to_string(scripts.join("sweep_bench.sh")).expect("source");
    let gate = "exit \"$GATE_RC\"";
    let moved = format!("{}\n{gate}\n", original.replacen(gate, "true", 1));
    std::fs::write(scripts.join("sweep_bench.sh"), &moved).expect("write");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    let level = {
        let at = find_position(&moved, "run_bench.sh\" \"${RB_ARGS[@]}\"");
        if at < 0 {
            find_position(&moved, "RB_ARGS=(")
        } else {
            at
        }
    };
    let expected = format!(
        "FAIL E sweep_bench 의 lite 게이트 종료가 GuideLLM 레벨 호출보다 **앞**에 있다  — gate@{} level@{level} — 게이트가 레벨 뒤면 교집합 상태가 다시 생긴다\n",
        find_position(&moved, gate)
    );
    let stdout = stdout_of(&output);
    assert!(stdout.contains(&expected), "{stdout}");
    assert!(stdout.ends_with("full ⊇ lite conformance: 40/41 FAIL\n"));
}

#[test]
fn a_publication_that_always_publishes_fails_only_the_gate_check() {
    let (_tree, scripts) = install("always-publish");
    std::fs::write(
        scripts.join("publish_benchmark_record.sh"),
        "#!/bin/bash\nprintf 'benchmark_mode: lite\\nverdict: not_applicable\\n'\n",
    )
    .expect("write");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("FAIL E 인증서 발행기가 실제 beta/gamma lite gate를 지킨다(양성 포함)\n")
    );
    assert!(stdout.ends_with("full ⊇ lite conformance: 40/41 FAIL\n"));
}

#[test]
fn a_reporting_owner_that_breaks_the_protocol_fails_every_dependent_check() {
    let (_tree, scripts) = install("bad-protocol");
    std::fs::write(
        scripts.join("bench_reporting_api.sh"),
        "#!/bin/bash\nprintf '{}\\n'\n",
    )
    .expect("write");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_of(&output);
    assert!(stdout.contains(
        "FAIL native report/certificate output and gate checks  — RuntimeError('native reporting returned no text: {}')\n"
    ));
    assert!(stdout.contains("FAIL render_report 가 lite 결손을 경고한다  — 결손을 조용히 넘기면 비교 불가 사실이 숨는다\n"));
    assert!(stdout.ends_with("full ⊇ lite conformance: 31/41 FAIL\n"));
}

#[test]
fn a_host_dead_d1_verdict_ends_the_report_with_the_original_type_error() {
    let (_tree, scripts) = install("d1-host-dead");
    let fixture = scripts
        .parent()
        .expect("skill")
        .join("fixtures/lite_raw_d1_connection_refused.json");
    let mut raw =
        loads_text(&decode_file_text(&std::fs::read(&fixture).expect("fixture")).expect("UTF-8"))
            .expect("JSON");
    if let PyValue::Dict(map) = &mut raw {
        map.set("host_health_after", PyValue::str("000"));
    }
    std::fs::write(&fixture, raw.dumps_ascii().expect("JSON")).expect("write");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    // The report is printed row by row: everything before the failing D1 row is on standard output, no summary.
    let normal = read_text("normal_stdout.txt");
    let d1_row = normal
        .find("PASS E D1 연결거부 픽스처")
        .expect("the D1 row of the normal report");
    let stdout = stdout_of(&output);
    assert_eq!(stdout, &normal[..d1_row]);
    assert_eq!(
        stderr_last_line(&output),
        "TypeError: can only concatenate str (not \"dict\") to str"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(stderr.starts_with("Traceback (most recent call last):\n"));
}

/// With no `CONFORMANCE_SCRIPTS_DIR` and no installation around the binary, nothing is checked. The binary under
/// test is placed outside any `rust/target/release` ancestor first: `CARGO_BIN_EXE_...` may itself live in an
/// installed layout (an installed Cargo run), where the layout correctly supplies the scripts directory and the
/// run would not end with status 2. The installed-layout positive is `a_copy_of_the_binary_inside_an_installation_...`.
#[test]
fn without_a_scripts_directory_nothing_is_checked() {
    let tree = Tree::new("isolated");
    assert!(
        !tree
            .root
            .ancestors()
            .any(|ancestor| ancestor.file_name().is_some_and(|name| name == "rust")),
        "NOT_RUN: the scratch location must not lie inside a `rust` folder"
    );
    let isolated = tree.path("conformance_full_superset_lite");
    // a hard link opens no file for writing; a copy where the scratch space is on another file system
    std::fs::hard_link(CLI, &isolated)
        .or_else(|_| std::fs::copy(CLI, &isolated).map(|_| ()))
        .expect("place binary");
    let output = launch(isolated.to_str().expect("path"), &[], None);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CONFORMANCE_SCRIPTS_DIR"));
}

#[test]
fn a_copy_of_the_binary_inside_an_installation_finds_its_scripts() {
    let (tree, _scripts) = install("derived");
    let release = tree.path(".claude/skills/adversarial-benchmark/rust/target/release");
    std::fs::create_dir_all(&release).expect("release directory");
    let installed = release.join("conformance_full_superset_lite");
    // A hard link of the built binary (the same file, so no write handle is ever open on it); a copy where the
    // scratch space is on another file system.
    std::fs::hard_link(CLI, &installed)
        .or_else(|_| std::fs::copy(CLI, &installed).map(|_| ()))
        .expect("place binary");
    let output = launch(installed.to_str().expect("path"), &[], None);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_of(&output), read_text("normal_stdout.txt"));
}

#[test]
fn an_undecodable_classifier_source_raises_before_any_report_is_printed() {
    let (tree, scripts) = install("bad-utf8");
    tree.write(
        ".claude/skills/adversarial-benchmark/rust/src/argv.rs",
        b"fn main() {}\xff\n",
    );
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr_last_line(&output)
        .starts_with("UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 12"));
}

/// The declared lite metrics protocol double of the frozen original case (`metrics-system-exit`): valid constants,
/// then a legitimate exception record of the given class and message for every other request. The record is the
/// protocol's: `lite_metrics_native._raise` maps the class name to the builtin, `SystemExit` included.
fn install_metrics_double(scripts: &Path, class: &str, message: &str) {
    let record = format!(
        "{{\"id\":\"python-consumer\",\"encoding\":\"json-values-v1\",\"kind\":\"exception\",\"type\":\"{class}\",\"message\":\"{message}\"}}"
    );
    let script = format!(
        "#!/bin/bash\nif [ \"$1\" = constants ]; then\ncat <<'JSON'\n{{\"LITE_VERDICTS\":[\"pass\",\"measurement_path_failed\",\"server_failed\"],\"LITE_EXIT\":{{\"pass\":0,\"measurement_path_failed\":6,\"server_failed\":7}}}}\nJSON\nelse\ncat >/dev/null\ncat <<'JSON'\n{record}\nJSON\nfi\n"
    );
    std::fs::write(scripts.join("lite_metrics_api.sh"), script).expect("write double");
}

/// `SystemExit` is not an `Exception`: the original's `except Exception` does not catch it, so nothing of the
/// report is printed and the string reason (and a newline) is the whole standard error, exit 1, no traceback. The
/// reason is a string whatever it looks like: "0" is not status 0 and "" is still printed as an empty line.
#[test]
fn a_reported_system_exit_ends_the_checker_with_its_string_reason() {
    for reason in ["declared-protocol-exit", "0", "7", ""] {
        let (_tree, scripts) = install("system-exit");
        install_metrics_double(&scripts, "SystemExit", reason);
        let output = cli(&[], &scripts);
        assert_eq!(output.status.code(), Some(1), "{reason:?}");
        assert!(
            output.stdout.is_empty(),
            "{reason:?}: the report is not printed"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{reason}\n"),
            "{reason:?}"
        );
    }
}

/// The ordinary exceptions of the same protocol stay caught checks: the report is printed, the failed check carries
/// the exception's `repr`, and the run ends with status 1 and no traceback.
#[test]
fn a_reported_ordinary_exception_is_still_a_failed_check() {
    let (_tree, scripts) = install("ordinary-exception");
    install_metrics_double(&scripts, "ValueError", "declared-protocol-error");
    let output = cli(&[], &scripts);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = stdout_of(&output);
    assert!(
        stdout
            .contains("FAIL E 판정기 적재·픽스처 판정  — ValueError('declared-protocol-error')\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("FAIL\n"));
}
