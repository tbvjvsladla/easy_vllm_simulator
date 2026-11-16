//! The `docker` stand-in of the frozen `dataset_prep` and `resolve_bench_tool` cases (test plane only: it never runs a
//! container). The recorded cases' `.fixture-bin/docker` is a two-line shell program that execs this helper with the
//! case family (`exec "$SELFTEST_FIXTURE_DOCKER_BIN" <family> "$@"`); before 2026-11-16 that file was a Python program
//! and this helper is its line-for-line native replacement (plan_26111615: no check runs Python). The behaviour, the
//! recorded `docker_calls.jsonl` lines (`json.dumps(argv, ensure_ascii=True)`), the output files and exit statuses are
//! the Python program's.
//!
//! * `dataset` / `dataset-stdout` — `guidellm preprocess` in a container: records the call (with the caller's PID and
//!   user normalised to `@SELF_PID@` / `@SELF_UID@:@SELF_GID@`), answers `rm` with 0 and `run` per
//!   `DATASET_FIXTURE_DOCKER_MODE` (`timeout`, `nonzero`, `no-output`, `empty`, `bad-json`, `wrong-row`,
//!   `invalid-utf8`, `stderr`, otherwise three rows). `dataset-stdout` also prints one stdout line on `run`, and
//!   `dataset-stdout-stderr` then one more stderr line.
//! * `bench-tool` — `docker image inspect <ref> --format {{json .RepoDigests}}`: records the call, writes
//!   `BENCH_TOOL_FIXTURE_STDOUT` / `BENCH_TOOL_FIXTURE_STDERR` and exits with `BENCH_TOOL_FIXTURE_RC`.

use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::process::ExitCode;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

fn record(args: &[OsString]) {
    let line = PyValue::List(
        args.iter()
            .map(|arg| PyValue::Str(PyStr::from_surrogateescape(arg.as_bytes())))
            .collect(),
    )
    .dumps_ascii()
    .expect("argv is JSON");
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("docker_calls.jsonl")
        .expect("docker_calls.jsonl");
    writeln!(out, "{line}").expect("record");
}

fn assertion(args: &[OsString]) -> ExitCode {
    let shown: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    eprintln!("AssertionError: {shown:?}");
    ExitCode::from(1)
}

fn dataset(args: Vec<OsString>, stdout_line: bool, final_diagnostic: bool) -> ExitCode {
    let parent = rustix::process::getppid().map_or(0, |pid| pid.as_raw_nonzero().get());
    let own_name = format!("guidellm-preprocess-{parent}");
    let own_user = format!(
        "{}:{}",
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw()
    );
    let mut saved = args.clone();
    for (flag, expected, replacement) in [
        (
            "--name",
            own_name.as_str(),
            "guidellm-preprocess-@SELF_PID@",
        ),
        ("--user", own_user.as_str(), "@SELF_UID@:@SELF_GID@"),
    ] {
        if let Some(at) = saved.iter().position(|arg| arg == flag) {
            if saved.get(at + 1).is_some_and(|value| value == expected) {
                saved[at + 1] = OsString::from(replacement);
            }
        }
    }
    if saved.len() > 2 && saved[0] == "rm" && saved[1] == "-f" && saved[2] == own_name.as_str() {
        saved[2] = OsString::from("guidellm-preprocess-@SELF_PID@");
    }
    record(&saved);
    if args.first().is_some_and(|arg| arg == "rm") {
        return ExitCode::SUCCESS;
    }
    if !args.first().is_some_and(|arg| arg == "run") {
        return assertion(&args);
    }
    let mode = std::env::var("DATASET_FIXTURE_DOCKER_MODE").unwrap_or_default();
    match mode.as_str() {
        "timeout" => std::thread::sleep(std::time::Duration::from_secs(3)),
        "nonzero" => return ExitCode::from(17),
        "no-output" => return ExitCode::SUCCESS,
        _ => {}
    }
    let mount = args
        .windows(2)
        .take(args.len().saturating_sub(1))
        .find(|pair| pair[0] == "-v" && pair[1].as_bytes().ends_with(b":/out"))
        .map(|pair| pair[1].clone());
    let Some(mount) = mount else {
        eprintln!("StopIteration");
        return ExitCode::from(1);
    };
    let bytes = mount.into_vec();
    let out =
        PathBuf::from(OsString::from_vec(bytes[..bytes.len() - 5].to_vec())).join("prepared.jsonl");
    let written: Vec<u8> = match mode.as_str() {
        "empty" => b"\n".to_vec(),
        "bad-json" => b"{\"prompt_tokens_count\":1024}\n{bad\n".to_vec(),
        "wrong-row" => b"[]\n".to_vec(),
        "invalid-utf8" => b"\xff\n".to_vec(),
        _ => {
            let mut rows = String::new();
            for (index, tokens) in [1024, 1024, 1025].iter().enumerate() {
                rows.push_str(&format!(
                    "{{\"prompt_tokens_count\": {tokens}, \"id\": {index}}}\n"
                ));
            }
            rows.push('\n');
            rows.into_bytes()
        }
    };
    std::fs::write(&out, written).expect("prepared.jsonl");
    if mode == "stderr" {
        eprintln!("fixture Docker diagnostic");
    }
    if stdout_line {
        let mut stdout = std::io::stdout();
        writeln!(stdout, "fixture Docker stdout")
            .and_then(|()| stdout.flush())
            .expect("stdout");
    }
    if final_diagnostic {
        eprintln!("fixture final diagnostic");
    }
    ExitCode::SUCCESS
}

fn bench_tool(args: Vec<OsString>) -> ExitCode {
    record(&args);
    let shape = args.len() >= 4
        && args[0] == "image"
        && args[1] == "inspect"
        && args[args.len() - 2] == "--format"
        && args[args.len() - 1] == "{{json .RepoDigests}}";
    if !shape {
        return assertion(&args);
    }
    let value = |name: &str| std::env::var_os(name).unwrap_or_default().into_vec();
    std::io::stdout()
        .write_all(&value("BENCH_TOOL_FIXTURE_STDOUT"))
        .expect("stdout");
    std::io::stdout().flush().expect("stdout");
    std::io::stderr()
        .write_all(&value("BENCH_TOOL_FIXTURE_STDERR"))
        .expect("stderr");
    let rc = String::from_utf8_lossy(&value("BENCH_TOOL_FIXTURE_RC"))
        .trim()
        .parse::<i32>();
    match rc {
        Ok(code) => ExitCode::from(u8::try_from(code.rem_euclid(256)).unwrap_or(1)),
        Err(_) => {
            eprintln!("ValueError: invalid literal for int()");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: selftest_fixture_docker dataset|dataset-stdout|dataset-stdout-stderr|bench-tool ARGS...");
        return ExitCode::from(2);
    }
    let family = args.remove(0);
    match family.to_str() {
        Some("dataset") => dataset(args, false, false),
        Some("dataset-stdout") => dataset(args, true, false),
        Some("dataset-stdout-stderr") => dataset(args, true, true),
        Some("bench-tool") => bench_tool(args),
        _ => {
            eprintln!("selftest_fixture_docker: unknown family {family:?}");
            ExitCode::from(2)
        }
    }
}
