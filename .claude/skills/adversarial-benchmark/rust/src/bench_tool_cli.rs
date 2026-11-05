//! Command line of `resolve_bench_tool`: the original's argparse interface (`--tool`, `--tool-version`, `--pin`,
//! `--platform`, `--json`, `--self-test`) with byte-for-byte usage, help and error text (logical program name
//! `resolve_bench_tool.py`). The Rust-only surface is the separate `resolve_bench_tool_api`.
//!
//! Exit status: 0 resolved (the image is on this node), 2 argument or pin-record error, 3 image absent (never
//! pulled). A digest the record does not know is a provenance note on standard error, never a refusal. Standard
//! output is the interpreter's (block-buffered into a pipe); an uncaught exception ends with a traceback marker and
//! `Class: message` on standard error and status 1.

use crate::argv::{self, bench_tool_args as arg, Outcome, BENCH_TOOL};
use crate::bench_tool_pin::{
    classify, default_pin_path, inspect, is_pin_error, load_pin, DEFAULT_TOOL, DIGEST_SOURCE,
    RC_ABSENT, RC_ARGS,
};
use crate::bench_tool_selftest::self_test;
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::dataset_prep::{dump_failure, Console};
use crate::dataset_prep_cli::{exit_status, StreamsConsole};
use crate::guidellm::Failure;
use crate::pydyn::Text;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::terminal;

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let mut console = StreamsConsole { io };
    let status = match main_with(args, &mut console) {
        Ok(status) => status,
        Err(failure) if failure.class == "SystemExit" => exit_status(&failure),
        Err(failure) => {
            let mut text = PyStr::new(TRACEBACK_HEADER);
            text.push_py(&traceback_line(&failure));
            text.push_str("\n");
            write_stderr(console.io, &text);
            1
        }
    };
    io.finish(status)
}

/// `Class: message` as the interpreter prints the last traceback line (library classes carry their module).
fn traceback_line(failure: &Failure) -> PyStr {
    let class = match failure.class {
        "JSONDecodeError" => "json.decoder.JSONDecodeError",
        "TimeoutExpired" => "subprocess.TimeoutExpired",
        other => other,
    };
    let mut text = PyStr::new(class);
    if !failure.message.is_empty() {
        text.push_str(": ");
        text.push_py(&failure.message);
    }
    text
}

fn text_or<'a>(value: Option<&'a PyStr>, default: &'a PyStr) -> &'a PyStr {
    value.unwrap_or(default)
}

/// `main(argv)`: the exit status, or the exception that ends the program (`SystemExit` included).
pub fn main_with(args: &[PyStr], console: &mut dyn Console) -> Result<i32, Failure> {
    let matches = match argv::parse_with(&BENCH_TOOL, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            console.help(BENCH_TOOL.help(terminal::columns()));
            return Err(Failure::new("SystemExit", "0"));
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(BENCH_TOOL.usage(terminal::columns()));
            text.push_str(BENCH_TOOL.prog);
            text.push_str(": error: ");
            text.push_py(&message);
            text.push_str("\n");
            console.error(&text);
            return Err(Failure::new("SystemExit", "2"));
        }
    };
    if matches.flag(arg::SELF_TEST) {
        return self_test(console);
    }
    let default_tool = PyStr::new(DEFAULT_TOOL);
    let tool = text_or(matches.value(arg::TOOL), &default_tool);
    let version = matches
        .value(arg::TOOL_VERSION)
        .map_or(PyValue::None, |version| PyValue::Str(version.clone()));
    let pin = matches
        .value(arg::PIN)
        .cloned()
        .unwrap_or_else(default_pin_path);

    let row = match load_pin(&PyValue::Str(pin), &PyValue::Str(tool.clone()), &version) {
        Ok(row) => row,
        Err(failure) if is_pin_error(&failure) => {
            let mut text = PyStr::new("[resolve_bench_tool] ERROR ");
            text.push_py(&failure.message);
            text.push_str("\n");
            console.error(&text);
            return Ok(RC_ARGS);
        }
        Err(failure) => return Err(failure),
    };
    let image_ref = crate::pydyn::or_none(row.get("image_ref"));
    let local = inspect(&image_ref)?.map_or(PyValue::None, PyValue::List);
    let entry = PyValue::Dict(row);
    let classification = classify(&entry, &local)?;
    let PyValue::Dict(row) = &entry else {
        unreachable!("the entry was built as a dict");
    };

    let recorded = crate::pydyn::or_none(row.get("index_digest"));
    let mut result = PyDict::new();
    result.set("tool", PyValue::Str(tool.clone()));
    result.set("version", crate::pydyn::or_none(row.get("version")));
    result.set(
        "version_source",
        crate::pydyn::or_none(row.get("version_source")),
    );
    result.set("image_ref", image_ref.clone());
    result.set("recorded_digest", recorded.clone());
    result.set(
        "observed_digest",
        classification
            .observed_digest
            .clone()
            .map_or(PyValue::None, PyValue::Str),
    );
    result.set("digest_source", PyValue::str(DIGEST_SOURCE));
    result.set(
        "matches_record",
        classification
            .matches_record
            .map_or(PyValue::None, PyValue::Bool),
    );
    result.set(
        "platform_note",
        matches
            .value(arg::PLATFORM)
            .map_or(PyValue::None, |note| PyValue::Str(note.clone())),
    );
    result.set("status", PyValue::str(classification.resolution.status()));
    if matches.flag(arg::JSON) {
        let document = PyValue::Dict(result).dumps_pretty().map_err(dump_failure)?;
        console.print(&document)?;
    }

    let code = classification.resolution.code();
    if code == RC_ABSENT {
        let message = Text::new()
            .lit("[resolve_bench_tool] ABSENT ")
            .val(&image_ref)
            .lit(" 가 이 노드에 없다. 자동 pull 하지 않는다(airgap-safe 불변식). 사전 스테이징:\n  docker pull ")
            .val(&image_ref)
            .lit("\n")
            .done();
        console.error(&message);
    } else if classification.matches_record == Some(false) {
        let observed = classification
            .observed_digest
            .clone()
            .map_or(PyValue::None, PyValue::Str);
        let message = Text::new()
            .lit("[resolve_bench_tool] NOTE ")
            .val(&image_ref)
            .lit(" 의 로컬 digest(")
            .val(&observed)
            .lit(")가 기록(")
            .val(&recorded)
            .lit(")과 다르다. 막지 않는다 — 실제로 돈 digest 를 결과·리포트에 적는다(측정 provenance). 기록을 갱신하려면 resolve_guidellm.py --record 를 쓴다.\n")
            .done();
        console.error(&message);
    }
    Ok(code)
}
