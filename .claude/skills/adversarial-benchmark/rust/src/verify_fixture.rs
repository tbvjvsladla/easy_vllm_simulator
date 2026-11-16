//! Private fixtures shared by the two Broad Search verification programs
//! (`selftest_broad_search_cell_args`, `selftest_broad_search_precheck`).
//!
//! Both programs copy the deployed `broad_search.sh` and the files it reaches into a throw-away git checkout and run
//! it there, so everything here is about building and driving that checkout: where the deployed files are read from
//! ([`resolve_repo_root`]), the temporary directory with its cleanup ([`TemporaryDirectory`]), the byte-for-byte copy
//! ([`IsolatedCheckout::copy_from`]), the children the checks run ([`run_text`], [`run_checked_inherited`]), the
//! printed report ([`Checks`]) and the few Python behaviours the checks lean on (`dict.get`, `or`, `in`).
//!
//! Failures are [`Raised`] values: the exception class and message the original's traceback ends with.

use crate::captured_process::{captured_text, run_captured_in, ChildSettings};
use crate::conformance::{dict_get, iterate_values, Raised};
use crate::dataset_commands::spawn_failure;
use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Where the deployed files the fixtures copy are read from. Without it the repository is taken from the installation
/// around the running binary; a branch name or an agent card never decides it.
pub const REPO_ROOT_ENV: &str = "BENCH_VERIFY_REPO_ROOT";

/// The script directory of the benchmark skill, relative to a repository root (`AB` of the originals).
pub const BENCHMARK_SCRIPTS: &str = ".claude/skills/adversarial-benchmark/scripts";

/// Campaign, cell and serving configuration of the fixtures.
pub const CAMPAIGN: &str = "camp-fx";
pub const CELL: &str = "cell-a";
pub const CONFIG: &str = "cfg-a";
/// The injected clock of every fixture call.
pub const NOW_UTC: &str = "2026-01-01T00:00:00Z";

/// `os.confstr("CS_PATH")` of glibc: the search path of `shutil.which` when `PATH` is not set at all.
const DEFAULT_SEARCH_PATH: &str = "/bin:/usr/bin";
/// The wall-clock limit of every `broad_search.sh` call and of the interpreter probe (`timeout=` of the originals).
const CHILD_TIMEOUT_SECONDS: u64 = 120;
const GIT_TIMEOUT_SECONDS: u64 = 60;
const TEMPORARY_NAME_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
const TEMPORARY_NAME_LENGTH: usize = 8;

// ── where the deployed files are ─────────────────────────────────────────────────

/// The repository the fixtures copy deployed files from: the explicit environment, then the installation of the
/// running binary (`<repo>/.claude/skills/adversarial-benchmark/rust/target/release/<program>`). `None` = unknown.
pub fn resolve_repo_root() -> Option<PathBuf> {
    environment_repo_root().or_else(installed_repo_root)
}

fn environment_repo_root() -> Option<PathBuf> {
    let value = std::env::var_os(REPO_ROOT_ENV).filter(|value| !value.is_empty())?;
    std::path::absolute(value).ok()
}

fn installed_repo_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let release = executable.parent()?;
    let target = release.parent()?;
    let rust = target.parent()?;
    let skill = rust.parent()?;
    let skills = skill.parent()?;
    let claude = skills.parent()?;
    let named = |directory: &Path, name: &str| directory.file_name().is_some_and(|n| n == name);
    let installed = named(release, "release")
        && named(target, "target")
        && named(rust, "rust")
        && named(skill, "adversarial-benchmark")
        && named(skills, "skills")
        && named(claude, ".claude");
    if installed {
        claude.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// What a verifier says when no repository is known (nothing was checked; exit status 2 is this port's).
pub fn repo_root_unknown_message(program: &str) -> String {
    format!(
        "[{program}] ERROR {REPO_ROOT_ENV} 이 없고 실행파일이 설치 경로(rust/target/release) 안에 있지 않아 저장소를 알 수 없다"
    )
}

// ── paths and files ──────────────────────────────────────────────────────────────

pub fn path_text(path: &Path) -> PyStr {
    PyStr::from_surrogateescape(path.as_os_str().as_bytes())
}

fn os_failure(error: &std::io::Error, path: &Path) -> Raised {
    Raised::from_failure(Failure::new(
        pyfs::os_error_class(error),
        &crate::cli::os_error_text(error, &path_text(path)),
    ))
}

fn from_failure(failure: Failure) -> Raised {
    Raised::from_failure(failure)
}

/// `shutil.which(name)`: the first executable regular file named `name` along `PATH`. A `PATH` that is set but empty
/// finds nothing; an unset one falls back to the system default search path.
pub fn which(name: &str) -> Option<PathBuf> {
    let search = std::env::var_os("PATH").unwrap_or_else(|| OsString::from(DEFAULT_SEARCH_PATH));
    if search.is_empty() {
        return None;
    }
    std::env::split_paths(&search)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            candidate.exists()
                && rustix::fs::access(candidate, rustix::fs::Access::EXEC_OK).is_ok()
                && !candidate.is_dir()
        })
}

/// `Path.exists()`: absence (or a path that cannot name a file) is `False`; any other error is raised.
pub fn exists(path: &Path) -> Result<bool, Raised> {
    const NOT_A_FILE_ERRNOS: [i32; 4] = [2, 20, 9, 40]; // ENOENT, ENOTDIR, EBADF, ELOOP
    match std::fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error)
            if error
                .raw_os_error()
                .is_some_and(|errno| NOT_A_FILE_ERRNOS.contains(&errno)) =>
        {
            Ok(false)
        }
        Err(error) => Err(os_failure(&error, path)),
    }
}

/// `Path.is_file()`.
pub fn is_file(path: &Path) -> Result<bool, Raised> {
    pyfs::is_file(&path_text(path)).map_err(from_failure)
}

/// `Path.unlink(missing_ok=True)`: only the absence of the file is forgiven.
pub fn unlink_if_present(path: &Path) -> Result<(), Raised> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(os_failure(&error, path)),
        _ => Ok(()),
    }
}

/// `Path.unlink()`.
pub fn unlink(path: &Path) -> Result<(), Raised> {
    pyfs::unlink(&path_text(path)).map_err(from_failure)
}

/// `Path.read_text(encoding="utf-8")`: strict UTF-8, universal newlines.
pub fn read_text(path: &Path) -> Result<String, Raised> {
    let bytes = pyfs::read_bytes(&path_text(path)).map_err(from_failure)?;
    let text = decode_file_text(&bytes)
        .map_err(|error| from_failure(crate::repeat_api_cli::load_failure(&error)))?;
    Ok(text
        .to_utf8()
        .expect("strictly decoded text holds no surrogates"))
}

/// `Path.read_bytes()`.
pub fn read_bytes(path: &Path) -> Result<Vec<u8>, Raised> {
    pyfs::read_bytes(&path_text(path)).map_err(from_failure)
}

/// `Path.write_bytes(data)`.
pub fn write_bytes(path: &Path, data: &[u8]) -> Result<(), Raised> {
    pyfs::write_bytes(&path_text(path), data).map_err(from_failure)
}

/// `json.loads(path.read_text(encoding="utf-8"))`.
pub fn load_json(path: &Path) -> Result<PyValue, Raised> {
    let text = read_text(path)?;
    loads_text(&PyStr::new(&text))
        .map_err(|error| from_failure(crate::repeat_api_cli::load_failure(&error)))
}

/// The fixtures' `_write`: parent folders first, then the text as UTF-8.
pub fn write_text(path: &Path, text: &PyStr) -> Result<(), Raised> {
    if let Some(parent) = path.parent() {
        pyfs::mkdir_parents_exist_ok(&path_text(parent)).map_err(from_failure)?;
    }
    pyfs::write_text(&path_text(path), text).map_err(from_failure)
}

pub fn write_str(path: &Path, text: &str) -> Result<(), Raised> {
    write_text(path, &PyStr::new(text))
}

/// `Path.chmod(mode)`.
pub fn chmod(path: &Path, mode: u32) -> Result<(), Raised> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| os_failure(&error, path))
}

/// `shutil.copy2(source, destination)`: the bytes, the permission bits and the times. A failure names the file it
/// concerns: the source when it cannot be opened or read, the destination when it cannot be written.
///
/// A destination that is the same file as the source (the same device and inode, as with a hard link) is refused with
/// `shutil.SameFileError` before anything is opened for writing: opening it would truncate the source itself.
pub fn copy2(source: &Path, destination: &Path) -> Result<(), Raised> {
    if is_same_file(source, destination) {
        return Err(Raised::new(
            "SameFileError",
            &format!(
                "PosixPath({}) and PosixPath({}) are the same file",
                path_text(source).repr(),
                path_text(destination).repr()
            ),
        )
        .with_qualified_class("shutil.SameFileError".to_string()));
    }
    let mut reader = std::fs::File::open(source).map_err(|error| os_failure(&error, source))?;
    let mut writer =
        std::fs::File::create(destination).map_err(|error| os_failure(&error, destination))?;
    let mut block = vec![0_u8; 64 * 1024];
    loop {
        let count = match reader.read(&mut block) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(os_failure(&error, source)),
        };
        writer
            .write_all(&block[..count])
            .map_err(|error| os_failure(&error, destination))?;
    }
    let metadata = reader
        .metadata()
        .map_err(|error| os_failure(&error, source))?;
    writer
        .set_permissions(metadata.permissions())
        .map_err(|error| os_failure(&error, destination))?;
    let mut times = std::fs::FileTimes::new();
    if let Ok(accessed) = metadata.accessed() {
        times = times.set_accessed(accessed);
    }
    if let Ok(modified) = metadata.modified() {
        times = times.set_modified(modified);
    }
    writer
        .set_times(times)
        .map_err(|error| os_failure(&error, destination))
}

/// `os.path.samefile`: both exist and name the same device and inode (an error reading either is "not the same").
fn is_same_file(first: &Path, second: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(first), std::fs::metadata(second)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

// ── temporary directory ──────────────────────────────────────────────────────────

/// `with tempfile.TemporaryDirectory(prefix=...)`: created `0700` under the system temporary root with a random
/// eight-character suffix, removed by [`close`](Self::close) on the normal path and by `Drop` when an error leaves
/// early (that removal cannot report a failure; the normal path does).
pub struct TemporaryDirectory {
    path: Option<PathBuf>,
}

impl TemporaryDirectory {
    pub fn create(prefix: &str) -> Result<Self, Raised> {
        let root = temporary_root()?;
        for _ in 0..10_000 {
            let path = root.join(format!("{prefix}{}", random_name()));
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(TemporaryDirectory { path: Some(path) }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(os_failure(&error, &path)),
            }
        }
        Err(from_failure(Failure::new(
            "FileExistsError",
            "[Errno 17] No usable temporary directory name found",
        )))
    }

    pub fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("a directory that is closed is no longer used")
    }

    /// Removes the directory and everything in it; a failure is an `OSError` (what leaving the `with` block raises).
    pub fn close(mut self) -> Result<(), Raised> {
        match self.path.take() {
            Some(path) => std::fs::remove_dir_all(&path).map_err(|error| os_failure(&error, &path)),
            None => Ok(()),
        }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn random_name() -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    // Rejection sampling keeps every alphabet letter equally likely.
    let limit = 256 - 256 % TEMPORARY_NAME_ALPHABET.len();
    let random = SystemRandom::new();
    let mut name = String::with_capacity(TEMPORARY_NAME_LENGTH);
    while name.len() < TEMPORARY_NAME_LENGTH {
        let mut byte = [0_u8; 1];
        if random.fill(&mut byte).is_err() {
            // No entropy source: the clock still separates two runs (the name only has to be unused).
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.subsec_nanos());
            byte[0] = nanos.to_le_bytes()[name.len() % 4];
        }
        if usize::from(byte[0]) < limit {
            name.push(char::from(
                TEMPORARY_NAME_ALPHABET[usize::from(byte[0]) % TEMPORARY_NAME_ALPHABET.len()],
            ));
        }
    }
    name
}

/// The fixed system roots `tempfile` tries on Linux after the environment, in its order.
const SYSTEM_TEMPORARY_ROOTS: [&str; 3] = ["/tmp", "/var/tmp", "/usr/tmp"];

/// `tempfile._candidate_tempdir_list()` for Linux: the non-empty values of `TMPDIR`, `TEMP` and `TMP` in that order,
/// the fixed system roots, and last the working directory (`.` when it cannot be read). `environment` looks a variable
/// up; the platform's own notion of a default root is deliberately not used, because it would hand back an unusable
/// `TMPDIR` instead of falling through to the next candidate.
fn temporary_candidates(
    environment: impl Fn(&str) -> Option<OsString>,
    working_directory: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = ["TMPDIR", "TEMP", "TMP"]
        .iter()
        .filter_map(|name| environment(name).filter(|value| !value.is_empty()))
        .map(PathBuf::from)
        .collect();
    candidates.extend(SYSTEM_TEMPORARY_ROOTS.map(PathBuf::from));
    candidates.push(working_directory.unwrap_or_else(|| PathBuf::from(".")));
    candidates
}

/// `tempfile.gettempdir()`: the first candidate in which a probe file can be created and removed. A candidate other
/// than `.` is made absolute first. When none works the error lists the candidates as they were given.
fn first_usable_temporary_root(candidates: &[PathBuf]) -> Result<PathBuf, Raised> {
    for candidate in candidates {
        let directory = if candidate.as_os_str() == "." {
            candidate.clone()
        } else {
            std::path::absolute(candidate).unwrap_or_else(|_| candidate.clone())
        };
        if directory_is_usable(&directory) {
            return Ok(directory);
        }
    }
    let shown = PyValue::List(
        candidates
            .iter()
            .map(|path| PyValue::Str(path_text(path)))
            .collect(),
    );
    Err(from_failure(Failure::new(
        "FileNotFoundError",
        &format!(
            "[Errno 2] No usable temporary directory found in {}",
            shown.repr()
        ),
    )))
}

fn temporary_root() -> Result<PathBuf, Raised> {
    first_usable_temporary_root(&temporary_candidates(
        |name| std::env::var_os(name),
        std::env::current_dir().ok(),
    ))
}

fn directory_is_usable(directory: &Path) -> bool {
    for _ in 0..100 {
        let probe = directory.join(random_name());
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&probe)
        {
            Ok(mut file) => {
                let written = file.write_all(b"blat");
                drop(file);
                let removed = std::fs::remove_file(&probe);
                return written.is_ok() && removed.is_ok();
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return false,
        }
    }
    false
}

// ── children ─────────────────────────────────────────────────────────────────────

/// A finished child as `subprocess.run(..., capture_output=True, text=True)` returns it.
pub struct TextRun {
    pub status: i64,
    pub stdout: String,
    pub stderr: String,
}

fn command_words(words: &[OsString]) -> (Vec<PyValue>, Vec<OsString>) {
    let shown = words
        .iter()
        .map(|word| PyValue::Str(PyStr::from_surrogateescape(word.as_bytes())))
        .collect();
    (shown, words.to_vec())
}

fn decode_stream(bytes: &[u8]) -> Result<String, Raised> {
    let text = captured_text(bytes).map_err(from_failure)?;
    Ok(text
        .to_utf8()
        .expect("strictly decoded text holds no surrogates"))
}

/// `subprocess.run(command, cwd=..., env=..., capture_output=True, text=True, timeout=120)`.
pub fn run_text(command: &[OsString], settings: &ChildSettings) -> Result<TextRun, Raised> {
    run_text_within(command, settings, CHILD_TIMEOUT_SECONDS)
}

/// [`run_text`] with the wall-clock limit of the call (`timeout=<seconds>`); the verification programs that do not
/// use the 120 seconds of the Broad Search ones (judge: 180, a full report render: 60) name theirs.
pub fn run_text_within(
    command: &[OsString],
    settings: &ChildSettings,
    seconds: u64,
) -> Result<TextRun, Raised> {
    let (shown, words) = command_words(command);
    let limit_text = seconds.to_string();
    let captured = run_captured_in(
        &words,
        &shown,
        Duration::from_secs(seconds),
        &limit_text,
        settings,
    )
    .map_err(from_failure)?;
    // `communicate` decodes the output stream first, then the error stream.
    let stdout = decode_stream(&captured.stdout)?;
    let stderr = decode_stream(&captured.stderr)?;
    Ok(TextRun {
        status: captured.status,
        stdout,
        stderr,
    })
}

/// `subprocess.run(command, check=True, timeout=60)`: the child shares this process's input and output. A child that
/// ran past the limit is killed and reaped; a failing child is a `CalledProcessError`.
pub fn run_checked_inherited(command: &[OsString]) -> Result<(), Raised> {
    let (shown, words) = command_words(command);
    let shown_command = PyValue::List(shown).repr();
    let mut child = Command::new(&words[0])
        .args(&words[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| from_failure(spawn_failure(&error, &words[0])))?;
    let deadline = Instant::now() + Duration::from_secs(GIT_TIMEOUT_SECONDS);
    let mut delay = 0.0005_f64;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => return Err(from_failure(spawn_failure(&error, &words[0]))),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(from_failure(Failure::new(
                "TimeoutExpired",
                &format!("Command '{shown_command}' timed out after {GIT_TIMEOUT_SECONDS} seconds"),
            )));
        }
        delay = (delay * 2.0).min(0.05).min(remaining.as_secs_f64());
        std::thread::sleep(Duration::from_secs_f64(delay));
    };
    if status.success() {
        return Ok(());
    }
    Err(from_failure(Failure::new(
        "CalledProcessError",
        &called_process_message(&shown_command, status),
    )))
}

/// `str(subprocess.CalledProcessError)`.
fn called_process_message(shown_command: &str, status: std::process::ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => {
            format!("Command '{shown_command}' returned non-zero exit status {code}.")
        }
        (None, Some(signal)) => match signal_name(signal) {
            Some(name) => {
                format!("Command '{shown_command}' died with <Signals.{name}: {signal}>.")
            }
            None => format!("Command '{shown_command}' died with unknown signal {signal}."),
        },
        (None, None) => format!("Command '{shown_command}' returned non-zero exit status -1."),
    }
}

/// `signal.Signals(number).name` on Linux.
fn signal_name(signal: i32) -> Option<&'static str> {
    const NAMES: [&str; 31] = [
        "SIGHUP",
        "SIGINT",
        "SIGQUIT",
        "SIGILL",
        "SIGTRAP",
        "SIGABRT",
        "SIGBUS",
        "SIGFPE",
        "SIGKILL",
        "SIGUSR1",
        "SIGSEGV",
        "SIGUSR2",
        "SIGPIPE",
        "SIGALRM",
        "SIGTERM",
        "SIGSTKFLT",
        "SIGCHLD",
        "SIGCONT",
        "SIGSTOP",
        "SIGTSTP",
        "SIGTTIN",
        "SIGTTOU",
        "SIGURG",
        "SIGXCPU",
        "SIGXFSZ",
        "SIGVTALRM",
        "SIGPROF",
        "SIGWINCH",
        "SIGIO",
        "SIGPWR",
        "SIGSYS",
    ];
    usize::try_from(signal)
        .ok()
        .and_then(|number| number.checked_sub(1))
        .and_then(|index| NAMES.get(index).copied())
}

// ── the printed report ───────────────────────────────────────────────────────────

/// The programs' `ck(label, cond, detail)`: one `  PASS label` / `  FAIL label` line per check, and the failed
/// checks' `label — detail` kept for the closing summary.
pub struct Checks<'a> {
    console: &'a mut dyn Console,
    failures: Vec<String>,
}

impl<'a> Checks<'a> {
    pub fn new(console: &'a mut dyn Console) -> Self {
        Checks {
            console,
            failures: Vec::new(),
        }
    }

    pub fn check(&mut self, label: &str, condition: bool, detail: &str) -> Result<(), Raised> {
        let verdict = if condition { "PASS" } else { "FAIL" };
        self.console
            .print(&PyStr::new(&format!("  {verdict} {label}")))
            .map_err(from_failure)?;
        if !condition {
            self.failures.push(if detail.is_empty() {
                label.to_string()
            } else {
                format!("{label} — {detail}")
            });
        }
        Ok(())
    }

    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    pub fn print(&mut self, line: &str) -> Result<(), Raised> {
        self.console.print(&PyStr::new(line)).map_err(from_failure)
    }

    /// `print(line, file=sys.stderr)`.
    pub fn print_error(&mut self, line: &str) {
        self.console.error(&PyStr::new(&format!("{line}\n")));
    }
}

/// The words of a command line as the operating system takes them.
pub fn words(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

/// `text[-count:]`.
pub fn tail(text: &str, count: usize) -> String {
    let skip = text.chars().count().saturating_sub(count);
    text.chars().skip(skip).collect()
}

/// `text[:count]`.
pub fn head(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

/// `f"rc={run.returncode} {run.stderr[-count:]}"`: the detail of most checks that ran `broad_search.sh`.
pub fn status_and_stderr_tail(run: &TextRun, count: usize) -> String {
    format!("rc={} {}", run.status, tail(&run.stderr, count))
}

// ── JSON values of the fixtures ──────────────────────────────────────────────────

pub fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

pub fn list(items: Vec<PyValue>) -> PyValue {
    PyValue::List(items)
}

pub fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn dump_failure(error: &crate::pyvalue::DumpError) -> Raised {
    let text = error.to_string();
    let message = text
        .strip_prefix(&format!("{}: ", error.python_type()))
        .unwrap_or(&text);
    Raised::new(error.python_type(), message)
}

/// `json.dumps(value)`: ASCII-escaped.
pub fn dumps(value: &PyValue) -> Result<PyStr, Raised> {
    value
        .dumps_ascii()
        .map(|text| PyStr::new(&text))
        .map_err(|error| dump_failure(&error))
}

/// `json.dumps(value, ensure_ascii=False)`.
pub fn dumps_unicode(value: &PyValue) -> Result<PyStr, Raised> {
    value.dumps_unicode().map_err(|error| dump_failure(&error))
}

/// `json.dumps(value, ensure_ascii=False, indent=2)`.
pub fn dumps_pretty(value: &PyValue) -> Result<PyStr, Raised> {
    value.dumps_pretty().map_err(|error| dump_failure(&error))
}

/// `json.dumps(value, ensure_ascii=False)` as text for a check's detail line.
pub fn detail_json(value: &PyValue) -> Result<String, Raised> {
    Ok(dumps_unicode(value)?.to_utf8().unwrap_or_default())
}

/// `json.dumps(value)` (ASCII-escaped) as text for a check's detail line.
pub fn detail_json_ascii(value: &PyValue) -> Result<String, Raised> {
    Ok(dumps(value)?.to_utf8().unwrap_or_default())
}

/// `value.get(key)`: only a dict has `get`; a missing key is `None`.
pub fn get(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    dict_get(value, key)
}

/// `value or default`.
pub fn or_default(value: PyValue, default: PyValue) -> PyValue {
    if value.is_truthy() {
        value
    } else {
        default
    }
}

/// `(value.get(key) or {})`.
pub fn get_object(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    Ok(or_default(get(value, key)?, PyValue::Dict(PyDict::new())))
}

/// `for item in (value or [])`.
pub fn items_or_empty(value: PyValue) -> Result<Vec<PyValue>, Raised> {
    iterate_values(&or_default(value, PyValue::List(Vec::new())))
}

/// `dict.pop(key, None)` on a loaded document: the document without that key (the order of the others kept). Only a
/// dict has `pop`.
pub fn without_key(document: PyValue, key: &str) -> Result<PyValue, Raised> {
    match document.peel() {
        PyValue::Dict(map) => {
            let mut rest = PyDict::new();
            for (name, value) in map.iter() {
                if !name.equals(key) {
                    rest.insert(name.clone(), value.clone());
                }
            }
            Ok(PyValue::Dict(rest))
        }
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'pop'", other.type_name()),
        )),
    }
}

/// `dict(template, id=value)` for a loaded JSON document: a copy with `id` set (an existing key keeps its place).
pub fn with_id(template: &PyValue, id: &str) -> Result<PyValue, Raised> {
    match template.peel() {
        PyValue::Dict(map) => {
            let mut copy = map.clone();
            copy.set("id", text(id));
            Ok(PyValue::Dict(copy))
        }
        PyValue::Str(own) if own.code_points().is_empty() => Ok(object(vec![("id", text(id))])),
        PyValue::List(items) if items.is_empty() => Ok(object(vec![("id", text(id))])),
        PyValue::Str(_) => Err(Raised::new(
            "ValueError",
            "dictionary update sequence element #0 has length 1; 2 is required",
        )),
        PyValue::List(items) => match items[0].peel() {
            PyValue::List(pair) if pair.len() == 2 => {
                let mut map = PyDict::new();
                for item in items {
                    match item.peel() {
                        PyValue::List(pair) if pair.len() == 2 => match pair[0].peel() {
                            PyValue::Str(key) => map.insert(key.clone(), pair[1].clone()),
                            _ => return Err(unsupported_dictionary_source()),
                        },
                        _ => return Err(unsupported_dictionary_source()),
                    }
                }
                map.set("id", text(id));
                Ok(PyValue::Dict(map))
            }
            PyValue::List(pair) => Err(Raised::new(
                "ValueError",
                &format!(
                    "dictionary update sequence element #0 has length {}; 2 is required",
                    pair.len()
                ),
            )),
            PyValue::Str(own) => Err(Raised::new(
                "ValueError",
                &format!(
                    "dictionary update sequence element #0 has length {}; 2 is required",
                    own.code_points().len()
                ),
            )),
            _ => Err(Raised::new(
                "TypeError",
                "cannot convert dictionary update sequence element #0 to a sequence",
            )),
        },
        other => Err(Raised::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

fn unsupported_dictionary_source() -> Raised {
    Raised::new(
        "TypeError",
        "a template that is a list of pairs is not a lockset document",
    )
}

/// `needle in value` for a loaded JSON value: substring of a string, member of a list, key of a dict.
pub fn contains(value: &PyValue, needle: &str) -> Result<bool, Raised> {
    match value.peel() {
        PyValue::Str(own) => Ok(own.to_utf8().is_some_and(|text| text.contains(needle))),
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items
            .iter()
            .any(|item| matches!(item.peel(), PyValue::Str(own) if own.equals(needle)))),
        PyValue::Dict(map) => Ok(map.contains_key(needle)),
        other => Err(Raised::new(
            "TypeError",
            &format!("argument of type '{}' is not iterable", other.type_name()),
        )),
    }
}

/// `value == "text"`.
pub fn is_text(value: &PyValue, expected: &str) -> bool {
    matches!(value.peel(), PyValue::Str(own) if own.equals(expected))
}

// ── the isolated checkout ────────────────────────────────────────────────────────

/// A temporary git checkout with a `PATH` shim directory beside it: the place the copies of the deployed files run.
/// The checkout is `<temporary>/repo`, the shims are `<temporary>/bin`; neither exists until something is written.
pub struct IsolatedCheckout {
    temp: TemporaryDirectory,
    root: PathBuf,
    bin_dir: PathBuf,
}

impl IsolatedCheckout {
    pub fn create(prefix: &str) -> Result<Self, Raised> {
        let temp = TemporaryDirectory::create(prefix)?;
        let root = temp.path().join("repo");
        let bin_dir = temp.path().join("bin");
        Ok(IsolatedCheckout {
            temp,
            root,
            bin_dir,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// The deployed files, byte for byte, at the same relative paths (nothing is re-implemented).
    pub fn copy_from(&self, source_root: &Path, relative_paths: &[&str]) -> Result<(), Raised> {
        for relative in relative_paths {
            let destination = self.root.join(relative);
            if let Some(parent) = destination.parent() {
                pyfs::mkdir_parents_exist_ok(&path_text(parent)).map_err(from_failure)?;
            }
            copy2(&source_root.join(relative), &destination)?;
        }
        Ok(())
    }

    /// `git init -q <root>`; the child shares this process's output.
    pub fn git_init(&self) -> Result<(), Raised> {
        run_checked_inherited(&[
            OsString::from("git"),
            OsString::from("init"),
            OsString::from("-q"),
            self.root.clone().into_os_string(),
        ])
    }

    /// Removes the whole temporary directory (see [`TemporaryDirectory::close`]).
    pub fn close(self) -> Result<(), Raised> {
        self.temp.close()
    }

    /// The campaign declaration both fixtures write: one main node, one cell assigned to it.
    pub fn write_campaign(&self) -> Result<(), Raised> {
        write_str(
            &self.root.join("campaigns").join("ACTIVE"),
            &format!("{CAMPAIGN}\n"),
        )?;
        let declaration = object(vec![
            ("schema_version", PyValue::int(1)),
            ("id", text(CAMPAIGN)),
            ("plan_ref", text("docs/plan/fixture.md")),
            (
                "nodes",
                list(vec![object(vec![
                    ("node_id", text("main")),
                    ("role", text("main")),
                    ("topology", text("single")),
                ])]),
            ),
            (
                "assignments",
                object(vec![(
                    "main",
                    list(vec![object(vec![("cell", text(CELL))])]),
                )]),
            ),
            (
                "control_variables",
                object(vec![
                    ("model", text("m")),
                    ("vllm_version", text("0.0.0")),
                    ("topology", text("single")),
                    ("target_gpu", text("fixture")),
                ]),
            ),
        ]);
        write_text(
            &self
                .root
                .join("campaigns")
                .join(CAMPAIGN)
                .join("campaign.yaml"),
            &dumps_unicode(&declaration)?,
        )
    }

    /// The serving environment of the configuration the cells name.
    pub fn write_serving_env(&self) -> Result<(), Raised> {
        write_str(
            &self
                .root
                .join("output/single/envs")
                .join(format!(".env.{CONFIG}")),
            "SERVING_PORT=18080\n",
        )
    }

    /// The directory of the sweep the cells name (`output/single/benchlog/sweep_<config>`).
    pub fn sweep_directory(&self) -> PathBuf {
        self.root
            .join("output/single/benchlog")
            .join(format!("sweep_{CONFIG}"))
    }

    /// `curl` shim: leaves a marker beside itself when it is called and prints `body` as the HTTP status.
    pub fn write_curl_shim(&self, body: &str) -> Result<(), Raised> {
        let shim = self.bin_dir.join("curl");
        write_str(
            &shim,
            &format!("#!/bin/sh\ntouch \"$(dirname \"$0\")/curl.called\"\nprintf {body}\n"),
        )?;
        chmod(&shim, 0o755)
    }

    pub fn curl_marker(&self) -> PathBuf {
        self.bin_dir.join("curl.called")
    }
}

// ── driving broad_search.sh ──────────────────────────────────────────────────────

/// Runs the copy of `broad_search.sh` inside an [`IsolatedCheckout`]: working directory = the checkout, `PATH` = the
/// shim directory in front of this process's own.
pub struct BroadSearchDriver {
    root: PathBuf,
    path_with_shims: OsString,
}

impl BroadSearchDriver {
    pub fn new(checkout: &IsolatedCheckout) -> Self {
        let mut path_with_shims = checkout.bin_dir().as_os_str().to_os_string();
        path_with_shims.push(":");
        path_with_shims.push(std::env::var_os("PATH").unwrap_or_default());
        BroadSearchDriver {
            root: checkout.root().to_path_buf(),
            path_with_shims,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `bash <checkout>/.../broad_search.sh args...`; `extra_environment` is added to the shimmed environment.
    pub fn run(
        &self,
        extra_environment: &[(&str, &str)],
        arguments: Vec<OsString>,
    ) -> Result<TextRun, Raised> {
        let mut command = vec![
            OsString::from("bash"),
            self.root
                .join(BENCHMARK_SCRIPTS)
                .join("broad_search.sh")
                .into_os_string(),
        ];
        command.extend(arguments);
        let mut environment = vec![(OsString::from("PATH"), self.path_with_shims.clone())];
        environment.extend(
            extra_environment
                .iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value))),
        );
        let settings = ChildSettings {
            current_dir: Some(self.root.clone()),
            env_set: environment,
            ..ChildSettings::default()
        };
        run_text(&command, &settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy2_never_writes_through_a_name_that_is_the_source_file() {
        use std::os::unix::fs::MetadataExt;
        let directory = TemporaryDirectory::create("copy2-same-file.").expect("directory");
        let source = directory.path().join("program");
        std::fs::write(&source, b"binary bytes").expect("source");
        let alias = directory.path().join("alias");
        std::fs::hard_link(&source, &alias).expect("alias");
        let inode = std::fs::metadata(&source).expect("metadata").ino();
        let refused = copy2(&source, &alias).expect_err("the same file is refused");
        assert_eq!(refused.failure().class, "SameFileError");
        assert_eq!(refused.qualified_class(), Some("shutil.SameFileError"));
        let other = directory.path().join("other");
        copy2(&source, &other).expect("a different destination is written");
        // Nothing was opened for writing through the shared inode.
        for name in [&source, &alias] {
            assert_eq!(std::fs::read(name).expect("read"), b"binary bytes");
            assert_eq!(std::fs::metadata(name).expect("metadata").ino(), inode);
        }
        assert_eq!(std::fs::read(&other).expect("read"), b"binary bytes");
    }

    #[test]
    fn head_and_tail_count_characters_not_bytes() {
        assert_eq!(tail("가나다라마", 2), "라마");
        assert_eq!(head("가나다라마", 2), "가나");
        assert_eq!(tail("ab", 5), "ab");
        assert_eq!(head("", 3), "");
    }

    #[test]
    fn a_lockset_template_gets_its_id_in_place() {
        let template = object(vec![
            ("id", text("<<FILL>>")),
            ("provenance", text("<<FILL>>")),
        ]);
        let copy = with_id(&template, "cell-a").expect("a document");
        assert_eq!(
            dumps_unicode(&copy).expect("dump").to_utf8().expect("text"),
            "{\"id\": \"cell-a\", \"provenance\": \"<<FILL>>\"}"
        );
        let bare = object(vec![("provenance", text("x"))]);
        assert_eq!(
            dumps_unicode(&with_id(&bare, "cell-a").expect("a document"))
                .expect("dump")
                .to_utf8()
                .expect("text"),
            "{\"provenance\": \"x\", \"id\": \"cell-a\"}"
        );
    }

    #[test]
    fn a_template_that_is_not_an_object_is_refused_like_dict_update() {
        let message = |value: PyValue| {
            with_id(&value, "c")
                .expect_err("refused")
                .failure()
                .message
                .to_utf8()
                .expect("text")
        };
        assert_eq!(message(PyValue::int(3)), "'int' object is not iterable");
        assert_eq!(message(PyValue::None), "'NoneType' object is not iterable");
        assert_eq!(
            message(text("ab")),
            "dictionary update sequence element #0 has length 1; 2 is required"
        );
        assert_eq!(
            message(list(vec![PyValue::int(1)])),
            "cannot convert dictionary update sequence element #0 to a sequence"
        );
        assert_eq!(
            message(list(vec![list(vec![text("a")])])),
            "dictionary update sequence element #0 has length 1; 2 is required"
        );
    }

    #[test]
    fn membership_follows_the_kind_of_the_value() {
        assert!(contains(&text("needs PyYAML here"), "PyYAML").expect("a string"));
        assert!(contains(&list(vec![text("PyYAML")]), "PyYAML").expect("a list"));
        assert!(!contains(&list(vec![text("PyYAML here")]), "PyYAML").expect("a list"));
        assert!(contains(&object(vec![("PyYAML", PyValue::None)]), "PyYAML").expect("a dict"));
        assert_eq!(
            contains(&PyValue::None, "PyYAML")
                .expect_err("not iterable")
                .failure()
                .message
                .to_utf8()
                .expect("text"),
            "argument of type 'NoneType' is not iterable"
        );
    }

    #[test]
    fn a_dict_loses_one_key_and_keeps_the_order_of_the_rest() {
        let document = object(vec![
            ("a", PyValue::int(1)),
            ("rubric_authority", text("weak")),
            ("b", PyValue::int(2)),
        ]);
        let rest = without_key(document, "rubric_authority").expect("a dict");
        assert_eq!(
            dumps_unicode(&rest).expect("dump").to_utf8().expect("text"),
            "{\"a\": 1, \"b\": 2}"
        );
        assert_eq!(
            without_key(list(Vec::new()), "x")
                .expect_err("no pop")
                .failure()
                .message
                .to_utf8()
                .expect("text"),
            "'list' object has no attribute 'pop'"
        );
    }

    #[test]
    fn a_temporary_directory_is_private_named_and_removed() {
        let directory = TemporaryDirectory::create("verify-fixture-unit.").expect("created");
        let path = directory.path().to_path_buf();
        let name = path
            .file_name()
            .expect("name")
            .to_str()
            .expect("UTF-8")
            .to_string();
        let suffix = name.strip_prefix("verify-fixture-unit.").expect("prefix");
        assert_eq!(suffix.len(), TEMPORARY_NAME_LENGTH);
        assert!(suffix
            .bytes()
            .all(|byte| TEMPORARY_NAME_ALPHABET.contains(&byte)));
        assert_eq!(
            std::fs::metadata(&path)
                .expect("exists")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        std::fs::write(path.join("file"), b"x").expect("write");
        directory.close().expect("removed");
        assert!(!path.exists());
    }

    #[test]
    fn a_dropped_temporary_directory_is_removed_too() {
        let path = {
            let directory = TemporaryDirectory::create("verify-fixture-drop.").expect("created");
            directory.path().to_path_buf()
        };
        assert!(!path.exists());
    }

    #[test]
    fn copy2_names_the_file_that_failed_and_keeps_mode_and_bytes() {
        let directory = TemporaryDirectory::create("verify-fixture-copy.").expect("created");
        let source = directory.path().join("source");
        let destination = directory.path().join("destination");
        std::fs::write(&source, b"payload").expect("write");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o751)).expect("mode");
        copy2(&source, &destination).expect("copied");
        assert_eq!(std::fs::read(&destination).expect("read"), b"payload");
        assert_eq!(
            std::fs::metadata(&destination)
                .expect("stat")
                .permissions()
                .mode()
                & 0o777,
            0o751
        );

        let missing = directory.path().join("missing");
        let error = copy2(&missing, &destination).expect_err("no source");
        assert_eq!(error.failure().class, "FileNotFoundError");
        assert!(error
            .failure()
            .message
            .to_utf8()
            .expect("text")
            .ends_with(&format!("'{}'", missing.display())));
        let into_missing_directory = directory.path().join("no-such-directory").join("file");
        let error = copy2(&source, &into_missing_directory).expect_err("no destination");
        assert!(error
            .failure()
            .message
            .to_utf8()
            .expect("text")
            .ends_with(&format!("'{}'", into_missing_directory.display())));
    }

    #[test]
    fn exists_forgives_absence_and_a_file_in_the_place_of_a_directory() {
        let directory = TemporaryDirectory::create("verify-fixture-exists.").expect("created");
        let file = directory.path().join("file");
        std::fs::write(&file, b"x").expect("write");
        assert!(exists(&file).expect("stat"));
        assert!(!exists(&directory.path().join("absent")).expect("absence"));
        assert!(!exists(&file.join("below-a-file")).expect("not a directory"));
        assert!(!exists(&directory.path().join("absent").join("deeper")).expect("absence"));
    }

    #[test]
    fn unlink_if_present_forgives_only_absence() {
        let directory = TemporaryDirectory::create("verify-fixture-unlink.").expect("created");
        let file = directory.path().join("file");
        std::fs::write(&file, b"x").expect("write");
        unlink_if_present(&file).expect("removed");
        unlink_if_present(&file).expect("already absent");
        assert!(unlink(&file).is_err());
        // a directory is not a file `unlink` may remove
        assert!(unlink_if_present(directory.path()).is_err());
    }

    #[test]
    fn signals_are_named_as_python_names_them() {
        assert_eq!(signal_name(9), Some("SIGKILL"));
        assert_eq!(signal_name(11), Some("SIGSEGV"));
        assert_eq!(signal_name(31), Some("SIGSYS"));
        assert_eq!(signal_name(0), None);
        assert_eq!(signal_name(40), None);
    }

    #[test]
    fn temporary_candidates_follow_the_tempfile_order() {
        let environment = |name: &str| match name {
            "TMPDIR" => Some(OsString::from("/first")),
            "TEMP" => Some(OsString::new()),
            "TMP" => Some(OsString::from("/third")),
            _ => None,
        };
        let candidates = temporary_candidates(environment, Some(PathBuf::from("/work")));
        let shown: Vec<_> = candidates
            .iter()
            .map(|path| path.to_str().expect("UTF-8"))
            .collect();
        assert_eq!(
            shown,
            ["/first", "/third", "/tmp", "/var/tmp", "/usr/tmp", "/work"]
        );
        let bare = temporary_candidates(|_| None, None);
        let shown: Vec<_> = bare
            .iter()
            .map(|path| path.to_str().expect("UTF-8"))
            .collect();
        assert_eq!(shown, ["/tmp", "/var/tmp", "/usr/tmp", "."]);
    }

    #[test]
    fn an_unusable_candidate_is_skipped_for_the_next_one_and_the_error_lists_all() {
        let directory = TemporaryDirectory::create("verify-fixture-root.").expect("created");
        let missing = directory.path().join("absent").join("nested");
        let usable = directory.path().to_path_buf();
        let chosen =
            first_usable_temporary_root(&[missing.clone(), usable.clone()]).expect("usable");
        assert_eq!(chosen, usable);
        // a file is no directory to probe in
        let file = directory.path().join("file");
        std::fs::write(&file, b"x").expect("write");
        assert_eq!(
            first_usable_temporary_root(&[file.clone(), usable.clone()]).expect("usable"),
            usable
        );
        let error =
            first_usable_temporary_root(&[missing.clone(), file.clone()]).expect_err("none");
        assert_eq!(error.failure().class, "FileNotFoundError");
        let expected = format!(
            "[Errno 2] No usable temporary directory found in {}",
            PyValue::List(vec![
                PyValue::Str(path_text(&missing)),
                PyValue::Str(path_text(&file))
            ])
            .repr()
        );
        assert_eq!(error.failure().message.to_utf8().expect("text"), expected);
    }
}
