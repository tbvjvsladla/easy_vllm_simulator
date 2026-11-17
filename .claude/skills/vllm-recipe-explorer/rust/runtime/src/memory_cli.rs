//! The command line of `preload_ram_gate.py` (its `main`): refuse a load whose requirement exceeds the available RAM.
//!
//! Exit status 0 passes (or skips an unknown checkpoint size), 7 refuses, 2 is an argument error, 1 an exception nothing handles
//! (its complete final `Class: message` block goes to standard error). The raw `--self-test` flag is looked for in the arguments
//! before any parsing, so it wins over every other argument, a bad one included. Program name in usage and help is the original
//! `.py` name. Standard error is written as it happens (a cache drop can take half a minute), standard output at the end.

use crate::cli::{emit, Run};
use crate::cliparse::{
    format_usage, parse as parse_arguments, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use crate::memory::{
    checkpoint_bytes_for, gate, GateArguments, Host, LogChoice, Provider, DEFAULT_FLOOR_MIB,
};
use crate::memory_selftest::self_test;
use crate::memory_system::{startup_stdio, StdioAtStart};
use crate::model_cli::{uncaught, Streams};
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;

/// The places the program writes: [`Host`]'s standard error and callable log, and standard output.
pub trait CliHost: Host {
    /// `sys.stdout.write(text)`.
    fn write_stdout(&mut self, text: &PyStr);

    /// `print(text)`: the text, then its newline, as two writes.
    fn print_stdout(&mut self, text: &PyStr) {
        self.write_stdout(text);
        self.write_stdout(&PyStr::new("\n"));
    }

    /// What `argparse` does for a usage error (`error()`): `print_usage(sys.stderr)`, then `exit(2, "prog: error: ...")`, which writes
    /// the message to `sys.stderr` again. Both go through `_print_message`, which suppresses `AttributeError` and `OSError`. A
    /// `sys.stderr` that is `None` makes `print_usage` choose `sys.stdout` for the usage (written there as a plain write), while the
    /// message, written to `None`, is dropped; with neither stream nothing is written. Nothing here can fail or stop the program.
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr);

    /// `print_help()` (the `--help` action): written to `sys.stdout`; where that is `None`, `_print_message` falls back to
    /// `sys.stderr`; where both are `None`, nothing. Failures are suppressed.
    fn parser_help(&mut self, help: &PyStr);

    /// The report of an exception nothing handled, written by the interpreter itself after the program stopped (so, unlike a
    /// program write, never through anything the program redirected).
    fn write_uncaught(&mut self, text: &PyStr) {
        let _ = self.write_stderr(text);
    }
}

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
    group: Option<usize>,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group,
    }
}

/// The parser of the original `main`.
pub fn spec() -> Spec {
    Spec {
        prog: "preload_ram_gate.sh",
        description: "로드-전 가용 RAM 게이트 (plan_26071019 §2.6)",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, None),
            opt(
                &["--self-test"],
                "self_test",
                Kind::Flag,
                Some("mmap 오프로드 파생 자체검사"),
                None,
            ),
            opt(
                &["--model-host-path"],
                "model_host_path",
                Kind::Text,
                Some("모델 디렉토리(호스트 경로) — index total_size 산출"),
                Some(0),
            ),
            opt(
                &["--checkpoint-bytes"],
                "checkpoint_bytes",
                Kind::Int,
                Some("체크포인트 바이트 직접 지정"),
                Some(0),
            ),
            opt(
                &["--tp"],
                "tp",
                Kind::Int,
                Some("텐서 병렬 수(노드당 로드 ≈ 전체/tp)"),
                None,
            ),
            opt(
                &["--floor-mib"],
                "floor_mib",
                Kind::Int,
                Some("가산 안전마진(기본 10240=워치독 임계)"),
                None,
            ),
            opt(
                &["--no-drop"],
                "no_drop",
                Kind::Flag,
                Some("drop-caches 자동 실행 생략"),
                None,
            ),
        ],
        group_required: true,
        positional: None,
    }
}

fn integer_or(
    values: &std::collections::BTreeMap<&'static str, Value>,
    dest: &str,
    default: i64,
) -> PyValue {
    match values.get(dest) {
        Some(Value::Int(number)) => PyValue::Int(number.clone()),
        _ => PyValue::int(default),
    }
}

fn gate_command(
    host: &mut dyn CliHost,
    values: &std::collections::BTreeMap<&'static str, Value>,
) -> Res<i32> {
    let mut checkpoint = match values.get("checkpoint_bytes") {
        Some(Value::Int(number)) => PyValue::Int(number.clone()),
        _ => PyValue::None,
    };
    let model_path = match values.get("model_host_path") {
        Some(Value::Text(path)) if !path.is_empty() => Some(path),
        _ => None,
    };
    if let Some(path) = model_path {
        let mut warnings = PyValue::List(Vec::new());
        checkpoint = checkpoint_bytes_for(&PyValue::Str(path.clone()), Some(&mut warnings))?;
        if let PyValue::List(items) = &warnings {
            for item in items {
                let mut line = PyStr::new("[preload-gate] ⚠ ");
                line.push_py(&item.display_text());
                line.push_str("\n");
                host.write_stderr(&line)?;
            }
        }
    }
    let tp = integer_or(values, "tp", 1);
    let floor = integer_or(values, "floor_mib", DEFAULT_FLOOR_MIB);
    let result = gate(
        host,
        &GateArguments {
            checkpoint_bytes: &checkpoint,
            tp: &tp,
            floor_mib: &floor,
            auto_drop: &PyValue::Bool(!values.contains_key("no_drop")),
            log: LogChoice::Default,
        },
    )?;
    let field = |name: &str| result.get(name).cloned().unwrap_or(PyValue::None);
    let ok = field("ok").is_truthy();
    let mut line = PyStr::new(if ok {
        "[preload-gate] PASS required="
    } else {
        "[preload-gate] REFUSE required="
    });
    line.push_py(&field("required_mib").display_text());
    line.push_str("MiB avail=");
    line.push_py(&field("avail_before_mib").display_text());
    line.push_str("→");
    line.push_py(&field("avail_after_mib").display_text());
    line.push_str("MiB dropped=");
    line.push_py(&field("dropped").display_text());
    line.push_str("\n");
    host.write_stderr(&line)?;
    Ok(if ok { 0 } else { 7 })
}

/// `main()`: the exit status, or the exception it ends with. `skill_root` is the root of the skill the program belongs to (the
/// self-test looks for the patch under the repository three directories above it).
pub fn execute(
    host: &mut dyn CliHost,
    skill_root: &PyStr,
    args: &[PyStr],
    columns: Option<&str>,
) -> Res<i32> {
    if args.iter().any(|arg| arg.equals("--self-test")) {
        return self_test(host, skill_root);
    }
    match parse_arguments(&spec(), args, columns) {
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            if !stdout.is_empty() {
                host.parser_help(&stdout);
            }
            if !stderr.is_empty() {
                // The parser formats usage and error as one text; the two routes are told apart by the usage the same formatter
                // generates (never by looking for a newline, which the error text may contain).
                let usage = PyStr::new(&format_usage(&spec(), terminal_width(columns)));
                let (usage, message) = split_usage(&usage, &stderr);
                host.parser_error(&usage, &message);
            }
            Ok(status)
        }
        Outcome::Values(values) => gate_command(host, &values),
    }
}

/// The usage the parser put first in `report`, and the rest; no usage at the start means no usage part.
pub(crate) fn split_usage(usage: &PyStr, report: &PyStr) -> (PyStr, PyStr) {
    let (head, all) = (usage.code_points(), report.code_points());
    if all.starts_with(head) {
        (
            usage.clone(),
            PyStr::from_code_points(all[head.len()..].to_vec()),
        )
    } else {
        (PyStr::default(), report.clone())
    }
}

/// The final `Class: message` block of an exception nothing handled, as it ends the run on standard error.
pub fn uncaught_block(failure: &Failure) -> PyStr {
    uncaught(Streams::default(), failure).stderr
}

/// `main()` as a process runs it: the exit status, with an exception nothing handles reported and turned into status 1.
pub fn run(
    host: &mut dyn CliHost,
    skill_root: &PyStr,
    args: &[PyStr],
    columns: Option<&str>,
) -> i32 {
    match execute(host, skill_root, args, columns) {
        Ok(status) => status,
        Err(failure) => {
            host.write_uncaught(&uncaught_block(&failure));
            1
        }
    }
}

/// `sys.stderr.write(text)`: the whole text (unencodable characters as `backslashreplace`) goes to `sink` at once, and a write
/// that fails is the `OSError` it raises, returned here and never swallowed: the caller (the gate's default log, the argument parser,
/// the warnings) stops at the failed write exactly where the original's `print` raised. Bytes a partial write already delivered stay
/// delivered; nothing is buffered or replayed.
pub fn write_text_to(sink: &mut dyn Write, text: &PyStr) -> Res<()> {
    sink.write_all(&text.to_backslashreplace_bytes())
        .map_err(|error| crate::stdio::os_failure(&error))
}

/// [`write_text_to`] the real standard error.
pub fn write_real_stderr(text: &PyStr) -> Res<()> {
    write_text_to(&mut std::io::stderr(), text)
}

/// `print(text)` onto the standard-output stream for a text that already ends with its newline (`print` writes the text and then its
/// own newline, as two writes); any other text is a plain write.
pub fn print_to_stream(streams: &mut Streams, text: &PyStr) {
    if text.ends_with_code_point(u32::from('\n')) {
        let points = text.code_points();
        streams.print(&PyStr::from_code_points(
            points[..points.len() - 1].to_vec(),
        ));
    } else {
        streams.write(text);
    }
}

/// The program's own host: standard error live, standard output kept for the end. Where the process started without a standard error
/// the program's `print(..., file=sys.stderr)` goes to the standard output stream, in order with the rest of it, and where it
/// started without either it goes nowhere (`sys.stderr` and `sys.stdout` are `None`); neither is ever reported as an error.
pub struct ProcessHost<'a, P: Provider> {
    provider: &'a mut P,
    streams: Streams,
    start: StdioAtStart,
}

impl<P: Provider> Provider for ProcessHost<'_, P> {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        self.provider.mem_available_mib()
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        self.provider.try_drop_caches()
    }
}

impl<P: Provider> Host for ProcessHost<'_, P> {
    /// `print(text, file=sys.stderr)`: standard error, else (it was closed at start) the standard output stream, else nowhere.
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        if self.start.stderr_open {
            write_real_stderr(text)
        } else {
            if self.start.stdout_open {
                print_to_stream(&mut self.streams, text);
            }
            Ok(())
        }
    }

    fn call_log(&mut self, _message: &PyStr) -> Res<()> {
        Err(Failure::new(
            "RuntimeError",
            "the command line holds no callable log",
        ))
    }
}

impl<P: Provider> CliHost for ProcessHost<'_, P> {
    fn write_stdout(&mut self, text: &PyStr) {
        self.streams.write(text);
    }

    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        if self.start.stderr_open {
            for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
                let _ = write_real_stderr(text);
            }
        } else if self.start.stdout_open && !usage.is_empty() {
            self.streams.write(usage);
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        if self.start.stdout_open {
            self.streams.write(help);
        } else if self.start.stderr_open {
            let _ = write_real_stderr(help);
        }
    }

    fn write_uncaught(&mut self, text: &PyStr) {
        if self.start.stderr_open {
            let _ = write_real_stderr(text);
        }
    }
}

/// Runs the program against `provider` and hands what it printed to standard output; the final exit status (a standard output
/// that cannot be written changes it, as it does in Python).
pub fn run_process<P: Provider>(
    provider: &mut P,
    skill_root: &PyStr,
    args: &[PyStr],
    columns: Option<&str>,
) -> i32 {
    let mut host = ProcessHost {
        provider,
        streams: Streams::default(),
        start: startup_stdio(),
    };
    let status = run(&mut host, skill_root, args, columns);
    let run: Run = host.streams.finish(status);
    emit(&run)
}
