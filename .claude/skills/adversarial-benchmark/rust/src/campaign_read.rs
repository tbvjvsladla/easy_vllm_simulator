//! Public `campaign_read`: the two read-only shapes of `campaign_init` that the shell callers use - `--active` and
//! `--derive KIND [--cell|--sweep|--context|--campaign-id VALUE]...` - answered natively, byte for byte as the retired
//! original `campaign_init.py` process answered them (stdout, stderr and exit status).
//!
//! **What is native.** Everything: no route starts a Python interpreter. The read side lives in `role_contract::campaign`; this
//! module adds what a *process* of the original showed around it: the one-line output, the `[campaign_init] FAIL` refusal
//! wiring and the Python 3.12 traceback text of an exception the original did not handle (frames of the original entry points,
//! of the `pathlib` calls and of the codec). The line numbers of those frames are the reviewed original's (frozen data).
//!
//! **Paths.** `CAMPAIGN_READ_COMPAT_OWNER` (exported by the launcher) names the program path beside it - the `__file__` the
//! original had; the checkout root is derived from it lexically (`parents[4]`), and nothing requires that file to exist.
//! A text profile this port does not model falls back to UTF-8 (documented residual). An observer line is written when
//! `CAMPAIGN_READ_ROUTE_OBSERVER` names a file; an unwritable requested file is a control failure.

use crate::cli::{PrintFailure, Streams};
use crate::pystr::PyStr as CallerText;
use crate::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pystr::PyStr as Text;
use role_contract::campaign::{self, Derived, OwnedModuleSite};
use role_contract::pyprofile::{self, encode_stream, Errors, TextProfile};
use role_contract::topology::{Crash, Detail, Frame, Where};
use role_contract::topology_cli::push_frame;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;

/// Names of the interpreter, owner, compatibility interpreter and observer variables.
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
    /// An exact read shape: answered by this process.
    NativeCurrentSource,
    /// Any other shape: the whole `campaign_init` command line (`campaign_args` + `campaign_write`) answers it.
    NotAReadShape,
}

impl Route {
    pub fn name(&self) -> &'static str {
        match self {
            Route::NativeCurrentSource => "NativeCurrentSource",
            Route::NotAReadShape => "NotAReadShape",
        }
    }
    fn reason(&self) -> Option<&str> {
        None
    }
}

/// The route of a command line. No route starts a Python interpreter: the original program is retired and its behavior is
/// the native one (the reviewed-source identity check, the explicit-interpreter route and the unmodelled-text-profile route
/// were the compatibility bridges of the porting period).
pub fn choose_route(args: &[Vec<u8>], _env: &Environment, _owner: &[u8]) -> Route {
    if parse_exact(args).is_none() {
        return Route::NotAReadShape;
    }
    Route::NativeCurrentSource
}

/// The text profile of a run: the environment's, or UTF-8 when the environment names one this port does not model
/// (documented residual: such a locale's codec is not reproduced).
fn profile_of(env: &Environment) -> TextProfile {
    env.text_profile().unwrap_or_else(|_| TextProfile::utf8())
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

/// Runs a read shape. `args` excludes the program name; the return value is the exit status. A vector that is not an exact read
/// shape is a control failure here (the public `campaign_init` program answers it through `campaign_write`).
pub fn run(args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>) -> i32 {
    let status = match parse_exact(args) {
        Some(command) => answer(&command, args, env, io),
        None => control_failure(io, "not a read shape (--active | --derive KIND ...): use campaign_init"),
    };
    io.finish(status)
}

/// Answers one read command natively (`--active`, `--derive ...`), whatever other options the command line carried.
pub fn run_command(command: &Command, env: &Environment, io: &mut Streams<'_>) -> i32 {
    let args = match command {
        Command::Active => vec![b"--active".to_vec()],
        Command::Derive(request) => vec![b"--derive".to_vec(), request.kind.as_bytes().to_vec()],
    };
    let status = answer(command, &args, env, io);
    io.finish(status)
}

fn answer(command: &Command, args: &[Vec<u8>], env: &Environment, io: &mut Streams<'_>) -> i32 {
    let Some(owner) = env.owner.as_deref() else {
        return control_failure(
            io,
            &format!("{OWNER_ENV} is not set; the launcher campaign_init.sh exports the program path beside it"),
        );
    };
    let route = Route::NativeCurrentSource;
    let profile = profile_of(env);
    let native = match pyprofile::with_profile(&profile, || business(command, owner, &profile)) {
        Ok(native) => native,
        Err(message) => return control_failure(io, &message),
    };
    if let Err(message) = observe(env, &route, Some(owner), Some(native.outcome()), args) {
        return control_failure(io, &message);
    }
    match native {
        Native::Line(bytes) => match io.print_encoded(&bytes) {
            Ok(()) => 0,
            Err(failure) => write_failed(io, owner, command, &profile, &failure),
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
