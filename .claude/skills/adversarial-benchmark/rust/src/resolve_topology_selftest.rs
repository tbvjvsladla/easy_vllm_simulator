//! `resolve_topology --self-test`: the original's own checks, run for real against this implementation.
//!
//! * **R** resolves a bare tree and a tree with a manifest but no parity owner through [`resolve`].
//! * **P** builds five small git checkouts that carry the checkout's actual parity owner and asks [`resolve`] about
//!   them: the owner is a real Python process, the verdicts are its own.
//! * **S** copies the six benchmark callers, the resolver launchers, the repeat-axis and preset closure, the
//!   configuration and the native programs into a fresh checkout and runs the callers there with no `--topology`:
//!   the topology each caller acts on must be the one the copied launcher resolved through the copied native CLI.
//!   The copies execute their own deployed files - the environment of the check run cannot substitute a binary.
//!
//! Where the original located itself by its file path, the sources are named by [`SelfTestSources`] (the repository
//! root of the parity owner and the configuration, the directory of the caller scripts, the directory of the
//! other native programs, and this program's own binaries); a source that is not configured is an error that ends
//! the run, never a shorter run. Printed check names are the original's. Nothing replays a transcript.

use crate::captured_process::ChildSettings;
use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::resolve_topology::{
    capture_text, resolve, Resolution, ResolveContext, RunLimit, EXIT_UNRESOLVED,
    EXIT_UNTERRAFORMED, PARITY_REL, TAG,
};
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The repository root: the checkout whose parity owner and configuration the checks use.
pub const REPO_ROOT_ENV: &str = "RESOLVE_TOPOLOGY_REPO_ROOT";
/// The directory of the six callers and the launchers (`.../adversarial-benchmark/scripts`).
pub const SCRIPTS_DIR_ENV: &str = "RESOLVE_TOPOLOGY_SCRIPTS_DIR";
/// The directory of the other deployed native programs (`.../adversarial-benchmark/rust/target/release`).
pub const RUNTIME_DIR_ENV: &str = "RESOLVE_TOPOLOGY_RUNTIME_DIR";
/// This program's command-line binary, when it is not the running executable.
pub const CLI_BINARY_ENV: &str = "RESOLVE_TOPOLOGY_BIN";
/// This program's API binary.
pub const API_BINARY_ENV: &str = "RESOLVE_TOPOLOGY_API_BIN";
/// Configuration error class: a source the checks need is not configured.
pub const CONFIGURATION_ERROR: &str = "ResolveTopologyConfigurationError";

const CLI_NAME: &str = "resolve_topology";
const API_NAME: &str = "resolve_topology_api";
const SKILL_REL: &str = ".claude/skills/adversarial-benchmark";
const NATIVE_REL: &str = ".claude/skills/adversarial-benchmark/rust/target/release";
/// The callers whose invocation of the resolver is executed.
const CALLERS: [&str; 6] = [
    "broad_search.sh",
    "judge_bench.sh",
    "lite_bench.sh",
    "run_bench.sh",
    "sweep_bench.sh",
    "max_envelope.sh",
];
/// The resolver's own scripts, copied with the callers from the scripts directory (the retained reference
/// implementation and the two launchers).
const RESOLVER_SCRIPTS: [&str; 3] = [
    "resolve_topology.py",
    "resolve_topology.sh",
    "resolve_topology_api.sh",
];
/// The scripts of the tools the callers reach after the resolver (repeat axis, measurement presets), copied byte for
/// byte from the repository root's scripts directory.
const DEPENDENCY_SCRIPTS: [&str; 8] = [
    "repeat_axis.py",
    "bench_preset.py",
    "repeat_axis.sh",
    "repeat_axis_api.sh",
    "repeat_axis_native.py",
    "repeat_axis_owner_query.py",
    "bench_preset.sh",
    "bench_preset_api.sh",
];
/// The deployed native programs of the other tools the callers run (taken from the runtime directory), and the private
///  helper the resolver runs by default (the isolated checkout's resolver finds it beside itself).
const RUNTIME_PROGRAMS: [&str; 5] = [
    "repeat_axis",
    "repeat_axis_api",
    "bench_preset",
    "bench_preset_api",
    crate::resolve_topology::HELPER_NAME,
];
/// Settings of the environment a test run may carry that would substitute a copied probe's own deployed files.
const CHILD_OVERRIDES: [&str; 10] = [
    "RESOLVE_TOPOLOGY_BIN",
    "RESOLVE_TOPOLOGY_API_BIN",
    "REPEAT_AXIS_BIN",
    "REPEAT_AXIS_API_BIN",
    "BENCH_PRESET_BIN",
    "BENCH_PRESET_API_BIN",
    "REPEAT_AXIS_REPO_ROOT",
    "REPEAT_AXIS_CAMPAIGN_ADAPTER",
    "BENCH_PRESET_REPO_ROOT",
    "BENCH_PRESET_SKILL_DIR",
];
/// The branch-name inference forms no caller may contain.
const BRANCH_FORMS: [&str; 7] = [
    "abbrev-ref",
    "symbolic-ref",
    "show-current",
    "branch --show",
    "multi-node)",
    "single-node)",
    "refs/heads/",
];
const GIT_LIMIT_SECONDS: u32 = 60;
const CALLER_LIMIT_SECONDS: u32 = 120;
/// How much of a failed check's detail is printed (`str(detail)[-600:]`).
const DETAIL_TAIL: usize = 600;

// ── sources ─────────────────────────────────────────────────────────────────────

/// Where the self-test reads the real checkout and which native programs it copies.
#[derive(Clone, Debug, Default)]
pub struct SelfTestSources {
    pub repo_root: Option<PathBuf>,
    pub scripts_dir: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
    pub cli_binary: Option<PathBuf>,
    pub api_binary: Option<PathBuf>,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

impl SelfTestSources {
    /// The environment of this process, then the installed layout around the executable
    /// (`.../adversarial-benchmark/rust/target/<profile>/<program>`).
    pub fn from_environment() -> Self {
        let exe = std::env::current_exe().ok();
        let skill_dir = exe.as_ref().and_then(|exe| {
            exe.ancestors()
                .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "rust"))
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        });
        let scripts_dir = env_path(SCRIPTS_DIR_ENV).or_else(|| {
            skill_dir
                .as_ref()
                .map(|dir| dir.join("scripts"))
                .filter(|dir| dir.is_dir())
        });
        let repo_root = env_path(REPO_ROOT_ENV).or_else(|| {
            skill_dir
                .as_ref()
                .and_then(|dir| dir.ancestors().nth(3))
                .map(Path::to_path_buf)
        });
        let runtime_dir = env_path(RUNTIME_DIR_ENV).or_else(|| {
            exe.as_ref()
                .and_then(|exe| exe.parent())
                .map(Path::to_path_buf)
        });
        let running_cli = exe
            .as_ref()
            .filter(|exe| exe.file_name().is_some_and(|name| name == CLI_NAME))
            .cloned();
        let cli_binary = running_cli
            .or_else(|| env_path(CLI_BINARY_ENV))
            .or_else(|| runtime_dir.as_ref().map(|dir| dir.join(CLI_NAME)));
        let api_binary =
            env_path(API_BINARY_ENV).or_else(|| runtime_dir.as_ref().map(|dir| dir.join(API_NAME)));
        SelfTestSources {
            repo_root,
            scripts_dir,
            runtime_dir,
            cli_binary,
            api_binary,
        }
    }

    fn required<'a>(
        value: &'a Option<PathBuf>,
        what: &str,
        env: &str,
    ) -> Result<&'a Path, Failure> {
        value.as_deref().ok_or_else(|| {
            Failure::new(
                CONFIGURATION_ERROR,
                &format!("{what} is not configured; set {env} (the original derived it from its own file location)"),
            )
        })
    }
}

// ── plumbing ────────────────────────────────────────────────────────────────────

fn path_text(path: &Path) -> PyStr {
    PyStr::from_surrogateescape(path.as_os_str().as_bytes())
}

fn path_value(path: &Path) -> PyValue {
    PyValue::Str(path_text(path))
}

fn os_failure(error: &std::io::Error, path: &Path) -> Failure {
    Failure::new(
        pyfs::os_error_class(error),
        &crate::cli::os_error_text(error, &path_text(path)),
    )
}

/// `with tempfile.TemporaryDirectory(prefix=...)`: removed when it goes out of scope, also on an error.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn create(prefix: &str) -> Result<TempDir, Failure> {
        let base = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for attempt in 0..100_u32 {
            let path = base.join(format!(
                "{prefix}{}-{stamp:x}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(os_failure(&error, &path)),
            }
        }
        Err(Failure::new(
            "FileExistsError",
            "no unused temporary directory name",
        ))
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// `write(path, text, mode)`: parent folders, then the text as UTF-8, then the mode when one is given.
fn write_file(path: &Path, text: &str, mode: Option<u32>) -> Result<(), Failure> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| os_failure(&error, parent))?;
    }
    std::fs::write(path, text).map_err(|error| os_failure(&error, path))?;
    if let Some(mode) = mode {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|error| os_failure(&error, path))?;
    }
    Ok(())
}

/// `shutil.copy2(source, destination)` (content and permission bits).
fn copy_file(source: &Path, destination: &Path) -> Result<(), Failure> {
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| os_failure(&error, source))
}

/// `shutil.copytree(source, destination)`: symbolic links are followed, the destination must not exist.
fn copy_tree(source: &Path, destination: &Path) -> Result<(), Failure> {
    let entries = std::fs::read_dir(source).map_err(|error| os_failure(&error, source))?;
    std::fs::create_dir(destination).map_err(|error| os_failure(&error, destination))?;
    for entry in entries {
        let entry = entry.map_err(|error| os_failure(&error, source))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = std::fs::metadata(&from).map_err(|error| os_failure(&error, &from))?;
        if metadata.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            copy_file(&from, &to)?;
        }
    }
    Ok(())
}

/// `open(path, encoding="utf-8").read()` of a text the checks inspect or copy: universal newlines, strict UTF-8.
fn read_text(path: &Path) -> Result<PyStr, Failure> {
    let bytes = std::fs::read(path).map_err(|error| os_failure(&error, path))?;
    decode_file_text(&bytes).map_err(|error| crate::repeat_api_cli::load_failure(&error))
}

/// `shutil.which(name)`: the first executable regular file named `name` along `PATH`.
fn which(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let search = std::env::var_os("PATH")?;
    std::env::split_paths(&search)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|data| data.is_file() && data.permissions().mode() & 0o111 != 0)
        })
}

fn contains(text: &PyStr, needle: &str) -> bool {
    text.to_utf8().unwrap_or_default().contains(needle)
}

fn words(items: &[&str]) -> PyValue {
    PyValue::List(items.iter().map(|item| PyValue::str(item)).collect())
}

/// `git -C root *args` with the output discarded (the original ignores it; an exception it raises is not ignored).
fn git(root: &Path, args: &[&str]) -> Result<(), Failure> {
    let mut command = vec![PyValue::str("git"), PyValue::str("-C"), path_value(root)];
    command.extend(args.iter().map(|argument| PyValue::str(argument)));
    capture_text(
        &PyValue::List(command),
        &RunLimit::seconds(GIT_LIMIT_SECONDS),
        &ChildSettings::default(),
    )
    .map(|_| ())
}

/// One printed check: `  [PASS] name`, or `  [FAIL] name  → detail`.
struct Report<'a> {
    console: &'a mut dyn Console,
    failures: Vec<String>,
}

impl Report<'_> {
    fn check(
        &mut self,
        name: &str,
        condition: bool,
        detail: impl FnOnce() -> PyValue,
    ) -> Result<(), Failure> {
        if condition {
            return self.console.print(&PyStr::new(&format!("  [PASS] {name}")));
        }
        self.failures.push(name.to_string());
        let shown = detail().display_text();
        let points = shown.code_points();
        let tail =
            PyStr::from_code_points(points[points.len().saturating_sub(DETAIL_TAIL)..].to_vec());
        let mut line = PyStr::new(&format!("  [FAIL] {name}  → "));
        line.push_py(&tail);
        self.console.print(&line)
    }
}

fn resolved(rc: &Resolution) -> PyValue {
    PyValue::Tuple(vec![PyValue::int(rc.code), PyValue::Str(rc.why.clone())])
}

// ── the checks ──────────────────────────────────────────────────────────────────

/// `_self_test()`: the exit status, 0 when every check passed.
pub fn self_test(
    console: &mut dyn Console,
    context: &ResolveContext,
    sources: &SelfTestSources,
) -> Result<i32, Failure> {
    let repo_root =
        SelfTestSources::required(&sources.repo_root, "the repository root", REPO_ROOT_ENV)?;
    let scripts_dir = SelfTestSources::required(
        &sources.scripts_dir,
        "the directory of the caller scripts",
        SCRIPTS_DIR_ENV,
    )?;
    let real_parity = repo_root.join(PARITY_REL);
    let mut report = Report {
        console,
        failures: Vec::new(),
    };
    manifest_only_checks(&mut report, context)?;
    if which("git").is_none() {
        report.check(
            "P0 git 이 있어야 메인 경로(4자일치 술어)를 실행으로 친다",
            false,
            || PyValue::str("git 없음"),
        )?;
    } else {
        parity_checks(&mut report, context, &real_parity)?;
    }
    caller_source_checks(&mut report, scripts_dir)?;
    if which("git").is_some() {
        caller_execution_checks(&mut report, sources, repo_root, scripts_dir, &real_parity)?;
    }
    if !report.failures.is_empty() {
        let names = PyValue::List(
            report
                .failures
                .iter()
                .map(|name| PyValue::str(name))
                .collect(),
        );
        let mut text = PyStr::new(&format!(
            "{TAG} --self-test FAIL {} 건: ",
            report.failures.len()
        ));
        text.push_py(&names.display_text());
        text.push_str("\n");
        report.console.error(&text);
        return Ok(1);
    }
    report.console.print(&PyStr::new(&format!(
        "{TAG} --self-test OK — R1~R7 · P1~P5 · S0·S0b~S11 전부 통과"
    )))?;
    Ok(0)
}

/// R: manifest-only unit coverage.
fn manifest_only_checks(report: &mut Report<'_>, context: &ResolveContext) -> Result<(), Failure> {
    let temp = TempDir::create("resolve_topology_R.")?;
    let bare = temp.join("bare");
    std::fs::create_dir(&bare).map_err(|error| os_failure(&error, &bare))?;
    let outcome = resolve(&path_value(&bare), &PyValue::None, context)?;
    report.check(
        "R1 manifest 없음 → 3 미테라포밍",
        outcome.code == EXIT_UNTERRAFORMED && matches!(outcome.topology, PyValue::None),
        || resolved(&outcome),
    )?;
    write_file(
        &bare.join("output/single/manifest.yaml"),
        "self_role: sub\ntopology: single\n",
        None,
    )?;
    let outcome = resolve(&path_value(&bare), &PyValue::None, context)?;
    report.check(
        "R2 manifest 있으나 parity 없음 → 5",
        outcome.code == EXIT_UNRESOLVED && matches!(outcome.topology, PyValue::None),
        || resolved(&outcome),
    )
}

/// The checkout the P scenarios build: a git repository on `branch` that carries the actual parity owner, a specialised
/// constitution with `header` as its topology, and one manifest per channel.
fn main_repo(
    base: &Path,
    parity_text: &PyStr,
    name: &str,
    branch: &str,
    header: &str,
    channels: &[(&str, &str)],
) -> Result<PathBuf, Failure> {
    let root = base.join(name);
    std::fs::create_dir(&root).map_err(|error| os_failure(&error, &root))?;
    git(&root, &["init", "-q"])?;
    git(
        &root,
        &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
    )?;
    write_parity(&root, parity_text)?;
    write_file(
        &root.join(".claude/rules/strategy.topology.md"),
        &format!("# t\n\n**topology: {header}** · layer: topology\n"),
        None,
    )?;
    git(&root, &["add", "-A"])?;
    for (channel, declared) in channels {
        write_file(
            &root.join(format!("output/{channel}/manifest.yaml")),
            &format!("self_role: main\ntopology: {declared}\n"),
            None,
        )?;
    }
    Ok(root)
}

fn write_parity(root: &Path, parity_text: &PyStr) -> Result<(), Failure> {
    let bytes = pyfs::strict_utf8(parity_text)
        .map_err(|error| Failure::new("UnicodeEncodeError", &error.message()))?;
    let target = root.join(PARITY_REL);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| os_failure(&error, parent))?;
    }
    std::fs::write(&target, bytes).map_err(|error| os_failure(&error, &target))
}

/// P: the real parity owner on small checkouts.
fn parity_checks(
    report: &mut Report<'_>,
    context: &ResolveContext,
    real_parity: &Path,
) -> Result<(), Failure> {
    let temp = TempDir::create("resolve_topology_P.")?;
    let parity_text = read_text(real_parity)?;
    let ask = |root: &Path| resolve(&path_value(root), &PyValue::None, context);

    let root = main_repo(
        &temp.path,
        &parity_text,
        "p1",
        "multi-node",
        "multi",
        &[("multi", "multi"), ("single", "single")],
    )?;
    let outcome = ask(&root)?;
    let multi = matches!(&outcome.topology, PyValue::Str(text) if text.equals("multi"));
    report.check(
        "P1 메인 multi-node 체크아웃 · 특화 헤더 multi · output/multi manifest 일치 → multi · 출처 4자일치",
        outcome.code == 0 && multi && outcome.why.to_utf8().is_some_and(|why| why.starts_with("topology_parity(")),
        || outcome.to_value(),
    )?;

    let root = main_repo(
        &temp.path,
        &parity_text,
        "p2",
        "feature-x",
        "multi",
        &[("multi", "multi")],
    )?;
    let outcome = ask(&root)?;
    report.check(
        "★P2 음성대조: 운영 브랜치가 아닌 이름 → 5(종전 관용구는 여기서 조용히 single)",
        outcome.code == EXIT_UNRESOLVED
            && matches!(outcome.topology, PyValue::None)
            && contains(&outcome.why, "BRANCH_UNKNOWN"),
        || resolved(&outcome),
    )?;

    let root = main_repo(
        &temp.path,
        &parity_text,
        "p3",
        "multi-node",
        "single",
        &[("multi", "multi")],
    )?;
    let outcome = ask(&root)?;
    report.check(
        "★P3 음성대조: 특화 헤더가 반대 토폴로지 → 5",
        outcome.code == EXIT_UNRESOLVED && contains(&outcome.why, "LAYER_HEADER_MISMATCH"),
        || resolved(&outcome),
    )?;

    let root = main_repo(
        &temp.path,
        &parity_text,
        "p4",
        "multi-node",
        "multi",
        &[("single", "single")],
    )?;
    let outcome = ask(&root)?;
    report.check(
        "★P4 통로 manifest 부재(반대 통로 manifest 만 있다) → 3(브랜치 필터만으로 고르지 않는다)",
        outcome.code == EXIT_UNTERRAFORMED && contains(&outcome.why, "absent"),
        || resolved(&outcome),
    )?;

    let root = main_repo(
        &temp.path,
        &parity_text,
        "p5",
        "single-node",
        "single",
        &[("single", "multi")],
    )?;
    let outcome = ask(&root)?;
    report.check(
        "★P5 음성대조: 통로 manifest 가 다른 topology 를 말한다 → 5",
        outcome.code == EXIT_UNRESOLVED && contains(&outcome.why, "MANIFEST_MISMATCH"),
        || resolved(&outcome),
    )
}

/// S0: no caller infers a branch name, and each calls the resolver launcher.
fn caller_source_checks(report: &mut Report<'_>, scripts_dir: &Path) -> Result<(), Failure> {
    for name in CALLERS {
        let text = read_text(&scripts_dir.join(name))?
            .to_utf8()
            .unwrap_or_default();
        let found: Vec<&str> = BRANCH_FORMS
            .iter()
            .copied()
            .filter(|form| text.contains(form))
            .collect();
        report.check(
            &format!(
                "S0 {name} 에 브랜치 이름 추론 형태가 없다({} 0) · 해소기를 부른다",
                BRANCH_FORMS.join("·")
            ),
            found.is_empty() && text.contains("resolve_topology.sh"),
            || words(&found),
        )?;
    }
    let probe =
        "BR=\"$(git symbolic-ref --short HEAD)\"\ncase \"$BR\" in multi-node) TOPO=multi;; esac\n";
    let hits: Vec<&str> = BRANCH_FORMS
        .iter()
        .copied()
        .filter(|form| probe.contains(form))
        .collect();
    report.check(
        "★S0b 음성대조: 다른 형태의 브랜치 추론(symbolic-ref · case multi-node)) 은 S0 형태 목록에 걸린다",
        hits == ["symbolic-ref", "multi-node)"],
        || words(&hits),
    )
}

/// What a caller printed: its exit status and its two streams, standard output first.
struct CallerRun {
    code: Option<i64>,
    output: PyStr,
}

/// The fresh checkout the S scenarios run in, with the environment its callers get.
struct CallerCheckout {
    root: PathBuf,
    scripts: PathBuf,
    marker: PathBuf,
    state: PathBuf,
    settings: ChildSettings,
}

impl CallerCheckout {
    fn run(&self, name: &str, arguments: &[&str]) -> Result<CallerRun, Failure> {
        let mut command = vec![PyValue::str("bash"), path_value(&self.scripts.join(name))];
        command.extend(arguments.iter().map(|argument| PyValue::str(argument)));
        let ran = capture_text(
            &PyValue::List(command),
            &RunLimit::seconds(CALLER_LIMIT_SECONDS),
            &self.settings,
        )?;
        let mut output = ran.stdout.clone();
        output.push_py(&ran.stderr);
        Ok(CallerRun {
            code: ran.code,
            output,
        })
    }

    /// The `--topology` the Flag gate stub was last called with.
    fn gate_topology(&self) -> Result<Option<String>, Failure> {
        if !self.marker.is_file() {
            return Ok(None);
        }
        let text = read_text(&self.marker)?.to_utf8().unwrap_or_default();
        let last = text
            .lines()
            .last()
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))?;
        let tokens: Vec<&str> = last.split_whitespace().collect();
        let Some(at) = tokens.iter().position(|token| *token == "--topology") else {
            return Ok(None);
        };
        tokens
            .get(at + 1)
            .map(|token| Some((*token).to_string()))
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))
    }

    fn state_topology(&self) -> Result<PyValue, Failure> {
        if !self.state.is_file() {
            return Ok(PyValue::None);
        }
        let document = loads_text(&read_text(&self.state)?)
            .map_err(|error| crate::repeat_api_cli::load_failure(&error))?;
        match document.peel() {
            PyValue::Dict(map) => Ok(map.get("topology").cloned().unwrap_or(PyValue::None)),
            other => Err(Failure::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'get'", other.type_name()),
            )),
        }
    }
}

fn gate_value(topology: &Option<String>) -> PyValue {
    topology.as_deref().map_or(PyValue::None, PyValue::str)
}

/// Builds the S checkout: the callers and their closure copied byte for byte, the native programs, the configuration,
/// a git repository on `multi-node`, the actual parity owner, one `multi` manifest and the Flag gate stub.
fn build_caller_checkout(
    temp: &TempDir,
    sources: &SelfTestSources,
    repo_root: &Path,
    scripts_dir: &Path,
    real_parity: &Path,
) -> Result<CallerCheckout, Failure> {
    let root = temp.join("repo");
    let scripts = root.join(SKILL_REL).join("scripts");
    std::fs::create_dir_all(&scripts).map_err(|error| os_failure(&error, &scripts))?;
    for name in CALLERS.iter().chain(RESOLVER_SCRIPTS.iter()) {
        copy_file(&scripts_dir.join(name), &scripts.join(name))?;
    }
    let dependency_dir = repo_root.join(SKILL_REL).join("scripts");
    for name in DEPENDENCY_SCRIPTS {
        copy_file(&dependency_dir.join(name), &scripts.join(name))?;
    }
    let native = root.join(NATIVE_REL);
    std::fs::create_dir_all(&native).map_err(|error| os_failure(&error, &native))?;
    let runtime = SelfTestSources::required(
        &sources.runtime_dir,
        "the directory of the deployed native programs",
        RUNTIME_DIR_ENV,
    )?;
    for program in RUNTIME_PROGRAMS {
        copy_file(&runtime.join(program), &native.join(program))?;
    }
    let cli = SelfTestSources::required(
        &sources.cli_binary,
        "this program's command-line binary",
        CLI_BINARY_ENV,
    )?;
    let api = SelfTestSources::required(
        &sources.api_binary,
        "this program's API binary",
        API_BINARY_ENV,
    )?;
    copy_file(cli, &native.join(CLI_NAME))?;
    copy_file(api, &native.join(API_NAME))?;
    // The measurement-shape presets (the sweep resolves them after the topology): the checkout's actual configuration.
    copy_tree(
        &repo_root.join(SKILL_REL).join("config"),
        &root.join(SKILL_REL).join("config"),
    )?;
    git(&root, &["init", "-q"])?;
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/multi-node"])?;
    write_parity(&root, &read_text(real_parity)?)?;
    write_file(
        &root.join(".claude/rules/strategy.topology.md"),
        "**topology: multi** · layer: topology\n",
        None,
    )?;
    git(&root, &["add", "-A"])?;
    write_file(
        &root.join("output/multi/manifest.yaml"),
        "self_role: main\ntopology: multi\n",
        None,
    )?;
    write_file(
        &root.join("output/multi/configs/_probe.yaml"),
        "model: x\nmax-model-len: 8192\n",
        None,
    )?;
    let marker = temp.join("mc_argv.txt");
    // The Flag gate owner of run_bench and lite_bench records its argv (a test-plane stub) and answers "not issued"
    // (3), so both scripts stop at exit 4 and the resolved topology is seen at the gate.
    write_file(
        &root.join(".claude/skills/terraforming_node/scripts/manifest_contract.py"),
        &format!(
            "import sys\nopen({}, 'a').write(' '.join(sys.argv[1:]) + '\\n')\nsys.exit(3)\n",
            path_text(&marker).repr()
        ),
        None,
    )?;
    // The probes execute their own deployed files, whatever the environment of this run substitutes.
    let settings = ChildSettings {
        current_dir: Some(root.clone()),
        env_remove: CHILD_OVERRIDES.iter().map(OsString::from).collect(),
        env_set: vec![(
            OsString::from("PYTHONDONTWRITEBYTECODE"),
            OsString::from("1"),
        )],
        input: None,
        own_process_group: false,
        pid_sink: None,
        null_stdin: false,
        join_process_group: None,
    };
    Ok(CallerCheckout {
        root,
        scripts,
        marker,
        state: temp.join("bs_state.json"),
        settings,
    })
}

fn sweep_arguments() -> [&'static str; 6] {
    [
        "_probe",
        "--backend",
        "openai",
        "--knob-reason",
        "backend=harmony_completion",
        "--dry-run",
    ]
}

/// S1-S11: the callers run for real in a fresh checkout.
fn caller_execution_checks(
    report: &mut Report<'_>,
    sources: &SelfTestSources,
    repo_root: &Path,
    scripts_dir: &Path,
    real_parity: &Path,
) -> Result<(), Failure> {
    let temp = TempDir::create("resolve_topology_S.")?;
    let checkout = build_caller_checkout(&temp, sources, repo_root, scripts_dir, real_parity)?;
    let state_path = path_text(&checkout.state).to_utf8().unwrap_or_default();
    let init_arguments: Vec<String> = [
        "init",
        "--sweep-id",
        "s",
        "--state",
        &state_path,
        "--cells",
        "c1",
        "--control-variable",
        "x",
        "--max-cells",
        "1",
        "--wall-clock-budget-s",
        "60",
        "--consecutive-failure-limit",
        "1",
        "--declared-by",
        "selftest",
        "--basis",
        "selftest",
        "--authority",
        "explore",
        "--now-utc",
        "2026-09-14T00:00:00Z",
        "--repeats",
        "3",
    ]
    .iter()
    .map(|word| (*word).to_string())
    .collect();
    let init: Vec<&str> = init_arguments.iter().map(String::as_str).collect();

    let ran = checkout.run("judge_bench.sh", &["_probe", "--authority", "weak"])?;
    report.check(
        "S1 judge_bench(--topology 없음) → manifest 통로 multi 로 해소(output/multi 경로에서 멈춘다)",
        ran.code == Some(2) && contains(&ran.output, "output/multi/benchlog") && contains(&ran.output, TAG),
        || PyValue::Tuple(vec![ran.code.map_or(PyValue::None, PyValue::int), PyValue::Str(ran.output.clone())]),
    )?;
    let ran = checkout.run("sweep_bench.sh", &sweep_arguments())?;
    report.check(
        "S2 sweep_bench --dry-run(--topology 없음) → topo=multi",
        ran.code == Some(0) && contains(&ran.output, "topo=multi"),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    let ran = checkout.run("lite_bench.sh", &["_probe"])?;
    let gate = checkout.gate_topology()?;
    report.check(
        "S3 lite_bench(--topology 없음) → 해소된 multi 가 Flag 게이트까지 흘러간다(exit 4)",
        ran.code == Some(4) && gate.as_deref() == Some("multi"),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                gate_value(&gate),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    remove_file(&checkout.marker)?;
    let ran = checkout.run("run_bench.sh", &["_probe"])?;
    let gate = checkout.gate_topology()?;
    report.check(
        "S4 run_bench(--topology 없음) → 해소된 multi 가 Flag 게이트까지 흘러간다(exit 4)",
        ran.code == Some(4) && gate.as_deref() == Some("multi"),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                gate_value(&gate),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    remove_file(&checkout.marker)?;
    let ran = checkout.run("broad_search.sh", &init)?;
    let recorded = checkout.state_topology()?;
    report.check(
        "S5 broad_search init(--topology 없음) → 상태 파일 topology=multi",
        ran.code == Some(0) && matches!(&recorded, PyValue::Str(text) if text.equals("multi")),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                recorded.clone(),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    let ran = checkout.run("max_envelope.sh", &["_probe", "--dry-run"])?;
    report.check(
        "S6 max_envelope --dry-run(--topology 없음) → topo=multi",
        ran.code == Some(0) && contains(&ran.output, "topo=multi"),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    let ran = checkout.run(
        "broad_search.sh",
        &[
            "cell",
            "--state",
            "/nonexistent/bs.json",
            "--now-utc",
            "2026-01-01T00:00:00Z",
            "--cell-key",
            "k",
            "--config",
            "c",
            "--axis-citation",
            "x",
            "--bench-budget-mib",
            "1",
        ],
    )?;
    report.check(
        "S7 broad_search cell 의 --confirm-risk 게이트(exit 5)는 토폴로지 해소보다 앞선다(인자 평면 가드 보존)",
        ran.code == Some(5) && !contains(&ran.output, TAG),
        || PyValue::Tuple(vec![ran.code.map_or(PyValue::None, PyValue::int), PyValue::Str(ran.output.clone())]),
    )?;

    // Negative control: a branch that is not an operating one - the old idiom silently chose `single` here.
    git(
        &checkout.root,
        &["symbolic-ref", "HEAD", "refs/heads/feature-x"],
    )?;
    remove_file(&checkout.state)?;
    let refused: [(&str, Vec<&str>, i64); 6] = [
        ("judge_bench.sh", vec!["_probe", "--authority", "weak"], 2),
        (
            "sweep_bench.sh",
            vec![
                "_probe",
                "--backend",
                "openai",
                "--knob-reason",
                "backend=harmony_completion",
                "--dry-run",
            ],
            2,
        ),
        ("lite_bench.sh", vec!["_probe"], 4),
        ("run_bench.sh", vec!["_probe"], 4),
        ("broad_search.sh", init.clone(), 2),
        ("max_envelope.sh", vec!["_probe", "--dry-run"], 2),
    ];
    for (name, arguments, wanted) in refused {
        let ran = checkout.run(name, &arguments)?;
        report.check(
            &format!("★S8 음성대조 {name}: 브랜치 feature-x → exit {wanted} · 해소 불가 사유(BRANCH_UNKNOWN) · single 로 접지 않는다"),
            ran.code == Some(wanted)
                && contains(&ran.output, "BRANCH_UNKNOWN")
                && !contains(&ran.output, "topo=single")
                && !checkout.marker.is_file()
                && !checkout.state.is_file(),
            || PyValue::Tuple(vec![ran.code.map_or(PyValue::None, PyValue::int), PyValue::Str(ran.output.clone())]),
        )?;
    }
    let ran = checkout.run(
        "judge_bench.sh",
        &["_probe", "--authority", "weak", "--topology", "single"],
    )?;
    report.check(
        "S9 --topology 명시는 해소기를 부르지 않는다(명시 > 파생) · output/single 경로",
        ran.code == Some(2)
            && contains(&ran.output, "output/single/benchlog")
            && !contains(&ran.output, TAG),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )?;
    git(
        &checkout.root,
        &["symbolic-ref", "HEAD", "refs/heads/multi-node"],
    )?;
    let output_dir = checkout.root.join("output");
    std::fs::remove_dir_all(&output_dir).map_err(|error| os_failure(&error, &output_dir))?;
    let ran = checkout.run("run_bench.sh", &["_probe"])?;
    report.check(
        "S10 manifest 가 한 장도 없는 트리 → run_bench exit 4(미테라포밍 info-only · smoke_clone A8 계약 보존)",
        ran.code == Some(4) && contains(&ran.output, "미테라포밍") && !checkout.marker.is_file(),
        || PyValue::Tuple(vec![ran.code.map_or(PyValue::None, PyValue::int), PyValue::Str(ran.output.clone())]),
    )?;
    let ran = checkout.run("judge_bench.sh", &["_probe", "--authority", "weak"])?;
    report.check(
        "S11 manifest 부재 트리 → judge_bench exit 2(fail-loud · --topology 명시 안내)",
        ran.code == Some(2) && contains(&ran.output, "--topology"),
        || {
            PyValue::Tuple(vec![
                ran.code.map_or(PyValue::None, PyValue::int),
                PyValue::Str(ran.output.clone()),
            ])
        },
    )
}

/// `os.remove(path)`: a missing file is the `FileNotFoundError` of the original.
fn remove_file(path: &Path) -> Result<(), Failure> {
    std::fs::remove_file(path).map_err(|error| os_failure(&error, path))
}
