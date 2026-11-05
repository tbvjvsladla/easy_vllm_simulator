//! `selftest_broad_search_cell_args` and `selftest_broad_search_precheck` as programs. Every scenario runs the real
//! binary, which copies the deployed files into its own temporary git checkout and runs `broad_search.sh` there:
//!
//! * the passing runs of the deployed files (every check line, the summary, an empty standard error);
//! * a mutated `broad_search.sh` in a private repository the program is pointed at (the guard of `cell --authority`
//!   removed, the precheck's refusal removed): the verifiers must fail the checks that guard, which shows the program
//!   copies and runs what it is given and that its checks can fail;
//! * the failures that are not check results: no `git`, a `git init` that prints and fails or is killed, a missing
//!   deployed file, a refused fixture `init`, a damaged or missing lockset template, an unknown repository;
//! * the PyYAML probe of the provenance check, in both outcomes;
//! * the temporary directory is gone after every run, the failing ones included.
//!
//! The programs run with a cleared environment that names only `PATH`, `TMPDIR` and the repository, so no override of
//! the machine running the checks reaches the copies.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use sweep_stop::broad_search_selftest_cli::{CELL_ARGS_DEPLOYED_FILES, PRECHECK_DEPLOYED_FILES};

const CELL_ARGS: &str = env!("CARGO_BIN_EXE_selftest_broad_search_cell_args");
const PRECHECK: &str = env!("CARGO_BIN_EXE_selftest_broad_search_precheck");
const CELL_ARGS_NAME: &str = "selftest_broad_search_cell_args";
const PRECHECK_NAME: &str = "selftest_broad_search_precheck";
const BROAD_SEARCH: &str = ".claude/skills/adversarial-benchmark/scripts/broad_search.sh";
const CAMPAIGN_INIT: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";
const LOCKSET_TEMPLATE: &str = "campaigns/_template/cells/_cell/lockset.json";
const RELEASE_DIR: &str = ".claude/skills/adversarial-benchmark/rust/target/release/";

/// The checkout the deployed files are read from (the same default as the other checks).
fn canonical_root() -> PathBuf {
    std::env::var_os("EASY_VLLM_SOURCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("the crate sits four levels below the checkout")
                .to_path_buf()
        })
}

/// A scratch directory of one scenario: `program-temporary-root/` is the `TMPDIR` of the program under test (it must be
/// empty again when the program ends), `repo/` a private repository, `bin/` fake tools.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "broad-selftest-check-{label}-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("program-temporary-root")).expect("scratch directory");
        Scratch { root }
    }

    fn tmp(&self) -> PathBuf {
        self.root.join("program-temporary-root")
    }

    fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Fails unless the program removed everything it created under its `TMPDIR`.
    fn assert_temporary_directory_removed(&self) {
        let left: Vec<_> = std::fs::read_dir(self.tmp())
            .expect("tmp")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A private repository holding `files` read from the canonical checkout (the release programs are hard-linked when
/// the file systems allow it, never written; everything else is a copy a scenario may change).
struct PrivateRepo {
    root: PathBuf,
}

impl PrivateRepo {
    fn new(scratch: &Scratch, files: &[&str], skip: &[&str]) -> PrivateRepo {
        let root = scratch.repo();
        let source = canonical_root();
        let wanted = files.iter().copied().chain([LOCKSET_TEMPLATE]);
        for relative in wanted.filter(|relative| !skip.contains(relative)) {
            let target = root.join(relative);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("directory");
            if target.exists() {
                continue;
            }
            let from = source.join(relative);
            let linked =
                relative.starts_with(RELEASE_DIR) && std::fs::hard_link(&from, &target).is_ok();
            if !linked {
                std::fs::copy(&from, &target)
                    .unwrap_or_else(|error| panic!("NOT_RUN: {}: {error}", from.display()));
            }
        }
        PrivateRepo { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Replaces `from` by `to` in a copied file; a replacement that changes nothing fails the scenario (a mutant that
    /// is not applied proves nothing).
    fn mutate(&self, relative: &str, from: &str, to: &str) {
        let path = self.path(relative);
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(
            text.matches(from).count(),
            1,
            "the mutated line must exist once in {relative}"
        );
        write_replacing(&path, text.replace(from, to).as_bytes());
    }

    fn replace_file(&self, relative: &str, content: &str) {
        write_replacing(&self.path(relative), content.as_bytes());
    }
}

/// Writes through a new file so a hard link to the canonical checkout is never written.
fn write_replacing(path: &Path, bytes: &[u8]) {
    let mode = std::fs::metadata(path)
        .map(|data| data.permissions().mode())
        .ok();
    let _ = std::fs::remove_file(path);
    std::fs::write(path, bytes).expect("write");
    if let Some(mode) = mode {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("mode");
    }
}

/// What a scenario sets on the program under test.
struct Launch<'a> {
    program: &'a str,
    repo: Option<&'a Path>,
    path: Option<&'a OsStr>,
    arguments: &'a [&'a str],
}

fn launch(scratch: &Scratch, launch: &Launch<'_>) -> Output {
    let mut command = Command::new(launch.program);
    command
        .env_clear()
        .current_dir(&scratch.root)
        .env("TMPDIR", scratch.tmp())
        .args(launch.arguments);
    let default_path = std::env::var_os("PATH").expect("PATH");
    command.env("PATH", launch.path.unwrap_or(default_path.as_os_str()));
    for name in ["HOME", "LANG", "LC_ALL"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if let Some(repo) = launch.repo {
        command.env("BENCH_VERIFY_REPO_ROOT", repo);
    }
    output_retrying_busy(&mut command)
}

fn run(scratch: &Scratch, program: &str, repo: &Path) -> Output {
    launch(
        scratch,
        &Launch {
            program,
            repo: Some(repo),
            path: None,
            arguments: &[],
        },
    )
}

/// Runs the command, retrying only while it cannot start because the executable is still open for writing
/// (`ETXTBSY`: another test's fork can hold a file this scenario has just created until that fork's exec).
fn output_retrying_busy(command: &mut Command) -> Output {
    const EXECUTABLE_FILE_BUSY: i32 = 26;
    for _ in 0..100 {
        match command.output() {
            Err(error) if error.raw_os_error() == Some(EXECUTABLE_FILE_BUSY) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other.expect("run"),
        }
    }
    panic!("the executable stayed busy for two seconds");
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8")
}

/// The first word after `PASS`/`FAIL` of each check line (the case letter), in order.
fn case_letters(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| line.starts_with("  PASS ") || line.starts_with("  FAIL "))
        .map(|line| {
            line[7..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

fn fake_tool(scratch: &Scratch, name: &str, body: &str) {
    let path = scratch.bin().join(name);
    std::fs::create_dir_all(scratch.bin()).expect("bin");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
}

// ── the deployed files pass ──────────────────────────────────────────────────────

#[test]
fn cell_args_passes_all_fourteen_checks_on_the_deployed_files() {
    let scratch = Scratch::new("cell-args-pass");
    let output = run(&scratch, CELL_ARGS, &canonical_root());
    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout_of(&output);
    assert!(!stdout.contains("  FAIL "), "{stdout}");
    assert_eq!(
        case_letters(&stdout),
        [
            "N1★", "N1★", "N1★", "N2★", "N2", "N2★", "N2", "N3★", "N3★", "N3★", "N3", "N4★", "N5★",
            "N6"
        ]
    );
    assert!(stdout
        .ends_with("[selftest_broad_search_cell_args] PASS — N1~N6(★ 교정 전 거동이면 FAIL)\n"));
    scratch.assert_temporary_directory_removed();
}

#[test]
fn precheck_passes_all_twenty_seven_checks_on_the_deployed_files() {
    let scratch = Scratch::new("precheck-pass");
    let output = run(&scratch, PRECHECK, &canonical_root());
    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
    let stdout = stdout_of(&output);
    assert!(!stdout.contains("  FAIL "), "{stdout}");
    assert_eq!(
        case_letters(&stdout),
        [
            "사본에는",
            "A",
            "A",
            "A",
            "A",
            "A",
            "B",
            "C",
            "D",
            "E",
            "E",
            "E",
            "E",
            "E",
            "F",
            "G",
            "H",
            "H",
            "I",
            "I",
            "J",
            "K",
            "K",
            "L",
            "L",
            "M",
            "M"
        ]
    );
    assert!(stdout.ends_with("[selftest_broad_search_precheck] PASS\n"));
    scratch.assert_temporary_directory_removed();
}

#[test]
fn the_command_line_is_ignored_like_the_original() {
    let scratch = Scratch::new("ignored-arguments");
    let plain = run(&scratch, CELL_ARGS, &canonical_root());
    let with_arguments = launch(
        &scratch,
        &Launch {
            program: CELL_ARGS,
            repo: Some(&canonical_root()),
            path: None,
            arguments: &["--help", "--no-such-option", "value"],
        },
    );
    assert_eq!(with_arguments.status.code(), Some(0));
    assert_eq!(stdout_of(&with_arguments), stdout_of(&plain));
    assert_eq!(stderr_of(&with_arguments), "");
}

// ── a mutated deployed script: the verifiers must fail what guards the mutation ──

#[test]
fn cell_args_fails_n4_when_cell_accepts_an_authority() {
    let scratch = Scratch::new("cell-args-mutant");
    let repo = PrivateRepo::new(&scratch, &CELL_ARGS_DEPLOYED_FILES, &[]);
    repo.mutate(
        BROAD_SEARCH,
        "  if [ -n \"$AUTHORITY\" ]; then\n",
        "  if false; then\n",
    );
    let output = run(&scratch, CELL_ARGS, &repo.root);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr_of(&output),
        "",
        "the cell_args summary is printed on standard output"
    );
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("  FAIL N4★ cell --authority → exit 2 · 측정기 미호출\n"),
        "{stdout}"
    );
    assert_eq!(stdout.matches("  FAIL ").count(), 1, "{stdout}");
    assert_eq!(stdout.matches("  PASS ").count(), 13, "{stdout}");
    assert!(stdout.contains("[selftest_broad_search_cell_args] FAIL 1건:\n  - N4★ cell --authority → exit 2 · 측정기 미호출 — rc=0 "));
    assert!(!stdout.contains("] PASS"));
    scratch.assert_temporary_directory_removed();
}

#[test]
fn precheck_fails_the_classification_checks_when_a_refusal_is_reported_as_cannot_judge() {
    let scratch = Scratch::new("precheck-mutant");
    let repo = PrivateRepo::new(&scratch, &PRECHECK_DEPLOYED_FILES, &[]);
    // Without its own branch a refusal falls through to the "cannot judge" branch: still exit 2, but the two
    // classifications prescribe different repairs, which is what the classification checks guard.
    repo.mutate(
        BROAD_SEARCH,
        "  if [ \"$_PCRC\" = \"2\" ] && [ \"$_PC_STATUS\" = \"refused\" ]; then\n",
        "  if false; then\n",
    );
    let output = run(&scratch, PRECHECK, &repo.root);
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_of(&output);
    let failed: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("  FAIL "))
        .collect();
    assert_eq!(
        failed,
        [
            "  FAIL A 표시 부재는 **거부**로 분류된다('판정 불가' 로 새지 않는다 — 두 분류는 처방이 다르다)",
            "  FAIL C provenance 목록 밖 값 → exit 2 · 거부로 분류",
        ]
    );
    assert_eq!(stdout.matches("  PASS ").count(), 25, "{stdout}");
    assert!(!stdout.contains("] PASS"), "{stdout}");
    let stderr = stderr_of(&output);
    assert!(
        stderr.starts_with(
            "[selftest_broad_search_precheck] FAIL 2건:\n  - A 표시 부재는 **거부**로"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("\n  - C provenance 목록 밖 값 → exit 2 · 거부로 분류 — rc=2 "),
        "{stderr}"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn precheck_fails_the_exit_checks_when_the_whole_precheck_is_removed() {
    let scratch = Scratch::new("precheck-no-gate");
    let repo = PrivateRepo::new(&scratch, &PRECHECK_DEPLOYED_FILES, &[]);
    repo.mutate(
        BROAD_SEARCH,
        "  if [ \"$_PCRC\" != \"0\" ] || { [ \"$_PC_STATUS\" != \"passed\" ] && [ \"$_PC_STATUS\" != \"not_applicable\" ]; }; then\n",
        "  if false; then\n",
    );
    repo.mutate(
        BROAD_SEARCH,
        "  if [ \"$_PCRC\" = \"2\" ] && [ \"$_PC_STATUS\" = \"refused\" ]; then\n",
        "  if false; then\n",
    );
    let output = run(&scratch, PRECHECK, &repo.root);
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_of(&output);
    for label in [
        "  FAIL A 부하 경로 · lockset 부재 → exit 2\n",
        "  FAIL B 재조립 경로 · lockset 부재 → exit 2\n",
        "  FAIL C provenance 목록 밖 값 → exit 2 · 거부로 분류\n",
        "  FAIL D 뼈대 복사본(provenance=<<FILL>>) → exit 2\n",
    ] {
        assert!(stdout.contains(label), "{label}\n{stdout}");
    }
    assert!(
        stdout.contains("  PASS E hand-authored → exit 0"),
        "{stdout}"
    );
    let stderr = stderr_of(&output);
    assert!(
        stderr.contains("건:\n  - A 부하 경로 · lockset 부재 → exit 2 — rc=0 "),
        "{stderr}"
    );
    scratch.assert_temporary_directory_removed();
}

// ── failures that are not check results ──────────────────────────────────────────

#[test]
fn a_path_without_git_is_the_named_failure_and_nothing_is_created() {
    for (program, name, tail) in [
        (CELL_ARGS, CELL_ARGS_NAME, ""),
        (PRECHECK, PRECHECK_NAME, "(부재를 통과로 접지 않는다)"),
    ] {
        let scratch = Scratch::new("no-git");
        std::fs::create_dir_all(scratch.bin()).expect("bin");
        let output = launch(
            &scratch,
            &Launch {
                program,
                repo: Some(&canonical_root()),
                path: Some(scratch.bin().as_os_str()),
                arguments: &[],
            },
        );
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert_eq!(stdout_of(&output), "", "{name}");
        assert_eq!(
            stderr_of(&output),
            format!("[{name}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다{tail}\n")
        );
        scratch.assert_temporary_directory_removed();
    }
}

#[test]
fn an_unknown_repository_is_status_two_before_anything_is_created() {
    for (installed, name) in [(CELL_ARGS, CELL_ARGS_NAME), (PRECHECK, PRECHECK_NAME)] {
        let scratch = Scratch::new("unknown-repo");
        // A copy outside any `rust/target/release` ancestry names no installation.
        let program = scratch.root.join("elsewhere").join(name);
        std::fs::create_dir_all(program.parent().expect("parent")).expect("directory");
        std::fs::copy(installed, &program).expect("copy");
        // An unset variable and an empty one both name nothing.
        for repo in [None, Some(Path::new(""))] {
            let output = launch(
                &scratch,
                &Launch {
                    program: program.to_str().expect("UTF-8"),
                    repo,
                    path: None,
                    arguments: &[],
                },
            );
            assert_eq!(output.status.code(), Some(2), "{name} {repo:?}");
            assert_eq!(stdout_of(&output), "");
            assert_eq!(
                stderr_of(&output),
                format!("[{name}] ERROR BENCH_VERIFY_REPO_ROOT 이 없고 실행파일이 설치 경로(rust/target/release) 안에 있지 않아 저장소를 알 수 없다\n")
            );
            scratch.assert_temporary_directory_removed();
        }
    }
}

/// An unusable `TMPDIR` (a nested path that does not exist) with `TEMP` and `TMP` unset: the temporary directory is
/// created under the first usable *fixed* system root, not under the working directory (which is only the last
/// resort). The fake `git` reports where the fixture repository was to be created and fails, so the run ends right
/// after the directory is chosen; the working directory must stay empty and the chosen directory must be gone.
#[test]
fn an_unusable_temporary_directory_variable_falls_through_to_the_system_root_before_the_working_directory(
) {
    for (program, name, prefix) in [
        (CELL_ARGS, CELL_ARGS_NAME, "bs-cellargs."),
        (PRECHECK, PRECHECK_NAME, "bs-precheck."),
    ] {
        let scratch = Scratch::new("unusable-temporary-root");
        fake_tool(
            &scratch,
            "git",
            "printf 'fixture-repo=%s\\n' \"$3\"\nexit 17",
        );
        let working_directory = scratch.root.join("working-directory");
        std::fs::create_dir_all(&working_directory).expect("directory");
        let mut command = Command::new(program);
        command
            .env_clear()
            .current_dir(&working_directory)
            .env("PATH", scratch.bin())
            .env("TMPDIR", scratch.root.join("unusable").join("absent"))
            .env("BENCH_VERIFY_REPO_ROOT", canonical_root());
        let output = output_retrying_busy(&mut command);
        assert_eq!(output.status.code(), Some(1), "{name}");
        let stdout = stdout_of(&output);
        let reported = stdout
            .strip_prefix("fixture-repo=")
            .and_then(|rest| rest.strip_suffix("/repo\n"))
            .unwrap_or_else(|| panic!("{name}: unexpected output {stdout:?}"));
        let chosen = Path::new(reported);
        assert_eq!(
            chosen.parent(),
            Some(Path::new("/tmp")),
            "{name}: {reported}"
        );
        assert!(
            chosen
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|leaf| leaf.starts_with(prefix)),
            "{name}: {reported}"
        );
        assert!(
            !chosen.exists(),
            "{name}: the chosen directory was not removed"
        );
        assert_eq!(
            std::fs::read_dir(&working_directory).expect("read").count(),
            0,
            "{name}: the working directory must stay untouched"
        );
        assert!(
            stderr_of(&output).ends_with("returned non-zero exit status 17.\n"),
            "{name}"
        );
    }
}

#[test]
fn the_installation_around_the_binary_names_the_repository() {
    let scratch = Scratch::new("installed-layout");
    let repo = PrivateRepo::new(&scratch, &CELL_ARGS_DEPLOYED_FILES, &[]);
    let program = repo.path(&format!("{RELEASE_DIR}{CELL_ARGS_NAME}"));
    std::fs::copy(CELL_ARGS, &program).expect("copy");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let output = launch(
        &scratch,
        &Launch {
            program: program.to_str().expect("UTF-8"),
            repo: None,
            path: None,
            arguments: &[],
        },
    );
    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_of(&output).ends_with("PASS — N1~N6(★ 교정 전 거동이면 FAIL)\n"));
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_git_init_that_prints_and_fails_is_a_called_process_error_after_its_own_output() {
    for (program, name) in [(CELL_ARGS, CELL_ARGS_NAME), (PRECHECK, PRECHECK_NAME)] {
        let scratch = Scratch::new("git-fails");
        fake_tool(
            &scratch,
            "git",
            "printf 'partial out\\n'\nprintf 'git: boom\\n' >&2\nexit 128",
        );
        let output = launch(
            &scratch,
            &Launch {
                program,
                repo: Some(&canonical_root()),
                path: Some(scratch.bin().as_os_str()),
                arguments: &[],
            },
        );
        assert_eq!(output.status.code(), Some(1), "{name}");
        // Git shares the program's output: what it printed is there, and no check line was printed.
        assert_eq!(stdout_of(&output), "partial out\n", "{name}");
        let stderr = stderr_of(&output);
        let prefix = format!(
            "git: boom\nTraceback (most recent call last):\n  (native: Python stack frames are not reproduced)\nsubprocess.CalledProcessError: Command '['git', 'init', '-q', '{}/{}",
            scratch.tmp().display(),
            if name == CELL_ARGS_NAME { "bs-cellargs." } else { "bs-precheck." }
        );
        assert!(stderr.starts_with(&prefix), "{stderr}");
        assert!(
            stderr.ends_with("/repo']' returned non-zero exit status 128.\n"),
            "{stderr}"
        );
        scratch.assert_temporary_directory_removed();
    }
}

#[test]
fn a_git_that_is_killed_by_a_signal_is_named_by_the_signal() {
    let scratch = Scratch::new("git-killed");
    fake_tool(&scratch, "git", "kill -9 $$");
    let output = launch(
        &scratch,
        &Launch {
            program: CELL_ARGS,
            repo: Some(&canonical_root()),
            path: Some(scratch.bin().as_os_str()),
            arguments: &[],
        },
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert!(
        stderr.ends_with("/repo']' died with <Signals.SIGKILL: 9>.\n"),
        "{stderr}"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_deployed_file_is_the_original_file_not_found_error() {
    for (program, files, name) in [
        (CELL_ARGS, &CELL_ARGS_DEPLOYED_FILES, CELL_ARGS_NAME),
        (PRECHECK, &PRECHECK_DEPLOYED_FILES, PRECHECK_NAME),
    ] {
        let scratch = Scratch::new("missing-file");
        let repo = PrivateRepo::new(&scratch, files, &[CAMPAIGN_INIT]);
        let output = run(&scratch, program, &repo.root);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert_eq!(stdout_of(&output), "", "{name}");
        assert_eq!(
            stderr_of(&output),
            format!(
                "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\nFileNotFoundError: [Errno 2] No such file or directory: '{}'\n",
                repo.path(CAMPAIGN_INIT).display()
            ),
            "{name}"
        );
        scratch.assert_temporary_directory_removed();
    }
}

#[test]
fn a_refused_fixture_init_is_a_system_exit_with_the_refusal_text() {
    for (program, files, name) in [
        (CELL_ARGS, &CELL_ARGS_DEPLOYED_FILES, CELL_ARGS_NAME),
        (PRECHECK, &PRECHECK_DEPLOYED_FILES, PRECHECK_NAME),
    ] {
        let scratch = Scratch::new("init-refused");
        let repo = PrivateRepo::new(&scratch, files, &[]);
        repo.replace_file(BROAD_SEARCH, "#!/bin/bash\necho boom >&2\nexit 3\n");
        let output = run(&scratch, program, &repo.root);
        assert_eq!(output.status.code(), Some(1), "{name}");
        // The precheck verifier prints its first check before the fixture `init`; the other prints nothing yet.
        let printed_before_init = if name == PRECHECK_NAME {
            "  PASS 사본에는 부하 스크립트가 없다(경로가 잘못 열려도 실행할 대상이 없다)\n"
        } else {
            ""
        };
        assert_eq!(stdout_of(&output), printed_before_init, "{name}");
        // The reason is the child's whole standard error (its own newline included) and the interpreter's newline.
        assert_eq!(
            stderr_of(&output),
            format!("[{name}] 픽스처 init 실패: boom\n\n"),
            "{name}"
        );
        scratch.assert_temporary_directory_removed();
    }
}

#[test]
fn a_damaged_lockset_template_raises_after_the_earlier_checks_were_printed() {
    let scratch = Scratch::new("template-damaged");
    let repo = PrivateRepo::new(&scratch, &PRECHECK_DEPLOYED_FILES, &[]);
    repo.replace_file(LOCKSET_TEMPLATE, "{");
    let output = run(&scratch, PRECHECK, &repo.root);
    assert_eq!(output.status.code(), Some(1));
    let stdout = stdout_of(&output);
    assert_eq!(
        case_letters(&stdout),
        ["사본에는", "A", "A", "A", "A", "A", "B", "C"],
        "the run stops where the template is read: {stdout}"
    );
    assert!(!stdout.contains("] PASS"));
    assert_eq!(
        stderr_of(&output),
        "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\njson.decoder.JSONDecodeError: Expecting property name enclosed in double quotes: line 1 column 2 (char 1)\n"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_lockset_template_is_not_skipped() {
    let scratch = Scratch::new("template-missing");
    let repo = PrivateRepo::new(&scratch, &PRECHECK_DEPLOYED_FILES, &[]);
    std::fs::remove_file(repo.path(LOCKSET_TEMPLATE)).expect("remove");
    let output = run(&scratch, PRECHECK, &repo.root);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        case_letters(&stdout_of(&output)).last().map(String::as_str),
        Some("C")
    );
    assert_eq!(
        stderr_of(&output),
        format!(
            "Traceback (most recent call last):\n  (native: Python stack frames are not reproduced)\nFileNotFoundError: [Errno 2] No such file or directory: '{}'\n",
            repo.path(LOCKSET_TEMPLATE).display()
        )
    );
    scratch.assert_temporary_directory_removed();
}

// ── the PyYAML probe ─────────────────────────────────────────────────────────────

fn real_python3() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join("python3"))
        .find(|candidate| candidate.is_file())
}

fn interpreter_has_yaml(python: &Path) -> bool {
    Command::new(python)
        .args(["-c", "import yaml"])
        .output()
        .expect("run python3")
        .status
        .success()
}

/// The interpreter says it has no PyYAML (an interpreter shim that fails only `-c "import yaml"`). The check then
/// takes its "node without PyYAML" branch. If the real interpreter does have PyYAML the writer records the mismatch
/// anyway, so that branch's expectation (no mismatch, a named gap) is false and the check must FAIL: absence that is
/// not real is not accepted as a pass. If the real interpreter lacks PyYAML the branch is true and passes.
#[test]
fn the_pyyaml_probe_decides_which_expectation_is_checked() {
    let python = real_python3().expect("NOT_RUN: no python3 on PATH");
    let scratch = Scratch::new("pyyaml-probe");
    fake_tool(
        &scratch,
        "python3",
        &format!(
            "if [ \"$1\" = \"-c\" ] && [ \"$2\" = \"import yaml\" ]; then exit 1; fi\nexec {} \"$@\"",
            python.display()
        ),
    );
    let path = std::env::join_paths(std::iter::once(scratch.bin()).chain(std::env::split_paths(
        &std::env::var_os("PATH").expect("PATH"),
    )))
    .expect("PATH");
    let output = launch(
        &scratch,
        &Launch {
            program: PRECHECK,
            repo: Some(&canonical_root()),
            path: Some(&path),
            arguments: &[],
        },
    );
    let stdout = stdout_of(&output);
    let absent_label =
        "E (PyYAML 부재 노드) 불일치 대신 '대조 불가' 가 기재된다 — 일치로 접지 않는다";
    let present_label =
        "E 서빙 gmu 0.80 ≠ 선언 target_gmu 0.85 가 provenance_mismatch[] 에 기재된다";
    assert!(
        !stdout.contains(present_label),
        "the shim's answer selects the absent-PyYAML branch: {stdout}"
    );
    if interpreter_has_yaml(&python) {
        assert!(
            stdout.contains(&format!("  FAIL {absent_label}\n")),
            "{stdout}"
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(stderr_of(&output).contains("E (PyYAML 부재 노드)"));
    } else {
        assert!(
            stdout.contains(&format!("  PASS {absent_label}\n")),
            "{stdout}"
        );
        assert_eq!(output.status.code(), Some(0));
    }
    scratch.assert_temporary_directory_removed();
}
