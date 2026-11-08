//! Public `campaign_read`: the two read-only shapes of `campaign_init.py` that the shell callers use - `--active` and
//! `--derive KIND [--cell|--sweep|--context|--campaign-id VALUE]...` - answered natively, byte for byte as the owner's
//! process answers them (stdout, stderr and exit status).
//!
//! **What is native.** The owner is the reviewed `campaign_init.py` (byte-identical to `REVIEWED_CAMPAIGN`). The read side
//! lives in `role_contract::campaign`; this module adds only what a *process* of the owner shows around it: the argument
//! grammar, the one-line output, the `[campaign_init] FAIL` refusal wiring and the Python 3.12 traceback of an exception the
//! owner does not handle (frames of the owner entry points, of the `pathlib` calls and of the codec).
//!
//! **Routes** (decided before any native business, recorded as one observer line when `CAMPAIGN_READ_ROUTE_OBSERVER` names a
//! file; an unwritable requested file is a control failure and nothing else happens):
//!
//! | route | when | what runs |
//! |---|---|---|
//! | `LegacyExplicit` | `CAMPAIGN_READ_PYTHON` is non-empty | that interpreter on the owner, any shape |
//! | `LegacyShape` | the arguments are not an exact handled shape | the compatibility interpreter on the owner |
//! | `LegacySourceVariant` | the owner is missing, unreadable or a different program | the compatibility interpreter on it |
//! | `LegacyProfile` | the text profile of the environment is not modelled | the compatibility interpreter on the owner |
//! | `NativeCurrentSource` | none of the above | this process, to the end |
//!
//! A legacy route replaces this process (`exec`): same PID, descriptors, environment and arguments, no child and no capture.
//! Once the native business has started nothing is retried in Python: a refusal, a crash, an encode failure and a write
//! failure are answered natively. The compatibility interpreter is `CAMPAIGN_READ_COMPAT_PYTHON` (default `python3`); the
//! owner is `CAMPAIGN_READ_COMPAT_OWNER`, which the launcher exports and which is also the `__file__` of the owner process.

use crate::cli::{PrintFailure, Streams};
use crate::pystr::PyStr as CallerText;
use crate::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pystr::PyStr as Text;
use role_contract::campaign::{self, Derived, OwnedModuleSite};
use role_contract::pyprofile::{self, encode_stream, Errors, TextProfile};
use role_contract::source_identity::{identify, Identity, REVIEWED_CAMPAIGN};
use role_contract::topology::{Crash, Detail, Frame, Where};
use role_contract::topology_cli::push_frame;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};

/// Names of the interpreter, owner, compatibility interpreter and observer variables.
pub const PYTHON_ENV: &str = "CAMPAIGN_READ_PYTHON";
pub const COMPAT_PYTHON_ENV: &str = "CAMPAIGN_READ_COMPAT_PYTHON";
pub const OWNER_ENV: &str = "CAMPAIGN_READ_COMPAT_OWNER";
pub const OBSERVER_ENV: &str = "CAMPAIGN_READ_ROUTE_OBSERVER";
/// Status of a run that could not be set up (private to this program; the owner never exits with 2 on these shapes).
pub const EXIT_CONTROL_FAILURE: i32 = 2;

// ── the owner's own source lines that appear in a traceback ──────────────────────────────

/// One owner frame: line, function, the stripped source line and the sub-expression Python marks with carets
/// (`""`: none). A unit check binds every row to the bytes of the reviewed owner, so an owner edit that moves one fails.
pub struct OwnerFrame {
    pub line: u32,
    pub function: &'static str,
    pub code: &'static str,
    pub expr: &'static str,
}

const ACTIVE_CALL: &str = "print(active_campaign_id()); return 0";
const DERIVE_CALL: &str =
    "print(derive_path(a.derive, cell=a.cell, sweep=a.sweep, context=a.context,";
const DERIVE_ARGUMENTS: &str =
    "derive_path(a.derive, cell=a.cell, sweep=a.sweep, context=a.context,";

/// `sys.exit(main())` at the end of the owner.
pub const MODULE_EXIT: OwnerFrame = OwnerFrame {
    line: 5485,
    function: "<module>",
    code: "sys.exit(main())",
    expr: "main()",
};
/// `main` reaching `active_campaign_id()` for `--active`; the marked part is the call itself.
pub const MAIN_ACTIVE_LOOKUP: OwnerFrame = OwnerFrame {
    line: 5120,
    function: "main",
    code: ACTIVE_CALL,
    expr: "active_campaign_id()",
};
/// The same line when the `print` fails (the whole `print(...)` call is marked).
pub const MAIN_ACTIVE_PRINT: OwnerFrame = OwnerFrame {
    expr: "print(active_campaign_id())",
    ..MAIN_ACTIVE_LOOKUP
};
/// `main` reaching `derive_path(...)` for `--derive`: a multi-line call, so Python shows the first line and marks from the call
/// to the end of that line.
pub const MAIN_DERIVE_LOOKUP: OwnerFrame = OwnerFrame {
    line: 5122,
    function: "main",
    code: DERIVE_CALL,
    expr: DERIVE_ARGUMENTS,
};
/// The same line when the `print` fails: the marked span is the whole stripped line, so Python prints no carets.
pub const MAIN_DERIVE_PRINT: OwnerFrame = OwnerFrame {
    expr: DERIVE_CALL,
    ..MAIN_DERIVE_LOOKUP
};
pub const DERIVE_ACTIVE_LOOKUP: OwnerFrame = OwnerFrame {
    line: 123,
    function: "derive_path",
    code: "camp = camp_id or active_campaign_id(repo_root)",
    expr: "active_campaign_id(repo_root)",
};
pub const ACTIVE_IS_FILE: OwnerFrame = OwnerFrame {
    line: 112,
    function: "active_campaign_id",
    code: "if pointer.is_file():",
    expr: "pointer.is_file()",
};
pub const ACTIVE_READ_TEXT: OwnerFrame = OwnerFrame {
    line: 113,
    function: "active_campaign_id",
    code: "name = pointer.read_text(encoding=\"utf-8\").strip()",
    expr: "pointer.read_text(encoding=\"utf-8\")",
};
pub const ACTIVE_IS_DIR: OwnerFrame = OwnerFrame {
    line: 114,
    function: "active_campaign_id",
    code: "if name and (base / name).is_dir():",
    expr: "(base / name).is_dir()",
};
/// `REPO_ROOT = Path(__file__).resolve().parents[4]` at import: Python marks the receiver with `~` and the subscript with `^`.
pub const IMPORT_REPO_ROOT: OwnerFrame = OwnerFrame {
    line: 32,
    function: "<module>",
    code: "REPO_ROOT = Path(__file__).resolve().parents[4]",
    expr: "",
};
const SUBSCRIPT: &str = "[4]";

/// Every row above, for the source-binding check.
pub const OWNER_FRAMES: [&OwnerFrame; 10] = [
    &MODULE_EXIT,
    &MAIN_ACTIVE_LOOKUP,
    &MAIN_ACTIVE_PRINT,
    &MAIN_DERIVE_LOOKUP,
    &MAIN_DERIVE_PRINT,
    &DERIVE_ACTIVE_LOOKUP,
    &ACTIVE_IS_FILE,
    &ACTIVE_READ_TEXT,
    &ACTIVE_IS_DIR,
    &IMPORT_REPO_ROOT,
];

// ── the exact argument grammar ───────────────────────────────────────────────────────────

/// `--derive KIND` and its optional values; absent options stay `None` (argparse's defaults).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeriveRequest {
    pub kind: String,
    pub cell: Option<String>,
    pub sweep: Option<String>,
    pub context: Option<String>,
    pub campaign_id: Option<String>,
}

/// The handled shapes. Anything else belongs to the owner's `argparse`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Active,
    Derive(DeriveRequest),
}

fn plain_word(word: &[u8]) -> Option<&str> {
    let ok = !word.is_empty()
        && word[0] != b'-'
        && word
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    ok.then(|| std::str::from_utf8(word).expect("ASCII"))
}

/// `[--active]`, or `[--derive KIND]` followed by distinct `--cell|--sweep|--context|--campaign-id VALUE` pairs in any order;
/// KIND and every value are non-empty ASCII words `[A-Za-z0-9._-]` that do not start with `-`. Abbreviations, `--x=y`,
/// duplicates, other options and positionals are not handled.
pub fn parse_exact(args: &[Vec<u8>]) -> Option<Command> {
    match args {
        [only] if only == b"--active" => return Some(Command::Active),
        [flag, kind, rest @ ..] if flag == b"--derive" && rest.len() % 2 == 0 => {
            let mut request = DeriveRequest {
                kind: plain_word(kind)?.to_string(),
                ..DeriveRequest::default()
            };
            for pair in rest.chunks(2) {
                let slot = match pair[0].as_slice() {
                    b"--cell" => &mut request.cell,
                    b"--sweep" => &mut request.sweep,
                    b"--context" => &mut request.context,
                    b"--campaign-id" => &mut request.campaign_id,
                    _ => return None,
                };
                if slot.is_some() {
                    return None;
                }
                *slot = Some(plain_word(&pair[1])?.to_string());
            }
            return Some(Command::Derive(request));
        }
        _ => {}
    }
    None
}

// ── process environment and routes ───────────────────────────────────────────────────────

/// What a run reads from its process besides the arguments.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// The owner program (`__file__` of the owner process); `None` when the launcher did not export it.
    pub owner: Option<Vec<u8>>,
    /// A non-empty `CAMPAIGN_READ_PYTHON`: explicit user intent.
    pub explicit_python: Option<OsString>,
    /// The interpreter of the other legacy routes.
    pub compat_python: OsString,
    pub observer: Option<OsString>,
    /// The text-profile variables as the environment holds them (name -> bytes).
    pub text_variables: Vec<(String, Vec<u8>)>,
}

const PROFILE_VARIABLES: [&str; 6] = [
    "LC_ALL",
    "LC_CTYPE",
    "LANG",
    "PYTHONUTF8",
    "PYTHONIOENCODING",
    "PYTHONCOERCECLOCALE",
];

impl Environment {
    pub fn from_process() -> Self {
        let non_empty = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Environment {
            owner: non_empty(OWNER_ENV).map(OsString::into_vec),
            explicit_python: non_empty(PYTHON_ENV),
            compat_python: non_empty(COMPAT_PYTHON_ENV)
                .unwrap_or_else(|| OsString::from("python3")),
            observer: non_empty(OBSERVER_ENV),
            text_variables: PROFILE_VARIABLES
                .iter()
                .filter_map(|name| {
                    std::env::var_os(name).map(|value| (name.to_string(), value.into_vec()))
                })
                .collect(),
        }
    }

    fn text_profile(&self) -> Result<TextProfile, String> {
        TextProfile::from_environment(&|name| {
            self.text_variables
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    NativeCurrentSource,
    LegacyExplicit,
    LegacyShape,
    LegacySourceVariant { reason: String },
    LegacyProfile { reason: String },
}

impl Route {
    pub fn name(&self) -> &'static str {
        match self {
            Route::NativeCurrentSource => "NativeCurrentSource",
            Route::LegacyExplicit => "LegacyExplicit",
            Route::LegacyShape => "LegacyShape",
            Route::LegacySourceVariant { .. } => "LegacySourceVariant",
            Route::LegacyProfile { .. } => "LegacyProfile",
        }
    }
    fn reason(&self) -> Option<&str> {
        match self {
            Route::LegacySourceVariant { reason } | Route::LegacyProfile { reason } => Some(reason),
            _ => None,
        }
    }
}

/// The interpreter command that replaces this process on a legacy route: `INTERPRETER OWNER ARGS...`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyExec {
    pub interpreter: OsString,
    pub owner: Vec<u8>,
    pub args: Vec<Vec<u8>>,
}

/// How a legacy route is carried out. The program's implementation replaces the process; a check records instead.
pub trait Legacy {
    /// Returns only when the interpreter cannot be started: the status a shell reports (127 not found, 126 not executable).
    fn replace_process(&mut self, spec: &LegacyExec) -> i32;
}

/// `exec`: the same PID, descriptors, environment and working directory; no child.
pub struct ExecLegacy;

impl Legacy for ExecLegacy {
    fn replace_process(&mut self, spec: &LegacyExec) -> i32 {
        use std::os::unix::process::CommandExt;
        let error = std::process::Command::new(&spec.interpreter)
            .arg(OsStr::from_bytes(&spec.owner))
            .args(spec.args.iter().map(|a| OsStr::from_bytes(a)))
            .exec();
        let status = if error.kind() == std::io::ErrorKind::PermissionDenied {
            126
        } else {
            127
        };
        eprintln!(
            "[campaign_read] ERROR 호환 인터프리터를 시작할 수 없다: {}: {error}",
            spec.interpreter.to_string_lossy()
        );
        status
    }
}

/// The routes that precede any native business, in the order they are asked.
pub fn choose_route(args: &[Vec<u8>], env: &Environment, owner: &[u8]) -> Route {
    if env.explicit_python.is_some() {
        return Route::LegacyExplicit;
    }
    if parse_exact(args).is_none() {
        return Route::LegacyShape;
    }
    match identify(owner, REVIEWED_CAMPAIGN) {
        Identity::Reviewed => {}
        Identity::Missing => {
            return Route::LegacySourceVariant {
                reason: "campaign_init.py is missing".to_string(),
            }
        }
        Identity::Different => {
            return Route::LegacySourceVariant {
                reason: "campaign_init.py is not the reviewed source".to_string(),
            }
        }
        Identity::Unreadable(why) => {
            return Route::LegacySourceVariant {
                reason: format!("campaign_init.py cannot be read ({why})"),
            }
        }
    }
    match env.text_profile() {
        Ok(_) => Route::NativeCurrentSource,
        Err(reason) => Route::LegacyProfile { reason },
    }
}

// ── the native business ──────────────────────────────────────────────────────────────

/// What the owner process does with a handled shape, before any byte is written.
#[derive(Debug)]
enum Native {
    /// `print(text)` succeeds: the encoded text (without the newline).
    Line(Vec<u8>),
    /// `PurgeGateRefusal`: `[campaign_init] FAIL <message>` on stderr, status 1.
    Refusal(Text),
    /// An exception the owner does not handle: the whole traceback on stderr, status 1.
    Traceback { text: Text, outcome: &'static str },
}

impl Native {
    fn outcome(&self) -> &'static str {
        match self {
            Native::Line(_) => "success",
            Native::Refusal(_) => "refusal",
            Native::Traceback { outcome, .. } => outcome,
        }
    }
}

/// Where a handled shape's exception arose, which decides the owner frames above it.
#[derive(Clone, Copy)]
enum Entry {
    Active,
    Derive,
}

impl Entry {
    fn lookup(self) -> &'static OwnerFrame {
        match self {
            Entry::Active => &MAIN_ACTIVE_LOOKUP,
            Entry::Derive => &MAIN_DERIVE_LOOKUP,
        }
    }
    fn print(self) -> &'static OwnerFrame {
        match self {
            Entry::Active => &MAIN_ACTIVE_PRINT,
            Entry::Derive => &MAIN_DERIVE_PRINT,
        }
    }
}

fn owner_frame(file: &[u8], frame: &OwnerFrame) -> Frame {
    Frame {
        file: file.to_vec(),
        line: frame.line,
        function: frame.function,
        code: frame.code,
        expr: frame.expr,
    }
}

fn render(frames: &[Frame], class: &str, message: &Text) -> Text {
    let mut last = Text::new(class);
    last.push_str(": ");
    last.push_py(message);
    render_final(frames, &last)
}

/// A traceback whose last line (`Class: message`) is already formed.
fn render_final(frames: &[Frame], last_line: &Text) -> Text {
    let mut out = Text::new("Traceback (most recent call last):\n");
    for frame in frames {
        push_frame(&mut out, frame);
    }
    out.push_py(last_line);
    out.push_str("\n");
    out
}

/// The module-level traceback of an import that fails on `REPO_ROOT = Path(__file__).resolve().parents[4]`.
pub fn import_traceback(owner: &[u8], crash: &Crash) -> Text {
    let mut out = Text::new("Traceback (most recent call last):\n");
    push_frame(&mut out, &owner_frame(owner, &IMPORT_REPO_ROOT));
    // Python marks the receiver with `~` and the subscript with `^`.
    let code = IMPORT_REPO_ROOT.code;
    let expression = code.trim_start_matches("REPO_ROOT = ");
    let start = code.chars().count() - expression.chars().count();
    let receiver = expression.chars().count() - SUBSCRIPT.chars().count();
    out.push_str(&format!(
        "    {}{}{}\n",
        " ".repeat(start),
        "~".repeat(receiver),
        "^".repeat(SUBSCRIPT.chars().count())
    ));
    for frame in Frame::pathlib_parents_index() {
        push_frame(&mut out, &frame);
    }
    out.push_str(crash.class);
    out.push_str(": ");
    out.push_py(&crash.message);
    out.push_str("\n");
    out
}

/// The frames of a crash raised inside `active_campaign_id` (or below it), from `main` down.
fn active_stack(site: &OwnedModuleSite, entry: Entry, crash: &Crash) -> Vec<Frame> {
    let mut frames = vec![
        owner_frame(&site.file, &MODULE_EXIT),
        owner_frame(&site.file, entry.lookup()),
    ];
    if matches!(entry, Entry::Derive) {
        frames.push(owner_frame(&site.file, &DERIVE_ACTIVE_LOOKUP));
    }
    let (call, below) = match &crash.site {
        Where::Campaign { frames: below, .. } if !below.is_empty() => {
            (&ACTIVE_READ_TEXT, below.clone())
        }
        _ => {
            // `T` leaves the frames of a failing `is_file`/`is_dir` empty; the failing path says which one it was.
            let pointer = campaign::join(&campaign::join(&site.root, b"campaigns"), b"ACTIVE");
            match &*crash.detail {
                Detail::Os { filename, .. } if *filename == pointer => {
                    (&ACTIVE_IS_FILE, Frame::pathlib_is_file())
                }
                _ => (&ACTIVE_IS_DIR, Frame::pathlib_is_dir()),
            }
        }
    };
    frames.push(owner_frame(&site.file, call));
    frames.extend(below);
    frames
}

fn business(command: &Command, owner: &[u8], profile: &TextProfile) -> Result<Native, String> {
    let entry = match command {
        Command::Active => Entry::Active,
        Command::Derive(_) => Entry::Derive,
    };
    let site = match campaign::module_site(owner) {
        Ok(site) => site,
        Err(crash) if crash.class == "IndexError" => {
            return Ok(Native::Traceback {
                text: import_traceback(owner, &crash),
                outcome: "crash",
            })
        }
        Err(crash) => {
            return Err(format!(
                "the owner import fails with {} whose Python frames are not reproduced",
                crash.class
            ))
        }
    };
    let answered: Result<Text, Crash> = match command {
        Command::Active => campaign::active_campaign_id(None, &site.root, &site.file),
        Command::Derive(request) => {
            let camp = request.campaign_id.as_deref().map(Text::new);
            match campaign::derive_path(
                &request.kind,
                request.cell.as_deref(),
                request.sweep.as_deref(),
                request.context.as_deref(),
                camp.as_ref(),
                None,
                site.as_site(),
            ) {
                Ok(Derived::Path(bytes)) => Ok(pyprofile::fs_decode(&bytes)),
                Ok(Derived::Refusal(refusal)) => return Ok(Native::Refusal(refusal.0)),
                Err(crash) => Err(crash),
            }
        }
    };
    let text = match answered {
        Ok(text) => text,
        Err(crash) if matches!(crash.site, Where::Opaque) => {
            return Err(format!(
                "{} without reproducible Python frames",
                crash.class
            ))
        }
        Err(crash) => {
            return Ok(Native::Traceback {
                text: render(
                    &active_stack(&site, entry, &crash),
                    crash.class,
                    &crash.message,
                ),
                outcome: "crash",
            })
        }
    };
    Ok(
        match encode_stream(&text, profile.stdout, profile.stdout_errors) {
            Ok(bytes) => Native::Line(bytes),
            Err(failure) => Native::Traceback {
                text: render(
                    &[
                        owner_frame(&site.file, &MODULE_EXIT),
                        owner_frame(&site.file, entry.print()),
                    ],
                    "UnicodeEncodeError",
                    &Text::new(&failure.message),
                ),
                outcome: "encode",
            },
        },
    )
}

// ── observer ─────────────────────────────────────────────────────────────────────────

fn observe(
    env: &Environment,
    route: &Route,
    owner: Option<&[u8]>,
    native_outcome: Option<&str>,
    args: &[Vec<u8>],
) -> Result<(), String> {
    let Some(path) = &env.observer else {
        return Ok(());
    };
    let mut line = PyDict::new();
    line.set("route", PyValue::str(route.name()));
    line.set(
        "legacy_reason",
        route.reason().map_or(PyValue::None, PyValue::str),
    );
    line.set(
        "native_outcome",
        native_outcome.map_or(PyValue::None, PyValue::str),
    );
    line.set("pid", PyValue::int(std::process::id()));
    line.set(
        "owner_path",
        owner.map_or(PyValue::None, |bytes| {
            PyValue::Str(CallerText::from_surrogateescape(bytes))
        }),
    );
    line.set(
        "shape",
        match parse_exact(args) {
            Some(Command::Active) => PyValue::str("active"),
            Some(Command::Derive(request)) => PyValue::str(&format!("derive:{}", request.kind)),
            None => PyValue::None,
        },
    );
    let text = PyValue::Dict(line)
        .dumps_ascii()
        .map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{text}"))
        .map_err(|error| format!("the route observation cannot be written: {error}"))
}

fn control_failure(io: &mut Streams<'_>, message: &str) -> i32 {
    let _ = io
        .stderr
        .write_all(format!("[campaign_read] ERROR {message}\n").as_bytes());
    EXIT_CONTROL_FAILURE
}

// ── the run ──────────────────────────────────────────────────────────────────────────

/// Runs the program. `args` excludes the program name; the return value is the exit status (a legacy route that starts its
/// interpreter never returns).
pub fn run(
    args: &[Vec<u8>],
    env: &Environment,
    io: &mut Streams<'_>,
    legacy: &mut dyn Legacy,
) -> i32 {
    let status = dispatch(args, env, io, legacy);
    io.finish(status)
}

fn dispatch(
    args: &[Vec<u8>],
    env: &Environment,
    io: &mut Streams<'_>,
    legacy: &mut dyn Legacy,
) -> i32 {
    let Some(owner) = env.owner.as_deref() else {
        return control_failure(
            io,
            &format!("{OWNER_ENV} is not set; the launcher campaign_init.sh exports the owner program beside it"),
        );
    };
    let route = choose_route(args, env, owner);
    if route != Route::NativeCurrentSource {
        if let Err(message) = observe(env, &route, Some(owner), None, args) {
            return control_failure(io, &message);
        }
        let interpreter = env
            .explicit_python
            .clone()
            .unwrap_or_else(|| env.compat_python.clone());
        return legacy.replace_process(&LegacyExec {
            interpreter,
            owner: owner.to_vec(),
            args: args.to_vec(),
        });
    }
    let command = parse_exact(args).expect("the route checked the grammar");
    let profile = env
        .text_profile()
        .expect("the route checked the text profile");
    let native = match pyprofile::with_profile(&profile, || business(&command, owner, &profile)) {
        Ok(native) => native,
        Err(message) => return control_failure(io, &message),
    };
    if let Err(message) = observe(env, &route, Some(owner), Some(native.outcome()), args) {
        return control_failure(io, &message);
    }
    match native {
        Native::Line(bytes) => match io.print_encoded(&bytes) {
            Ok(()) => 0,
            Err(failure) => write_failed(io, owner, &command, &profile, &failure),
        },
        Native::Refusal(message) => {
            let mut text = Text::new("[campaign_init] FAIL ");
            text.push_py(&message);
            text.push_str("\n");
            write_error_stream(io, &text, &profile);
            1
        }
        Native::Traceback { text, .. } => {
            write_error_stream(io, &text, &profile);
            1
        }
    }
}

/// stderr: the stdout codec with `backslashreplace` (never fails).
fn write_error_stream(io: &mut Streams<'_>, text: &Text, profile: &TextProfile) {
    let bytes = encode_stream(text, profile.stdout, Errors::BackslashReplace).unwrap_or_default();
    let _ = io.stderr.write_all(&bytes);
}

/// A write of the one output line failed at once: the owner dies with the traceback of that `print` and status 1.
fn write_failed(
    io: &mut Streams<'_>,
    owner: &[u8],
    command: &Command,
    profile: &TextProfile,
    failure: &PrintFailure,
) -> i32 {
    let entry = match command {
        Command::Active => Entry::Active,
        Command::Derive(_) => Entry::Derive,
    };
    let text = render_final(
        &[
            owner_frame(owner, &MODULE_EXIT),
            owner_frame(owner, entry.print()),
        ],
        &Text::new(&failure.line()),
    );
    write_error_stream(io, &text, profile);
    1
}
