//! `main()` of `classify_failure.py` over `sys.argv`, `sys.stdin` and the real files: the argument parser, the log read (`--log` file with
//! `errors="replace"`, or standard input), the registry read, the first-match search over the registry in order and the JSON report
//! (`ensure_ascii=False`, `indent=2`), ending in the status the source gives (0 for `requirements-fixable`, 1 for any other matched class, 2 for no
//! match).
//!
//! Outcomes this port does not decide are returned as a named [`Refusal`] (status 70, a report on standard error) rather than approximated: a lone
//! surrogate in the text, an invalid-escape `DeprecationWarning` of a signature (visible for the script run as `__main__`), a pattern error whose
//! traceback site was not captured, a `\N{name}` escape, and any non-decoded `PYTHONIOENCODING`.
//!
//! The default registry path is the source's `os.path.join(os.path.dirname(__file__), "..", "failure_patterns.yaml")` computed from this process's
//! `argv[0]` (the script path the legacy entry hands over), so the `scripts/..` segment stays literal.

use crate::classify_source::{load_patterns, matched_lines, SourceFailure};
use crate::classify_traceback::{block_report, report, Site};
use crate::host::Sink;
use crate::http_direct::PyException;
use crate::main_cmd::{argument_error, basename, intern, split_usage};
use crate::py_errors::{builtin, os_error_with_filename};
use crate::py_regex_engine::{compile, EngineFailure, PatternSource};
use crate::py_regex_parse::ParseFailure;
use crate::runtime_patch::StdioProfile;
use crate::thrown::Done;
use crate::torch_main::Refusal;
use crate::wheel_main::ascii_encode_error;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStringExt;

pub const PROGRAM: &str = "classify_failure.py";
const UNKNOWN_NOTE: &str =
    "알려진 패턴 미매칭 → Model-C: LLM 이 {proposed_class, evidence} 제시 후 사람 승인.";

/// What a run is on.
pub struct ClassifyWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub profile: StdioProfile,
    /// All bytes of standard input (read only when `--log` is absent or empty).
    pub read_stdin: &'a dyn Fn() -> std::io::Result<Vec<u8>>,
}

pub enum Stop {
    Exit(i32),
    /// A report already in its final text (an uncaught exception's traceback, a failed argparse write).
    Rendered(PyStr),
    Refused(Refusal),
}

type Flow<T> = Result<T, Stop>;

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Stop {
    Stop::Refused(Refusal {
        boundary,
        detail: detail.into(),
    })
}

/// The path the traceback frames name for the script: `argv[0]` when it ends in the script's name (made absolute against the working directory when
/// relative, without normalising), else the canonical name.
pub fn script_file(argv0: Option<&PyStr>) -> PyStr {
    let Some(path) = argv0.filter(|path| basename(path) == PROGRAM) else {
        return PyStr::new(PROGRAM);
    };
    let is_relative = path.code_points().first() != Some(&u32::from('/'));
    match std::env::current_dir() {
        Ok(cwd) if is_relative => {
            use std::os::unix::ffi::OsStrExt;
            let mut absolute = PyStr::from_surrogateescape(cwd.as_os_str().as_bytes());
            absolute.push_str("/");
            absolute.push_py(path);
            absolute
        }
        _ => path.clone(),
    }
}

/// `os.path.join(os.path.dirname(script), "..", "failure_patterns.yaml")`.
pub fn default_patterns_path(script: &PyStr) -> PyStr {
    let points = script.code_points();
    let slash = u32::from('/');
    let head = match points.iter().rposition(|&c| c == slash) {
        Some(at) => {
            let mut head = &points[..=at];
            // dirname strips trailing slashes unless the head is all slashes
            if head.iter().any(|&c| c != slash) {
                while head.len() > 1 && head[head.len() - 1] == slash {
                    head = &head[..head.len() - 1];
                }
            }
            head.to_vec()
        }
        None => Vec::new(),
    };
    let mut path = PyStr::from_code_points(head.clone());
    if !head.is_empty() && head.last() != Some(&slash) {
        path.push_str("/");
    }
    path.push_str("../failure_patterns.yaml");
    path
}

fn spec(prog: &str, default_patterns: &PyStr) -> Spec {
    let _ = default_patterns;
    let option = |flags: &'static [&'static str], dest, kind, help| Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group: None,
    };
    Spec {
        prog: intern(prog),
        description: "",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None),
            option(
                &["--log"],
                "log",
                Kind::Text,
                Some("로그 파일(미지정 시 stdin)"),
            ),
            option(&["--patterns"], "patterns", Kind::Text, None),
        ],
        group_required: false,
        positional: None,
    }
}

struct Arguments {
    log: Option<PyStr>,
    patterns: PyStr,
}

fn without_extra_blank_line(help: &PyStr) -> PyStr {
    let points = help.code_points();
    let quadruple: Vec<u32> = "\n\n\n\n".chars().map(u32::from).collect();
    match points
        .windows(4)
        .position(|window| window == quadruple.as_slice())
    {
        Some(at) => {
            let mut fixed = points[..at + 2].to_vec();
            fixed.extend_from_slice(&points[at + 4..]);
            PyStr::from_code_points(fixed)
        }
        None => help.clone(),
    }
}

fn parse_arguments(world: &mut ClassifyWorld<'_>, default_patterns: &PyStr) -> Flow<Arguments> {
    let program = world
        .argv
        .first()
        .map_or_else(|| PROGRAM.to_string(), basename);
    let spec = spec(&program, default_patterns);
    let usage = PyStr::new(&format_usage(&spec, terminal_width(world.columns)));
    let values: BTreeMap<&'static str, Value>;
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(found) => {
            values = found;
            let text = |dest: &str| match values.get(dest) {
                Some(Value::Text(text)) => Some(text.clone()),
                _ => None,
            };
            return Ok(Arguments {
                log: text("log"),
                patterns: text("patterns").unwrap_or_else(|| default_patterns.clone()),
            });
        }
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let (head, rest) = split_usage(&usage, &stderr);
            (status, stdout, stderr, head, rest)
        }
    };
    let mut message = None;
    if !stdout.is_empty() {
        world.host.parser_help(&without_extra_blank_line(&stdout));
    }
    if !stderr.is_empty() || !rest.is_empty() {
        world.host.parser_error(&head, &rest);
        message = Some(rest);
    }
    if let Some(mut failure) = world.host.take_parser_failure() {
        if failure.raised.cause.is_none() {
            if let Some(context) = message.as_ref().and_then(argument_error) {
                failure.raised.cause = Some(Box::new(context));
                failure.raised.via_context = true;
            }
        }
        return Err(Stop::Rendered(failure.raised.report()));
    }
    Err(Stop::Exit(status))
}

/// Universal newlines: `\r\n` and `\r` become `\n`.
fn universal_newlines(points: &[u32]) -> PyStr {
    let mut out = Vec::with_capacity(points.len());
    let mut index = 0;
    while index < points.len() {
        if points[index] == 13 {
            out.push(10);
            if points.get(index + 1) == Some(&10) {
                index += 1;
            }
        } else {
            out.push(points[index]);
        }
        index += 1;
    }
    PyStr::from_code_points(out)
}

fn path_bytes(name: &PyStr) -> Flow<std::path::PathBuf> {
    let bytes = name.to_surrogateescape_bytes().map_err(|_| {
        refusal(
            "path_encoding",
            "the path cannot be encoded for the file system",
        )
    })?;
    Ok(std::path::PathBuf::from(std::ffi::OsString::from_vec(
        bytes,
    )))
}

/// `open(a.log, encoding="utf-8", errors="replace").read()`.
fn read_log(name: &PyStr, script_path: &PyStr) -> Flow<PyStr> {
    let path = path_bytes(name)?;
    let failed = |error: &std::io::Error| match os_error_with_filename(error, name) {
        Some(exception) => Stop::Rendered(report(&exception, Site::OpenLog, script_path)),
        None => refusal("open_failure", error.to_string()),
    };
    let metadata = std::fs::metadata(&path);
    let bytes = match std::fs::File::open(&path) {
        Err(error) => return Err(failed(&error)),
        Ok(file) => {
            if metadata.map(|m| m.is_dir()).unwrap_or(false) {
                return Err(failed(&std::io::Error::from_raw_os_error(21)));
            }
            use std::io::Read;
            let mut bytes = Vec::new();
            let mut file = file;
            file.read_to_end(&mut bytes)
                .map_err(|error| refusal("read_failure", error.to_string()))?;
            bytes
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let points: Vec<u32> = text.chars().map(u32::from).collect();
    Ok(universal_newlines(&points))
}

/// `sys.stdin.read()`.
fn read_stdin(world: &ClassifyWorld<'_>) -> Flow<PyStr> {
    let bytes = (world.read_stdin)().map_err(|error| refusal("stdin_read", error.to_string()))?;
    if world.profile == StdioProfile::AsciiStrict && bytes.iter().any(|&b| b > 127) {
        return Err(refusal(
            "ascii_stdin_decode",
            "a non-ASCII byte on standard input under PYTHONIOENCODING=ascii:strict raises a UnicodeDecodeError whose traceback was not captured",
        ));
    }
    let text = PyStr::from_surrogateescape(&bytes);
    Ok(universal_newlines(text.code_points()))
}

fn refused_by_source(failure: SourceFailure, site: Site, script_path: &PyStr) -> Stop {
    match failure {
        SourceFailure::Raised(error) => {
            // only OSError sites of the registry open and the escape decode have captured or derived frames
            let known = matches!(error.class, "FileNotFoundError" | "IsADirectoryError" | "PermissionError" | "NotADirectoryError" | "OSError")
                && site == Site::OpenPatterns
                || error.class == "UnicodeDecodeError" && site == Site::EscapeDecode && error.attributes.iter().any(|(n, v)| {
                    *n == "encoding" && matches!(v, crate::http_direct::ErrorValue::Plain(PyValue::Str(s)) if s.equals("unicodeescape"))
                });
            if known {
                Stop::Rendered(report(&error, site, script_path))
            } else {
                refusal(
                    "registry_error_traceback_site",
                    format!(
                        "{}: {} raised while reading the registry; its traceback site was not captured",
                        error.class,
                        error.message.to_utf8().unwrap_or_default()
                    ),
                )
            }
        }
        SourceFailure::OutOfScope { boundary, detail } => refusal(boundary, detail),
    }
}

fn json_text(text: &PyStr) -> PyStr {
    let mut out = PyStr::default();
    text.write_json(&mut out, false);
    out
}

fn is_line_break(point: u32) -> bool {
    matches!(
        point,
        0x0A | 0x0B | 0x0C | 0x0D | 0x1C | 0x1D | 0x1E | 0x85 | 0x2028 | 0x2029
    )
}

/// `print(text)` of the report, with the text layer's encoding profile.
fn print_report(
    world: &mut ClassifyWorld<'_>,
    text: &PyStr,
    site: Site,
    script_path: &PyStr,
) -> Flow<()> {
    if world.profile == StdioProfile::AsciiStrict {
        if let Some(block) = ascii_encode_error(text) {
            return Err(Stop::Rendered(block_report(&block, site, script_path)));
        }
    }
    let written: Done<()> = world
        .host
        .out(text)
        .and_then(|()| world.host.out(&PyStr::new("\n")));
    written.map_err(|failure| {
        Stop::Rendered(block_report(&failure.raised.report(), site, script_path))
    })
}

fn object_text(entries: &[(&str, &PyStr)]) -> PyStr {
    let mut out = PyStr::new("{\n");
    for (index, (key, value)) in entries.iter().enumerate() {
        out.push_str(&format!("  \"{key}\": "));
        out.push_py(&json_text(value));
        out.push_str(if index + 1 < entries.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    out.push_str("}");
    out
}

pub fn main(world: &mut ClassifyWorld<'_>) -> Flow<()> {
    let script_path = script_file(world.argv.first());
    let default_patterns = default_patterns_path(&script_path);
    let arguments = parse_arguments(world, &default_patterns)?;
    let text = match arguments.log.as_ref().filter(|log| !log.is_empty()) {
        Some(log) => read_log(log, &script_path)?,
        None => read_stdin(world)?,
    };
    let loaded = load_patterns(&PyValue::Str(arguments.patterns.clone())).map_err(|failure| {
        let site = match &failure {
            SourceFailure::Raised(error) if error.class == "UnicodeDecodeError" => {
                Site::EscapeDecode
            }
            _ => Site::OpenPatterns,
        };
        refused_by_source(failure, site, &script_path)
    })?;
    if let Some(escape) = &loaded.first_invalid_escape {
        return Err(refusal(
            "invalid_escape_warning",
            format!(
                "a signature holds an invalid escape ({escape:?}); the DeprecationWarning of the script run as __main__ is not reproduced"
            ),
        ));
    }
    let class_key = PyStr::new("class");
    for record in &loaded.patterns {
        let Some(PyValue::Str(signature)) = record.get("signature") else {
            continue;
        };
        let compiled =
            compile(PatternSource::Text(signature)).map_err(|failure| match failure {
                EngineFailure::Parse(ParseFailure::Re(error))
                    if error.sub_depth == 1 && error.site != 0 =>
                {
                    Stop::Rendered(report(
                        &crate::classify_source::re_error_exception(&error),
                        Site::Parse {
                            raise_line: error.site,
                        },
                        &script_path,
                    ))
                }
                EngineFailure::Parse(ParseFailure::Re(error)) => refusal(
                    "re_error_traceback_site",
                    format!(
                    "re.error {:?} was raised at a parser site whose traceback was not captured",
                    error.msg.to_utf8().unwrap_or_default()
                ),
                ),
                EngineFailure::Parse(ParseFailure::Overflow(message))
                | EngineFailure::Parse(ParseFailure::Value(message)) => {
                    refusal("re_error_traceback_site", message)
                }
                EngineFailure::Parse(ParseFailure::Unsupported { boundary, detail })
                | EngineFailure::Unsupported { boundary, detail } => refusal(boundary, detail),
            })?;
        let first = compiled
            .finditer_text(&text)
            .and_then(|mut matches| matches.next_span())
            .map_err(|failure| match failure {
                EngineFailure::Unsupported { boundary, detail }
                | EngineFailure::Parse(ParseFailure::Unsupported { boundary, detail }) => {
                    refusal(boundary, detail)
                }
                other => refusal("search_failure", format!("{other:?}")),
            })?;
        let Some((start, end)) = first else { continue };
        let Some(PyValue::Str(class)) = record.get("class") else {
            let mut error = builtin("KeyError", "'class'");
            error.args = vec![PyValue::str("class").into()];
            let _ = &class_key;
            return Err(Stop::Rendered(report(&error, Site::ClassKey, &script_path)));
        };
        let note = match record.get("note") {
            Some(PyValue::Str(note)) => note.clone(),
            _ => PyStr::new(""),
        };
        let evidence = matched_lines(
            &PyValue::Str(text.clone()),
            &PyValue::Str(signature.clone()),
            &PyValue::int(5),
            &PyValue::int(1000),
        )
        .map_err(|failure| match failure {
            SourceFailure::OutOfScope { boundary, detail } => refusal(boundary, detail),
            SourceFailure::Raised(error) => refusal(
                "matched_lines_failure",
                format!(
                    "{}: {}",
                    error.class,
                    error.message.to_utf8().unwrap_or_default()
                ),
            ),
        })?;
        let evidence = match evidence.value {
            PyValue::Str(lines) if !lines.is_empty() => lines,
            _ => {
                let stop = end.min(start + 200);
                PyStr::from_code_points(text.code_points()[start..stop].to_vec())
            }
        };
        let signature_text = signature.clone();
        let out = object_text(&[
            ("class", class),
            ("matched_signature", &signature_text),
            ("note", &note),
            ("evidence", &evidence),
        ]);
        print_report(world, &out, Site::PrintMatched, &script_path)?;
        return Err(Stop::Exit(if class.equals("requirements-fixable") {
            0
        } else {
            1
        }));
    }
    let evidence = if text.strip().is_empty() {
        PyStr::new("")
    } else {
        let stripped = text.strip();
        let points = stripped.code_points();
        let from = points
            .iter()
            .rposition(|&c| is_line_break(c))
            .map_or(0, |at| at + 1);
        let last = &points[from..];
        PyStr::from_code_points(last[..last.len().min(200)].to_vec())
    };
    let class = PyStr::new("unknown");
    let note = PyStr::new(UNKNOWN_NOTE);
    let out = object_text(&[("class", &class), ("note", &note), ("evidence", &evidence)]);
    print_report(world, &out, Site::PrintUnknown, &script_path)?;
    Err(Stop::Exit(2))
}

#[allow(dead_code)]
fn unused(_: PyException) {}
