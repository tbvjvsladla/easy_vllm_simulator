//! The operations that write a sweep state: opening a Broad Search (`broad-init`), recording one finished cell of it
//! (`broad-cell`), and recording the cell a standalone `sweep_bench.sh --campaign-cell` measured
//! (`campaign-cell-record` · plan_26111708 B L12) with the same record builder.

use super::document::{get, get_or_empty_object, load_file, load_text, or_empty, write_pretty};
use super::{Console, Context, Done, Stop};
use crate::assemble_sweep_meta::is_digit_text;
use crate::dataset_prep::dump_failure;
use crate::guidellm::Failure;
use crate::ospath;
use crate::pydyn::{as_get_target, iterate, key_error};
use crate::pynumber::int_from_str;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_text::cat;
use num_bigint::BigInt;

fn text(value: PyStr) -> PyValue {
    PyValue::Str(value)
}

/// The value of a declared positive count variable: digits only, and above zero; anything else ends the run with 2.
/// `isdigit()` also accepts superscripts, which `int()` then refuses with its own `ValueError`.
fn positive(context: &Context<'_>, console: &mut Console, name: &str) -> Result<PyValue, Stop> {
    let raw = context.required(name)?;
    let number = if is_digit_text(&raw) {
        let number = int_from_str(&raw).map_err(Failure::from)?;
        (number > BigInt::from(0)).then_some(number)
    } else {
        None
    };
    match number {
        Some(number) => Ok(PyValue::int(number)),
        None => {
            console.warn(&format!(
                "[broad_search] ERROR {name} 는 양의 정수여야 한다(받은 값: {})\n",
                raw.repr()
            ));
            Err(Stop::Exit(2))
        }
    }
}

/// `broad-init STATE`: the declared budget and the cells to run, written as a fresh state file.
pub(super) fn init(context: &Context<'_>, console: &mut Console) -> Done {
    let listed = context.required("CELLS")?;
    let cells: Vec<PyStr> = split_commas(&listed)
        .into_iter()
        .map(|cell| cell.strip())
        .filter(|cell| !cell.is_empty())
        .collect();
    if cells.is_empty() {
        console.warn("[broad_search] ERROR --cells 가 비었다\n");
        return Err(Stop::Exit(2));
    }
    let sweep_id = context.required("SWEEP_ID")?;
    let topology = context.required("TOPO")?;
    let control = context.required("CTRL")?;
    let authority = context.required("AUTHORITY")?;
    let started = context.required("NOW")?;
    let mut budget = PyDict::new();
    budget.set("max_cells", positive(context, console, "MAX_CELLS")?);
    budget.set("wall_clock_budget_s", positive(context, console, "WALL")?);
    budget.set(
        "consecutive_failure_limit",
        positive(context, console, "FAILLIMIT")?,
    );
    budget.set("repeats", positive(context, console, "REP_N")?);
    budget.set("repeats_source", text(context.required("REP_SRC")?));
    budget.set("declared_by", text(context.required("DECLARED_BY")?));
    budget.set("basis", text(context.required("BASIS")?));
    let budget = PyValue::Dict(budget);
    let planned = cells.len();
    let mut state = PyDict::new();
    state.set("schema_version", PyValue::int(1));
    state.set("sweep_id", text(sweep_id.clone()));
    state.set("topology", text(topology));
    state.set("control_variable", text(control));
    state.set("rubric_authority", text(authority));
    state.set("started_utc", text(started));
    state.set("declared_budget", budget.clone());
    state.set("cells_planned", PyValue::int(planned));
    state.set(
        "cells_remaining",
        PyValue::List(cells.into_iter().map(text).collect()),
    );
    state.set("cells", PyValue::List(Vec::new()));
    write_pretty(context.argument(0)?, &PyValue::Dict(state), false)?;
    let shown = budget.dumps_unicode().map_err(dump_failure)?;
    console.print(cat!(
        "[broad_search] init — sweep=",
        sweep_id,
        " cells=",
        planned.to_string(),
        " 예산=",
        shown
    ));
    Ok(())
}

/// `CELLS.split(",")`.
fn split_commas(value: &PyStr) -> Vec<PyStr> {
    let comma = u32::from(',');
    value
        .code_points()
        .split(|&point| point == comma)
        .map(|part| PyStr::from_code_points(part.to_vec()))
        .collect()
}

/// `_load(name)`: a JSON document of the sweep directory, or nothing for an unreadable or damaged one.
fn load_sweep_file(sweep_dir: &PyStr, name: &[&str]) -> Result<Option<PyValue>, Failure> {
    let mut path = sweep_dir.clone();
    for part in name {
        path = ospath::join_all(&path, &[*part]);
    }
    crate::pyfs::load_json_or_none(&path)
}

/// `value or {}`.
fn or_empty_object(value: Option<PyValue>) -> PyValue {
    match value {
        Some(found) if found.is_truthy() => found,
        _ => PyValue::Dict(PyDict::new()),
    }
}

const BENCH_MODE_KEYS: [&str; 5] = [
    "bench_mode",
    "bench_mode_source",
    "downgrade_reason",
    "downgrade_reason_source",
    "downgrade_correlation",
];

/// The five bench-mode fields of a cell: the classifier's record when this sweep measured, otherwise `null`s with the
/// reason they are empty.
fn bench_mode_fields(
    context: &Context<'_>,
    classification: &PyValue,
    sweep_dir: &PyStr,
    index: &PyValue,
) -> Result<Vec<(&'static str, PyValue)>, Failure> {
    let outcome = get(classification, "cell_outcome")?;
    let empty = |source: PyStr| -> Vec<(&'static str, PyValue)> {
        BENCH_MODE_KEYS
            .iter()
            .map(|&key| {
                let value = if key == "bench_mode_source" {
                    text(source.clone())
                } else {
                    PyValue::None
                };
                (key, value)
            })
            .collect()
    };
    if !matches!(outcome.peel(), PyValue::Str(own) if own.equals("measured")) {
        return Ok(empty(cat!(
            "not_applicable(cell_outcome=",
            outcome.display_text(),
            " — 측정이 성립하지 않은 셀에는 bench_mode 가 없다)"
        )));
    }
    // The original added the scripts directory to its import path here; the variable is still part of the contract.
    context.required("SDIR")?;
    let index_path = ospath::join_all(sweep_dir, &["sweep_index.json"]);
    let pair =
        crate::classify_cell::read_bench_mode_record(&text(index_path), index, &PyValue::None)?;
    let PyValue::Tuple(parts) = &pair else {
        unreachable!("the record reader returns a pair")
    };
    let (record, status) = (&parts[0], &parts[1]);
    if matches!(record, PyValue::None) {
        return Ok(empty(cat!(
            "not_evaluated(판정 기록 ",
            status.display_text(),
            ")"
        )));
    }
    let members = as_get_target(record)?;
    Ok(BENCH_MODE_KEYS
        .iter()
        .map(|&key| (key, members.get(key).cloned().unwrap_or(PyValue::None)))
        .collect())
}

/// `get(key)` of every key, in order, as a new object.
fn pick(source: &PyValue, keys: &[&'static str]) -> Result<PyValue, Failure> {
    let members = as_get_target(source)?;
    let mut picked = PyDict::new();
    for &key in keys {
        picked.set(key, members.get(key).cloned().unwrap_or(PyValue::None));
    }
    Ok(PyValue::Dict(picked))
}

fn member(source: &PyValue, key: &str) -> Result<PyValue, Failure> {
    get(source, key)
}

/// The verdict line of a cell: `verdict · authority · source · floor` and the reason code when there is one.
fn verdict_narrative(verdict: &PyValue) -> Result<PyValue, Failure> {
    if !verdict.is_truthy() {
        return Ok(PyValue::None);
    }
    let headline = get(verdict, "verdict")?;
    let rubric = get_or_empty_object(verdict, "rubric")?;
    let authority = get(&rubric, "authority")?;
    let rubric = get_or_empty_object(verdict, "rubric")?;
    let source = get(&rubric, "source")?;
    let rubric = get_or_empty_object(verdict, "rubric")?;
    let floor = get(&rubric, "floor")?;
    let code = get(verdict, "reason_code")?;
    let suffix = if code.is_truthy() {
        let code = as_get_target(verdict)?
            .get("reason_code")
            .cloned()
            .ok_or_else(|| key_error("reason_code"))?;
        cat!(" · reason_code=", code.display_text())
    } else {
        PyStr::default()
    };
    Ok(text(cat!(
        headline.display_text(),
        " · authority=",
        authority.display_text(),
        " · source=",
        source.display_text(),
        " · floor=",
        floor.display_text(),
        suffix
    )))
}

/// `broad-cell STATE`: records one cell in the state file (a cell with the same key is replaced).
pub(super) fn cell(context: &Context<'_>, console: &mut Console) -> Done {
    let state_path = context.argument(0)?;
    let mut state = load_file(state_path)?;
    let (cell, outcome) = cell_record(context, Citation::Declared)?;
    replace_cell(&mut state, cell)?;
    write_pretty(state_path, &state, false)?;
    let key = context.required("CELL_KEY")?;
    console.print(cat!(
        "[broad_search] cell ",
        key,
        " → ",
        outcome.display_text()
    ));
    Ok(())
}

/// Where a cell record's axis citation comes from: the `--axis-citation` argument of the Broad Search cell, or nowhere (a
/// standalone campaign-cell sweep has no axis argument — plan_26111708 B L12).
#[derive(Clone, Copy)]
enum Citation {
    Declared,
    Absent,
}

/// What a standalone campaign-cell record says about the fields only a Broad Search declares (plan_26111708 B L12): the
/// value is `null` and this sentence is its source, so an absent declaration never reads as a declared one.
const STANDALONE_ORIGIN: &str = "sweep_bench --campaign-cell";
const AXIS_CITATION_ABSENT: &str =
    "absent(단독 sweep_bench --campaign-cell — 축 인용은 broad_search cell 의 --axis-citation 선언이다 · plan_26111708 B L12)";
const DECLARED_BUDGET_ABSENT: &str =
    "absent(단독 sweep_bench --campaign-cell — 스윕 예산 선언은 broad_search init 의 몫이다 · 이 상태는 셀 기록만 든다 · plan_26111708 B L12)";

/// `campaign-cell-record STATE`: the record of one cell measured by `sweep_bench.sh --campaign-cell`, written into the
/// campaign's sweep state — the record `campaign_init --cell-set --sweep-state` reads. plan_26111708 B L12: only the Broad
/// Search transaction wrote it, so a campaign measured with sweep_bench alone (P2 single · P3 multi) left `sweeps/` empty
/// and its measurement was entered by hand. The record is the one `broad-cell` builds ([`cell_record`]); the place is
/// never the caller's choice: STATE is the derivation of `campaigns/ACTIVE` (`campaign_init --derive sweep`), so it must be
/// `campaigns/<id>/sweeps/<name>.json`, and outside a campaign (`_bootstrap`) the writer is a no-op. A state that does not
/// exist yet is opened as a standalone state whose absent budget says so.
pub(super) fn campaign_cell(context: &Context<'_>, console: &mut Console) -> Done {
    let state_path = context.argument(0)?;
    let campaign = match campaign_of_state(state_path) {
        Ok(campaign) => campaign,
        Err(reason) => {
            console.warn(&format!("[sweep_bench] ERROR campaign-cell-record: {reason}\n"));
            return Err(Stop::Exit(2));
        }
    };
    let Some(campaign) = campaign else {
        console.print(cat!(
            "[sweep_bench] ⓘ 캠페인 밖(ACTIVE=_bootstrap) — 캠페인 sweep 기록을 쓰지 않는다(writer no-op): ",
            state_path.clone()
        ));
        return Ok(());
    };
    let fresh = !ospath::lexists(state_path);
    let mut state = if fresh { standalone_state(context, state_path)? } else { load_file(state_path)? };
    let (mut cell, outcome) = cell_record(context, Citation::Absent)?;
    cell.set("axis_citation_source", text(PyStr::new(AXIS_CITATION_ABSENT)));
    replace_cell(&mut state, cell)?;
    if fresh {
        crate::pyfs::mkdir_parents_exist_ok(&ospath::split(state_path).0)?;
    }
    write_pretty(state_path, &state, false)?;
    let key = context.required("CELL_KEY")?;
    let shown = ospath::relpath(state_path, &context.required("REPO")?)?;
    console.print(cat!(
        "[sweep_bench] 캠페인 sweep 기록(",
        PyStr::new(&campaign),
        "): ",
        shown.clone(),
        " · cell ",
        key.clone(),
        " → ",
        outcome.display_text()
    ));
    console.print(cat!(
        "[sweep_bench]   셀 상태는 이 기록에서 읽는다: campaign_init.sh --cell-set ",
        key,
        " --outcome ",
        outcome.display_text(),
        " --sweep-state ",
        shown,
        " --utc <UTC>"
    ));
    Ok(())
}

/// The campaign a sweep state belongs to, from its place: `campaigns/<id>/sweeps/<name>.json` → `Some(id)`, the reserved
/// `_bootstrap` instance → `None` (no campaign); any other place is refused (it was not derived from the declaration).
fn campaign_of_state(path: &PyStr) -> Result<Option<String>, String> {
    let shown = path.to_utf8().unwrap_or_default();
    let parts: Vec<&str> = shown.split('/').filter(|part| !part.is_empty() && *part != ".").collect();
    let refuse = || format!("{shown:?} 는 campaigns/<id>/sweeps/<name>.json 이 아니다 — 자리는 campaign_init.sh --derive sweep 의 파생이다(호출자가 고르지 않는다)");
    if parts.len() < 4 || parts.contains(&"..") {
        return Err(refuse());
    }
    let tail = &parts[parts.len() - 4..];
    let named = tail[3].strip_suffix(".json").is_some_and(|stem| !stem.is_empty());
    if tail[0] != "campaigns" || tail[2] != "sweeps" || !named {
        return Err(refuse());
    }
    match tail[1] {
        "_bootstrap" => Ok(None),
        "_template" => Err(format!("{shown:?} 는 뼈대(_template)다 — 인스턴스가 아니다")),
        id => Ok(Some(id.to_string())),
    }
}

/// The state a standalone campaign-cell sweep opens: no declared budget (said so), no planned cells, the record list.
fn standalone_state(context: &Context<'_>, state_path: &PyStr) -> Result<PyValue, Failure> {
    let name = ospath::split(state_path).1.to_utf8().unwrap_or_default();
    let sweep_id = name.strip_suffix(".json").unwrap_or(&name).to_string();
    let mut state = PyDict::new();
    state.set("schema_version", PyValue::int(1));
    state.set("sweep_id", text(PyStr::new(&sweep_id)));
    state.set("topology", text(context.required("TOPO")?));
    state.set("origin", text(PyStr::new(STANDALONE_ORIGIN)));
    state.set("started_utc", text(context.required("STARTED")?));
    state.set("declared_budget", PyValue::None);
    state.set("declared_budget_source", text(PyStr::new(DECLARED_BUDGET_ABSENT)));
    state.set("cells_remaining", PyValue::List(Vec::new()));
    state.set("cells", PyValue::List(Vec::new()));
    Ok(PyValue::Dict(state))
}

/// The record of one finished cell from the classification (`CLS`), the sweep directory and the shell's variables, and
/// the outcome it carries. plan_26111708 B L12: the one record builder — `broad-cell` and `campaign-cell-record` both
/// write what this returns (the citation is the only field whose source differs).
fn cell_record(context: &Context<'_>, citation: Citation) -> Result<(PyDict, PyValue), Failure> {
    let classification = load_text(&context.required("CLS")?)?;
    let sweep_dir = context.required("SWEEPDIR")?;
    let index = or_empty_object(load_sweep_file(&sweep_dir, &["sweep_index.json"])?);
    let verdict = or_empty_object(load_sweep_file(&sweep_dir, &["verdict.json"])?);
    let outcome = get(&classification, "cell_outcome")?;
    let carried = matches!(outcome.peel(), PyValue::Str(own) if own.equals("measured"));
    let (index, verdict) = if carried {
        (index, verdict)
    } else {
        (PyValue::Dict(PyDict::new()), PyValue::Dict(PyDict::new()))
    };
    let judged = if carried {
        or_empty_object(load_sweep_file(&sweep_dir, &["level_01", "measured.json"])?)
    } else {
        PyValue::Dict(PyDict::new())
    };
    let meta = get_or_empty_object(&index, "meta")?;
    let mut vector = PyDict::new();
    let levels = get(&index, "levels")?;
    if levels.is_truthy() {
        for level in iterate(&levels)? {
            let measured = get_or_empty_object(&level, "measured")?;
            let key = get(&level, "level")?.display_text();
            vector.insert(key, get(&measured, "decode_tps")?);
        }
    }
    let carried_text = |reason: PyStr| text(reason);
    let mut cell = PyDict::new();
    cell.set("cell_key", text(context.required("CELL_KEY")?));
    cell.set(
        "sweep_dir",
        if carried {
            text(ospath::relpath(&sweep_dir, &context.required("REPO")?)?)
        } else {
            PyValue::None
        },
    );
    cell.set(
        "sweep_generated_utc",
        if carried {
            get(&index, "generated_utc")?
        } else {
            PyValue::None
        },
    );
    cell.set("config", text(context.required("CONFIG")?));
    cell.set("started_utc", text(context.required("STARTED")?));
    cell.set("ended_utc", text(context.required("ENDED")?));
    let outcome = as_get_target(&classification)?
        .get("cell_outcome")
        .cloned()
        .ok_or_else(|| key_error("cell_outcome"))?;
    cell.set("cell_outcome", outcome.clone());
    cell.set("void_reason", get(&classification, "void_reason")?);
    cell.set(
        "void_reason_source",
        get(&classification, "void_reason_source")?,
    );
    cell.set("note", get(&classification, "note")?);
    cell.set("kill_events", get(&classification, "kill_events")?);
    cell.set(
        "sweep_artifacts_carried",
        if carried {
            PyValue::Bool(true)
        } else {
            carried_text(cat!(
                "false(cell_outcome=",
                get(&classification, "cell_outcome")?.display_text(),
                " — 이 셀은 index·판정점·verdict 를 쓰지 않았다 · 디렉터리의 것은 이전 스윕)"
            ))
        },
    );
    cell.set("lite_verdict", get(&classification, "lite_verdict")?);
    cell.set(
        "lite_verdict_source",
        get(&classification, "lite_verdict_source")?,
    );
    cell.set("lite_raw_status", get(&classification, "lite_raw_status")?);
    cell.set(
        "coordinates",
        pick(
            &meta,
            &[
                "quantization",
                "max_model_len",
                "kv_cache_dtype",
                "tensor_parallel_size",
                "image_tag",
                "image_digest",
                "moe_backend",
                "attention_backend",
                "gpu_memory_utilization",
                "max_num_seqs",
            ],
        )?,
    );
    cell.set(
        "concurrency_vector",
        if vector.is_empty() {
            PyValue::None
        } else {
            PyValue::Dict(vector)
        },
    );
    cell.set(
        "repetition",
        if matches!(get(&classification, "cell_outcome")?.peel(), PyValue::Str(own) if own.equals("measured")) {
            get(&index, "repetition")?
        } else {
            PyValue::None
        },
    );
    for (key, value) in bench_mode_fields(context, &classification, &sweep_dir, &index)? {
        cell.set(key, value);
    }
    let mut capacity = PyDict::new();
    capacity.set(
        "kv_cache_memory_bytes",
        member(&meta, "kv_cache_memory_bytes")?,
    );
    capacity.set("max_model_len", member(&meta, "max_model_len")?);
    cell.set("capacity", PyValue::Dict(capacity));
    let mut measurement = PyDict::new();
    for key in [
        "bench_tool",
        "bench_tool_version",
        "bench_tool_version_source",
        "vllm_version",
        "gpu_model",
        "attention_backend_source",
        "attention_backend_declared",
        "attention_backend_mismatch",
        "attention_backend_candidates",
        "moe_backend_source",
        "moe_backend_declared",
        "moe_backend_mismatch",
    ] {
        measurement.set(key, member(&meta, key)?);
    }
    for (name, key) in [
        ("error_rate", "error_rate"),
        ("server_error_rate", "server_error_rate"),
        ("tool_boundary_errors", "tool_boundary_errors"),
        ("server_errors", "server_errors"),
        ("error_split_source", "error_split_source"),
        ("completed_requests", "completed"),
        ("num_prompts", "num_prompts"),
    ] {
        measurement.set(name, member(&judged, key)?);
    }
    cell.set("measurement", PyValue::Dict(measurement));
    cell.set("verdict_narrative", verdict_narrative(&verdict)?);
    cell.set(
        "axis_citation",
        match citation {
            Citation::Declared => text(context.required("CITATION")?),
            Citation::Absent => PyValue::None,
        },
    );
    cell.set(
        "next_intent",
        context.non_empty("NEXT_INTENT").map_or(PyValue::None, text),
    );
    cell.set("thermal_uncalibrated", uncalibrated(context));
    cell.set(
        "thermal_uncalibrated_ack",
        PyValue::Bool(
            context
                .env
                .get("ACK_UNCAL")
                .is_some_and(|own| own.equals("1")),
        ),
    );
    cell.set("cell_provenance", provenance(context)?);
    if let Some(reason) = context.non_empty("SERVE_FAILED_REASON") {
        cell.set("serve_failed_reason", text(reason.clone()));
        let earlier = or_empty(&get_cell(&cell, "note"));
        cell.set("note", text(cat!(earlier, " / ", reason)));
    }
    Ok((cell, outcome))
}

fn get_cell(cell: &PyDict, key: &str) -> PyValue {
    cell.get(key).cloned().unwrap_or(PyValue::None)
}

/// `[x for x in THERMAL_UNCAL.split(",") if x] or None`.
fn uncalibrated(context: &Context<'_>) -> PyValue {
    let listed = context.env.get("THERMAL_UNCAL").unwrap_or_default();
    let names: Vec<PyValue> = split_commas(&listed)
        .into_iter()
        .filter(|name| !name.is_empty())
        .map(text)
        .collect();
    if names.is_empty() {
        PyValue::None
    } else {
        PyValue::List(names)
    }
}

/// The precheck verdict of the cell as the shell passed it: its JSON, or an `unreadable` note holding the raw text.
fn provenance(context: &Context<'_>) -> Result<PyValue, Failure> {
    let raw = context
        .non_empty("CELL_PROV")
        .unwrap_or_else(|| PyStr::new("null"));
    match crate::pyjson::loads_text(&raw) {
        Ok(parsed) => Ok(parsed),
        Err(error) if error.is_value_error() => {
            let mut note = PyDict::new();
            note.set("status", PyValue::str("unreadable"));
            note.set(
                "raw",
                context.env.get("CELL_PROV").map_or(PyValue::None, text),
            );
            Ok(PyValue::Dict(note))
        }
        Err(error) => Err(super::document::load_failure(error)),
    }
}

/// Replaces the cell with the same key (or appends it) and takes the key out of the remaining cells.
fn replace_cell(state: &mut PyValue, cell: PyDict) -> Result<(), Failure> {
    let key = cell.get("cell_key").cloned().unwrap_or(PyValue::None);
    let cells = get(state, "cells")?;
    let mut kept = Vec::new();
    if cells.is_truthy() {
        for old in iterate(&cells)? {
            let old_key = get(&old, "cell_key")?;
            if old_key != key {
                kept.push(old);
            }
        }
    }
    kept.push(PyValue::Dict(cell));
    let remaining = get(state, "cells_remaining")?;
    let mut still = Vec::new();
    if remaining.is_truthy() {
        for name in iterate(&remaining)? {
            if name != key {
                still.push(name);
            }
        }
    }
    let PyValue::Dict(members) = state else {
        unreachable!("the state was read through `get`, so it is an object")
    };
    members.set("cells", PyValue::List(kept));
    members.set("cells_remaining", PyValue::List(still));
    Ok(())
}
