//! `main()` of `regen_requirements.py` (lines 149-184) and its `--self-test` branch (`self_test`, lines 109-146).
//!
//! ```text
//! regen_requirements.py [-h] (--from-wheel-url FROM_WHEEL_URL | --use-installed | --self-test) [-o OUT]
//! ```
//!
//! The order of the original is kept: the command line is parsed (the mode group is required and exclusive), `--self-test` runs and returns, else the
//! requirements are read (installed metadata or the wheel at the URL), `parse_requires` filters them, and only then is `OUT` opened for writing
//! (created or truncated in place, symbolic links followed, the name taken as given, `-` being a file called `-`); the file is written whole BEFORE
//! anything is printed, so a failing print leaves the finished file behind. The report lines go to standard output through the shared console
//! (a closed standard output ends with status 120 at exit; a strict ASCII profile fails the first non-ASCII line with its `UnicodeEncodeError`).
//!
//! Exit status: 0; 2 (argparse); 1 (an exception nothing handles, as a traceback). A situation outside the named scope of the shared pieces is
//! [`Stop::Refused`] (status 70): an unsupported scheme or proxy, a lone surrogate in the output text, an unmodelled `PYTHONPATH`.
//!
//! The self-test is the source's own checks run on the native pieces: its mocks of `importlib.metadata` and `urlretrieve` become inputs of the
//! shared helpers (`identity_of`, `wheel_metadata_via` with a file copy), so the temporary directories it creates really exist and are removed.

use crate::fetch_https::FetchEnvironment;
use crate::host::Sink;
use crate::http_direct::{Dial, PyException};
use crate::installed_dist::InstalledProfile;
use crate::main_cmd::{argument_error, basename, intern, split_usage};
use crate::py_errors::{builtin, os_error_with_filename};
use crate::regen_flow::{
    identity_of, installed_metadata, wheel_metadata, wheel_metadata_via, FlowError, Stage,
};
use crate::regen_traceback::{report, PrintSite, Scheme, Site};
use crate::requirements_rules::{parse_requirement_texts, Parsed};
use crate::runtime_patch::StdioProfile;
use crate::tempdir::{with_temporary_directory, Provenance, TempBase};
use crate::thrown::Done;
use crate::torch_main::Refusal;
use crate::wheel_main::ascii_encode_error;
use crate::zip_archive::write_stored_archive;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::cliparse::{
    format_usage, parse, terminal_width, Kind, Opt, Outcome, Spec, Value,
};
use std::collections::BTreeMap;
use std::os::unix::ffi::OsStringExt;

pub const PROGRAM: &str = "regen_requirements.py";
const INSTALLED_SOURCE: &str = "importlib.metadata.distribution('vllm').metadata (container)";
const SELF_TEST_PASS: &str = "[regen] self-test PASS: exact version, prior guard, wheel/installed metadata, malformed identity";
const EXEMPTION_NOTE: &str = "KNOWN_INCOMPAT exemption: transformers<5.15.0 omitted for exact METADATA Version 0.32.0; compatibility and smoke unverified.";
/// The text line 180 of the original searches the rendered header for.
const EXEMPTION_MARKER: &str = "KNOWN_INCOMPAT exemption:";
const EXEMPTION_PRINT: &str = "[regen] KNOWN_INCOMPAT exemption: transformers<5.15.0 omitted for exact 0.32.0; compatibility and smoke unverified";

/// Where the executable's temporary directory base comes from.
pub enum BaseChoice {
    /// `tempfile.gettempdir()` of this process (probed on first use, once).
    Process,
    /// A directory the caller fixed (the checks' owned one).
    Fixed(TempBase),
}

/// What a run is on.
pub struct RegenWorld<'a> {
    pub host: &'a mut dyn Sink,
    pub argv: &'a [PyStr],
    pub columns: Option<&'a str>,
    pub profile: StdioProfile,
    pub environment: &'a FetchEnvironment,
    pub dialer: &'a dyn Dial,
    pub pythonpath: Option<std::ffi::OsString>,
    pub base: BaseChoice,
    /// The product's own declaration of the temp directories and metadata reads of this run.
    pub provenance: Provenance,
    resolved_base: Option<TempBase>,
}

impl<'a> RegenWorld<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host: &'a mut dyn Sink,
        argv: &'a [PyStr],
        columns: Option<&'a str>,
        profile: StdioProfile,
        environment: &'a FetchEnvironment,
        dialer: &'a dyn Dial,
        pythonpath: Option<std::ffi::OsString>,
        base: BaseChoice,
    ) -> RegenWorld<'a> {
        RegenWorld {
            host,
            argv,
            columns,
            profile,
            environment,
            dialer,
            pythonpath,
            base,
            provenance: Provenance::default(),
            resolved_base: None,
        }
    }

    fn temp_base(&mut self) -> Result<TempBase, Box<PyException>> {
        if self.resolved_base.is_none() {
            self.resolved_base = Some(match &self.base {
                BaseChoice::Process => TempBase::of_process()?,
                BaseChoice::Fixed(base) => base.clone(),
            });
        }
        Ok(self.resolved_base.clone().expect("resolved above"))
    }
}

/// How `main` stopped before returning.
pub enum Stop {
    Exit(i32),
    /// An exception nothing handles and the site its traceback starts from.
    Raised {
        error: Box<PyException>,
        site: Site,
    },
    /// A report already in its final text (a failed `print`, a failed write of argparse's output).
    Rendered(PyStr),
    Refused(Refusal),
}

impl Stop {
    pub fn report(&self, script: &PyStr) -> Option<PyStr> {
        match self {
            Stop::Raised { error, site } => Some(report(error, site, script)),
            Stop::Rendered(text) => Some(text.clone()),
            Stop::Exit(_) | Stop::Refused(_) => None,
        }
    }
}

type Flow<T> = Result<T, Stop>;

/// An exception of the temporary-directory handling: no source frame belongs to it, so it prints its block alone.
impl From<Box<PyException>> for Stop {
    fn from(error: Box<PyException>) -> Stop {
        Stop::Raised {
            error,
            site: Site::Wheel {
                stage: Stage::Other,
                scheme: Scheme::Other,
            },
        }
    }
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> Stop {
    Stop::Refused(Refusal {
        boundary,
        detail: detail.into(),
    })
}

pub fn spec(prog: &str) -> Spec {
    let option = |flags: &'static [&'static str], dest, kind, help, group| Opt {
        flags,
        dest,
        kind,
        help,
        required: false,
        group,
    };
    Spec {
        prog: intern(prog),
        description: "",
        options: vec![
            option(&["-h", "--help"], "help", Kind::Help, None, None),
            option(
                &["--from-wheel-url"],
                "from_wheel_url",
                Kind::Text,
                Some("wheel URL (호스트, 빌드 전 1차 생성)"),
                Some(0),
            ),
            option(
                &["--use-installed"],
                "use_installed",
                Kind::Flag,
                Some("컨테이너 내부 정합 재생성"),
                Some(0),
            ),
            option(
                &["--self-test"],
                "self_test",
                Kind::Flag,
                Some("오프라인 회귀 검사"),
                Some(0),
            ),
            option(&["-o", "--out"], "out", Kind::Text, None, None),
        ],
        group_required: true,
        positional: None,
    }
}

/// The parsed command line.
struct Arguments {
    from_wheel_url: Option<PyStr>,
    use_installed: bool,
    self_test: bool,
    out: PyStr,
}

fn arguments_of(values: &BTreeMap<&'static str, Value>) -> Arguments {
    let text = |dest: &str| match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    };
    Arguments {
        from_wheel_url: text("from_wheel_url"),
        use_installed: values.contains_key("use_installed"),
        self_test: values.contains_key("self_test"),
        out: text("out").unwrap_or_else(|| PyStr::new("requirements.txt")),
    }
}

/// The common help formatter puts a description paragraph after the usage; this parser has none: the four line breaks after the usage are two.
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

fn parse_arguments(world: &mut RegenWorld<'_>) -> Flow<Arguments> {
    let program = world
        .argv
        .first()
        .map_or_else(|| PROGRAM.to_string(), basename);
    let spec = spec(&program);
    let usage = PyStr::new(&format_usage(&spec, terminal_width(world.columns)));
    let (status, stdout, stderr, head, rest) = match parse(&spec, &world.argv[1..], world.columns) {
        Outcome::Values(values) => return Ok(arguments_of(&values)),
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

/// The path the traceback frames name for the script: `argv[0]` when it ends in the script's name (made absolute against the working directory
/// when it is relative and has a directory part), else the canonical name.
pub fn script_file(argv0: Option<&PyStr>) -> PyStr {
    let Some(path) = argv0.filter(|path| basename(path) == PROGRAM) else {
        return PyStr::new(PROGRAM);
    };
    let points = path.code_points();
    let is_relative = points.first() != Some(&u32::from('/'));
    let has_directory = points.contains(&u32::from('/'));
    match std::env::current_dir() {
        Ok(cwd) if is_relative && has_directory => {
            use std::os::unix::ffi::OsStrExt;
            let mut absolute = PyStr::from_surrogateescape(cwd.as_os_str().as_bytes());
            absolute.push_str("/");
            absolute.push_py(path);
            absolute
        }
        _ => path.clone(),
    }
}

fn scheme_of(url: &PyStr) -> Scheme {
    let text = url.to_utf8().unwrap_or_default().to_ascii_lowercase();
    match text.split_once(':').map(|(scheme, _)| scheme) {
        Some("http") => Scheme::Http,
        Some("https") => Scheme::Https,
        Some("file") => Scheme::File,
        _ => Scheme::Other,
    }
}

fn flow_failure(error: FlowError, site: impl Fn(Stage) -> Site) -> Stop {
    match error {
        FlowError::Raised { error, stage } => Stop::Raised {
            error,
            site: site(stage),
        },
        FlowError::OutOfScope { boundary, detail } => Stop::Refused(Refusal {
            boundary: Box::leak(boundary.into_boxed_str()),
            detail,
        }),
        FlowError::Transport(transport) => refusal(
            "fetch_transport",
            format!(
                "{} failed: {} (its Python spelling was not captured)",
                transport.phase, transport.detail
            ),
        ),
    }
}

/// `print(text)` for one report line of the run.
fn print_line(
    world: &mut RegenWorld<'_>,
    text: &PyStr,
    site: PrintSite,
    script: &PyStr,
) -> Flow<()> {
    use crate::regen_traceback::print_report;
    if world.profile == StdioProfile::AsciiStrict {
        if let Some(block) = ascii_encode_error(text) {
            return Err(Stop::Rendered(print_report(site, &block, script)));
        }
    }
    let written: Done<()> = world
        .host
        .out(text)
        .and_then(|()| world.host.out(&PyStr::new("\n")));
    written.map_err(|failure| Stop::Rendered(print_report(site, &failure.raised.report(), script)))
}

/// Whether any requirement text starts (up to `< > = ! [`, a space or `;`) with the package `transformers`, after `strip().lower()`.
fn mentions_transformers(requirements: &[PyStr]) -> bool {
    requirements.iter().any(|requirement| {
        let points = requirement.code_points();
        let end = points
            .iter()
            .position(|&code| "<>=![ ;".chars().any(|ch| u32::from(ch) == code))
            .unwrap_or(points.len());
        let name = PyStr::from_code_points(points[..end].to_vec()).strip();
        name.to_utf8().is_some_and(|name| {
            agent_runtime_support::pylower::python_lower(&name) == "transformers"
        })
    })
}

/// The header text and the report lines of a finished read.
struct Report {
    header: PyStr,
    exemption_printed: bool,
}

fn build_header(
    source: &PyStr,
    version: &PyStr,
    requirements: &[PyStr],
    parsed: &Parsed,
) -> Report {
    let mut header = PyStr::new(
        "# vLLM 런타임 의존성 — wheel 의 Requires-Dist(METADATA) 기준(권위 소스).\n# source: ",
    );
    header.push_py(source);
    header.push_str("\n# distribution METADATA Version: ");
    header.push_py(version);
    header.push_str(
        "\n# base 제공분(torch/torchvision/torchaudio/setuptools)은 제외(--no-deps 보호).\n# extra(fastapi[standard] 등)는 그대로 — pip 가 빌드 시 transitive(uvicorn→uvloop) 해소.\n",
    );
    if !parsed.applied.is_empty() {
        header.push_str("# ⚠ KNOWN_INCOMPAT 천장 적용(시간드리프트 회귀 차단; upstream 수정 시 regen_requirements.py KNOWN_INCOMPAT 에서 제거): ");
        header.push_str(&joined(&parsed.applied, "; "));
        header.push_str("\n");
    }
    let exact_version = version.equals(crate::requirements_rules::EXEMPT_VERSION);
    if exact_version && mentions_transformers(requirements) {
        header.push_str(&format!("# {EXEMPTION_NOTE}\n"));
    }
    // line 180 of the original asks the RENDERED header, not the condition that wrote the note: any header text that carries the marker counts,
    // the `# source:` line included
    let exemption_printed = exact_version && contains_text(&header, EXEMPTION_MARKER);
    Report {
        header,
        exemption_printed,
    }
}

/// `needle in text` for code points.
fn contains_text(text: &PyStr, needle: &str) -> bool {
    let needle: Vec<u32> = needle.chars().map(u32::from).collect();
    text.code_points()
        .windows(needle.len())
        .any(|window| window == needle)
}

fn joined(items: &[PyStr], separator: &str) -> String {
    items
        .iter()
        .map(|item| item.to_utf8().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(separator)
}

/// `open(a.out, "w", encoding="utf-8")` and the one write of `text`: the file is created or truncated in place (links followed).
fn write_output(out: &PyStr, text: &PyStr) -> Flow<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let site = Site::OpenOutput;
    let name = out.to_surrogateescape_bytes().map_err(|_| {
        refusal(
            "output_name",
            "the output name cannot be encoded for the file system",
        )
    })?;
    if name.contains(&0) {
        return Err(Stop::Raised {
            error: Box::new(builtin("ValueError", "embedded null byte")),
            site,
        });
    }
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(name));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o666)
        .open(&path)
        .map_err(|error| match os_error_with_filename(&error, out) {
            Some(raised) => Stop::Raised {
                error: Box::new(raised),
                site,
            },
            None => refusal("output_open", error.to_string()),
        })?;
    let Some(encoded) = text.to_utf8() else {
        return Err(refusal(
            "utf8_output",
            "the text holds a lone surrogate, which a strict UTF-8 write refuses; not modelled",
        ));
    };
    file.write_all(encoded.as_bytes())
        .map_err(|error| refusal("output_write", error.to_string()))
}

/// The source's `_self_test`: its checks over the native pieces. The first failing check is its `AssertionError`.
fn self_test(world: &mut RegenWorld<'_>) -> Flow<()> {
    fn check(condition: bool, label: &str) -> Result<(), Stop> {
        if condition {
            Ok(())
        } else {
            Err(Stop::Raised {
                error: Box::new(builtin("AssertionError", label)),
                site: Site::OpenOutput,
            })
        }
    }
    let texts =
        |items: &[&str]| -> Vec<PyStr> { items.iter().map(|item| PyStr::new(item)).collect() };
    let requirements = texts(&["transformers>=5.16.1,<5.20.0", "fastapi>=0.133.0"]);
    for version in [
        PyValue::None,
        PyValue::str("0.31.0"),
        PyValue::str("0.32.1"),
        PyValue::str("0.32.0+local"),
    ] {
        let parsed = parse_requirement_texts(
            &requirements,
            crate::requirements_rules::is_exempt_version(&version),
        );
        let guarded = parsed
            .lines
            .iter()
            .any(|line| line.to_utf8().is_some_and(|line| line.contains("<5.15.0")));
        check(
            guarded
                && parsed
                    .applied
                    .iter()
                    .any(|note| note.equals("transformers<5.15.0")),
            &format!(
                "older/unknown guard: {}",
                version.display_text().to_utf8().unwrap_or_default()
            ),
        )?;
    }
    let parsed = parse_requirement_texts(&requirements, true);
    check(
        joined(&parsed.lines, "|") == "fastapi>=0.133.0,<0.137.0|transformers>=5.16.1,<5.20.0",
        "exact-version requirements",
    )?;
    check(
        joined(&parsed.applied, "|") == "fastapi<0.137.0",
        "fastapi ceiling",
    )?;
    let metadata = PyStr::new(
        "Metadata-Version: 2.3\nName: vllm\nVersion: 0.32.0\nRequires-Dist: transformers>=5.16.1,<5.20.0\nRequires-Dist: fastapi>=0.133.0\n\n",
    );
    let installed_ok = matches!(identity_of(&metadata), Ok((requires, version)) if requires == requirements && version.equals("0.32.0"))
        && matches!(identity_of(&metadata), Ok((requires, _)) if requires == requirements);
    check(installed_ok, "installed metadata")?;
    let base = world.temp_base()?;
    let archive = write_stored_archive(&[(
        "vllm-0.32.0.dist-info/METADATA",
        metadata.to_utf8().unwrap_or_default().as_bytes(),
    )]);
    let mut provenance = std::mem::take(&mut world.provenance);
    let outcome = with_temporary_directory(
        &base,
        &mut provenance,
        |directory, provenance| -> Result<(), Stop> {
            let source = directory.join("source.whl");
            std::fs::write(&source, &archive)
                .map_err(|error| refusal("self_test_write", error.to_string()))?;
            let copy = |destination: &std::path::Path| {
                std::fs::copy(&source, destination)
                    .map(|_| ())
                    .map_err(|error| {
                        let shown = PyStr::new(&destination.to_string_lossy());
                        os_error_with_filename(&error, &shown)
                            .map(PyException::into)
                            .unwrap_or(FlowError::OutOfScope {
                                boundary: "self_test_copy".to_string(),
                                detail: error.to_string(),
                            })
                    })
            };
            let first = wheel_metadata_via(&base, provenance, copy);
            let wheel_ok = matches!(&first, Ok((requires, version)) if *requires == requirements && version.equals("0.32.0"))
                && matches!(wheel_metadata_via(&base, provenance, copy), Ok((requires, _)) if requires == requirements);
            check(wheel_ok, "wheel metadata")
        },
    );
    world.provenance = provenance;
    outcome?;
    for bad in [
        "Name: vllm\n\n",
        "Name: vllm\nVersion: 0.32.0\nVersion: 0.31.0\n\n",
        "Name: other\nVersion: 0.32.0\n\n",
        "Name: vllm\nVersion: bad version\n\n",
    ] {
        check(
            identity_of(&PyStr::new(bad)).is_err(),
            "invalid metadata accepted",
        )?;
    }
    let script = script_file(world.argv.first());
    print_line(
        world,
        &PyStr::new(SELF_TEST_PASS),
        PrintSite::Written,
        &script,
    )
}

pub fn main(world: &mut RegenWorld<'_>) -> Flow<()> {
    let script = script_file(world.argv.first());
    let arguments = parse_arguments(world)?;
    if arguments.self_test {
        return self_test(world);
    }
    let (requirements, version, source) = if arguments.use_installed {
        let profile = InstalledProfile::from_pythonpath(world.pythonpath.as_deref())
            .map_err(|failure| refusal("installed_profile", format!("{failure:?}")))?;
        let (requires, version) = installed_metadata(&profile, &mut world.provenance)
            .map_err(|error| flow_failure(error, |stage| Site::Installed { stage }))?;
        (requires, version, PyStr::new(INSTALLED_SOURCE))
    } else {
        let url = arguments
            .from_wheel_url
            .clone()
            .expect("the mode group is required: a wheel URL or another mode");
        let scheme = scheme_of(&url);
        let base = world.temp_base()?;
        let environment = world.environment;
        let dialer = world.dialer;
        let (requires, version) = wheel_metadata(
            &PyValue::Str(url.clone()),
            environment,
            dialer,
            &base,
            &mut world.provenance,
        )
        .map_err(|error| flow_failure(error, |stage| Site::Wheel { stage, scheme }))?;
        (requires, version, url)
    };
    let parsed = parse_requirement_texts(
        &requirements,
        version.equals(crate::requirements_rules::EXEMPT_VERSION),
    );
    let report = build_header(&source, &version, &requirements, &parsed);
    let mut body = report.header.clone();
    body.push_str(&joined(&parsed.lines, "\n"));
    body.push_str("\n");
    write_output(&arguments.out, &body)?;
    let mut written = PyStr::new("[regen] ");
    written.push_py(&arguments.out);
    written.push_str(&format!(" : {} packages (source: ", parsed.lines.len()));
    written.push_py(&source);
    written.push_str(")");
    print_line(world, &written, PrintSite::Written, &script)?;
    let mut shown_version = PyStr::new("[regen] distribution METADATA Version: ");
    shown_version.push_py(&version);
    print_line(world, &shown_version, PrintSite::Version, &script)?;
    if report.exemption_printed {
        print_line(
            world,
            &PyStr::new(EXEMPTION_PRINT),
            PrintSite::Exemption,
            &script,
        )?;
    }
    if !parsed.applied.is_empty() {
        let text = format!("[regen] ⚠ KNOWN_INCOMPAT 천장 적용: {} — S1 게이트서 재평가/만료 확인(KNOWN_INCOMPAT).", joined(&parsed.applied, ", "));
        print_line(world, &PyStr::new(&text), PrintSite::Ceilings, &script)?;
    }
    Ok(())
}

#[cfg(test)]
mod header_tests {
    use super::*;

    fn texts(items: &[&str]) -> Vec<PyStr> {
        items.iter().map(|item| PyStr::new(item)).collect()
    }

    /// The header of a finished read and whether the exemption line is printed, for a source text, a METADATA version and requirement lines.
    fn render(source: &str, version: &str, requirements: &[&str]) -> (String, bool) {
        let requirements = texts(requirements);
        let version = PyStr::new(version);
        let parsed = parse_requirement_texts(&requirements, version.equals("0.32.0"));
        let report = build_header(&PyStr::new(source), &version, &requirements, &parsed);
        (report.header.to_utf8().unwrap(), report.exemption_printed)
    }

    const NOTE_LINE: &str = "# KNOWN_INCOMPAT exemption: transformers<5.15.0 omitted for exact METADATA Version 0.32.0; compatibility and smoke unverified.\n";

    #[test]
    fn the_exact_version_with_transformers_writes_the_note_and_prints_the_line() {
        let (header, printed) = render(
            "file:///a/pkg.whl",
            "0.32.0",
            &["transformers>=5.16.1", "fastapi>=0.133"],
        );
        assert!(header.contains(NOTE_LINE));
        assert_eq!(header.matches("KNOWN_INCOMPAT exemption:").count(), 1);
        assert!(printed);
    }

    #[test]
    fn without_the_marker_anywhere_nothing_is_printed() {
        // exact version, no transformers: no note, and the source text carries no marker
        assert!(!render("file:///a/pkg.whl", "0.32.0", &["fastapi>=0.133"]).1);
        assert!(!render("file:///a/pkg.whl", "0.32.0", &["fastapi"])
            .0
            .contains("KNOWN_INCOMPAT exemption:"));
        // another version: the ceiling applies, no note, no line
        let (header, printed) = render("file:///a/pkg.whl", "0.31.0", &["transformers>=5.16.1"]);
        assert!(!header.contains("KNOWN_INCOMPAT exemption:") && !printed);
    }

    #[test]
    fn a_marker_in_the_source_text_triggers_the_line_exactly_as_the_original_condition_does() {
        // the original asks the rendered header: the `# source:` line is part of it, so the marker there counts for the exact version only
        let marked = "file:///tmp/KNOWN_INCOMPAT exemption: x/pkg.whl";
        let (header, printed) = render(marked, "0.32.0", &["fastapi>=0.133"]);
        assert!(
            !header.contains(NOTE_LINE),
            "no transformers requirement: the note itself is not written"
        );
        assert!(header.contains(marked));
        assert!(printed);
        let (_, printed) = render(marked, "0.31.0", &["fastapi>=0.133"]);
        assert!(!printed, "the version condition still applies");
        // with transformers present the note is written as well and the line is printed once
        let (header, printed) = render(marked, "0.32.0", &["transformers>=5"]);
        assert!(header.contains(NOTE_LINE) && printed);
    }
}
