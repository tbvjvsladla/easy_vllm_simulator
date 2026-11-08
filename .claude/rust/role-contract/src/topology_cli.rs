//! The command line of `topology_parity.py` (`main`, its `argparse` parser, `--self-test`) over [`crate::topology`].
//!
//! The parser is the project's own model of the two `argparse` parsers the original builds (a main parser with
//! `--self-test` and a required-by-use subcommand `evaluate`, whose options are `--repo`, `--expect {single,multi}` and
//! `--format {json,value}`): unique-prefix abbreviations, `--opt=value`, the `error:` texts and exit status 2, `-h` output.
//! The terminal width of `argparse` help is the 80-column default (a `COLUMNS` override is not modelled).
//!
//! stdout is encoded by the codec the locale names (UTF-8 or ASCII with strict errors, so a non-ASCII report under the
//! plain C locale is the original's `UnicodeEncodeError` crash); stderr always uses `backslashreplace`, as Python does.
//! A [`Crash`] prints as a Python traceback made of logical frames (source metadata, never a native stack).

use crate::pyprofile::{encode_stream, fs_decode, with_profile, Errors, TextProfile};
use crate::topology::{self, Crash, Detail, Frame, Where};
use crate::topology_doc::DESCRIPTION;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_CONTRACT_VIOLATION: i32 = 5;
const PROG: &str = "topology_parity.py";

/// What the process leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// The process environment the original's output depends on.
#[derive(Debug, Clone)]
pub struct Environment {
    /// The Python text profile of the process environment (stdout/stderr codec, file-system and preferred encodings).
    pub profile: TextProfile,
    /// The logical path of the original owner script (shown in traceback frames; metadata, never read or run).
    pub source_owner: Vec<u8>,
    /// `TMPDIR` for the self-test's temporary directories (`/tmp` when unset).
    pub tmpdir: Option<Vec<u8>>,
}

// ── argparse model ──────────────────────────────────────────────────────────────

const MAIN_USAGE: &str = "usage: topology_parity.py [-h] [--self-test] {evaluate} ...\n";
const EVAL_USAGE: &str = "usage: topology_parity.py evaluate [-h] [--repo REPO]\n                                   [--expect {single,multi}]\n                                   [--format {json,value}]\n";
const EVAL_HELP_TAIL: &str = "\noptions:\n  -h, --help            show this help message and exit\n  --repo REPO\n  --expect {single,multi}\n                        호출부가 기대하는 토폴로지(어긋나면 RED)\n  --format {json,value}\n";

fn main_help() -> String {
    format!(
        "{MAIN_USAGE}\n{DESCRIPTION}\npositional arguments:\n  {{evaluate}}\n    evaluate   4자일치 판정\n\noptions:\n  -h, --help   show this help message and exit\n  --self-test  헤더 파서 자체검사\n"
    )
}

fn eval_help() -> String {
    format!("{EVAL_USAGE}{EVAL_HELP_TAIL}")
}

enum Parsed {
    Help(String),
    Error {
        usage: &'static str,
        prog: &'static str,
        message: String,
    },
    Run {
        self_test: bool,
        command: Option<String>,
        repo: Vec<u8>,
        expect: Option<String>,
        value_format: bool,
    },
}

fn text(bytes: &[u8]) -> PyStr {
    fs_decode(bytes)
}

fn repr(bytes: &[u8]) -> String {
    text(bytes).repr()
}

fn negative_number(arg: &[u8]) -> bool {
    arg.len() > 1
        && arg[0] == b'-'
        && arg[1..].iter().all(|b| b.is_ascii_digit() || *b == b'.')
        && arg[1..].iter().any(u8::is_ascii_digit)
}

/// argparse's "looks like an option" (`O`) test for a value position.
fn option_like(arg: &[u8]) -> bool {
    arg.len() > 1 && arg[0] == b'-' && !negative_number(arg) && !arg.contains(&b' ')
}

/// A unique match of `arg` (a `--long` option, abbreviated or not, without `=value`) among `candidates`.
fn match_long<'a>(arg: &[u8], candidates: &[&'a str]) -> Option<&'a str> {
    if arg.starts_with(b"--") {
        let exact: Vec<&&str> = candidates.iter().filter(|c| c.as_bytes() == arg).collect();
        if exact.len() == 1 {
            return Some(exact[0]);
        }
        let prefixed: Vec<&&str> = candidates
            .iter()
            .filter(|c| c.as_bytes().starts_with(arg))
            .collect();
        if prefixed.len() == 1 {
            return Some(prefixed[0]);
        }
        return None;
    }
    candidates.iter().find(|c| c.as_bytes() == arg).copied()
}

fn parse(args: &[Vec<u8>]) -> Parsed {
    let mut self_test = false;
    let mut extras: Vec<Vec<u8>> = Vec::new();
    let mut index = 0;
    let mut command: Option<String> = None;
    let mut sub_args: &[Vec<u8>] = &[];
    while index < args.len() {
        let arg = &args[index];
        if arg == b"--" {
            index += 1;
            if let Some(next) = args.get(index) {
                if next != b"evaluate" {
                    return Parsed::Error {
                        usage: MAIN_USAGE,
                        prog: PROG,
                        message: format!(
                            "argument cmd: invalid choice: {} (choose from 'evaluate')",
                            repr(next)
                        ),
                    };
                }
                command = Some("evaluate".to_string());
                sub_args = &args[index + 1..];
            }
            break;
        }
        if option_like(arg) {
            match match_long(arg, &["-h", "--help", "--self-test"]) {
                Some("-h") | Some("--help") => return Parsed::Help(main_help()),
                Some("--self-test") => self_test = true,
                _ => extras.push(arg.clone()),
            }
            index += 1;
            continue;
        }
        if arg != b"evaluate" {
            return Parsed::Error {
                usage: MAIN_USAGE,
                prog: PROG,
                message: format!(
                    "argument cmd: invalid choice: {} (choose from 'evaluate')",
                    repr(arg)
                ),
            };
        }
        command = Some("evaluate".to_string());
        sub_args = &args[index + 1..];
        break;
    }
    let (mut repo, mut expect, mut value_format) = (b".".to_vec(), None, false);
    if command.is_some() {
        let long = ["--help", "--repo", "--expect", "--format"];
        let mut i = 0;
        while i < sub_args.len() {
            let arg = &sub_args[i];
            if arg == b"--" {
                extras.extend(sub_args[i + 1..].iter().cloned());
                break;
            }
            if !option_like(arg) {
                extras.push(arg.clone());
                i += 1;
                continue;
            }
            let (name_part, inline): (&[u8], Option<&[u8]>) =
                match arg.iter().position(|b| *b == b'=') {
                    Some(at) if arg.starts_with(b"--") => (&arg[..at], Some(&arg[at + 1..])),
                    _ => (&arg[..], None),
                };
            let matched = if name_part == b"-h" {
                Some("--help")
            } else {
                match_long(name_part, &long)
            };
            let Some(option) = matched else {
                extras.push(arg.clone());
                i += 1;
                continue;
            };
            if option == "--help" {
                return Parsed::Help(eval_help());
            }
            let value: Vec<u8> = match inline {
                Some(value) => value.to_vec(),
                None => match sub_args.get(i + 1) {
                    Some(next) if !option_like(next) && next != b"--" => {
                        i += 1;
                        next.clone()
                    }
                    _ => {
                        return Parsed::Error {
                            usage: EVAL_USAGE,
                            prog: "topology_parity.py evaluate",
                            message: format!("argument {option}: expected one argument"),
                        };
                    }
                },
            };
            match option {
                "--repo" => repo = value,
                "--expect" => {
                    if value != b"single" && value != b"multi" {
                        return Parsed::Error { usage: EVAL_USAGE, prog: "topology_parity.py evaluate", message: format!("argument --expect: invalid choice: {} (choose from 'single', 'multi')", repr(&value)) };
                    }
                    expect = Some(String::from_utf8_lossy(&value).to_string());
                }
                _ => {
                    if value != b"json" && value != b"value" {
                        return Parsed::Error { usage: EVAL_USAGE, prog: "topology_parity.py evaluate", message: format!("argument --format: invalid choice: {} (choose from 'json', 'value')", repr(&value)) };
                    }
                    value_format = value == b"value";
                }
            }
            i += 1;
        }
    }
    if !extras.is_empty() {
        let shown: Vec<String> = extras
            .iter()
            .map(|e| text(e).to_utf8().unwrap_or_else(|| text(e).repr()))
            .collect();
        return Parsed::Error {
            usage: MAIN_USAGE,
            prog: PROG,
            message: format!("unrecognized arguments: {}", shown.join(" ")),
        };
    }
    Parsed::Run {
        self_test,
        command,
        repo,
        expect,
        value_format,
    }
}

// ── encodings ───────────────────────────────────────────────────────────────────

/// `print(text)` to stdout: the profile's codec and error handler; an unencodable character is the original's crash.
fn encode_stdout(out: &PyStr, profile: &TextProfile) -> Result<Vec<u8>, Crash> {
    encode_stream(out, profile.stdout, profile.stdout_errors).map_err(|failure| {
        Crash::new(
            "UnicodeEncodeError",
            &PyStr::new(&failure.message),
            Where::Print,
        )
        .with_detail(Detail::Encode {
            encoding: failure.encoding,
            object: out.clone(),
            start: failure.start,
            end: failure.end,
            reason: failure.reason.clone(),
        })
    })
}

/// stderr: the stdout encoding with `backslashreplace` (never fails).
fn encode_stderr(out: &PyStr, profile: &TextProfile) -> Vec<u8> {
    encode_stream(out, profile.stdout, Errors::BackslashReplace).unwrap_or_default()
}

// ── traceback ───────────────────────────────────────────────────────────────────

/// One traceback frame as Python 3.12 prints it: the `File` line, the stripped source line and, when the marked expression is
/// only part of that line, the caret line. (Exposed unchanged for other Python-compatible command lines.)
pub fn push_frame(out: &mut PyStr, frame: &Frame) {
    out.push_py(&PyStr::new("  File \""));
    out.push_py(&text(&frame.file));
    out.push_py(&PyStr::new(&format!(
        "\", line {}, in {}\n",
        frame.line, frame.function
    )));
    if frame.code.is_empty() {
        return;
    }
    out.push_py(&PyStr::new(&format!("    {}\n", frame.code)));
    if frame.expr.is_empty() || frame.expr == frame.code {
        return;
    }
    if let Some(at) = frame.code.find(frame.expr) {
        let start = frame.code[..at].chars().count();
        let width = frame.expr.chars().count();
        out.push_py(&PyStr::new(&format!(
            "    {}{}\n",
            " ".repeat(start),
            "^".repeat(width)
        )));
    }
}

/// The traceback text of a crash: logical frames, then `Class: message`.
pub fn format_traceback(crash: &Crash, env: &Environment) -> PyStr {
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    let owner = |line, function, code, expr| Frame {
        file: env.source_owner.clone(),
        line,
        function,
        code,
        expr,
    };
    let module = owner(397, "<module>", "sys.exit(main())", "main()");
    let main_run = owner(
        384,
        "main",
        "res = evaluate(args.repo, expect=args.expect)",
        "evaluate(args.repo, expect=args.expect)",
    );
    let mut frames: Vec<Frame> = Vec::new();
    match &crash.site {
        Where::Opaque => {
            out.push_py(&PyStr::new(
                "  (native: Python stack frames are not reproduced)\n",
            ));
        }
        Where::Print => frames.extend([
            module,
            owner(
                392,
                "main",
                "print(json.dumps(res, ensure_ascii=False, indent=2, sort_keys=True))",
                "print(json.dumps(res, ensure_ascii=False, indent=2, sort_keys=True))",
            ),
        ]),
        Where::Evaluate { line, code, expr } => {
            frames.extend([module, main_run, owner(*line, "evaluate", code, expr)])
        }
        Where::GitStderr {
            call_line,
            call_code,
            call_expr,
        } => {
            frames.extend([
                module,
                main_run,
                owner(*call_line, "evaluate", call_code, call_expr),
            ]);
            frames.push(owner(
                87,
                "_git",
                "proc = subprocess.run([\"git\", \"-C\", str(repo), *args],",
                "subprocess.run([\"git\", \"-C\", str(repo), *args],",
            ));
            frames.extend(topology::subprocess_frames());
        }
        Where::Campaign {
            module: module_file,
            frames: below,
        } => {
            if below.is_empty() {
                out.push_py(&PyStr::new(
                    "  (native: Python stack frames are not reproduced)\n",
                ));
            } else {
                frames.extend([
                    module,
                    main_run,
                    owner(
                        262,
                        "evaluate",
                        "camp_id = mod.active_campaign_id(str(root))",
                        "mod.active_campaign_id(str(root))",
                    ),
                ]);
                frames.push(Frame {
                    file: module_file.clone(),
                    line: 113,
                    function: "active_campaign_id",
                    code: "name = pointer.read_text(encoding=\"utf-8\").strip()",
                    expr: "pointer.read_text(encoding=\"utf-8\")",
                });
                frames.extend(below.iter().cloned());
            }
        }
    }
    for frame in &frames {
        push_frame(&mut out, frame);
    }
    out.push_py(&PyStr::new(&format!("{}: ", crash.class)));
    out.push_py(&crash.message);
    out.push_str("\n");
    out
}

// ── JSON ────────────────────────────────────────────────────────────────────────

/// `sort_keys=True`: dict keys in code-point order, recursively. A dict whose keys are not all strings has no JSON key order
/// here (the original's `TypeError` for mixed keys): a named boundary.
fn sort_keys(value: &PyValue) -> Result<PyValue, String> {
    Ok(match value.peel() {
        PyValue::Dict(dict) => {
            if dict
                .keyed_entries()
                .any(|(non_string, _, _)| non_string.is_some())
            {
                return Err("a dict with non-string keys in the report is outside the modelled JSON key order".to_string());
            }
            let mut entries: Vec<&(PyStr, PyValue)> = dict.iter().collect();
            entries.sort_by(|a, b| a.0.code_points().cmp(b.0.code_points()));
            let mut sorted = PyDict::new();
            for (key, inner) in entries {
                sorted.insert(key.clone(), sort_keys(inner)?);
            }
            PyValue::Dict(sorted)
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(sort_keys).collect::<Result<_, _>>()?)
        }
        PyValue::Tuple(items) => {
            PyValue::Tuple(items.iter().map(sort_keys).collect::<Result<_, _>>()?)
        }
        other => other.clone(),
    })
}

fn report_text(report: &PyValue) -> Result<PyStr, Crash> {
    let sorted = sort_keys(report)
        .map_err(|why| Crash::new("TypeError", &PyStr::new(&why), Where::Opaque))?;
    sorted.dumps_pretty().map_err(|error| {
        Crash::new(
            "TypeError",
            &PyStr::new(&format!("{error:?}")),
            Where::Opaque,
        )
    })
}

// ── self test ───────────────────────────────────────────────────────────────────

fn temp_directory(env: &Environment, tag: u32) -> Vec<u8> {
    let base = env
        .tmpdir
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| b"/tmp".to_vec());
    let seed = std::process::id() as u64
        ^ (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            << 3)
        ^ (tag as u64);
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut state = seed | 1;
    let mut name = b"tmp".to_vec();
    for _ in 0..8 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        name.push(alphabet[(state % alphabet.len() as u64) as usize]);
    }
    crate::campaign::join(&base, &name)
}

/// `_self_test()`: the header parser against throwaway files in a private temporary directory (and the git enumeration of a
/// non-git directory), then the PASS line on stderr. Nothing of the checkout is read.
fn self_test(env: &Environment) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let path = |b: &[u8]| std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b));
    let cleanup = |dir: &[u8]| {
        let _ = std::fs::remove_dir_all(path(dir));
    };
    let dir = temp_directory(env, 1);
    std::fs::create_dir(path(&dir)).map_err(|e| e.to_string())?;
    let outcome = (|| -> Result<(), String> {
        let file = |name: &str, body: &str| -> Result<Vec<u8>, String> {
            let p = crate::campaign::join(&dir, name.as_bytes());
            std::fs::write(path(&p), body).map_err(|e| e.to_string())?;
            Ok(p)
        };
        let check = |name: &str, ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(format!("[topology_parity] self-test FAIL: {name}"))
            }
        };
        let good = file(
            "good.topology.md",
            "# t\n\n**topology: multi** · layer: topology\n\n> 본문\n",
        )?;
        check(
            "양성: 선언을 읽는다",
            topology::read_layer_header(&good).0 == Some("multi"),
        )?;
        let bad = file("bad.topology.md", "# t\n\n선언이 없다\n")?;
        check(
            "음성: 선언 부재",
            topology::read_layer_header(&bad).0.is_none(),
        )?;
        let dup = file(
            "dup.topology.md",
            "**topology: multi**\n**topology: single**\n",
        )?;
        let dup_res = topology::read_layer_header(&dup);
        check(
            "음성: 선언 2개는 모호하므로 거부",
            dup_res.0.is_none() && dup_res.1.to_utf8().unwrap_or_default().contains("2개"),
        )?;
        let late = file(
            "late.topology.md",
            &format!(
                "{}**topology: single**\n",
                "\n".repeat(topology::HEADER_WINDOW)
            ),
        )?;
        check(
            "음성: 창 밖 선언은 읽지 않는다",
            topology::read_layer_header(&late).0.is_none(),
        )?;
        let loose = file("loose.topology.md", "topology: multi\n")?;
        check(
            "음성: 닫힌 형식만 받는다",
            topology::read_layer_header(&loose).0.is_none(),
        )?;
        let empty = file("empty.topology.md", "")?;
        check(
            "음성: 빈 파일",
            topology::read_layer_header(&empty).0.is_none(),
        )?;
        Ok(())
    })();
    cleanup(&dir);
    outcome?;
    let plain = temp_directory(env, 2);
    std::fs::create_dir(path(&plain)).map_err(|e| e.to_string())?;
    let listed = topology::layer_files(&plain);
    cleanup(&plain);
    match listed {
        Ok(files) if files.is_empty() => Ok(()),
        _ => Err("[topology_parity] self-test FAIL: 비-git 트리에서 열거는 빈 목록".to_string()),
    }
}

// ── main ────────────────────────────────────────────────────────────────────────

fn error_output(usage: &str, prog: &str, message: &str) -> CliOutput {
    CliOutput {
        status: EXIT_USAGE,
        stdout: Vec::new(),
        stderr: format!("{usage}{prog}: error: {message}\n").into_bytes(),
    }
}

/// `main(argv)`: exactly the owner CLI's streams and status for `args` (the arguments after the program name).
pub fn run(args: &[Vec<u8>], env: &Environment) -> CliOutput {
    with_profile(&env.profile, || run_in_profile(args, env))
}

fn run_in_profile(args: &[Vec<u8>], env: &Environment) -> CliOutput {
    let crash_output = |crash: Crash| {
        if matches!(*crash.detail, Detail::SourceUnmatched) {
            // not an exception of the original: the helper's own unmet condition (the callers route before this is reached)
            return CliOutput {
                status: 2,
                stdout: Vec::new(),
                stderr: encode_stderr(
                    &PyStr::new(&format!(
                        "topology_parity: source version compatibility is required ({})\n",
                        crash.message.to_utf8().unwrap_or_default()
                    )),
                    &env.profile,
                ),
            };
        }
        CliOutput {
            status: 1,
            stdout: Vec::new(),
            stderr: encode_stderr(&format_traceback(&crash, env), &env.profile),
        }
    };
    let (self_test_flag, command, repo, expect, value_format) = match parse(args) {
        Parsed::Help(text) => {
            return CliOutput {
                status: EXIT_OK,
                stdout: text.into_bytes(),
                stderr: Vec::new(),
            }
        }
        Parsed::Error {
            usage,
            prog,
            message,
        } => return error_output(usage, prog, &message),
        Parsed::Run {
            self_test,
            command,
            repo,
            expect,
            value_format,
        } => (self_test, command, repo, expect, value_format),
    };
    if self_test_flag {
        return match self_test(env) {
            Ok(()) => CliOutput {
                status: EXIT_OK,
                stdout: Vec::new(),
                stderr: encode_stderr(
                    &PyStr::new("[topology_parity] self-test PASS (6 헤더 케이스 + 열거 1)\n"),
                    &env.profile,
                ),
            },
            Err(message) => CliOutput {
                status: 1,
                stdout: Vec::new(),
                stderr: encode_stderr(&PyStr::new(&format!("{message}\n")), &env.profile),
            },
        };
    }
    if command.as_deref() != Some("evaluate") {
        return CliOutput {
            status: EXIT_USAGE,
            stdout: Vec::new(),
            stderr: MAIN_USAGE.as_bytes().to_vec(),
        };
    }
    let report = match topology::evaluate(&repo, expect.as_deref()) {
        Ok(report) => report,
        Err(crash) => return crash_output(crash),
    };
    let field = |name: &str| match report.peel() {
        PyValue::Dict(d) => d.get(name).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    let passed = matches!(field("verdict").peel(), PyValue::Str(t) if t.equals("PASS"));
    if value_format {
        let topology_value = field("topology");
        if !passed || !topology_value.is_truthy() {
            let mut err = PyStr::default();
            if let PyValue::List(reasons) = field("reasons").peel() {
                for reason in reasons {
                    err.push_str("[topology_parity] ");
                    err.push_py(&reason.display_text());
                    err.push_str("\n");
                }
            }
            return CliOutput {
                status: EXIT_CONTRACT_VIOLATION,
                stdout: Vec::new(),
                stderr: encode_stderr(&err, &env.profile),
            };
        }
        let mut line = topology_value.display_text();
        line.push_str("\n");
        return match encode_stdout(&line, &env.profile) {
            Ok(stdout) => CliOutput {
                status: EXIT_OK,
                stdout,
                stderr: Vec::new(),
            },
            Err(crash) => crash_output(crash),
        };
    }
    let mut body = match report_text(&report) {
        Ok(body) => body,
        Err(crash) => return crash_output(crash),
    };
    body.push_str("\n");
    match encode_stdout(&body, &env.profile) {
        Ok(stdout) => CliOutput {
            status: if passed {
                EXIT_OK
            } else {
                EXIT_CONTRACT_VIOLATION
            },
            stdout,
            stderr: Vec::new(),
        },
        Err(crash) => crash_output(crash),
    }
}

/// The repository an invocation would evaluate: `Some(repo)` when `args` parse to a plain `evaluate` run (not help, not a usage
/// error, not `--self-test`), `None` otherwise. Used to decide the source-version route before anything runs.
pub fn evaluate_target(args: &[Vec<u8>]) -> Option<Vec<u8>> {
    match parse(args) {
        Parsed::Run {
            self_test: false,
            command: Some(_),
            repo,
            ..
        } => Some(repo),
        _ => None,
    }
}
