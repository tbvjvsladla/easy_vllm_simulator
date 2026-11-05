//! Topology resolution for the benchmark callers: the library of `resolve_topology.py`.
//!
//! A benchmark script that is started without `--topology` asks this module which topology the checkout has. The
//! answer is never inferred here: the manifest is the authority of the topology facts, the branch only selects the
//! manifest channel, and the four-way parity predicate (`topology_parity.py`, a Python process of the checkout - the
//! policy owner) judges whether branch, declared header, manifest and campaign agree. This module reads the manifests
//! that exist, runs that owner once and interprets its verdict; it reproduces none of the owner's rules and has no
//! silent default.
//!
//! The four functions of the original keep their names and meaning:
//!
//! | original | here |
//! |---|---|
//! | `_manifests(repo)` | [`manifests`] |
//! | `_manifest_topology_line(path)` | [`manifest_topology_line`] |
//! | `_run(argv, timeout=60)` | [`run`] |
//! | `resolve(repo, python=None)` | [`resolve`] |
//!
//! Values that come from a caller or from the owner's JSON keep Python's dynamic typing ([`PyValue`]) because the
//! original accepts any shape and fails with the exception Python raises for it (`.get` on a list, `str.join` over an
//! integer). Which interpreter runs the owner when the caller names none is context ([`ResolveContext`]), not a
//! global: the command line default is `python3`, as the original's callers used.

use crate::captured_process::{
    captured_text, run_captured_in, run_captured_within, Captured, ChildSettings,
};
use crate::dataset_commands::exec_argument;
use crate::dataset_prep::{LineDecoder, TEXT_READ_CHUNK};
use crate::guidellm::Failure;
use crate::ospath;
use crate::pydyn::{as_get_target, join, or_none, Text};
use crate::pyjson::{loads_text, LoadError};
use crate::pystr::PyStr;
use crate::pyvalue::{float_repr, PyDict, PyValue};
use num_traits::ToPrimitive;
use std::ffi::OsString;
use std::io::Read;
use std::time::Duration;

/// Resolved.
pub const EXIT_OK: i32 = 0;
/// Command-line error.
pub const EXIT_USAGE: i32 = 2;
/// There is no manifest to judge (not terraformed).
pub const EXIT_UNTERRAFORMED: i32 = 3;
/// The contract is violated, ambiguous, or the judge is absent.
pub const EXIT_UNRESOLVED: i32 = 5;
/// The label the original prefixes every message with.
pub const TAG: &str = "[resolve_topology]";
/// The parity owner, relative to the checkout (`os.path.join(".claude", "policies", "runtime", ...)`).
pub const PARITY_REL: &str = ".claude/policies/runtime/topology_parity.py";
/// Environment variable naming the interpreter that runs the owner when the caller names none.
pub const PYTHON_ENV: &str = "RESOLVE_TOPOLOGY_PYTHON";
/// The interpreter of the command line when [`PYTHON_ENV`] is not set (what the original's callers invoked).
pub const DEFAULT_PYTHON: &str = "python3";
/// The `timeout=60` default of `_run`, in seconds.
pub const RUN_TIMEOUT_SECONDS: u32 = 60;
/// How many trailing characters of the owner's error text a refusal quotes (`[-300:]`).
const QUOTED_TAIL: usize = 300;
/// The class of an input outside what this port models (the original accepts any Python object).
pub const UNSUPPORTED_INPUT: &str = "UnsupportedInput";

// ── context ─────────────────────────────────────────────────────────────────────

/// What a resolution needs besides its arguments: the interpreter that runs the parity owner when the caller names
/// none (the original used `sys.executable`; a native program has no interpreter of its own).
#[derive(Clone, Debug)]
pub struct ResolveContext {
    default_python: PyStr,
}

impl ResolveContext {
    pub fn new(default_python: PyStr) -> Self {
        ResolveContext { default_python }
    }

    /// [`PYTHON_ENV`] (an empty value counts as unset), else [`DEFAULT_PYTHON`].
    pub fn from_environment() -> Self {
        let named = std::env::var_os(PYTHON_ENV)
            .filter(|value| !value.is_empty())
            .map(|value| {
                PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(
                    value.as_os_str(),
                ))
            });
        ResolveContext::new(named.unwrap_or_else(|| PyStr::new(DEFAULT_PYTHON)))
    }

    pub fn default_python(&self) -> &PyStr {
        &self.default_python
    }
}

// ── _manifests ──────────────────────────────────────────────────────────────────

/// `sorted(p for p in glob.glob(os.path.join(repo, "output", "*", "manifest.yaml")) if os.path.isfile(p))`.
///
/// The repository path is part of the glob pattern, so `*`, `?` and `[` in it are patterns, as in the original.
pub fn manifests_of(repo: &PyStr) -> Vec<PyStr> {
    let pattern = ospath::join_all(repo, &["output", "*", "manifest.yaml"]);
    let mut found: Vec<PyStr> = ospath::glob(&pattern)
        .into_iter()
        .filter(ospath::isfile)
        .collect();
    found.sort_by(|a, b| a.code_points().cmp(b.code_points()));
    found
}

/// `_manifests(repo)` for a caller-supplied value: only a string is a path (`os.fspath`).
pub fn manifests(repo: &PyValue) -> Result<Vec<PyStr>, Failure> {
    Ok(manifests_of(&ospath::fspath(repo)?))
}

// ── _manifest_topology_line ─────────────────────────────────────────────────────

/// The value of a line that starts with `topology:`: what follows the first colon up to the first `#`, stripped of
/// white space and then of quote characters; `None` when nothing is left.
fn topology_value(line: &PyStr) -> Option<PyStr> {
    let colon = line
        .code_points()
        .iter()
        .position(|&point| point == u32::from(':'))?;
    let rest = &line.code_points()[colon + 1..];
    let end = rest
        .iter()
        .position(|&point| point == u32::from('#'))
        .unwrap_or(rest.len());
    let stripped = PyStr::from_code_points(rest[..end].to_vec()).strip();
    let is_quote = |point: &u32| *point == u32::from('\'') || *point == u32::from('"');
    let points = stripped.code_points();
    let from = points.iter().position(|point| !is_quote(point));
    let value = match from {
        None => Vec::new(),
        Some(from) => {
            let to = points
                .iter()
                .rposition(|point| !is_quote(point))
                .expect("one is not a quote")
                + 1;
            points[from..to].to_vec()
        }
    };
    (!value.is_empty()).then(|| PyStr::from_code_points(value))
}

fn starts_with_topology_key(line: &PyStr) -> bool {
    let key: Vec<u32> = "topology:".chars().map(u32::from).collect();
    line.code_points().starts_with(&key)
}

/// The first `topology:` line of the file, read the way `for line in open(path, encoding="utf-8")` reads it: one raw
/// read of up to 8 KiB at a time, decoded incrementally with universal newlines, stopping at the first match. A bad
/// byte fails the read that holds it, so a first match in an earlier read is returned before the bad byte is seen,
/// while a bad byte in the same read as the match is an error. Any `OSError` (missing file, a directory, no
/// permission, a failing read) means no value.
fn first_topology_line(path: &PyStr) -> Result<Option<PyStr>, Failure> {
    let os_path = crate::pyfs::os_string(path).map_err(crate::pyfs::FsFailure::into_failure)?;
    let Ok(mut file) = std::fs::File::open(&os_path) else {
        return Ok(None);
    };
    let mut decoder = LineDecoder::default();
    let mut block = vec![0_u8; TEXT_READ_CHUNK];
    loop {
        let count = match file.read(&mut block) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Ok(None),
        };
        let at_end = count == 0;
        if at_end {
            decoder.finish()?;
        } else {
            decoder.feed(&block[..count])?;
        }
        // `for line in handle` discards a line once it is examined: only the lines this read completed are held, so
        // memory follows the read block and the unfinished line, never the lines already passed.
        for line in std::mem::take(&mut decoder.lines) {
            if starts_with_topology_key(&line) {
                return Ok(topology_value(&line));
            }
        }
        if at_end {
            return Ok(None);
        }
    }
}

/// `_manifest_topology_line(path)` for a caller-supplied value. A string is a path; an integer would be a file
/// descriptor to `open()` and is outside what this port models; any other type is the `TypeError` of `open()`.
pub fn manifest_topology_line(path: &PyValue) -> Result<Option<PyStr>, Failure> {
    match path.peel() {
        PyValue::Int(_) | PyValue::Bool(_) => Err(Failure::new(
            UNSUPPORTED_INPUT,
            "an integer is a file descriptor to open(); only a path string is modelled",
        )),
        other => first_topology_line(&ospath::fspath(other)?),
    }
}

// ── _run ────────────────────────────────────────────────────────────────────────

/// `(returncode, stdout, stderr)` of `_run`: the code is `None` when the process did not run to completion.
#[derive(Clone, Debug)]
pub struct RunOutput {
    pub code: Option<i64>,
    pub stdout: PyStr,
    pub stderr: PyStr,
}

impl RunOutput {
    /// The tuple as a JSON-like array.
    pub fn to_value(&self) -> PyValue {
        PyValue::List(vec![
            self.code.map_or(PyValue::None, PyValue::int),
            PyValue::Str(self.stdout.clone()),
            PyValue::Str(self.stderr.clone()),
        ])
    }

    fn not_run(reason: PyStr) -> RunOutput {
        RunOutput {
            code: None,
            stdout: PyStr::default(),
            stderr: reason,
        }
    }
}

/// A positive duration with the text `str(timeout)` gives for it.
#[derive(Clone, Debug)]
pub struct RunLimit {
    span: Duration,
    text: String,
}

impl RunLimit {
    pub fn seconds(count: u32) -> RunLimit {
        RunLimit {
            span: Duration::from_secs(u64::from(count)),
            text: count.to_string(),
        }
    }

    /// The limit a caller's `timeout` value means. An `int` or a finite `float`; zero and negative values are an
    /// already expired deadline (the child starts, is killed and `TimeoutExpired` follows, as in the original).
    pub fn from_value(timeout: &PyValue) -> Result<RunLimit, Failure> {
        let unsupported = |message: &str| Failure::new(UNSUPPORTED_INPUT, message);
        let (seconds, text) = match timeout.peel() {
            PyValue::Int(number) => (number.to_f64().unwrap_or(f64::INFINITY), number.to_string()),
            PyValue::Float(number) => (*number, float_repr(*number)),
            _ => return Err(unsupported(
                "timeout must be an int or a float (no deadline and other types are not modelled)",
            )),
        };
        if !seconds.is_finite() {
            return Err(unsupported("timeout must be a finite number"));
        }
        let span = if seconds <= 0.0 {
            Duration::ZERO
        } else {
            Duration::try_from_secs_f64(seconds)
                .ok()
                .filter(|span| std::time::Instant::now().checked_add(*span).is_some())
                .ok_or_else(|| unsupported("timeout is beyond the representable clock range"))?
        };
        Ok(RunLimit { span, text })
    }

    pub fn span(&self) -> Duration {
        self.span
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The words of a command line: each as the operating system gets it, and as `repr` shows it.
struct CommandLine {
    words: Vec<OsString>,
    shown: Vec<PyValue>,
}

fn command_line(argv: &PyValue) -> Result<CommandLine, Failure> {
    let PyValue::List(items) = argv.peel() else {
        return Err(Failure::new(
            UNSUPPORTED_INPUT,
            "argv must be a list (a string command line and other types are not modelled)",
        ));
    };
    let words = items
        .iter()
        .map(exec_argument)
        .collect::<Result<Vec<_>, _>>()?;
    if words.is_empty() {
        return Err(Failure::new("IndexError", "list index out of range"));
    }
    Ok(CommandLine {
        words,
        shown: items.iter().map(|item| item.peel().clone()).collect(),
    })
}

/// `subprocess.run(argv, capture_output=True, text=True, timeout=...)` with every failure raised: the output is
/// decoded as strict UTF-8 with universal newlines, stdout first.
pub fn capture_text(
    argv: &PyValue,
    limit: &RunLimit,
    settings: &ChildSettings,
) -> Result<RunOutput, Failure> {
    let line = command_line(argv)?;
    let captured = run_captured_in(
        &line.words,
        &line.shown,
        limit.span(),
        limit.text(),
        settings,
    )?;
    decode(captured)
}

fn decode(captured: Captured) -> Result<RunOutput, Failure> {
    let stdout = captured_text(&captured.stdout)?;
    let stderr = captured_text(&captured.stderr)?;
    Ok(RunOutput {
        code: Some(captured.status),
        stdout,
        stderr,
    })
}

/// `except (OSError, subprocess.SubprocessError)`: the failures `_run` turns into a value.
fn is_caught_by_run(failure: &Failure) -> bool {
    matches!(
        failure.class,
        "TimeoutExpired"
            | "OSError"
            | "PermissionError"
            | "FileNotFoundError"
            | "ProcessLookupError"
            | "InterruptedError"
            | "ChildProcessError"
            | "BlockingIOError"
            | "FileExistsError"
            | "NotADirectoryError"
            | "IsADirectoryError"
            | "BrokenPipeError"
            | "ConnectionAbortedError"
            | "ConnectionResetError"
            | "TimeoutError"
            | "ConnectionRefusedError"
    )
}

/// `_run(argv, timeout)`: a process that cannot start or runs past the limit is `(None, "", str(exception))`; a
/// decoding failure of its output and a malformed command line are not caught.
pub fn run(argv: &PyValue, timeout: &PyValue) -> Result<RunOutput, Failure> {
    let limit = RunLimit::from_value(timeout)?;
    run_limited(argv, &limit)
}

fn run_limited(argv: &PyValue, limit: &RunLimit) -> Result<RunOutput, Failure> {
    let line = command_line(argv)?;
    match run_captured_within(&line.words, &line.shown, limit.span(), limit.text()).map(decode) {
        Ok(decoded) => decoded,
        Err(failure) if is_caught_by_run(&failure) => Ok(RunOutput::not_run(failure.message)),
        Err(failure) => Err(failure),
    }
}

// ── resolve ─────────────────────────────────────────────────────────────────────

/// `(rc, topology | None, source or reason)`.
#[derive(Clone, Debug)]
pub struct Resolution {
    pub code: i32,
    pub topology: PyValue,
    pub why: PyStr,
}

impl Resolution {
    fn without_topology(code: i32, why: PyStr) -> Resolution {
        Resolution {
            code,
            topology: PyValue::None,
            why,
        }
    }

    /// The tuple as a JSON-like array.
    pub fn to_value(&self) -> PyValue {
        PyValue::List(vec![
            PyValue::int(self.code),
            self.topology.clone(),
            PyValue::Str(self.why.clone()),
        ])
    }
}

/// The last `QUOTED_TAIL` characters of the stripped text (`text.strip()[-300:]`).
fn quoted_tail(text: &PyStr) -> PyStr {
    let points = text.strip();
    let points = points.code_points();
    let from = points.len().saturating_sub(QUOTED_TAIL);
    PyStr::from_code_points(points[from..].to_vec())
}

/// `json.loads(out) if out.strip() else None`, where a text that is not JSON is `None` and a document nested beyond
/// the interpreter's recursion limit is the `RecursionError` it raises.
fn owner_verdict(out: &PyStr) -> Result<Option<PyValue>, Failure> {
    if out.strip().is_empty() {
        return Ok(None);
    }
    match loads_text(out) {
        Ok(value) => Ok(Some(value)),
        Err(error @ LoadError::TooDeep { .. }) => Err(crate::repeat_api_cli::load_failure(&error)),
        Err(_) => Ok(None),
    }
}

fn unterraformed(why: PyStr) -> Resolution {
    Resolution::without_topology(EXIT_UNTERRAFORMED, why)
}

fn unresolved(why: PyStr) -> Resolution {
    Resolution::without_topology(EXIT_UNRESOLVED, why)
}

/// `resolve(repo, python=None)`: the topology of the checkout `repo`, from the parity owner's verdict.
///
/// No manifest at all is "not terraformed" (3); a manifest without the owner file, an owner that cannot judge, a
/// RED verdict or one without a topology is "unresolved" (5); a verdict whose manifest leg is not `ok` is
/// "not terraformed" (3). Reads only. `python` falsy (`None`, `""`, `0`, ...) means the context's interpreter.
pub fn resolve(
    repo: &PyValue,
    python: &PyValue,
    context: &ResolveContext,
) -> Result<Resolution, Failure> {
    let interpreter = if python.is_truthy() {
        python.peel().clone()
    } else {
        PyValue::Str(context.default_python().clone())
    };
    let repo = ospath::abspath(&ospath::fspath(repo)?)?;
    if manifests_of(&repo).is_empty() {
        return Ok(unterraformed(PyStr::new(
            "output/*/manifest.yaml 이 한 장도 없다(토폴로지 사실 부재 · 존재는 policy:TERRAFORM_FLAG_GATE 의 몫). \
             terraforming_node 로 manifest 를 채우거나 --topology 를 명시하라",
        )));
    }
    let parity = ospath::join_all(&repo, &[PARITY_REL]);
    if !ospath::isfile(&parity) {
        return Ok(unresolved(
            Text::new()
                .lit("4자일치 판정자(")
                .lit(PARITY_REL)
                .lit(")가 없다 — manifest는 있지만 어느 통로가 이 노드의 사실인지 확인할 수 없다")
                .done(),
        ));
    }
    let argv = PyValue::List(vec![
        interpreter,
        PyValue::Str(parity),
        PyValue::str("evaluate"),
        PyValue::str("--repo"),
        PyValue::Str(repo),
        PyValue::str("--format"),
        PyValue::str("json"),
    ]);
    let owner = run_limited(&argv, &RunLimit::seconds(RUN_TIMEOUT_SECONDS))?;
    let verdict = owner_verdict(&owner.stdout)?;
    let code = owner.code;
    let usable = matches!(code, Some(0 | 5));
    let Some(PyValue::Dict(verdict)) = verdict.filter(|_| usable) else {
        let evidence = if owner.stderr.is_empty() {
            &owner.stdout
        } else {
            &owner.stderr
        };
        let shown_code = code.map_or(PyStr::new("None"), |code| PyStr::new(&code.to_string()));
        return Ok(unresolved(
            Text::new()
                .lit("4자일치 판정자가 판정하지 못했다(rc=")
                .str(&shown_code)
                .lit("): ")
                .str(&quoted_tail(evidence))
                .done(),
        ));
    };
    let passed = matches!(verdict.get("verdict"), Some(PyValue::Str(text)) if text.equals("PASS"));
    let topology = or_none(verdict.get("topology"));
    if !passed || !topology.is_truthy() {
        let reasons = match verdict.get("reasons") {
            Some(reasons) if reasons.is_truthy() => join(" / ", reasons)?,
            _ => PyStr::new("사유 없음"),
        };
        return Ok(unresolved(
            Text::new()
                .lit("4자일치 RED — ")
                .str(&reasons)
                .lit(" (자동 교정하지 않는다 · 체크아웃 전환인지 선언 수정인지는 사람이 정한다 · 급하면 --topology 명시)")
                .done(),
        ));
    }
    let legs = or_none(verdict.get("legs"));
    let legs = if legs.is_truthy() {
        legs
    } else {
        PyValue::Dict(PyDict::new())
    };
    let leg = or_none(as_get_target(&legs)?.get("manifest"));
    let leg = if leg.is_truthy() {
        leg
    } else {
        PyValue::Dict(PyDict::new())
    };
    let leg = as_get_target(&leg)?;
    let status = or_none(leg.get("status"));
    if !matches!(&status, PyValue::Str(text) if text.equals("ok")) {
        return Ok(unterraformed(
            Text::new()
                .lit("체크아웃 통로 output/")
                .val(&topology)
                .lit(" 의 manifest 다리가 ")
                .fixed(&PyStr::new(&status.repr()))
                .lit(" 다 — 브랜치 필터만으로는 토폴로지를 고르지 않는다(사실 권위 부재). terraforming_node 로 채우거나 --topology 를 명시하라")
                .done(),
        ));
    }
    let judged = or_none(verdict.get("legs_judged"));
    let judged = if judged.is_truthy() {
        join(",", &judged)?
    } else {
        PyStr::default()
    };
    Ok(Resolution {
        code: EXIT_OK,
        topology: topology.clone(),
        why: Text::new()
            .lit("topology_parity(4자일치 PASS · 판정 다리 ")
            .str(&judged)
            .lit(" · manifest=")
            .val(&or_none(leg.get("path")))
            .lit(")")
            .done(),
    })
}
