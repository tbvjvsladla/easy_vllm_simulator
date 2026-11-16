//! The command lines of the five reporting programs, each with the original's argparse interface, outputs, exit
//! codes and failure timing:
//!
//! | program | does | exit |
//! |---|---|---|
//! | `render_report` | full or `--lite-only` performance report | 0 published, 2 input error |
//! | `render_sweep_map` | exploration map (`--self-test` runs R1–R18) | 0 published, 2 input error, 3 ranking found |
//! | `render_max_report` | Max-mode envelope report | 0 published, 2 input error |
//! | `publish_benchmark_record` | full or lite certificate | 0 published or not due, 2 input error, 3 lite not passed |
//! | `bench_doc_naming` | the naming owner's self-test (any argument is ignored, as in the original) | 0 pass |
//!
//! Output order follows the original: a document is written (`--stdout`: to standard output) *after* its name was
//! decided, a file is created before the text that fills it is encoded, and the campaign evidence registration of
//! the certificate runs after the certificate exists. Exceptions the original does not catch end the program with
//! the traceback marker and `Class: message` (status 1).

use crate::argv::{
    max_report_args, publish_args, render_report_args, sweep_map_args, PUBLISH_BENCHMARK_RECORD,
    RENDER_MAX_REPORT, RENDER_REPORT, RENDER_SWEEP_MAP,
};
use crate::bench_naming::{
    bench_filename, scan_bench_dir, NAMING_COLLISION_EXHAUSTED, NAMING_SOURCE_UNREADABLE,
};
use crate::classify_cell::{bench_mode_kind, read_bench_mode_record};
use crate::cli::{on_large_stack, Streams};
use crate::guidellm::Failure;
use crate::json_stream::{dump_indented, open_for_writing, write_text_once, FileSink};
use crate::lite_metrics::{build as build_lite_metrics, read_lite_verdict};
use crate::ospath;
use crate::pydyn::as_get_target;
use crate::pyfs;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::render_max_report;
use crate::render_report::{
    build_lite_md, build_md, lite_identity, manifest_path_labels_value, Environment,
    LITE_ONLY_ERROR_CLASS,
};
use crate::render_sweep_map::{assert_no_ranking_at, build, render_markdown, RENDER_ERROR};
use crate::report_cli::{finish, parse_or_finish, parser_error, Run};
use crate::report_support::{display, load_json_or_exit, py, repo_root, Res};
use crate::reporting_selftest::{run_naming_self_test, run_sweep_map_self_test};
use std::process::{Command, Stdio};

type Matches = crate::argv::Matches;

/// A command-line value, with Python's truthiness: an empty string is "not given".
fn option_text(matches: &Matches, action: usize) -> Option<PyStr> {
    matches
        .value(action)
        .filter(|text| !text.is_empty())
        .cloned()
}

fn as_value(text: &PyStr) -> PyValue {
    PyValue::Str(text.clone())
}

fn none_or(text: &Option<PyStr>) -> PyValue {
    text.as_ref().map_or(PyValue::None, as_value)
}

/// `out_dir or os.path.join(repo_root(anchor), "docs", "benchmark")`.
fn output_directory(out_dir: &Option<PyStr>, anchor: &PyStr) -> Res<PyStr> {
    match out_dir {
        Some(directory) => Ok(directory.clone()),
        None => Ok(ospath::join_all(
            &repo_root(&as_value(anchor))?,
            &["docs", "benchmark"],
        )),
    }
}

/// `os.makedirs(outdir, exist_ok=True)`, then write `text` to `outdir/name` (opened before it is encoded).
fn write_document(directory: &PyStr, name: &PyStr, text: &PyStr) -> Res<PyStr> {
    pyfs::mkdir_parents_exist_ok(directory)?;
    let path = ospath::join(directory, &[name]);
    pyfs::write_text(&path, text)?;
    Ok(path)
}

fn runs_to_status(run: Run, outcome: Res<i32>, io: &mut Streams<'_>) -> i32 {
    let status = finish(run, outcome, io);
    io.finish(status)
}

// ── render_report ─────────────────────────────────────────────────────────────

const TAG_REPORT: &str = "[render_report]";

/// `render_report`: runs the program and returns its exit status. `args` excludes the program name.
pub fn run_render_report(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match parse_or_finish(&RENDER_REPORT, args, io) {
        Ok(matches) => matches,
        Err(status) => return status,
    };
    let (run, outcome) = on_large_stack(move || {
        let mut run = Run::default();
        let outcome = render_report_body(&matches, &mut run);
        (run, outcome)
    });
    runs_to_status(run, outcome, io)
}

fn render_report_body(matches: &Matches, run: &mut Run) -> Res<i32> {
    use render_report_args as a;
    let sweep_index = option_text(matches, a::SWEEP_INDEX);
    let verdict_json = option_text(matches, a::VERDICT_JSON);
    let lite_raw = option_text(matches, a::LITE_RAW_JSON);
    let roofline_json = option_text(matches, a::ROOFLINE_JSON);
    let bench_mode_json = option_text(matches, a::BENCH_MODE_JSON);
    let out_dir = option_text(matches, a::OUT_DIR);
    let to_stdout = matches.flag(a::STDOUT);
    if matches.flag(a::LITE_ONLY) {
        if sweep_index.is_some()
            || verdict_json.is_some()
            || roofline_json.is_some()
            || bench_mode_json.is_some()
            || lite_raw.is_none()
        {
            run.warn("[render_report] ERROR --lite-only 는 --lite-raw-json 만 받는다(--sweep-index/--verdict-json/--roofline-json/--bench-mode-json 과 섞지 않는다)\n");
            return Ok(2);
        }
        return render_lite_report(run, &lite_raw.expect("checked"), &out_dir, to_stdout);
    }
    let (Some(sweep_index), Some(verdict_json), None) = (&sweep_index, &verdict_json, &lite_raw)
    else {
        run.warn("[render_report] ERROR full 모드는 --sweep-index 와 --verdict-json 이 필수다(--lite-raw-json 은 --lite-only 전용)\n");
        return Ok(2);
    };
    let index = load_json_or_exit(
        &as_value(sweep_index),
        TAG_REPORT,
        &PyValue::str("sweep-index"),
        &mut run.diagnostics,
    )?;
    let verdict = load_json_or_exit(
        &as_value(verdict_json),
        TAG_REPORT,
        &PyValue::str("verdict-json"),
        &mut run.diagnostics,
    )?;
    let roofline = match &roofline_json {
        Some(path) => load_json_or_exit(
            &as_value(path),
            TAG_REPORT,
            &PyValue::str("roofline-json"),
            &mut run.diagnostics,
        )?,
        None => PyValue::None,
    };
    let record =
        read_bench_mode_record(&as_value(sweep_index), &index, &none_or(&bench_mode_json))?;
    let PyValue::Tuple(pair) = &record else {
        unreachable!("read_bench_mode_record returns a pair")
    };
    let (bench_record, bench_status) = (&pair[0], &pair[1]);
    if bench_mode_json.is_some() && matches!(bench_record.peel(), PyValue::None) {
        let mut message = py("[render_report] ERROR --bench-mode-json 을 쓸 수 없다: ");
        message.push_py(&display(bench_status));
        message.push_str("\n");
        run.diagnostics.write(&message);
        return Ok(2);
    }
    let meta_or_empty = {
        let meta = as_get_target(&index)?
            .get("meta")
            .cloned()
            .unwrap_or(PyValue::None);
        if meta.is_truthy() {
            meta
        } else {
            PyValue::Dict(PyDict::new())
        }
    };
    let topology = as_get_target(&meta_or_empty)?
        .get("topology")
        .cloned()
        .unwrap_or(PyValue::None);
    let topology_text = if topology.is_truthy() {
        display(&topology)
    } else {
        PyStr::default()
    };
    let root = repo_root(&as_value(sweep_index))?;
    let manifest = ospath::join(
        &root,
        &[&py("output"), &topology_text, &py("manifest.yaml")],
    );
    let labels = manifest_path_labels_value(&as_value(&manifest))?;
    let markdown = build_md(
        &index,
        &verdict,
        &roofline,
        bench_record,
        bench_status,
        &labels,
    )?;
    let meta = as_get_target(&index)?
        .get("meta")
        .cloned()
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let directory = output_directory(&out_dir, sweep_index)?;
    let generated = as_get_target(&index)?
        .get("generated_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    let existing = if to_stdout {
        PyValue::None
    } else {
        PyValue::Dict(scan_bench_dir(&as_value(&directory), &PyValue::None)?)
    };
    let name = bench_filename(
        &PyValue::str("bench_report"),
        &meta,
        &generated,
        &existing,
        &PyValue::str("md"),
    )?;
    if to_stdout {
        run.write_plain(markdown);
        return Ok(0);
    }
    let path = write_document(&directory, &name, &markdown)?;
    let verdict_name = as_get_target(&verdict)?
        .get("verdict")
        .cloned()
        .unwrap_or(PyValue::None);
    let mut message = py("[render_report] 발행(항상): ");
    message.push_py(&path);
    message.push_str(" (verdict=");
    message.push_py(&display(&verdict_name));
    message.push_str(")\n");
    run.diagnostics.write(&message);
    run.print(path);
    Ok(0)
}

/// `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$` as a whole-string match (`\d` is any Unicode decimal digit).
fn is_utc_second_text(text: &PyStr) -> bool {
    let points = text.code_points();
    if points.len() != 20 {
        return false;
    }
    points
        .iter()
        .enumerate()
        .all(|(position, &point)| match position {
            4 | 7 => point == u32::from('-'),
            10 => point == u32::from('T'),
            13 | 16 => point == u32::from(':'),
            19 => point == u32::from('Z'),
            _ => crate::pystr::is_decimal_digit(point),
        })
}

fn render_lite_report(
    run: &mut Run,
    lite_raw: &PyStr,
    out_dir: &Option<PyStr>,
    to_stdout: bool,
) -> Res<i32> {
    let raw = load_json_or_exit(
        &as_value(lite_raw),
        TAG_REPORT,
        &PyValue::str("lite-raw-json"),
        &mut run.diagnostics,
    )?;
    if !matches!(raw.peel(), PyValue::Dict(_)) {
        run.warn("[render_report] ERROR --lite-raw-json 이 객체가 아니다\n");
        return Ok(2);
    }
    let measured_utc = as_get_target(&raw)?
        .get("measured_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    if !matches!(measured_utc.peel(), PyValue::Str(text) if is_utc_second_text(text)) {
        run.warn("[render_report] ERROR lite raw 에 measured_utc(UTC 초) 가 없다 — 측정시각을 날조하지 않는다\n");
        return Ok(2);
    }
    let built = build_lite_metrics(&raw).and_then(|lite| {
        lite_identity(&raw, &Environment::Process).map(|identity| (lite, identity))
    });
    let (lite, identity) = match built {
        Ok(pair) => pair,
        Err(failure) if failure.class == LITE_ONLY_ERROR_CLASS || failure.class == "SystemExit" => {
            let mut message = py("[render_report] ERROR 경량 리포트를 세울 수 없다: ");
            message.push_py(&failure.message);
            message.push_str("\n");
            run.diagnostics.write(&message);
            return Ok(2);
        }
        Err(failure) => return Err(failure),
    };
    let markdown = build_lite_md(&raw, &lite.to_value(), &identity)?;
    let directory = output_directory(out_dir, lite_raw)?;
    let existing = if to_stdout {
        PyValue::None
    } else {
        PyValue::Dict(scan_bench_dir(&as_value(&directory), &PyValue::None)?)
    };
    let name = match bench_filename(
        &PyValue::str("bench_report"),
        &identity,
        &measured_utc,
        &existing,
        &PyValue::str("md"),
    ) {
        Ok(name) => name,
        Err(failure)
            if failure.class == NAMING_COLLISION_EXHAUSTED
                || failure.class == NAMING_SOURCE_UNREADABLE =>
        {
            let mut message =
                py("[render_report] ERROR 명명 충돌 — 덮어쓰지도 새 접미를 발명하지도 않는다: ");
            message.push_py(&failure.message);
            message.push_str("\n");
            run.diagnostics.write(&message);
            return Ok(2);
        }
        Err(failure) => return Err(failure),
    };
    if to_stdout {
        run.write_plain(markdown);
        return Ok(0);
    }
    let path = write_document(&directory, &name, &markdown)?;
    let mut message = py("[render_report] 경량 리포트 발행(lite): ");
    message.push_py(&path);
    message.push_str("\n");
    run.diagnostics.write(&message);
    run.print(path);
    Ok(0)
}

// ── render_sweep_map ──────────────────────────────────────────────────────────

/// `render_sweep_map`: runs the program and returns its exit status. `args` excludes the program name.
pub fn run_render_sweep_map(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match parse_or_finish(&RENDER_SWEEP_MAP, args, io) {
        Ok(matches) => matches,
        Err(status) => return status,
    };
    if matches.flag(sweep_map_args::SELF_TEST) {
        let report = on_large_stack(run_sweep_map_self_test);
        let mut run = Run::default();
        run.diagnostics.write_str(&report.stderr);
        for line in report.stdout.lines() {
            run.print(py(line));
        }
        return runs_to_status(run, Ok(report.exit_code), io);
    }
    let (run, outcome) = on_large_stack(move || {
        let mut run = Run::default();
        let outcome = sweep_map_body(&matches, &mut run);
        (run, outcome)
    });
    runs_to_status(run, outcome, io)
}

/// `str(exc)` of an `OSError` or `ValueError` the map's input handler catches.
fn is_input_error(failure: &Failure) -> bool {
    failure.class == RENDER_ERROR
        || failure.class == "ValueError"
        || failure.class.ends_with("Error") && is_os_error_class(failure.class)
        || matches!(failure.class, "JSONDecodeError" | "UnicodeDecodeError")
}

fn is_os_error_class(class: &str) -> bool {
    matches!(
        class,
        "OSError"
            | "FileNotFoundError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "PermissionError"
            | "FileExistsError"
            | "BlockingIOError"
            | "ChildProcessError"
            | "ConnectionError"
            | "InterruptedError"
            | "ProcessLookupError"
            | "TimeoutError"
    )
}

fn read_json_document(path: &PyStr) -> Res<PyValue> {
    let location = ospath::fspath(&as_value(path))?;
    let bytes = pyfs::read_bytes(&location)?;
    let text = crate::pyjson::decode_file_text(&bytes)
        .map_err(|error| crate::repeat_api_cli::load_failure(&error))?;
    crate::pyjson::loads_text(&text).map_err(|error| crate::repeat_api_cli::load_failure(&error))
}

fn sweep_map_body(matches: &Matches, run: &mut Run) -> Res<i32> {
    let (state_path, stop_path, md_path) = (
        option_text(matches, sweep_map_args::STATE),
        option_text(matches, sweep_map_args::STOP_JSON),
        option_text(matches, sweep_map_args::OUT_MD),
    );
    let (Some(state_path), Some(stop_path), Some(md_path)) = (state_path, stop_path, md_path)
    else {
        run.warn("[render_sweep_map] ERROR --state · --stop-json · --out-md 는 필수다\n");
        return Ok(2);
    };
    let json_path = option_text(matches, sweep_map_args::OUT_JSON);
    let document = (|| {
        let state = read_json_document(&state_path)?;
        let stop = read_json_document(&stop_path)?;
        build(&state, &stop)
    })();
    let document = match document {
        Ok(document) => document,
        Err(failure) if is_input_error(&failure) => {
            let mut message = py("[render_sweep_map] ERROR ");
            message.push_py(&failure.message);
            message.push_str("\n");
            run.diagnostics.write(&message);
            return Ok(2);
        }
        Err(failure) => return Err(failure),
    };
    // From `main()`: module, `main` and `assert_no_ranking` are the first three interpreter frames. Only the ranking
    // refusal is a handled outcome (exit 3); a recursion failure ends the program like any uncaught exception.
    match assert_no_ranking_at(&document, "$", 3) {
        Ok(()) => {}
        Err(failure) if failure.class == RENDER_ERROR => {
            let mut message = py("[render_sweep_map] ERROR ");
            message.push_py(&failure.message);
            message.push_str("\n");
            run.diagnostics.write(&message);
            return Ok(3);
        }
        Err(failure) => return Err(failure),
    }
    // `with open(out_md, "w") as handle: handle.write(render_markdown(doc))`: the file exists before the text is
    // rendered, and is closed before the JSON file is opened.
    let mut markdown_file = open_for_writing(&md_path)?;
    let markdown = render_markdown(&document)?;
    write_text_once(&mut markdown_file, &markdown)?;
    drop(markdown_file);
    if let Some(json_path) = &json_path {
        // `json.dump(doc, handle, ensure_ascii=False, indent=2)`: chunk by chunk, the file closed (and what was
        // written kept) whether or not the dump failed. The outermost container is interpreter frame 5.
        let mut sink = FileSink::new(open_for_writing(json_path)?);
        let dumped = dump_indented(&document, 2, 5, &mut sink);
        let closed = sink.close();
        // `with open(...)` closes while unwinding: a close (flush) failure replaces an earlier dump failure.
        closed?;
        dumped?;
    }
    let mut line = py("[render_sweep_map] MAP=");
    line.push_py(&md_path);
    if let Some(json_path) = &json_path {
        line.push_str(" JSON=");
        line.push_py(json_path);
    }
    run.print(line);
    Ok(0)
}

// ── render_max_report ─────────────────────────────────────────────────────────

const TAG_MAX: &str = "[render_max]";

/// `render_max_report`: runs the program and returns its exit status. `args` excludes the program name.
pub fn run_render_max_report(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match parse_or_finish(&RENDER_MAX_REPORT, args, io) {
        Ok(matches) => matches,
        Err(status) => return status,
    };
    let (run, outcome) = on_large_stack(move || {
        let mut run = Run::default();
        let outcome = max_report_body(&matches, &mut run);
        (run, outcome)
    });
    runs_to_status(run, outcome, io)
}

fn max_report_body(matches: &Matches, run: &mut Run) -> Res<i32> {
    let max_index = matches
        .value(max_report_args::MAX_INDEX)
        .cloned()
        .expect("argparse enforces --max-index");
    let out_dir = option_text(matches, max_report_args::OUT_DIR);
    let to_stdout = matches.flag(max_report_args::STDOUT);
    let index = load_json_or_exit(
        &as_value(&max_index),
        TAG_MAX,
        &PyValue::str("max-index"),
        &mut run.diagnostics,
    )?;
    let markdown = render_max_report::build_md(&index)?;
    let meta = as_get_target(&index)?
        .get("meta")
        .cloned()
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let directory = output_directory(&out_dir, &max_index)?;
    let generated = as_get_target(&index)?
        .get("generated_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    let existing = if to_stdout {
        PyValue::None
    } else {
        PyValue::Dict(scan_bench_dir(&as_value(&directory), &PyValue::None)?)
    };
    let name = bench_filename(
        &PyValue::str("max_envelope"),
        &meta,
        &generated,
        &existing,
        &PyValue::str("md"),
    )?;
    if to_stdout {
        run.write_plain(markdown);
        return Ok(0);
    }
    let path = write_document(&directory, &name, &markdown)?;
    let ceiling = as_get_target(&index)?
        .get("safe_ceiling_max_model_len")
        .cloned()
        .unwrap_or(PyValue::None);
    let mut message = py("[render_max] 발행: ");
    message.push_py(&path);
    message.push_str(" (안전상한=");
    message.push_py(&display(&ceiling));
    message.push_str(")\n");
    run.diagnostics.write(&message);
    run.print(path);
    Ok(0)
}

// ── publish_benchmark_record ──────────────────────────────────────────────────

const TAG_PUBLISH: &str = "[publish_record]";
const CAMPAIGN_WRITER_REL: [&str; 4] = [".claude", "skills", "terraforming_node", "scripts"];
/// The campaign evidence writer's launcher (another owner's entry point; run with `bash`, never a Python interpreter).
const CAMPAIGN_WRITER_LAUNCHER: &str = "campaign_init.sh";

/// `publish_benchmark_record`: runs the program and returns its exit status. `args` excludes the program name.
pub fn run_publish_benchmark_record(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let matches = match parse_or_finish(&PUBLISH_BENCHMARK_RECORD, args, io) {
        Ok(matches) => matches,
        Err(status) => return status,
    };
    let (run, outcome) = on_large_stack(move || {
        let mut run = Run::default();
        let outcome = publish_body(&matches, &mut run);
        (run, outcome)
    });
    match outcome {
        Err(failure) if failure.class == "ArgumentError" => {
            let (text, status) = parser_error(
                &PUBLISH_BENCHMARK_RECORD,
                &failure.message.to_utf8().unwrap_or_default(),
            );
            crate::cli::write_stderr(io, &text);
            io.finish(status)
        }
        other => runs_to_status(run, other, io),
    }
}

struct PublishOptions {
    sweep_index: Option<PyStr>,
    verdict_json: Option<PyStr>,
    lite_raw_json: Option<PyStr>,
    out_dir: Option<PyStr>,
    to_stdout: bool,
}

fn publish_body(matches: &Matches, run: &mut Run) -> Res<i32> {
    let options = PublishOptions {
        sweep_index: option_text(matches, publish_args::SWEEP_INDEX),
        verdict_json: option_text(matches, publish_args::VERDICT_JSON),
        lite_raw_json: option_text(matches, publish_args::LITE_RAW_JSON),
        out_dir: option_text(matches, publish_args::OUT_DIR),
        to_stdout: matches.flag(publish_args::STDOUT),
    };
    if let Some(lite_raw) = &options.lite_raw_json {
        if options.sweep_index.is_some() || options.verdict_json.is_some() {
            return Err(Failure::new(
                "ArgumentError",
                "--lite-raw-json 은 --sweep-index/--verdict-json 과 함께 줄 수 없다",
            ));
        }
        return publish_lite_only(run, lite_raw, &options);
    }
    let (Some(sweep_index), Some(verdict_json)) = (&options.sweep_index, &options.verdict_json)
    else {
        return Err(Failure::new(
            "ArgumentError",
            "full 경로는 --sweep-index 와 --verdict-json 이 필수다(lite-only 셀은 --lite-raw-json)",
        ));
    };
    let index = load_json_or_exit(
        &as_value(sweep_index),
        TAG_PUBLISH,
        &PyValue::str("sweep-index"),
        &mut run.diagnostics,
    )?;
    let verdict = load_json_or_exit(
        &as_value(verdict_json),
        TAG_PUBLISH,
        &PyValue::str("verdict-json"),
        &mut run.diagnostics,
    )?;
    if let Some(published) = publish_demoted_lite(run, &index, sweep_index, &options)? {
        let _ = published;
        return Ok(0);
    }
    let verdict_name = as_get_target(&verdict)?
        .get("verdict")
        .cloned()
        .unwrap_or(PyValue::None);
    if !matches!(verdict_name.peel(), PyValue::Str(text) if text.equals("PASS")) {
        let mut message = py("[publish_record] verdict=");
        message.push_py(&display(&verdict_name));
        message.push_str(" ≠ PASS → 인증서 미발행(report 는 render_report 가 항상 발행)\n");
        run.diagnostics.write(&message);
        return Ok(0);
    }
    if let Some(unmet) = full_definition_unmet(&index, sweep_index)? {
        let mut message = py("[publish_record] ");
        message.push_py(&unmet);
        message.push_str(" → 인증서(benchmark_mode: full) 미발행 — bench_mode·강등 사유 확정은 classify_cell, 강등 셀의 발행 통로는 lite(plan_26091407 §4.5)\n");
        run.diagnostics.write(&message);
        return Ok(0);
    }
    let yaml = crate::publish_record::build_yaml(&index, &verdict)?;
    let meta = as_get_target(&index)?
        .get("meta")
        .cloned()
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let directory = output_directory(&options.out_dir, sweep_index)?;
    let generated = as_get_target(&index)?
        .get("generated_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    let existing = if options.to_stdout {
        PyValue::None
    } else {
        PyValue::Dict(scan_bench_dir(&as_value(&directory), &PyValue::None)?)
    };
    let name = bench_filename(
        &PyValue::str("benchmark"),
        &meta,
        &generated,
        &existing,
        &PyValue::str("yaml"),
    )?;
    if options.to_stdout {
        run.write_plain(yaml);
        return Ok(0);
    }
    let path = write_document(&directory, &name, &yaml)?;
    let mut message = py("[publish_record] 인증서 발행(PASS): ");
    message.push_py(&path);
    message.push_str("\n");
    run.diagnostics.write(&message);
    register_campaign_evidence(run, sweep_index, &path, &meta)?;
    run.print(path);
    Ok(0)
}

/// β: a lite-only cell's raw reading becomes a lite certificate when its own verdict is `pass`.
fn publish_lite_only(run: &mut Run, lite_raw: &PyStr, options: &PublishOptions) -> Res<i32> {
    let raw = load_json_or_exit(
        &as_value(lite_raw),
        TAG_PUBLISH,
        &PyValue::str("lite-raw-json"),
        &mut run.diagnostics,
    )?;
    let passed = read_lite_verdict(&raw).is_some_and(|verdict| verdict.label() == "pass");
    if !passed {
        let shown = match raw.peel() {
            PyValue::Dict(map) => map.get("lite_verdict").cloned().unwrap_or(PyValue::None),
            _ => PyValue::None,
        };
        run.warn(&format!(
            "[publish_record] lite_verdict={} ≠ pass → lite 인증서 미발행(lite 불통과·판정 부재는 인증서를 열지 않는다)\n",
            shown.repr()
        ));
        return Ok(3);
    }
    let meta = match lite_identity(&raw, &Environment::Process) {
        Ok(meta) => meta,
        Err(failure) if failure.class == LITE_ONLY_ERROR_CLASS => {
            let mut message = py("[publish_record] ERROR lite 인증서 강한 키를 세우지 못했다: ");
            message.push_py(&failure.message);
            message.push_str("\n");
            run.diagnostics.write(&message);
            return Ok(2);
        }
        Err(failure) => return Err(failure),
    };
    let measured = as_get_target(&raw)?
        .get("measured_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    if !measured.is_truthy() {
        run.warn("[publish_record] ERROR lite raw 에 measured_utc 가 없다 — 측정시각을 날조하지 않는다\n");
        return Ok(2);
    }
    let lite = build_lite_metrics(&raw)?.to_value();
    let source = as_get_target(&raw)?
        .get("lite_verdict_source")
        .cloned()
        .unwrap_or(PyValue::None);
    let yaml = crate::publish_record::build_lite_yaml(
        &meta,
        &lite,
        &measured,
        &PyValue::str(crate::publish_record::ENTRY_LITE_ONLY),
        &source,
    )?;
    let path = write_certificate(run, &yaml, &meta, &measured, options, lite_raw)?;
    if let Some(path) = path {
        let mut message = py("[publish_record] lite 등급 인증서 발행(β): ");
        message.push_py(&path);
        message.push_str("\n");
        run.diagnostics.write(&message);
        let mut with_config = as_get_target(&meta)?.clone();
        with_config.set(
            "config_name",
            as_get_target(&raw)?
                .get("config_name")
                .cloned()
                .unwrap_or(PyValue::None),
        );
        register_campaign_evidence(run, lite_raw, &path, &PyValue::Dict(with_config))?;
        run.print(path);
    }
    Ok(0)
}

/// `_write_certificate`: name by the naming owner, then write (or, with `--stdout`, print). `None` for stdout.
fn write_certificate(
    run: &mut Run,
    yaml: &PyStr,
    meta: &PyValue,
    measured: &PyValue,
    options: &PublishOptions,
    anchor: &PyStr,
) -> Res<Option<PyStr>> {
    let directory = output_directory(&options.out_dir, anchor)?;
    let existing = if options.to_stdout {
        PyValue::None
    } else {
        PyValue::Dict(scan_bench_dir(&as_value(&directory), &PyValue::None)?)
    };
    let name = bench_filename(
        &PyValue::str("benchmark"),
        meta,
        measured,
        &existing,
        &PyValue::str("yaml"),
    )?;
    if options.to_stdout {
        run.write_plain(yaml.clone());
        return Ok(None);
    }
    write_document(&directory, &name, yaml).map(Some)
}

/// γ: a demoted cell (bench-mode record says downgraded-lite) whose lite verdict is `pass` gets a lite certificate.
/// `Some(path-or-"-")` when it applies (published or printed), `None` when it does not apply.
fn publish_demoted_lite(
    run: &mut Run,
    index: &PyValue,
    sweep_index: &PyStr,
    options: &PublishOptions,
) -> Res<Option<PyStr>> {
    let repetition = as_get_target(index)?
        .get("repetition")
        .cloned()
        .unwrap_or(PyValue::None);
    let requested = match repetition.peel() {
        PyValue::Dict(map) => map.get("requested").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    if !matches!(requested.peel(), PyValue::Int(_)) {
        return Ok(None);
    }
    let pair = read_bench_mode_record(&as_value(sweep_index), index, &PyValue::None)?;
    let PyValue::Tuple(pair) = &pair else {
        unreachable!("read_bench_mode_record returns a pair")
    };
    let record = &pair[0];
    if matches!(record.peel(), PyValue::None) || bench_mode_kind(record) != "downgraded-lite" {
        return Ok(None);
    }
    let lite_value = as_get_target(index)?
        .get("lite")
        .cloned()
        .unwrap_or(PyValue::None);
    let lite = if matches!(lite_value.peel(), PyValue::Dict(_)) {
        lite_value
    } else {
        PyValue::Dict(PyDict::new())
    };
    let verdict = as_get_target(&lite)?
        .get("lite_verdict")
        .cloned()
        .unwrap_or(PyValue::None);
    if !matches!(verdict.peel(), PyValue::Str(text) if text.equals("pass")) {
        run.warn(&format!(
            "[publish_record] 강등 셀이지만 lite 판정={} ≠ pass → lite 인증서 미발행\n",
            verdict.repr()
        ));
        return Ok(None);
    }
    let meta = as_get_target(index)?
        .get("meta")
        .cloned()
        .unwrap_or_else(|| PyValue::Dict(PyDict::new()));
    let generated = as_get_target(index)?
        .get("generated_utc")
        .cloned()
        .unwrap_or(PyValue::None);
    let raw_json = as_get_target(&lite)?
        .get("raw_json")
        .cloned()
        .unwrap_or(PyValue::None);
    let mut source = py("sweep_index.lite.lite_verdict ← ");
    source.push_py(&display(&raw_json));
    let yaml = crate::publish_record::build_lite_yaml(
        &meta,
        &lite,
        &generated,
        &PyValue::str(crate::publish_record::ENTRY_DEMOTED),
        &PyValue::Str(source),
    )?;
    let written = write_certificate(run, &yaml, &meta, &generated, options, sweep_index)?;
    match written {
        None => Ok(Some(py("-"))),
        Some(path) => {
            let reason = as_get_target(record)?
                .get("downgrade_reason")
                .cloned()
                .unwrap_or(PyValue::None);
            let mut message =
                py("[publish_record] lite 등급 인증서 발행(γ 강등 · downgrade_reason=");
            message.push_py(&display(&reason));
            message.push_str("): ");
            message.push_py(&path);
            message.push_str("\n");
            run.diagnostics.write(&message);
            register_campaign_evidence(run, sweep_index, &path, &meta)?;
            run.print(path.clone());
            Ok(Some(path))
        }
    }
}

/// The reason a full certificate cannot be claimed, or `None`: for a sweep with a repetition axis the bench-mode
/// decision record must exist and say `full`. The decision is the classification owner's; it is only read.
fn full_definition_unmet(index: &PyValue, sweep_index: &PyStr) -> Res<Option<PyStr>> {
    let repetition = as_get_target(index)?
        .get("repetition")
        .cloned()
        .unwrap_or(PyValue::None);
    let requested = match repetition.peel() {
        PyValue::Dict(map) => map.get("requested").cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    let PyValue::Int(requested_count) = requested.peel() else {
        return Ok(None);
    };
    let pair = read_bench_mode_record(&as_value(sweep_index), index, &PyValue::None)?;
    let PyValue::Tuple(pair) = &pair else {
        unreachable!("read_bench_mode_record returns a pair")
    };
    let (record, status) = (&pair[0], &pair[1]);
    if matches!(record.peel(), PyValue::None) {
        let mut text = py(&format!(
            "반복 축 산출물(요청 반복 {requested_count})인데 bench_mode 판정 기록을 쓸 수 없다 — "
        ));
        text.push_py(&display(status));
        text.push_str(" · full 정의 충족을 확인하지 못했다");
        return Ok(Some(text));
    }
    let kind = bench_mode_kind(record);
    if kind == "full" {
        return Ok(None);
    }
    let field = |key: &str| -> PyStr {
        display(
            &as_get_target(record)
                .ok()
                .and_then(|map| map.get(key).cloned())
                .unwrap_or(PyValue::None),
        )
    };
    let mut text = py(&format!("bench_mode 판정 기록 = {kind}(bench_mode="));
    text.push_py(&field("bench_mode"));
    text.push_str(" · downgrade_reason=");
    text.push_py(&field("downgrade_reason"));
    text.push_str(" · 출처 ");
    text.push_py(&field("bench_mode_source"));
    text.push_str(")");
    Ok(Some(text))
}

/// `str(value or "").strip()`.
fn stripped_text(value: &PyValue) -> PyStr {
    if value.is_truthy() {
        display(value).strip()
    } else {
        PyStr::default()
    }
}

/// Leaves the campaign pointer for a certificate where it was born: the campaign writer of another owner is called
/// through its launcher (`bash campaign_init.sh --evidence-add ...`; this program starts no Python) with the
/// certificate's repository-relative path. A missing launcher is reported, a failing one is a warning; neither changes
/// the certificate that already exists.
fn register_campaign_evidence(
    run: &mut Run,
    anchor: &PyStr,
    certificate: &PyStr,
    meta: &PyValue,
) -> Res<()> {
    let root = repo_root(&as_value(anchor))?;
    let mut writer = ospath::join_all(&root, &CAMPAIGN_WRITER_REL);
    writer = ospath::join_all(&writer, &[CAMPAIGN_WRITER_LAUNCHER]);
    if !ospath::isfile(&writer) {
        let mut message = py("[publish_record] campaigns writer 부재(");
        message.push_py(&writer);
        message.push_str(") — 인증서는 발행됐지만 증거 포인터를 아무도 적지 않았다\n");
        run.diagnostics.write(&message);
        return Ok(());
    }
    if active_campaign_is_bootstrap(&root) {
        // campaign_init --evidence-add is a no-op (rc 0, nothing on stdout) when ACTIVE resolves to `_bootstrap`;
        // deciding that here avoids starting the writer for a call that cannot write.
        return Ok(());
    }
    let Ok(relative) = ospath::relpath(certificate, &root) else {
        return Ok(());
    };
    let meta_map = as_get_target(meta)?;
    let node = stripped_text(meta_map.get("measured_node").unwrap_or(&PyValue::None));
    let cell = stripped_text(meta_map.get("config_name").unwrap_or(&PyValue::None));
    let os_text = |text: &PyStr| -> std::ffi::OsString {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(text.to_surrogateescape_bytes().unwrap_or_default())
    };
    let mut command = Command::new("bash");
    command
        .arg(os_text(&writer))
        .args(["--evidence-add", "--kind", "certificate", "--path"])
        .arg(os_text(&relative));
    if !node.is_empty() {
        command.arg("--node").arg(os_text(&node));
    }
    if !cell.is_empty() {
        command.arg("--cell").arg(os_text(&cell));
    }
    command.current_dir(os_text(&root)).stdin(Stdio::inherit());
    let output = command.output().map_err(|error| {
        Failure::new(
            crate::pyfs::os_error_class(&error),
            &crate::cli::os_error_text(&error, &py("bash")),
        )
    })?;
    if !output.status.success() {
        let stderr = crate::pyjson::decode_file_text(&output.stderr)
            .map_err(|error| crate::repeat_api_cli::load_failure(&error))?;
        let mut message = py("[publish_record] ⚠ 캠페인 증거 포인터 등재 실패 — ");
        message.push_py(&stderr.strip());
        message.push_str("\n");
        run.diagnostics.write(&message);
    }
    Ok(())
}

// ── bench_doc_naming ──────────────────────────────────────────────────────────

/// `bench_doc_naming`: the naming owner's self-test. The original runs it whatever the arguments are.
pub fn run_bench_doc_naming(_args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let report = on_large_stack(run_naming_self_test);
    let mut run = Run::default();
    for line in report.stdout.lines() {
        run.print(py(line));
    }
    let outcome = match report.failure {
        Some(failure) => Err(failure),
        None => Ok(0),
    };
    runs_to_status(run, outcome, io)
}

/// True when campaign_init.active_campaign_id() would return `_bootstrap`: no readable ACTIVE pointer, an empty name, a
/// name without an instance directory, or the explicit reserved id. An unreadable (non-UTF-8) pointer is not decided here.
fn active_campaign_is_bootstrap(root: &PyStr) -> bool {
    use std::os::unix::ffi::OsStringExt;
    let campaigns = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        root.to_surrogateescape_bytes().unwrap_or_default(),
    ))
    .join("campaigns");
    let pointer = campaigns.join("ACTIVE");
    if !pointer.is_file() {
        return true;
    }
    let Ok(text) = std::fs::read_to_string(&pointer) else {
        return false;
    };
    let name = text.trim();
    name.is_empty() || name == "_bootstrap" || !campaigns.join(name).is_dir()
}
