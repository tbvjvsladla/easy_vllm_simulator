//! `bench_reporting_api`: the Rust-only surface of the reporting and naming owners, a separate program so that the
//! four legacy command lines keep answering exactly what the originals did.
//!
//! * `evaluate-jsonl` - every function of the five original modules that a bridge or a differential check needs,
//!   as a transport. Each stdin line is `{id, operation, arguments}` where `operation` is `<owner>.<function>`
//!   (`render_report.build_md`, `doc_naming.bench_filename`, ...) or a bare function name together with an
//!   `owner` field. A line yields `{id, kind: value, value}` or `{id, kind: exception, type, message}`, plus
//!   `stderr` when the call wrote to standard error before returning or raising (the `SystemExit` of a failed
//!   `load`). Owners: `render_report`, `render_sweep_map`, `render_max_report`, `publish_benchmark_record`,
//!   `doc_naming`. Arguments follow the Python signatures (names, defaults and the `TypeError` a wrong call
//!   raises); tuples come back as JSON arrays; path arguments are relative to the working directory and the files
//!   they name are read when the operation runs. The naming exceptions are reported by their own class names
//!   (`NamingCollisionExhausted`, `NamingSourceUnreadable`), the map's by `RenderError`, the lite identity's by
//!   `LiteIdentityError`.
//!   Opt-in: `arguments_json` (each argument as raw JSON text, decoded separately) and `result_json: true` (the
//!   result as compact ASCII JSON text in `value_json`).
//! * `constants` - the module constants as one JSON object, read from this crate.
//! * `native-help` - this list.

use crate::bench_naming::{
    bench_filename, gpu_key, kst_tokens, measurement_ts, scan_bench_dir, KIND_DEFAULT_EXTENSIONS,
};
use crate::cli::{on_large_stack, write_stderr, Streams};
use crate::guidellm::Failure;
use crate::pyjson::{decode_file_text, loads_objects};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::render_max_report;
use crate::render_report::{
    bench_mode_view, build_lite_md, build_md, full_measurement_config, label_path_value,
    lite_identity, lite_measurement_config, manifest_path_labels_value, measurement_config_section,
    repetition_section, Environment, DEFAULT_RECORD_STATUS, LITE_BENCH_TOOL,
    LITE_BENCH_TOOL_VERSION_SOURCE, LITE_DECLARED_DETAIL, LITE_REPEATS_SOURCE,
    MEASUREMENT_CONFIG_KEYS, MEASUREMENT_CONFIG_TITLE,
};
use crate::render_sweep_map::{
    assert_no_ranking, build, render_markdown, FORBIDDEN_HEADERS, FORBIDDEN_KEYS,
};
use crate::repeat_api_cli::{
    exception_record, load_failure, print_failed, print_text, row_arguments, strings, value_record,
    Bound, Signature, Wire,
};
use crate::report_support::{
    fnum, grep_env, grep_yaml, load_json_or_exit, na, na_with_suffix, read_text_or_empty,
    repo_root, table_cell, yaml_scalar, Diagnostics,
};

const NATIVE_HELP: &str = "bench_reporting_api Rust-only commands:\n  bench_reporting_api evaluate-jsonl   JSON lines on stdin: {id, operation, arguments} (API differential transport)\n  bench_reporting_api constants        the module constants as one JSON object\n  bench_reporting_api native-help      this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = match args.first().and_then(PyStr::to_utf8).as_deref() {
        Some("evaluate-jsonl") if args.len() == 1 => run_jsonl(io),
        Some("constants") if args.len() == 1 => {
            print_text(io, &constants().dumps_ascii().unwrap_or_default())
        }
        Some("native-help") if args.len() == 1 => match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure),
        },
        _ => {
            write_stderr(io, &PyStr::new(NATIVE_HELP));
            2
        }
    };
    io.finish(status)
}

/// The constants of the original modules, from this crate.
pub fn constants() -> PyValue {
    let mut map = PyDict::new();
    let kinds: Vec<(&str, &str)> = KIND_DEFAULT_EXTENSIONS.to_vec();
    let mut extensions = PyDict::new();
    for (kind, extension) in kinds {
        extensions.set(kind, PyValue::str(extension));
    }
    map.set("BENCH_KIND_DEFAULT_EXT", PyValue::Dict(extensions));
    map.set(
        "MEASUREMENT_CONFIG_TITLE",
        PyValue::str(MEASUREMENT_CONFIG_TITLE),
    );
    map.set("MEASUREMENT_CONFIG_KEYS", strings(&MEASUREMENT_CONFIG_KEYS));
    map.set("LITE_BENCH_TOOL", PyValue::str(LITE_BENCH_TOOL));
    map.set(
        "LITE_BENCH_TOOL_VERSION_SOURCE",
        PyValue::str(LITE_BENCH_TOOL_VERSION_SOURCE),
    );
    map.set("LITE_DECLARED_DETAIL", PyValue::str(LITE_DECLARED_DETAIL));
    map.set("LITE_REPEATS_SOURCE", PyValue::str(LITE_REPEATS_SOURCE));
    map.set("FORBIDDEN_KEYS", strings(&FORBIDDEN_KEYS));
    map.set("FORBIDDEN_HEADERS", strings(&FORBIDDEN_HEADERS));
    map.set(
        "ENTRY_BETA",
        PyValue::str(crate::publish_record::ENTRY_LITE_ONLY),
    );
    map.set(
        "ENTRY_GAMMA",
        PyValue::str(crate::publish_record::ENTRY_DEMOTED),
    );
    map.set(
        "LITE_CERT_SOFT_KEYS",
        strings(&crate::publish_record::LITE_CERT_SOFT_KEYS),
    );
    PyValue::Dict(map)
}

// ── Python call binding ─────────────────────────────────────────────────────────

const fn signature(function: &'static str, positional: &'static [&'static str]) -> Signature {
    Signature {
        function,
        positional,
        keyword_only: &[],
        optional: &[],
    }
}

const fn with_defaults(
    function: &'static str,
    positional: &'static [&'static str],
    optional: &'static [&'static str],
) -> Signature {
    Signature {
        function,
        positional,
        keyword_only: &[],
        optional,
    }
}

/// The argument as given, or `default` when the call left it out.
fn given(call: &Bound<'_>, arguments: &PyDict, name: &str, default: PyValue) -> PyValue {
    if arguments.contains_key(name) {
        call.get(name)
    } else {
        default
    }
}

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn str_result(value: PyStr) -> PyValue {
    PyValue::Str(value)
}

fn lines_value(lines: Vec<PyStr>) -> PyValue {
    PyValue::List(lines.into_iter().map(PyValue::Str).collect())
}

/// One operation of one owner, with the arguments as a Python call gives them. Whatever the call wrote to
/// standard error is appended to `diagnostics`.
pub fn evaluate_operation(
    operation: &str,
    arguments: &PyDict,
    diagnostics: &mut Diagnostics,
) -> Result<PyValue, Failure> {
    let (owner, function) = operation.split_once('.').ok_or_else(|| {
        Failure::new(
            "ValueError",
            &format!(
                "operation {} is not <owner>.<function>",
                PyStr::new(operation).repr()
            ),
        )
    })?;
    match (owner, function) {
        ("render_report", "load") => load_operation(arguments, "[render_report]", diagnostics),
        ("render_max_report", "load") => load_operation(arguments, "[render_max]", diagnostics),
        ("publish_benchmark_record", "load") => {
            load_operation(arguments, "[publish_record]", diagnostics)
        }
        ("render_report", "na") => {
            let call = with_defaults("na", &["v", "suffix"], &["suffix"]).bind(arguments)?;
            let suffix = given(&call, arguments, "suffix", text(""));
            Ok(str_result(na_with_suffix(call.get_ref("v"), &suffix)))
        }
        ("render_max_report", "na") => {
            let call = signature("na", &["v"]).bind(arguments)?;
            Ok(str_result(na(call.get_ref("v"))))
        }
        ("render_sweep_map", "_na") => {
            let call = signature("_na", &["v"]).bind(arguments)?;
            let value = call.get("v");
            Ok(if matches!(value.peel(), PyValue::None) {
                text("N/A")
            } else {
                value
            })
        }
        ("render_report", "fnum") => {
            let call = with_defaults("fnum", &["v", "fmt", "suffix"], &["fmt", "suffix"])
                .bind(arguments)?;
            let fmt = given(&call, arguments, "fmt", text("%.2f"));
            let suffix = given(&call, arguments, "suffix", text(""));
            fnum(call.get_ref("v"), &fmt, &suffix)
        }
        ("render_report", "_cell") => {
            let call = signature("_cell", &["v"]).bind(arguments)?;
            Ok(str_result(table_cell(call.get_ref("v"))))
        }
        ("publish_benchmark_record", "scalar") => {
            let call = signature("scalar", &["v"]).bind(arguments)?;
            Ok(str_result(yaml_scalar(call.get_ref("v"))))
        }
        ("render_report" | "render_max_report" | "publish_benchmark_record", "repo_root") => {
            let call = signature("repo_root", &["start"]).bind(arguments)?;
            Ok(str_result(repo_root(call.get_ref("start"))?))
        }
        ("render_report", "_grep_yaml") | ("render_report", "_grep_env") => {
            let is_yaml = function == "_grep_yaml";
            let call = signature(
                if is_yaml { "_grep_yaml" } else { "_grep_env" },
                &["text", "key"],
            )
            .bind(arguments)?;
            let (body, key) = (call.get("text"), call.get("key"));
            // `re.escape(key)` runs before `re.search(..., text)`.
            let PyValue::Str(key) = key.peel() else {
                return Err(Failure::new(
                    "TypeError",
                    &format!(
                        "decoding to str: need a bytes-like object, {} found",
                        key.type_name()
                    ),
                ));
            };
            let PyValue::Str(body) = body.peel() else {
                return Err(Failure::new(
                    "TypeError",
                    &format!(
                        "expected string or bytes-like object, got '{}'",
                        body.type_name()
                    ),
                ));
            };
            let found = if is_yaml {
                grep_yaml(body, key)
            } else {
                grep_env(body, key)
            };
            Ok(found.map_or(PyValue::None, PyValue::Str))
        }
        ("render_report", "_read_text") => {
            let call = signature("_read_text", &["path"]).bind(arguments)?;
            Ok(str_result(read_text_or_empty(call.get_ref("path"))?))
        }
        ("render_report", "manifest_path_labels") => {
            let call = signature("manifest_path_labels", &["manifest_path"]).bind(arguments)?;
            manifest_path_labels_value(call.get_ref("manifest_path"))
        }
        ("render_report", "_label_path") => {
            let call = signature("_label_path", &["value", "labels"]).bind(arguments)?;
            label_path_value(call.get_ref("value"), call.get_ref("labels"))
        }
        ("render_report", "_bench_mode_view") => {
            let call = signature("_bench_mode_view", &["index", "record", "record_status"])
                .bind(arguments)?;
            Ok(str_result(bench_mode_view(
                call.get_ref("record"),
                call.get_ref("record_status"),
            )?))
        }
        ("render_report", "_repetition_section") => {
            let call = signature(
                "_repetition_section",
                &["index", "bench_mode_record", "bench_mode_status"],
            )
            .bind(arguments)?;
            Ok(lines_value(repetition_section(
                call.get_ref("index"),
                call.get_ref("bench_mode_record"),
                call.get_ref("bench_mode_status"),
            )?))
        }
        ("render_report", "measurement_config_section") => {
            let call = signature("measurement_config_section", &["values"]).bind(arguments)?;
            Ok(lines_value(measurement_config_section(
                call.get_ref("values"),
            )?))
        }
        ("render_report", "full_measurement_config") => {
            let call = signature(
                "full_measurement_config",
                &["index", "bench_mode_record", "bench_mode_status"],
            )
            .bind(arguments)?;
            full_measurement_config(
                call.get_ref("index"),
                call.get_ref("bench_mode_record"),
                call.get_ref("bench_mode_status"),
            )
        }
        ("render_report", "lite_measurement_config") => {
            signature("lite_measurement_config", &[]).bind(arguments)?;
            lite_measurement_config()
        }
        ("render_report", "build_md") => {
            let call = with_defaults(
                "build_md",
                &[
                    "index",
                    "verdict",
                    "roofline",
                    "bench_mode_record",
                    "bench_mode_status",
                    "path_labels",
                ],
                &["bench_mode_record", "bench_mode_status", "path_labels"],
            )
            .bind(arguments)?;
            let record = given(&call, arguments, "bench_mode_record", PyValue::None);
            let status = given(
                &call,
                arguments,
                "bench_mode_status",
                text(DEFAULT_RECORD_STATUS),
            );
            let labels = given(&call, arguments, "path_labels", PyValue::None);
            Ok(str_result(build_md(
                call.get_ref("index"),
                call.get_ref("verdict"),
                call.get_ref("roofline"),
                &record,
                &status,
                &labels,
            )?))
        }
        ("render_report", "lite_identity") => {
            let call = with_defaults("lite_identity", &["raw", "environ"], &["environ"])
                .bind(arguments)?;
            let environ = given(&call, arguments, "environ", PyValue::None);
            if matches!(environ.peel(), PyValue::None) {
                lite_identity(call.get_ref("raw"), &Environment::Process)
            } else {
                lite_identity(call.get_ref("raw"), &Environment::Mapping(&environ))
            }
        }
        ("render_report", "build_lite_md") => {
            let call = signature("build_lite_md", &["raw", "lite", "identity"]).bind(arguments)?;
            Ok(str_result(build_lite_md(
                call.get_ref("raw"),
                call.get_ref("lite"),
                call.get_ref("identity"),
            )?))
        }
        ("render_sweep_map", "assert_no_ranking") => {
            let call = with_defaults("assert_no_ranking", &["payload", "path"], &["path"])
                .bind(arguments)?;
            let path = given(&call, arguments, "path", text("$"));
            let PyValue::Str(path) = path.peel() else {
                return Err(Failure::new("TypeError", "path must be a string"));
            };
            assert_no_ranking(call.get_ref("payload"), &path.to_utf8().unwrap_or_default())?;
            Ok(PyValue::None)
        }
        ("render_sweep_map", "build") => {
            let call = signature("build", &["state", "stop"]).bind(arguments)?;
            build(call.get_ref("state"), call.get_ref("stop"))
        }
        ("render_sweep_map", "render_markdown") => {
            let call = signature("render_markdown", &["doc"]).bind(arguments)?;
            Ok(str_result(render_markdown(call.get_ref("doc"))?))
        }
        ("render_max_report", "build_md") => {
            let call = signature("build_md", &["idx"]).bind(arguments)?;
            Ok(str_result(render_max_report::build_md(
                call.get_ref("idx"),
            )?))
        }
        ("publish_benchmark_record", "build_yaml") => {
            let call = signature("build_yaml", &["index", "verdict"]).bind(arguments)?;
            Ok(str_result(crate::publish_record::build_yaml(
                call.get_ref("index"),
                call.get_ref("verdict"),
            )?))
        }
        ("publish_benchmark_record", "build_lite_yaml") => {
            let call = signature(
                "build_lite_yaml",
                &[
                    "meta",
                    "lite",
                    "measured_utc",
                    "entry_path",
                    "verdict_source",
                ],
            )
            .bind(arguments)?;
            Ok(str_result(crate::publish_record::build_lite_yaml(
                call.get_ref("meta"),
                call.get_ref("lite"),
                call.get_ref("measured_utc"),
                call.get_ref("entry_path"),
                call.get_ref("verdict_source"),
            )?))
        }
        ("doc_naming", "kst_tokens") => {
            let call = signature("kst_tokens", &["generated_utc"]).bind(arguments)?;
            let (token, minute, second) = kst_tokens(call.get_ref("generated_utc"));
            Ok(PyValue::Tuple(vec![
                str_result(token),
                str_result(minute),
                str_result(second),
            ]))
        }
        ("doc_naming", "gpu_key") => {
            let call = signature("gpu_key", &["gpu_model"]).bind(arguments)?;
            Ok(str_result(gpu_key(call.get_ref("gpu_model"))))
        }
        ("doc_naming", "measurement_ts") => {
            let call = signature("measurement_ts", &["text"]).bind(arguments)?;
            Ok(measurement_ts(call.get_ref("text"))?.map_or(PyValue::None, PyValue::Str))
        }
        ("doc_naming", "scan_bench_dir") => {
            let call =
                with_defaults("scan_bench_dir", &["out_dir", "kind"], &["kind"]).bind(arguments)?;
            let kind = given(&call, arguments, "kind", PyValue::None);
            Ok(PyValue::Dict(scan_bench_dir(
                call.get_ref("out_dir"),
                &kind,
            )?))
        }
        ("doc_naming", "bench_filename") => {
            let call = with_defaults(
                "bench_filename",
                &["kind", "meta", "generated_utc", "existing", "ext"],
                &["existing", "ext"],
            )
            .bind(arguments)?;
            let existing = given(&call, arguments, "existing", PyValue::None);
            let ext = given(&call, arguments, "ext", PyValue::None);
            Ok(str_result(bench_filename(
                call.get_ref("kind"),
                call.get_ref("meta"),
                call.get_ref("generated_utc"),
                &existing,
                &ext,
            )?))
        }
        _ => Err(Failure::new(
            "ValueError",
            &format!("unknown operation {}", PyStr::new(operation).repr()),
        )),
    }
}

fn load_operation(
    arguments: &PyDict,
    tag: &str,
    diagnostics: &mut Diagnostics,
) -> Result<PyValue, Failure> {
    let call = signature("load", &["path", "label"]).bind(arguments)?;
    load_json_or_exit(
        call.get_ref("path"),
        tag,
        call.get_ref("label"),
        diagnostics,
    )
}

// ── evaluate-jsonl ──────────────────────────────────────────────────────────────

/// The operation name of a row: `owner.function`, or `function` with the row's `owner` field.
fn row_operation(map: &PyDict) -> Result<String, Failure> {
    let name = match map.get("operation") {
        Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
        Some(other) => other.repr(),
        None => return Err(Failure::new("KeyError", "'operation'")),
    };
    match map.get("owner") {
        Some(PyValue::Str(owner)) if !name.contains('.') => {
            Ok(format!("{}.{name}", owner.to_utf8().unwrap_or_default()))
        }
        _ => Ok(name),
    }
}

fn with_stderr(record: PyValue, diagnostics: &Diagnostics) -> PyValue {
    match record {
        PyValue::Dict(mut map) if !diagnostics.text().is_empty() => {
            map.set("stderr", PyValue::Str(diagnostics.text().clone()));
            PyValue::Dict(map)
        }
        other => other,
    }
}

fn evaluate_row(row: &PyValue) -> PyValue {
    let PyValue::Dict(map) = row else {
        return exception_record(
            PyValue::None,
            &Failure::new("TypeError", "operation row is not an object"),
            Wire::Values,
        );
    };
    let id = map.get("id").cloned().unwrap_or(PyValue::None);
    let wire = match map.get("result_json") {
        Some(PyValue::Bool(true)) => Wire::JsonText,
        _ => Wire::Values,
    };
    let mut diagnostics = Diagnostics::default();
    let outcome = (|| {
        let operation = row_operation(map)?;
        evaluate_operation(&operation, &row_arguments(map, true)?, &mut diagnostics)
    })();
    let record = match outcome {
        Ok(value) => value_record(id, value, wire),
        Err(failure) => exception_record(id, &failure, wire),
    };
    with_stderr(record, &diagnostics)
}

fn run_jsonl(io: &mut Streams<'_>) -> i32 {
    let mut bytes = Vec::new();
    if let Err(error) = io.stdin.read_to_end(&mut bytes) {
        write_stderr(
            io,
            &PyStr::new(&format!("[bench_reporting_api] ERROR stdin: {error}\n")),
        );
        return 2;
    }
    // The transport is deliberately strict UTF-8 in and ASCII out; it is not the legacy stream behaviour.
    let text = match decode_file_text(&bytes) {
        Ok(text) => text
            .to_utf8()
            .expect("strict decoding yields no surrogates"),
        Err(error) => {
            write_stderr(
                io,
                &PyStr::new(&format!("[bench_reporting_api] ERROR stdin: {error}\n")),
            );
            return 2;
        }
    };
    let output = on_large_stack(|| {
        let mut out = String::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let record = match loads_objects(line) {
                Ok(row) => evaluate_row(&row),
                Err(error) => exception_record(PyValue::None, &load_failure(&error), Wire::Values),
            };
            match record.dumps_ascii() {
                Ok(json) => out.push_str(&json),
                Err(error) => {
                    let id = match &record {
                        PyValue::Dict(map) => map.get("id").cloned().unwrap_or(PyValue::None),
                        _ => PyValue::None,
                    };
                    let text = error.to_string();
                    let message = text
                        .strip_prefix(&format!("{}: ", error.python_type()))
                        .unwrap_or(&text);
                    out.push_str(
                        &exception_record(
                            id,
                            &Failure::new(error.python_type(), message),
                            Wire::Values,
                        )
                        .dumps_ascii()
                        .unwrap_or_default(),
                    );
                }
            }
            out.push('\n');
        }
        out
    });
    match io.print_lines(&PyStr::new(&output)) {
        Ok(()) => 0,
        Err(failure) => print_failed(io, &failure),
    }
}
