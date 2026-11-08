//! The operations that read one JSON document (a file, an argument or standard input) and print a value from it or
//! derive a line or a file from it. Each is one former inline program; the document's shape is whatever the
//! program tolerated, so a document of an unexpected type fails exactly where the original's expression did.

use super::document::{
    get, get_or_empty_object, left_justified, load_file, load_text, or_empty, set_member,
    starts_with, write_compact, write_pretty,
};
use super::{Console, Context, Done, Stop};
use crate::dataset_prep::dump_failure;
use crate::guidellm::Failure;
use crate::pydyn::{as_get_target, dict_of, iterate, iterate_borrowed, join, or_none, subscript};
use crate::pynumber::int_call;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::repeat_text::cat;
use num_bigint::BigInt;

fn empty_text() -> PyStr {
    PyStr::default()
}

fn attribute_error(value: &PyValue, name: &str) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute '{name}'", value.type_name()),
    )
}

/// `print(d.get("status") if isinstance(d, dict) and isinstance(d.get("status"), str) else "")` for the document on
/// standard input; an unparsable document is read as `None`.
pub(super) fn provenance_status(context: &Context<'_>, console: &mut Console) -> Done {
    let document = match crate::pyjson::loads_text(&context.stdin) {
        Ok(document) => document,
        Err(error) if error.is_value_error() => PyValue::None,
        Err(error) => return Err(super::document::load_failure(error).into()),
    };
    let status = match dict_of(&document)
        .and_then(|map| map.get("status"))
        .map(PyValue::peel)
    {
        Some(PyValue::Str(text)) => text.clone(),
        _ => empty_text(),
    };
    console.print(status);
    Ok(())
}

/// `sys.exit(0 if json.load(open(path)).get("measurement_ok") else 1)`.
pub(super) fn measurement_ok(context: &Context<'_>) -> Done {
    let document = load_file(context.argument(0)?)?;
    if get(&document, "measurement_ok")?.is_truthy() {
        Ok(())
    } else {
        Err(Stop::Exit(1))
    }
}

/// `broad-measurement-ok PATH`: the measurement check of `broad_search.sh`. The original pasted the path into Python
/// source between single quotes, so a quote, a backslash or a line end in the path changed what was read (or did not
/// compile). Under the recorded value contract the path is the literal argument, and the check is exactly
/// [`measurement_ok`]: the same loader, JSON, type, truth and error behaviour, with no second evaluator.
pub(super) fn broad_measurement_ok(context: &Context<'_>) -> Done {
    measurement_ok(context)
}

/// The outcome of the last recorded cell with this key (`""` when there is none).
pub(super) fn cell_outcome(context: &Context<'_>, console: &mut Console) -> Done {
    let state = load_file(context.argument(0)?)?;
    let cells = get(&state, "cells")?;
    let wanted = context.argument(1)?;
    let mut last: Option<PyValue> = None;
    if cells.is_truthy() {
        for cell in iterate(&cells)? {
            let key = get(&cell, "cell_key")?;
            if matches!(key.peel(), PyValue::Str(own) if own == wanted) {
                last = Some(cell);
            }
        }
    }
    let outcome = match last {
        Some(cell) => get(&cell, "cell_outcome")?,
        None => PyValue::None,
    };
    console.print(or_empty(&outcome));
    Ok(())
}

/// The cell classification's `void_reason` when its source names the blackbox events or the lite raw.
pub(super) fn event_reason(context: &Context<'_>, console: &mut Console) -> Done {
    let classification = load_text(&context.stdin)?;
    let source = get(&classification, "void_reason_source")?;
    let source = if source.is_truthy() {
        source
    } else {
        PyValue::str("")
    };
    let PyValue::Str(source) = source.peel() else {
        return Err(attribute_error(&source, "startswith").into());
    };
    let line = if starts_with(source, "events(") || starts_with(source, "lite_raw(") {
        or_empty(&get(&classification, "void_reason")?)
    } else {
        empty_text()
    };
    console.print(line);
    Ok(())
}

/// `print(json.load(sys.stdin).get(key) or "")`.
pub(super) fn stdin_member(context: &Context<'_>, console: &mut Console, key: &str) -> Done {
    let document = load_text(&context.stdin)?;
    console.print(or_empty(&get(&document, key)?));
    Ok(())
}

/// `print(json.load(open(path)).get(key) or "")`.
pub(super) fn file_member(context: &Context<'_>, console: &mut Console, key: &str) -> Done {
    let document = load_file(context.argument(0)?)?;
    console.print(or_empty(&get(&document, key)?));
    Ok(())
}

/// `print(json.load(sys.stdin)[key])`.
pub(super) fn stdin_required(context: &Context<'_>, console: &mut Console, key: &str) -> Done {
    let document = load_text(&context.stdin)?;
    console.print(subscript(&document, key)?.display_text());
    Ok(())
}

/// `print("\n".join(json.load(sys.stdin).get("events_scanned") or []))`.
pub(super) fn events_scanned(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_text(&context.stdin)?;
    let scanned = get(&document, "events_scanned")?;
    let scanned = if scanned.is_truthy() {
        scanned
    } else {
        PyValue::List(Vec::new())
    };
    console.print(join("\n", &scanned)?);
    Ok(())
}

/// The model path and tensor-parallel size of a sweep index's `meta`, as the two words the judge script reads.
pub(super) fn judge_model_tp(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    let meta = get_or_empty_object(&document, "meta")?;
    let members = as_get_target(&meta)?;
    let size = match members.get("tensor_parallel_size") {
        Some(size) => size.clone(),
        None => or_none(members.get("tp")),
    };
    let size = match int_call(&size) {
        Ok(number) => Some(number),
        Err(error) if error.is_type_or_value_error() => None,
        Err(error) => return Err(Failure::from(error).into()),
    };
    let model_path = or_none(members.get("model_path"));
    let model_path = if model_path.is_truthy() {
        model_path.display_text()
    } else {
        PyStr::new("NA")
    };
    let size = match size {
        Some(number) if number > BigInt::from(0) => number.to_string(),
        _ => "-".to_string(),
    };
    console.print(cat!(model_path, " ", size));
    Ok(())
}

/// The one-line verdict summary of a judged index.
pub(super) fn judge_log(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    let verdict = get(&document, "verdict")?;
    let rubric = get_or_empty_object(&document, "rubric")?;
    let authority = get(&rubric, "authority")?;
    let source = get(&rubric, "source")?;
    let floor = get(&rubric, "floor")?;
    let reason_code = get(&document, "reason_code")?;
    console.print(cat!(
        "[judge_bench] verdict=",
        verdict.display_text(),
        " authority=",
        authority.display_text(),
        " source=",
        source.display_text(),
        " floor=",
        floor.display_text(),
        " reason_code=",
        reason_code.display_text()
    ));
    Ok(())
}

/// `print(((json.load(open(path)).get("rubric") or {}).get("authority")) or "")`.
pub(super) fn authority(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    let rubric = get_or_empty_object(&document, "rubric")?;
    console.print(or_empty(&get(&rubric, "authority")?));
    Ok(())
}

/// `print(json.dumps(sys.stdin.read()))`: the text as one ASCII JSON string.
pub(super) fn json_escape_stdin(context: &Context<'_>, console: &mut Console) -> Done {
    let escaped = PyValue::Str(context.stdin.clone())
        .dumps_ascii()
        .map_err(dump_failure)?;
    console.print(PyStr::new(&escaped));
    Ok(())
}

/// `print(json.load(open(path)).get("lite_verdict", ""))`: the stored value as it is (a stored `null` prints `None`).
/// This is not [`read_lite_verdict`], which accepts only the vocabulary.
pub(super) fn lite_verdict(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    let stored = as_get_target(&document)?
        .get("lite_verdict")
        .cloned()
        .unwrap_or_else(|| PyValue::str(""));
    console.print(stored.display_text());
    Ok(())
}

/// `print(json.load(open(path))["dataset_kind"])`.
pub(super) fn dataset_kind(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    console.print(subscript(&document, "dataset_kind")?.display_text());
    Ok(())
}

/// The sidecar of the measuring tool: the pin document from standard input plus the request format and where it came
/// from, written beside the run.
pub(super) fn tool_sidecar(context: &Context<'_>) -> Done {
    let mut document = load_text(&context.stdin)?;
    let endpoint = context.required("GL_ENDPOINT")?;
    set_member(&mut document, "endpoint", PyValue::Str(endpoint))?;
    let source = context
        .non_empty("GL_BACKEND_SOURCE")
        .unwrap_or_else(|| PyStr::new("unknown"));
    set_member(&mut document, "backend_source", PyValue::Str(source))?;
    let output = context.required("GL_OUT")?;
    write_pretty(&output, &document, true)?;
    Ok(())
}

/// The GuideLLM column mapper of a dataset spec, with the token-count columns (the measuring form).
pub(super) fn column_mapper(context: &Context<'_>, console: &mut Console) -> Done {
    let spec = load_file(context.argument(0)?)?;
    console.print(crate::dataset_prep::column_mapper_arg(
        &spec,
        &PyValue::Bool(true),
    )?);
    Ok(())
}

/// The `--data` argument for a dataset file of the given kind.
pub(super) fn data_arg(context: &Context<'_>, console: &mut Console) -> Done {
    let kind = PyValue::Str(context.argument(0)?.clone());
    let path = PyValue::Str(context.argument(1)?.clone());
    console.print(crate::dataset_prep::data_arg(&kind, &path)?);
    Ok(())
}

/// Writes the `dataset` member of a knob record to a file (no indent, no final newline).
pub(super) fn extract_dataset(context: &Context<'_>) -> Done {
    let record = load_file(context.argument(0)?)?;
    let dataset = subscript(&record, "dataset")?.clone();
    write_compact(context.argument(1)?, &dataset)?;
    Ok(())
}

/// The knob resolution a dry run reports: a header, one line per knob, the dataset.
pub(super) fn dry_knobs(context: &Context<'_>, console: &mut Console) -> Done {
    let record = load_file(context.argument(0)?)?;
    let preset = get(&record, "preset")?;
    let cap_source = get(&get_or_empty_object(&record, "serving_cap")?, "source")?;
    console.print(cat!(
        "[sweep_bench] DRY-RUN knob 해소(프리셋 ",
        preset.display_text(),
        " · 상한 ",
        cap_source.display_text(),
        "):"
    ));
    let knobs = subscript(&record, "knobs")?;
    let PyValue::Dict(knobs) = knobs.peel() else {
        return Err(attribute_error(knobs, "items").into());
    };
    for (name, knob) in knobs.iter() {
        let value = get(knob, "value")?;
        let source = get(knob, "source")?;
        let grade = get(knob, "grade")?;
        let reason = get(knob, "reason")?;
        let filtered = get(knob, "filtered_out")?;
        let excluded = if filtered.is_truthy() {
            let mut values = Vec::new();
            for item in iterate_borrowed(subscript(knob, "filtered_out")?)?.as_slice() {
                values.push(subscript(item, "value")?.clone());
            }
            cat!(" · 제외 ", PyValue::List(values).display_text())
        } else {
            empty_text()
        };
        console.print(cat!(
            "  ",
            left_justified(&PyValue::Str(name.clone()), 16),
            " = ",
            left_justified(&value, 18),
            " source=",
            left_justified(&source, 8),
            " grade=",
            grade.display_text(),
            " reason=",
            reason.display_text(),
            excluded
        ));
    }
    let dataset = get(&record, "dataset")?
        .dumps_unicode()
        .map_err(dump_failure)?;
    console.print(cat!("  dataset          = ", dataset));
    Ok(())
}

/// The recorded levels of an existing sweep index that are plain integers, space separated.
pub(super) fn prior_levels(context: &Context<'_>, console: &mut Console) -> Done {
    let document = load_file(context.argument(0)?)?;
    let levels = get(&document, "levels")?;
    let mut words: Vec<PyStr> = Vec::new();
    if levels.is_truthy() {
        for entry in iterate(&levels)? {
            let Some(map) = dict_of(&entry) else { continue };
            if let Some(PyValue::Int(level)) = map.get("level").map(PyValue::peel) {
                words.push(PyStr::new(&level.to_string()));
            }
        }
    }
    let mut line = PyStr::default();
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            line.push_str(" ");
        }
        line.push_py(word);
    }
    console.print(line);
    Ok(())
}

/// The stored lite verdict if it is one of the vocabulary, else nothing; an unreadable record is nothing too.
pub(super) fn read_lite_verdict(context: &Context<'_>, console: &mut Console) -> Done {
    let raw = load_file(context.argument(0)?).unwrap_or(PyValue::None);
    let verdict = crate::lite_metrics::read_lite_verdict(&raw)
        .map_or_else(empty_text, |verdict| PyStr::new(verdict.label()));
    console.print(verdict);
    Ok(())
}

/// The log lines of the bench-mode record the classifier printed, read from standard input.
pub(super) fn bench_mode_log(context: &Context<'_>, console: &mut Console) -> Done {
    let record = load_text(&context.stdin)?;
    let mode = get(&record, "bench_mode")?;
    let reason = get(&record, "downgrade_reason")?;
    let correlation = get(&record, "downgrade_correlation")?;
    let path = get(&record, "record_path")?;
    let source = get(&record, "bench_mode_source")?;
    let scan = get(&record, "events_scan")?;
    let summary = crate::classify_cell::events_scan_summary(&scan)?;
    let summary = if summary.is_truthy() {
        summary.display_text()
    } else {
        PyStr::new("대조 노드 기록 없음")
    };
    console.print(cat!(
        "[sweep_bench] bench_mode=",
        mode.display_text(),
        " · 사유 ",
        reason.display_text(),
        " · 대조 ",
        correlation.display_text(),
        " · 기록 ",
        path.display_text(),
        "\n[sweep_bench]   출처: ",
        source.display_text(),
        "\n[sweep_bench]   ",
        summary
    ));
    Ok(())
}
