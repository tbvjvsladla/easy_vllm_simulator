//! Command line of `verdict_rule`.
//!
//! The legacy surface is the original argparse interface ([`crate::argv::VERDICT_RULE`]) with its exit codes
//! (`0` verdict printed, `2` input or usage error, `1` uncaught exception). Rust-only positional words, which the
//! original rejected, add:
//!
//! * `evaluate-jsonl` - the API as a differential-test transport. Each stdin line is `{id, operation, ...}` with
//!   `operation` one of `rubric_candidate {raw}`, `build_verdict {measured, roofline, opts}` or `validate_cli
//!   {arguments}` (the self-test facade `_st_opts`: it appends `spec_supported=false` and `e_search="no"` after the
//!   validator's options); each answer line is `{id, kind: "value", value}` or `{id, kind: "exception", type,
//!   message}`.
//! * `spec-row --index PATH --measured PATH --human-value TEXT --level-rel TEXT` - the `SPEC_ROW` computation of
//!   `judge_bench.sh` (declaration, `accept_len`, provenance and evidence, joined by Unit Separator).
//! * `authority-check --state PATH` - the `rubric_authority` check of `broad_search.sh` (the value must be one of the
//!   authorities; printed when it is, exit 2 with the original's message when it is not).
//! * `native-help`.

use crate::argv::{self, verdict_args as arg, Outcome, VERDICT_RULE};
use crate::cli::{on_large_stack, print_failed, raise, read_path, write_stderr, Streams};
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pynumber::PyException;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::roofline::{
    accept_len_state, AcceptLenState, ACCEPT_LEN_ABSENT, ACCEPT_LEN_DECLARED,
    ACCEPT_LEN_DECLARED_ABSENT, ACCEPT_LEN_MEASURED,
};
use crate::terminal;
use crate::verdict::{
    build_verdict, get, rubric_candidate, validate_cli, CliArguments, SPEC_DECLARED,
};
use crate::verdict_selftest::run_self_test;

const NATIVE_HELP: &str = "verdict_rule Rust-only commands (the legacy options are unchanged; see -h):\n  verdict_rule evaluate-jsonl      JSON lines on stdin: {id, operation, ...} (API differential transport)\n  verdict_rule spec-row --index PATH --measured PATH --human-value TEXT --level-rel TEXT\n                                   the SPEC_ROW of judge_bench.sh (Unit Separator joined fields)\n  verdict_rule authority-check --state PATH\n                                   the rubric_authority check of broad_search.sh\n  verdict_rule native-help         this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let first = args.first().and_then(PyStr::to_utf8);
    match (first.as_deref(), args.len()) {
        (Some("evaluate-jsonl"), 1) => run_jsonl(io),
        (Some("spec-row"), _) => run_spec_row(&args[1..], io),
        (Some("authority-check"), _) => run_authority_check(&args[1..], io),
        (Some("native-help"), 1) => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        _ => run_legacy(args, io),
    }
}

fn load_error_text(error: &LoadError) -> String {
    let text = error.to_string();
    text.strip_prefix("RecursionError: ")
        .unwrap_or(&text)
        .to_string()
}

/// `_load(path, label)`: open + `json.load`, every failure becomes the message printed with exit 2.
fn load(path: &PyStr) -> Result<PyValue, String> {
    let bytes = read_path(path)?;
    let text = decode_file_text(&bytes).map_err(|error| load_error_text(&error))?;
    loads_text(&text).map_err(|error| load_error_text(&error))
}

fn run_legacy(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match argv::parse_with(&VERDICT_RULE, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(VERDICT_RULE.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                VERDICT_RULE.usage(terminal::columns()),
                VERDICT_RULE.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    if matches.flag(arg::SELF_TEST) {
        let report = run_self_test();
        if !report.stdout.is_empty() {
            if let Err(failure) = io.write_plain(&PyStr::new(&report.stdout)) {
                return print_failed(io, &failure, "");
            }
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    let measured_path = matches.value(arg::MEASURED).filter(|p| !p.is_empty());
    let roofline_path = matches.value(arg::ROOFLINE).filter(|p| !p.is_empty());
    let (Some(measured_path), Some(roofline_path)) = (measured_path, roofline_path) else {
        write_stderr(
            io,
            &PyStr::new("[verdict] ERROR --measured 와 --roofline 은 필수다(--self-test 제외)\n"),
        );
        return 2;
    };

    let text = |action: usize, default: &str| {
        PyValue::Str(
            matches
                .value(action)
                .cloned()
                .unwrap_or_else(|| PyStr::new(default)),
        )
    };
    let optional = |action: usize| matches.float(action).map_or(PyValue::None, PyValue::Float);
    let arguments = CliArguments {
        authority: text(arg::AUTHORITY, "weak"),
        reference_tps: optional(arg::REFERENCE_TPS),
        target_tps: optional(arg::TARGET_TPS),
        tolerance: PyValue::Float(matches.float(arg::TOLERANCE).unwrap_or(0.15)),
        node_vram_gib: matches
            .value(arg::NODE_VRAM_GIB)
            .cloned()
            .map_or(PyValue::None, PyValue::Str),
        balance_tol: PyValue::Float(matches.float(arg::BALANCE_TOL).unwrap_or(0.10)),
        spec_declared: text(arg::SPEC_DECLARED, "unknown"),
    };
    let (mut opts, errors) = match validate_cli(&arguments) {
        Ok(result) => result,
        Err(error) => return raise(io, "", &error),
    };
    if !errors.is_empty() {
        let mut text = PyStr::default();
        for message in &errors {
            text.push_str("[verdict] ERROR ");
            text.push_py(message);
            text.push_str("\n");
        }
        write_stderr(io, &text);
        return 2;
    }
    opts.set(
        "spec_supported",
        PyValue::Bool(matches.flag(arg::SPEC_SUPPORTED)),
    );
    opts.set("e_search", text(arg::E_SEARCH, "no"));

    let (measured_path, roofline_path) = (measured_path.clone(), roofline_path.clone());
    let finished = on_large_stack(
        move || -> Result<Result<PyStr, PyException>, (&'static str, String)> {
            let measured = load(&measured_path).map_err(|message| ("measured", message))?;
            let roofline = load(&roofline_path).map_err(|message| ("roofline", message))?;
            Ok(
                build_verdict(&measured, &roofline, &PyValue::Dict(opts)).and_then(|verdict| {
                    verdict.dumps_pretty().map_err(|error| {
                        let text = error.to_string();
                        let (class, message) =
                            text.split_once(": ").unwrap_or(("ValueError", &text));
                        PyException::new(
                            if class == "RecursionError" {
                                "RecursionError"
                            } else {
                                "ValueError"
                            },
                            message,
                        )
                    })
                }),
            )
        },
    );
    match finished {
        Err((label, message)) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[verdict] ERROR {label}: {message}\n")),
            );
            2
        }
        Ok(Err(error)) => raise(io, "", &error),
        Ok(Ok(json)) => match io.print(&json) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
    }
}

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

fn exception_record(id: PyValue, error: &PyException) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id);
    map.set("kind", PyValue::str("exception"));
    map.set("type", PyValue::str(error.class));
    map.set("message", PyValue::str(&error.message));
    PyValue::Dict(map)
}

fn value_record(id: PyValue, value: PyValue) -> PyValue {
    let mut map = PyDict::new();
    map.set("id", id);
    map.set("kind", PyValue::str("value"));
    map.set("value", value);
    PyValue::Dict(map)
}

/// The facade of the self-test helper `_st_opts` over an arguments object.
fn validate_facade(arguments: &PyValue) -> Result<PyValue, PyException> {
    let PyValue::Dict(given) = arguments else {
        return Err(PyException::new(
            "AttributeError",
            format!("'{}' object has no attribute 'pop'", arguments.type_name()),
        ));
    };
    let mut cli = CliArguments::default();
    let mut spec_supported = PyValue::Bool(false);
    let mut e_search = PyValue::str("no");
    for (key, value) in given.iter() {
        match key.to_utf8().as_deref() {
            Some("spec_supported") => spec_supported = value.clone(),
            Some("e_search") => e_search = value.clone(),
            Some("authority") => cli.authority = value.clone(),
            Some("reference_tps") => cli.reference_tps = value.clone(),
            Some("target_tps") => cli.target_tps = value.clone(),
            Some("tolerance") => cli.tolerance = value.clone(),
            Some("node_vram_gib") => cli.node_vram_gib = value.clone(),
            Some("balance_tol") => cli.balance_tol = value.clone(),
            Some("spec_declared") => cli.spec_declared = value.clone(),
            _ => {
                return Err(PyException::new(
                    "TypeError",
                    format!(
                        "_validate_cli() got an unexpected keyword argument {}",
                        key.repr()
                    ),
                ))
            }
        }
    }
    let (mut opts, errors) = validate_cli(&cli)?;
    opts.set("spec_supported", spec_supported);
    opts.set("e_search", e_search);
    Ok(PyValue::List(vec![
        PyValue::Dict(opts),
        PyValue::List(errors.into_iter().map(PyValue::Str).collect()),
    ]))
}

fn require<'a>(row: &'a PyValue, key: &str) -> Result<&'a PyValue, PyException> {
    match row {
        PyValue::Dict(map) => map
            .get(key)
            .ok_or_else(|| PyException::new("KeyError", PyStr::new(key).repr())),
        _ => Err(PyException::new(
            "TypeError",
            "operation row is not an object",
        )),
    }
}

fn evaluate_row(row: &PyValue) -> PyValue {
    let id = get(row, "id").cloned().unwrap_or(PyValue::None);
    let outcome = (|| -> Result<PyValue, PyException> {
        let operation = require(row, "operation")?;
        match operation.display_text().to_utf8().as_deref() {
            Some("rubric_candidate") => Ok(rubric_candidate(require(row, "raw")?)?.to_value()),
            Some("build_verdict") => build_verdict(
                require(row, "measured")?,
                require(row, "roofline")?,
                require(row, "opts")?,
            ),
            Some("validate_cli") => validate_facade(require(row, "arguments")?),
            _ => Err(PyException::new(
                "ValueError",
                format!("unknown operation {}", operation.repr()),
            )),
        }
    })();
    match outcome {
        Ok(value) => value_record(id, value),
        Err(error) => exception_record(id, &error),
    }
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[verdict] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[verdict] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match crate::pyjson::loads(line) {
                Ok(row) => evaluate_row(&row),
                Err(error) => exception_record(
                    PyValue::None,
                    &PyException::new("JSONDecodeError", error.to_string()),
                ),
            };
            match record.dumps_ascii() {
                Ok(json) => out.push_str(&json),
                Err(error) => out.push_str(
                    &exception_record(
                        PyValue::None,
                        &PyException::new("ValueError", error.to_string()),
                    )
                    .dumps_ascii()
                    .unwrap_or_default(),
                ),
            }
            out.push('\n');
        }
        out
    });
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure, ""),
    }
}

// ── spec-row ────────────────────────────────────────────────────────────────────

const SPEC_ROW_USAGE: &str = "usage: verdict_rule spec-row --index PATH --measured PATH --human-value TEXT --level-rel TEXT\n";

fn run_spec_row(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let mut values: [Option<PyStr>; 4] = [None, None, None, None];
    let names = ["--index", "--measured", "--human-value", "--level-rel"];
    let mut at = 0;
    while at < args.len() {
        let slot = args[at]
            .to_utf8()
            .and_then(|name| names.iter().position(|n| *n == name));
        match (slot, args.get(at + 1)) {
            (Some(slot), Some(value)) if values[slot].is_none() => {
                values[slot] = Some(value.clone());
                at += 2;
            }
            _ => {
                write_stderr(io, &PyStr::new(SPEC_ROW_USAGE));
                return 2;
            }
        }
    }
    let [Some(index), Some(measured), Some(human), Some(level_rel)] = values else {
        write_stderr(io, &PyStr::new(SPEC_ROW_USAGE));
        return 2;
    };
    match on_large_stack(move || spec_row(&index, &measured, &human, &level_rel)) {
        Ok(line) => match io.print(&line) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        Err(error) => raise(io, "", &error),
    }
}

/// `json.load(open(path, encoding="utf-8"))`: the same I/O, decoding and parsing semantics as every other reader of
/// the roofline module (exception class by errno, strict UTF-8, universal newlines).
fn json_document(path: &PyStr) -> Result<PyValue, PyException> {
    crate::roofline::json_file(path)
}

/// The `SPEC_ROW` of `judge_bench.sh`: `(declared, accept_len value, source, evidence)` joined by `\x1f`.
fn spec_row(
    index: &PyStr,
    measured: &PyStr,
    human: &PyStr,
    level_rel: &PyStr,
) -> Result<PyStr, PyException> {
    let document = json_document(index)?;
    let meta = get(&document, "meta")?;
    let meta = if meta.is_truthy() {
        meta.clone()
    } else {
        PyValue::Dict(PyDict::new())
    };
    let declared_raw = get(&meta, "spec_declared")?.clone();
    let declared = if SPEC_DECLARED
        .iter()
        .any(|name| matches!(&declared_raw, PyValue::Str(s) if s.equals(name)))
    {
        declared_raw.display_text()
    } else {
        PyStr::new("unknown")
    };
    let md = json_document(measured)?;
    let md_dict = matches!(md, PyValue::Dict(_));
    let raw = if md_dict {
        get(&md, "accept_len")?.clone()
    } else {
        PyValue::None
    };
    let state = accept_len_state(&raw)?;
    let axis_source = if md_dict {
        get(&md, "spec_axis_source")?.clone()
    } else {
        PyValue::None
    };
    let join = |parts: &[&PyStr]| {
        let mut out = PyStr::default();
        for part in parts {
            out.push_py(part);
        }
        out
    };
    let t = PyStr::new;
    let axis_note = if axis_source.is_truthy() {
        join(&[
            &t(" · measured.spec_axis_source="),
            &axis_source.display_text(),
        ])
    } else {
        PyStr::default()
    };
    let repr = |value: &PyValue| PyStr::new(&value.repr());
    let (value_text, source, evidence): (PyStr, &str, PyStr) = if !human.is_empty() {
        let mut evidence = t("argv(--accept-len)");
        if let AcceptLenState::Valid(valid) = &state {
            evidence = join(&[
                &evidence,
                &t(" · shadowed measured="),
                &repr(&PyValue::Float(*valid)),
                &t("("),
                level_rel,
                &t(")"),
            ]);
        }
        (human.clone(), ACCEPT_LEN_DECLARED, evidence)
    } else if let AcceptLenState::Valid(valid) = &state {
        (
            repr(&PyValue::Float(*valid)),
            ACCEPT_LEN_MEASURED,
            join(&[level_rel, &axis_note]),
        )
    } else if declared.equals("off") {
        let evidence = join(&[
            &t("sweep_index.meta.spec_declared=off("),
            &get(&meta, "spec_declared_source")?.display_text(),
            &t(") · "),
            level_rel,
            &t(" accept_len="),
            &repr(&raw),
        ]);
        (t("-"), ACCEPT_LEN_DECLARED_ABSENT, evidence)
    } else {
        let state_name = match state {
            AcceptLenState::Unset => "unset",
            _ => "invalid",
        };
        let evidence = join(&[
            level_rel,
            &t(" accept_len="),
            &repr(&raw),
            &t("("),
            &t(state_name),
            &t(")"),
            &axis_note,
        ]);
        (t("-"), ACCEPT_LEN_ABSENT, evidence)
    };
    let unit_separator = u32::from('\u{1f}');
    let mut out = PyStr::default();
    for (position, field) in [declared, value_text, PyStr::new(source), evidence]
        .iter()
        .enumerate()
    {
        if position > 0 {
            out.push_code_point(unit_separator);
        }
        for &point in field.code_points() {
            out.push_code_point(if point == unit_separator {
                u32::from(' ')
            } else {
                point
            });
        }
    }
    Ok(out)
}

// ── authority-check ─────────────────────────────────────────────────────────────

/// The snippet of `broad_search.sh`: `(json.load(state) or {}).get("rubric_authority")` must be an authority.
fn run_authority_check(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let [flag, path] = args else {
        write_stderr(
            io,
            &PyStr::new("usage: verdict_rule authority-check --state PATH\n"),
        );
        return 2;
    };
    if !flag.equals("--state") {
        write_stderr(
            io,
            &PyStr::new("usage: verdict_rule authority-check --state PATH\n"),
        );
        return 2;
    }
    let path = path.clone();
    let checked = on_large_stack(move || -> Result<Result<PyValue, PyStr>, PyException> {
        let document = json_document(&path)?;
        let document = if document.is_truthy() {
            document
        } else {
            PyValue::Dict(PyDict::new())
        };
        let authority = get(&document, "rubric_authority")?.clone();
        if crate::verdict::AUTHORITIES
            .iter()
            .any(|name| matches!(&authority, PyValue::Str(s) if s.equals(name)))
        {
            return Ok(Ok(authority));
        }
        let mut message = PyStr::new("[broad_search] ERROR 상태 파일의 rubric_authority=");
        message.push_str(&authority.repr());
        message.push_str(" 가 없거나 값역 ");
        message.push_str(&crate::verdict::AUTHORITIES.join("|"));
        message.push_str(" 밖이다 — 권한 없이 판정하지 않는다(기본값을 넣지 않는다)\n");
        Ok(Err(message))
    });
    match checked {
        Err(error) => raise(io, "", &error),
        Ok(Err(message)) => {
            write_stderr(io, &message);
            2
        }
        Ok(Ok(authority)) => match io.print(&authority.display_text()) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
    }
}
