//! Command line of `classify_cell`: only the original program's argparse interface ([`crate::argv::CLASSIFY_CELL`]).
//! Usage, help and argument-error text is byte-for-byte the original's, including the logical program name
//! `classify_cell.py`; nothing Rust-only is added (that is the separate `classify_cell_api`).
//!
//! The two modes the original has, in the original order of checks, reads and writes:
//!
//! * **cell-closing** (`--serve-rc`, `--measure-rc`, `--started-utc`, `--ended-utc`): the cell's outcome as JSON;
//! * **bench-mode** (`--sweep-index`): the sweep's `bench_mode` as JSON, optionally written next to the index
//!   (`--write-bench-mode`) with the atomic write of [`crate::classify_cell::write_bench_mode_record`].
//!
//! Exits: 0 = classification produced; 2 = argument or input error (message on stderr); an uncaught exception ends
//! with a traceback (only its final `Class: message` line is reproduced) and status 1. Warnings written while working
//! come before whatever follows them, as in the original.

use crate::argv::{self, classify_args as arg, Outcome, CLASSIFY_CELL};
use crate::classify_cell::{
    bench_mode_record_path, classify, classify_bench_mode, correlate_nodes, events_node_plan,
    events_scan_summary, parse_utc, path_segment, read_cell_lite, read_events, scan_node_events,
    write_bench_mode_record, BenchModeRequest, Diagnostics, Moment, GENERATED_BY,
};
use crate::classify_cell_owner::Owners;
use crate::classify_cell_selftest::run_self_test;
use crate::cli::{on_large_stack, print_failed, write_stderr, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::ospath;
use crate::pydyn::{dict_of, or_none, Text};
use crate::pyfs::os_string;
use crate::pyjson::{decode_file_text, loads_text, LoadError};
use crate::pynumber::int_from_str;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::terminal;
use num_bigint::BigInt;

/// What the heavy part of a run hands back to the thread that owns the streams: the lines `print` is called with, in
/// order, then the stderr text and the status.
struct Finished {
    prints: Vec<PyStr>,
    stderr: PyStr,
    exit_code: i32,
}

/// The parsed command line, as plain values (the worker thread owns them).
struct CliArgs {
    serve_rc: Option<BigInt>,
    measure_rc: Option<PyStr>,
    started_utc: Option<PyStr>,
    ended_utc: Option<PyStr>,
    events: Vec<PyStr>,
    events_from_repo: Option<PyStr>,
    topology: Option<PyStr>,
    manifest: Option<PyStr>,
    tolerance_s: BigInt,
    cross_node_tolerance_s: Option<BigInt>,
    cross_node_tolerance_source: Option<PyStr>,
    sweep_index: Option<PyStr>,
    write_bench_mode: bool,
    lite_raw: Option<PyStr>,
}

impl CliArgs {
    fn from_matches(matches: &argv::Matches) -> Self {
        let text = |action: usize| matches.value(action).cloned();
        CliArgs {
            serve_rc: matches.int(arg::SERVE_RC).cloned(),
            measure_rc: text(arg::MEASURE_RC),
            started_utc: text(arg::STARTED_UTC),
            ended_utc: text(arg::ENDED_UTC),
            events: matches.values_of(arg::EVENTS).to_vec(),
            events_from_repo: text(arg::EVENTS_FROM_REPO),
            topology: text(arg::TOPOLOGY),
            manifest: text(arg::MANIFEST),
            tolerance_s: matches
                .int(arg::TOLERANCE_S)
                .cloned()
                .unwrap_or_else(|| BigInt::from(0)),
            cross_node_tolerance_s: matches.int(arg::CROSS_NODE_TOLERANCE_S).cloned(),
            cross_node_tolerance_source: text(arg::CROSS_NODE_TOLERANCE_SOURCE),
            sweep_index: text(arg::SWEEP_INDEX),
            write_bench_mode: matches.flag(arg::WRITE_BENCH_MODE),
            lite_raw: text(arg::LITE_RAW),
        }
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], owners: &Owners, io: &mut Streams<'_>) -> i32 {
    let status = run_legacy(args, owners, io);
    io.finish(status)
}

fn run_legacy(args: &[PyStr], owners: &Owners, io: &mut Streams<'_>) -> i32 {
    let matches = match argv::parse_with(&CLASSIFY_CELL, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(CLASSIFY_CELL.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                CLASSIFY_CELL.usage(terminal::columns()),
                CLASSIFY_CELL.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    if matches.flag(arg::SELF_TEST) {
        let report = on_large_stack(|| run_self_test(owners));
        return match io.print_lines(&PyStr::new(&report.stdout)) {
            Ok(()) => {
                write_stderr(io, &PyStr::new(&report.stderr));
                report.exit_code
            }
            Err(failure) => print_failed(io, &failure, &report.stderr),
        };
    }
    let parsed = CliArgs::from_matches(&matches);
    let owners_for_worker = owners.clone();
    let finished = on_large_stack(move || process(&parsed, &owners_for_worker));
    for line in &finished.prints {
        if let Err(failure) = io.print(line) {
            let so_far = finished.stderr.to_utf8().unwrap_or_default();
            return print_failed(io, &failure, &so_far);
        }
    }
    write_stderr(io, &finished.stderr);
    finished.exit_code
}

/// The part of `main` after the argument parse.
fn process(args: &CliArgs, owners: &Owners) -> Finished {
    let mut diagnostics = Diagnostics::default();
    match program(args, owners, &mut diagnostics) {
        Ok((prints, exit_code)) => Finished {
            prints,
            stderr: diagnostics.stderr,
            exit_code,
        },
        Err(failure) => {
            let mut stderr = diagnostics.stderr;
            stderr.push_str(TRACEBACK_HEADER);
            stderr.push_py(&failure.line());
            stderr.push_str("\n");
            Finished {
                prints: Vec::new(),
                stderr,
                exit_code: 1,
            }
        }
    }
}

fn error_line(diagnostics: &mut Diagnostics, message: &str) -> Result<(Vec<PyStr>, i32), Failure> {
    error_text(diagnostics, &PyStr::new(message))
}

/// `sys.stderr.write("[classify_cell] ERROR %s\n" % message)` and exit 2.
fn error_text(
    diagnostics: &mut Diagnostics,
    message: &PyStr,
) -> Result<(Vec<PyStr>, i32), Failure> {
    diagnostics.write(
        &Text::new()
            .lit("[classify_cell] ERROR ")
            .str(message)
            .lit("\n")
            .done(),
    );
    Ok((Vec::new(), 2))
}

fn truthy_text(text: &Option<PyStr>) -> bool {
    text.as_ref().is_some_and(|value| !value.is_empty())
}

fn text_or_none(text: &Option<PyStr>) -> PyValue {
    text.clone().map_or(PyValue::None, PyValue::Str)
}

/// `print(json.dumps(value, ensure_ascii=False, indent=2))`.
fn json_print(value: &PyValue) -> Result<PyStr, Failure> {
    value.dumps_pretty().map_err(|error| {
        let rendered = error.to_string();
        Failure::new(
            error.python_type(),
            rendered
                .strip_prefix(&format!("{}: ", error.python_type()))
                .unwrap_or(&rendered),
        )
    })
}

fn program(
    args: &CliArgs,
    owners: &Owners,
    diagnostics: &mut Diagnostics,
) -> Result<(Vec<PyStr>, i32), Failure> {
    let sweep_index = truthy_text(&args.sweep_index);
    let cell_args: Vec<&str> = [
        ("--serve-rc", args.serve_rc.is_some()),
        ("--measure-rc", args.measure_rc.is_some()),
        ("--started-utc", args.started_utc.is_some()),
        ("--ended-utc", args.ended_utc.is_some()),
    ]
    .into_iter()
    .filter(|(_, given)| *given)
    .map(|(name, _)| name)
    .collect();
    if args.write_bench_mode && !sweep_index {
        return error_line(
            diagnostics,
            "--write-bench-mode 는 --sweep-index 가 필요하다",
        );
    }
    let source_given = args
        .cross_node_tolerance_source
        .as_ref()
        .is_some_and(|source| !source.strip().is_empty());
    if args.cross_node_tolerance_s.is_none() == source_given {
        return error_line(
            diagnostics,
            "--cross-node-tolerance-s 와 --cross-node-tolerance-source 는 함께 준다 — 노드를 넘는 허용오차는 출처 없는 숫자로 두지 않는다",
        );
    }
    if args
        .cross_node_tolerance_s
        .as_ref()
        .is_some_and(|seconds| seconds < &BigInt::from(0))
    {
        return error_line(diagnostics, "--cross-node-tolerance-s 는 0 이상이다");
    }
    let cross_s = args
        .cross_node_tolerance_s
        .clone()
        .map_or(PyValue::None, PyValue::Int);
    let cross_source = match args.cross_node_tolerance_source.as_ref().map(PyStr::strip) {
        Some(stripped) if !stripped.is_empty() => PyValue::Str(stripped),
        _ => PyValue::None,
    };
    let tolerance = PyValue::Int(args.tolerance_s.clone());

    // `gather(plan)`
    let gather = |plan: &PyValue, diagnostics: &mut Diagnostics| -> Result<Vec<PyValue>, Failure> {
        let mut scans = if matches!(plan.peel(), PyValue::None) {
            Vec::new()
        } else {
            match scan_node_events(&text_or_none(&args.events_from_repo), plan, diagnostics)? {
                PyValue::List(items) => items,
                _ => Vec::new(),
            }
        };
        if !args.events.is_empty() {
            let event_paths: Vec<PyValue> = args.events.iter().cloned().map(PyValue::Str).collect();
            let (lines, read) = read_events(&event_paths, diagnostics)?;
            let mut scan = PyDict::new();
            scan.set("node", PyValue::str("explicit"));
            scan.set("remote", PyValue::Bool(false));
            scan.set("files", PyValue::List(read));
            scan.set("lines", PyValue::List(lines));
            scan.set("repo", PyValue::None);
            scans.push(PyValue::Dict(scan));
        }
        Ok(scans)
    };
    // `_scan_desc(scans, plan)`
    let scan_desc = |scans: &[PyValue]| -> (Vec<PyValue>, PyStr) {
        let files: Vec<PyValue> = scans
            .iter()
            .filter_map(|scan| dict_of(scan).and_then(|map| map.get("files")))
            .flat_map(|files| match files.peel() {
                PyValue::List(items) => items.clone(),
                _ => Vec::new(),
            })
            .collect();
        let mut text = PyStr::default();
        for (index, file) in files.iter().enumerate() {
            if index > 0 {
                text.push_str(",");
            }
            text.push_py(&file.display_text());
        }
        if files.is_empty() {
            text = PyStr::new("none");
        }
        (files, text)
    };

    // ── bench-mode mode ──────────────────────────────────────────────────────────
    if sweep_index {
        let index_path = args.sweep_index.clone().expect("checked");
        if !cell_args.is_empty() {
            return error_line(
                diagnostics,
                &format!(
                    "--sweep-index(bench-mode 모드)와 셀 종결 인자({})를 섞지 않는다 — 셀 기록은 판정 기록({})을 읽는다",
                    cell_args.join(", "),
                    crate::classify_cell::BENCH_MODE_RECORD_NAME
                ),
            );
        }
        if args.topology.is_some() {
            return error_line(
                diagnostics,
                "bench-mode 에서는 --topology 를 받지 않는다 — 측정의 토폴로지는 sweep_index meta.topology 가 말한다(측정 기록과 인자가 갈리면 어느 쪽도 고르지 않는다)",
            );
        }
        let index_doc = match read_json_file(&index_path) {
            Ok(doc) => doc,
            Err(ReadFailure::Unreadable(why)) => {
                return error_text(
                    diagnostics,
                    &Text::new()
                        .lit("sweep-index 판독 실패(")
                        .str(&index_path)
                        .lit("): ")
                        .str(&why)
                        .done(),
                );
            }
            Err(ReadFailure::Raised(failure)) => return Err(failure),
        };
        if dict_of(&index_doc).is_none() {
            return error_text(
                diagnostics,
                &Text::new()
                    .lit("sweep-index 가 객체가 아니다(")
                    .str(&index_path)
                    .lit(")")
                    .done(),
            );
        }
        let index_map = dict_of(&index_doc).expect("checked");
        let mut plan = PyValue::None;
        if truthy_text(&args.events_from_repo) {
            let meta_value = or_none(index_map.get("meta"));
            let empty = PyDict::new();
            let meta = dict_of(&meta_value).unwrap_or(&empty);
            let topology = or_none(meta.get("topology"));
            let repo = args.events_from_repo.clone().expect("checked");
            let manifest = match &args.manifest {
                Some(manifest) if !manifest.is_empty() => PyValue::Str(manifest.clone()),
                _ if path_segment(&topology) => {
                    let PyValue::Str(topology_text) = topology.peel() else {
                        unreachable!("a path segment is a string")
                    };
                    PyValue::Str(ospath::join_all(
                        &repo,
                        &[
                            "output",
                            &topology_text.to_utf8().unwrap_or_default(),
                            "manifest.yaml",
                        ],
                    ))
                }
                _ => PyValue::None,
            };
            let made = events_node_plan(
                &topology,
                &or_none(meta.get("measured_node")),
                &or_none(meta.get("measured_node_source")),
                &manifest,
                &PyValue::Str(repo),
                owners,
            )?;
            if let Some(reason) = dict_of(&made).and_then(|map| map.get("undeterminable")) {
                if reason.is_truthy() {
                    let map = dict_of(&made).expect("a dict");
                    let node = match map.get("self_node") {
                        Some(node) if node.is_truthy() => node.display_text(),
                        _ => Text::new()
                            .lit("미해소(")
                            .val(&or_none(map.get("self_node_source")))
                            .lit(")")
                            .done(),
                    };
                    diagnostics.write(
                        &Text::new()
                            .lit("[classify_cell] WARN 대조 대상 노드를 정하지 못했다 — ")
                            .val(reason)
                            .lit(" (이 노드 ")
                            .str(&node)
                            .lit(" 의 기록만 대조하고 나머지는 not_scanned 로 기록된다 · 대조 불가는 강등 트리거가 아니다)\n")
                            .done(),
                    );
                }
            }
            plan = made;
        }
        let scans = gather(&plan, diagnostics)?;
        let (files, _) = scan_desc(&scans);
        let lines: Vec<PyValue> = scans
            .iter()
            .filter_map(|scan| dict_of(scan).and_then(|map| map.get("lines")))
            .flat_map(|lines| match lines.peel() {
                PyValue::List(items) => items.clone(),
                _ => Vec::new(),
            })
            .collect();
        let events_scanned = PyValue::List(files.clone());
        let node_scans = PyValue::List(scans);
        let lines_value = PyValue::List(lines);
        let decided = classify_bench_mode(&BenchModeRequest {
            index: &index_doc,
            event_lines: &lines_value,
            events_scanned: &events_scanned,
            tolerance_s: &tolerance,
            node_scans: &node_scans,
            plan: &plan,
            cross_node_tolerance_s: &cross_s,
            cross_node_tolerance_source: &cross_source,
        })?;
        let PyValue::Dict(mut record) = decided else {
            unreachable!("a bench-mode record is a dict")
        };
        record.set("schema_version", PyValue::int(1));
        record.set("kind", PyValue::str("bench_mode_record"));
        record.set("generated_by", PyValue::str(GENERATED_BY));
        record.set("sweep_index", PyValue::Str(index_path.clone()));
        record.set(
            "sweep_index_generated_utc",
            or_none(index_map.get("generated_utc")),
        );
        record.set("events_scanned", events_scanned);
        record.set("tolerance_s", tolerance);
        let mut record = PyValue::Dict(record);
        if args.write_bench_mode {
            match write_bench_mode_record(&PyValue::Str(index_path.clone()), &record) {
                Ok(path) => {
                    if let PyValue::Dict(map) = &mut record {
                        map.set("record_path", PyValue::Str(path));
                    }
                }
                Err(failure) if is_os_error(&failure) => {
                    let target = bench_mode_record_path(&PyValue::Str(index_path.clone()))?;
                    diagnostics.write(
                        &Text::new()
                            .lit("[classify_cell] ERROR bench_mode 기록 실패(")
                            .str(&target)
                            .lit("): ")
                            .str(&failure.message)
                            .lit("\n")
                            .done(),
                    );
                    return Ok((vec![json_print(&record)?], 2));
                }
                Err(failure) => return Err(failure),
            }
        }
        return Ok((vec![json_print(&record)?], 0));
    }

    // ── cell-closing mode ────────────────────────────────────────────────────────
    let measure_rc: PyValue = match &args.measure_rc {
        Some(text) if text.equals("absent") || text.is_empty() => PyValue::None,
        Some(text) => match int_from_str(text) {
            Ok(number) => PyValue::Int(number),
            Err(_) => {
                return error_line(diagnostics, "--measure-rc 는 정수 또는 `absent`");
            }
        },
        None => {
            return error_line(
                diagnostics,
                "--measure-rc 미지정 — 부재는 `absent` 로 **명시**하라(빠뜨림과 부재를 구분한다)",
            );
        }
    };
    let missing: Vec<&str> = [
        ("--serve-rc", args.serve_rc.is_none()),
        ("--started-utc", args.started_utc.is_none()),
        ("--ended-utc", args.ended_utc.is_none()),
    ]
    .into_iter()
    .filter(|(_, absent)| *absent)
    .map(|(name, _)| name)
    .collect();
    if !missing.is_empty() {
        return error_line(
            diagnostics,
            &format!("필수 인자 부재: {}", missing.join(", ")),
        );
    }
    let mut plan = PyValue::None;
    if truthy_text(&args.events_from_repo) {
        let Some(topology) = &args.topology else {
            return error_line(
                diagnostics,
                "셀 종결 모드의 --events-from-repo 는 --topology 가 필요하다 — 대조 대상 노드를 정하지 않고 저장소의 모든 노드 events 를 대조하지 않는다",
            );
        };
        let repo = args.events_from_repo.clone().expect("checked");
        let manifest = match &args.manifest {
            Some(manifest) if !manifest.is_empty() => manifest.clone(),
            _ => ospath::join_all(
                &repo,
                &[
                    "output",
                    &topology.to_utf8().unwrap_or_default(),
                    "manifest.yaml",
                ],
            ),
        };
        let made = events_node_plan(
            &PyValue::Str(topology.clone()),
            &PyValue::None,
            &PyValue::None,
            &PyValue::Str(manifest),
            &PyValue::Str(repo),
            owners,
        )?;
        if let Some(map) = dict_of(&made) {
            if let Some(reason) = map
                .get("undeterminable")
                .filter(|reason| reason.is_truthy())
            {
                let node = match map.get("self_node") {
                    Some(node) if node.is_truthy() => node.display_text(),
                    _ => Text::new()
                        .lit("미해소(")
                        .val(&or_none(map.get("self_node_source")))
                        .lit(")")
                        .done(),
                };
                diagnostics.write(
                    &Text::new()
                        .lit("[classify_cell] WARN 대조 대상 노드를 정하지 못했다 — ")
                        .val(reason)
                        .lit(" (이 노드 ")
                        .str(&node)
                        .lit(" 의 기록만 대조하고 나머지는 not-scanned 로 기록된다)\n")
                        .done(),
                );
            }
        }
        plan = made;
    }
    let scans = gather(&plan, diagnostics)?;
    let (files, scanned_text) = scan_desc(&scans);
    let started_text = PyValue::Str(args.started_utc.clone().expect("checked"));
    let ended_text = PyValue::Str(args.ended_utc.clone().expect("checked"));
    let bounds = parse_utc(&started_text, "--started-utc")
        .and_then(|started| parse_utc(&ended_text, "--ended-utc").map(|ended| (started, ended)));
    let (started, ended) = match bounds {
        Ok(bounds) => bounds,
        Err(failure) if failure.class == "ClassifyError" => {
            return error_text(diagnostics, &failure.message);
        }
        Err(failure) => return Err(failure),
    };
    let (hits, correlation, per_node) = correlate_nodes(
        &PyValue::List(scans),
        Moment::At(started),
        Moment::At(ended),
        &tolerance,
        &cross_s,
        &cross_source,
    )?;
    let plan_for_doc = if matches!(plan.peel(), PyValue::None) {
        let mut explicit = PyDict::new();
        explicit.set(
            "source",
            PyValue::str("explicit(호출자가 대상 events 를 선언했다 — 노드 계획 없음)"),
        );
        PyValue::Dict(explicit)
    } else {
        plan.clone()
    };
    let mut scan_doc = PyDict::new();
    scan_doc.set("plan", plan_for_doc);
    scan_doc.set("nodes", PyValue::List(per_node));
    scan_doc.set("cross_node_tolerance_s", cross_s.clone());
    scan_doc.set("cross_node_tolerance_source", cross_source.clone());
    let scan_doc = PyValue::Dict(scan_doc);
    let mut scanned_text = scanned_text;
    if !matches!(plan.peel(), PyValue::None) {
        scanned_text = Text::new()
            .str(&scanned_text)
            .lit(" · ")
            .val(&events_scan_summary(&scan_doc)?)
            .done();
    }
    let lite = read_cell_lite(&text_or_none(&args.lite_raw), Some(Moment::At(started)))?;
    let out = classify(
        &PyValue::Int(args.serve_rc.clone().expect("checked")),
        &measure_rc,
        &PyValue::List(hits),
        &PyValue::Str(scanned_text),
        &PyValue::str(correlation),
        &lite,
    )?;
    let PyValue::Dict(mut out) = out else {
        unreachable!("a classification is a dict")
    };
    if let PyValue::Dict(lite_map) = &lite {
        for (key, value) in lite_map.iter() {
            out.insert(key.clone(), value.clone());
        }
    }
    out.set("events_scanned", PyValue::List(files));
    out.set("tolerance_s", tolerance);
    out.set("downgrade_correlation", PyValue::str(correlation));
    out.set("events_scan", scan_doc);
    Ok((vec![json_print(&PyValue::Dict(out))?], 0))
}

/// An `OSError` of the original (an `os_error_class` family name).
fn is_os_error(failure: &Failure) -> bool {
    matches!(
        failure.class,
        "OSError"
            | "FileNotFoundError"
            | "PermissionError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "FileExistsError"
            | "BlockingIOError"
            | "InterruptedError"
            | "ChildProcessError"
            | "ConnectionError"
            | "ProcessLookupError"
            | "TimeoutError"
    )
}

enum ReadFailure {
    /// `OSError` or `ValueError` (the original's `except (OSError, ValueError)`): the text of `str(exception)`.
    Unreadable(PyStr),
    /// An exception the original does not catch.
    Raised(Failure),
}

/// `with open(path, encoding="utf-8") as handle: json.load(handle)`.
fn read_json_file(path: &PyStr) -> Result<PyValue, ReadFailure> {
    let os = match os_string(path) {
        Ok(os) => os,
        Err(error) => {
            let failure = error.into_failure();
            return Err(ReadFailure::Unreadable(failure.message));
        }
    };
    let bytes = std::fs::read(std::path::Path::new(&os)).map_err(|error| {
        ReadFailure::Unreadable(PyStr::new(&crate::cli::os_error_text(&error, path)))
    })?;
    let text = decode_file_text(&bytes)
        .map_err(|error| ReadFailure::Unreadable(PyStr::new(&error.to_string())))?;
    match loads_text(&text) {
        Ok(value) => Ok(value),
        Err(error @ LoadError::TooDeep { .. }) => {
            let rendered = error.to_string();
            Err(ReadFailure::Raised(Failure::new(
                "RecursionError",
                rendered
                    .strip_prefix("RecursionError: ")
                    .unwrap_or(&rendered),
            )))
        }
        Err(error) => Err(ReadFailure::Unreadable(PyStr::new(&error.to_string()))),
    }
}
