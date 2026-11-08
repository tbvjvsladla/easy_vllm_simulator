//! Command line of `resolve_guidellm`: the original's argparse interface (`--version`, `--repo`, `--record`,
//! `--record-path`, `--json`, `--self-test`, abbreviations included) with byte-for-byte usage, help and error text
//! (logical program name `resolve_guidellm.py`). The Rust-only surface is the separate `resolve_guidellm_api`.
//!
//! Exit status: 0 resolved, 2 argument or response error, 3 the latest release could not be reached (declare the
//! version instead; there is no fallback to a remembered version). A declared `--version` is not validated and needs no
//! network; an empty one means "none declared". Any other exception ends the program with a traceback marker and
//! `Class: message` on standard error and status 1.

use crate::argv::{self, resolve_guidellm_args as arg, Outcome, RESOLVE_GUIDELLM};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::dataset_prep::{dump_failure, Console};
use crate::dataset_prep_cli::{exit_status, StreamsConsole};
use crate::guidellm::Failure;
use crate::guidellm_release::{
    default_record_path, fetch_latest, is_os_error, is_value_error, merge_record, resolution,
    version_from_tag, ReleaseTransport, RC_ARGS, RC_OK, RC_UNRESOLVED, REPO_DEFAULT,
};
use crate::guidellm_release_http::HttpsTransport;
use crate::guidellm_release_selftest::self_test;
use crate::pydyn::{as_get_target, or_none, Text};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::terminal;

/// Runs the program with the production HTTPS transport and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let mut console = StreamsConsole { io };
    let status = match main_with(args, &mut console, &HttpsTransport::from_process()) {
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
        "InvalidURL" => "http.client.InvalidURL",
        "IncompleteRead" => "http.client.IncompleteRead",
        "URLError" => "urllib.error.URLError",
        "HTTPError" => "urllib.error.HTTPError",
        other => other,
    };
    let mut text = PyStr::new(class);
    if !failure.message.is_empty() {
        text.push_str(": ");
        text.push_py(&failure.message);
    }
    text
}

/// `main(argv)` with the given transport for the release lookup: the exit status, or the exception that ends the
/// program (`SystemExit` included).
pub fn main_with(
    args: &[PyStr],
    console: &mut dyn Console,
    transport: &dyn ReleaseTransport,
) -> Result<i32, Failure> {
    let matches = match argv::parse_with(&RESOLVE_GUIDELLM, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            console.help(RESOLVE_GUIDELLM.help(terminal::columns()));
            return Err(Failure::new("SystemExit", "0"));
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(RESOLVE_GUIDELLM.usage(terminal::columns()));
            text.push_str(RESOLVE_GUIDELLM.prog);
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
    let repo = matches.value(arg::REPO).map_or_else(
        || PyValue::str(REPO_DEFAULT),
        |repo| PyValue::Str(repo.clone()),
    );

    let declared = matches
        .value(arg::VERSION)
        .filter(|version| !version.is_empty());
    let mut res = match declared {
        Some(version) => resolution(
            &PyValue::Str(version.clone()),
            &PyValue::str("declared(--version)"),
            &repo,
            &PyValue::None,
        ),
        None => match latest_resolution(&repo, transport) {
            Ok(res) => res,
            Err(failure) if is_os_error(failure.class) => {
                let message = Text::new()
                    .lit("[resolve_guidellm] UNRESOLVED 최신 릴리즈를 조회하지 못했다(")
                    .lit(failure.class)
                    .lit(": ")
                    .str(&failure.message)
                    .lit(").\n  → 조용히 옛 버전으로 떨어지지 않는다. 쓸 버전을 선언하라: --version <x.y.z>\n")
                    .done();
                console.error(&message);
                return Ok(RC_UNRESOLVED);
            }
            Err(failure) if is_value_error(failure.class) => {
                let message = Text::new()
                    .lit("[resolve_guidellm] ERROR ")
                    .str(&failure.message)
                    .lit("\n")
                    .done();
                console.error(&message);
                return Ok(RC_ARGS);
            }
            Err(failure) => return Err(failure),
        },
    };

    if matches.flag(arg::RECORD) {
        let record_path = matches
            .value(arg::RECORD_PATH)
            .cloned()
            .unwrap_or_else(default_record_path);
        let recorded = merge_record(
            &PyValue::Str(record_path),
            &PyValue::Dict(res.clone()),
            &PyValue::None,
        )?;
        res.set("record_path", recorded);
    }
    if matches.flag(arg::JSON) {
        let document = PyValue::Dict(res).dumps_pretty().map_err(dump_failure)?;
        console.print(&document)?;
    } else {
        let field = |name: &str| or_none(res.get(name));
        let summary = Text::new()
            .lit("guidellm ")
            .val(&field("version"))
            .lit(" (")
            .val(&field("version_source"))
            .lit(")\n  image: ")
            .val(&field("image_ref"))
            .lit("\n  ")
            .val(&field("pull_command"))
            .done();
        console.print(&summary)?;
    }
    Ok(RC_OK)
}

/// The resolution of the latest release: what the upstream says is the latest tag, its publication time as the upstream
/// gave it, and where that was read. Whatever goes wrong is reported, never replaced by a remembered version.
pub fn latest_resolution(
    repo: &PyValue,
    transport: &dyn ReleaseTransport,
) -> Result<PyDict, Failure> {
    let release = fetch_latest(repo, transport)?;
    let tag = or_none(as_get_target(&release)?.get("tag_name"));
    let version = version_from_tag(&tag)?;
    let source = Text::new()
        .lit("measured(GitHub Releases latest · ")
        .val(repo)
        .lit(")")
        .done();
    let published = or_none(as_get_target(&release)?.get("published_at"));
    Ok(resolution(
        &PyValue::Str(version),
        &PyValue::Str(source),
        repo,
        &published,
    ))
}
