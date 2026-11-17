//! `selftest_sweep_repeats` as a program. Every scenario runs the real binary on a private copy of the deployed tree;
//! the program copies the deployed scripts and programs into its own temporary git checkout, runs the shell chain
//! there (sweep, judge, repeat axis, classify, report, publish, broad search) and, for case G, the installed
//! conformance program:
//!
//! * the passing run of the deployed tree: the 80 check lines (75 + C1~C4 · plan_26111708 B L12) and the closing line byte for byte as the original
//!   printed them, no diagnostics, the temporary directory gone, and **nothing in the private tree changed**
//!   (contents, permissions, inodes, modification times, and no new file such as a byte-code cache);
//! * arguments: everything but `-v` is ignored (`--help` included), `-v` adds the sweep output after the first check;
//! * the named stops: no `git` on `PATH` (before any fixture), an unknown repository (this port's exit 2), a
//!   deployed script that is missing (the copy's `FileNotFoundError`, nothing printed, no fixture left);
//! * mutated deployed files in a private tree: a changed log line fails only the check that reads it; a hint reader that
//!   raises on import fails only its check (and is imported once, late, under its own name); a reader that exits ends
//!   the run after its ten lines; an unusable owner interpreter is a named failure after those ten lines; a missing
//!   conformance program fails only case G;
//! * the concurrent fresh-directory claim is really concurrent: the deployed claim code is instrumented to write its
//!   start and end, and the two claims overlap in time while exactly one succeeds;
//! * the product environment is inherited as the original inherited it: an owner-root override that points the
//!   inner chain at the outer tree hides the sandbox campaign and fails four named checks (the tests themselves
//!   clear such overrides).
//!
//! The programs run with a cleared environment that names only `PATH`, `TMPDIR`, the repository and (where a scenario
//! says so) the interpreter of the owner bridge or the override under test.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PROGRAM: &str = env!("CARGO_BIN_EXE_selftest_sweep_repeats");
/// The file name of the program (as a copy is named).
const PROGRAM_NAME: &str = "selftest_sweep_repeats";
const NORMAL_STDOUT: &str = include_str!("data/sweep_repeats/normal_stdout.txt");

const RELEASE: &str = ".claude/skills/adversarial-benchmark/rust/target/release";
const READER: &str = ".claude/skills/hint-publisher/scripts/render_bench_section.sh";
const SWEEP_SCRIPT: &str = ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh";

/// The deployed tree the files are read from (the same default as the other checks).
fn canonical_root() -> PathBuf {
    std::env::var_os("BENCH_VERIFY_REPO_ROOT")
        .or_else(|| std::env::var_os("EASY_VLLM_SOURCE_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .expect("the crate sits four levels below the checkout")
                .to_path_buf()
        })
}

/// A scratch directory of one scenario: `program-temporary-root/` is the `TMPDIR` of the program under test (it must
/// be empty again when the program ends), `repo/` a private tree and `empty-path/` a directory with no program in it.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "sweep-repeats-check-{label}-{}-{serial}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("program-temporary-root")).expect("scratch directory");
        std::fs::create_dir_all(root.join("empty-path")).expect("empty search path");
        Scratch { root }
    }

    fn tmp(&self) -> PathBuf {
        self.root.join("program-temporary-root")
    }

    fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn empty_path(&self) -> PathBuf {
        self.root.join("empty-path")
    }

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

/// Copies `source` to `destination` as a **new file** (new inode, the source's bytes and permission bits). An
/// existing name is unlinked first, so a destination that is a hard link to the source (or to a deployed program)
/// cannot truncate the shared inode; nothing of the deployed tree is ever linked into a private tree.
fn copy_detached(source: &Path, destination: &Path) {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).expect("parent directory");
    }
    if let Ok(existing) = std::fs::symlink_metadata(destination) {
        assert!(
            !existing.is_dir(),
            "{} is a directory",
            destination.display()
        );
        std::fs::remove_file(destination).expect("remove the existing name");
    }
    let before = std::fs::metadata(source).expect("source metadata");
    std::fs::copy(source, destination)
        .unwrap_or_else(|error| panic!("NOT_RUN: {}: {error}", source.display()));
    let after = std::fs::metadata(destination).expect("destination metadata");
    assert!(
        before.dev() != after.dev() || before.ino() != after.ino(),
        "{} shares the inode of {}",
        destination.display(),
        source.display()
    );
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("directory");
    for entry in std::fs::read_dir(from).expect("read directory") {
        let entry = entry.expect("entry");
        let name = entry.file_name();
        if matches!(
            name.to_string_lossy().as_ref(),
            "target" | "node_modules" | "__pycache__" | ".git"
        ) {
            continue;
        }
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(&name));
        } else if kind.is_file() {
            copy_detached(&entry.path(), &to.join(&name));
        }
    }
}

/// A private tree with everything the program reads: the benchmark scripts and their owners, the release programs
/// (as new files), the campaign template, and the five sources the conformance program reads as static text.
fn private_tree(scratch: &Scratch) -> PathBuf {
    let source = canonical_root();
    let target = scratch.repo();
    let _ = std::fs::remove_dir_all(&target);
    for relative in [
        ".claude/policies",
        ".claude/schemas",
        ".claude/skills/hint-publisher",
        ".claude/skills/terraforming_node",
        ".claude/skills/wiki-desk",
        ".claude/skills/adversarial-benchmark/scripts",
        ".claude/skills/adversarial-benchmark/config",
        ".claude/skills/adversarial-benchmark/fixtures",
        "campaigns/_template",
    ] {
        copy_tree(&source.join(relative), &target.join(relative));
    }
    for relative in [
        ".claude/skills/adversarial-benchmark/bench_tool_pin.json",
        ".claude/skills/adversarial-benchmark/rust/src/classify_cell.rs",
        ".claude/skills/adversarial-benchmark/rust/src/classify_cell_cli.rs",
        ".claude/skills/adversarial-benchmark/rust/src/argv.rs",
        ".claude/skills/adversarial-benchmark/rust/src/assemble_sweep_meta.rs",
        ".claude/skills/adversarial-benchmark/rust/src/bench_shell/broad.rs",
        ".claude/skills/adversarial-benchmark/rust/src/campaign_write.rs",
        ".claude/skills/vllm-recipe-explorer/rust/runtime/src/recipe_cmd/simulate_body.rs",
    ] {
        if source.join(relative).is_file() {
            copy_detached(&source.join(relative), &target.join(relative));
        }
    }
    for entry in std::fs::read_dir(source.join(RELEASE)).expect("release programs") {
        let entry = entry.expect("entry");
        if entry.file_type().expect("file type").is_file() {
            copy_detached(&entry.path(), &target.join(RELEASE).join(entry.file_name()));
        }
    }
    target
}

/// Puts a forwarding script in place of the native program of a private tree (the program itself moves to `home`,
/// outside the tree, because the verifier copies the tree's files one by one and the script must find the program
/// from wherever it is copied), so a test can watch or alter what the deployed callers get from it without touching
/// the program's source. `body` runs with `$real` naming the program.
fn interpose_bench_shell(tree: &Path, home: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let program = tree.join(RELEASE).join("bench_shell");
    let real = home.join("bench_shell.real");
    std::fs::copy(&program, &real).expect("keep the native program outside the tree");
    std::fs::write(
        &program,
        format!("#!/bin/bash\nreal='{}'\n{body}\n", real.display()),
    )
    .expect("write the forwarding script");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("make it executable");
}

/// Replaces `from` with `to` in a file of a private tree; the replacement must change something.
fn mutate(tree: &Path, relative: &str, from: &str, to: &str) {
    let path = tree.join(relative);
    let text = std::fs::read_to_string(&path).expect("read mutated file");
    assert_eq!(
        text.matches(from).count(),
        1,
        "{relative} must hold {from:?} exactly once"
    );
    std::fs::write(&path, text.replace(from, to)).expect("write mutated file");
}

struct Run {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Run {
    fn from(output: Output) -> Run {
        Run {
            status: output.status.code(),
            stdout: String::from_utf8(output.stdout).expect("stdout is text"),
            stderr: String::from_utf8(output.stderr).expect("stderr is text"),
        }
    }

    fn passed(&self) -> usize {
        self.stdout
            .lines()
            .filter(|line| line.starts_with("  [PASS] "))
            .count()
    }

    fn failures(&self) -> Vec<&str> {
        self.stdout
            .lines()
            .filter(|line| line.starts_with("  [FAIL] "))
            .collect()
    }
}

/// The settings of one run: `PATH`, `TMPDIR`, the repository and the extra variables.
struct Setup<'a> {
    tree: Option<&'a Path>,
    search_path: Option<PathBuf>,
    extra: Vec<(&'a str, &'a OsStr)>,
    arguments: Vec<&'a str>,
}

impl<'a> Setup<'a> {
    fn on(tree: &'a Path) -> Setup<'a> {
        Setup {
            tree: Some(tree),
            search_path: None,
            extra: Vec::new(),
            arguments: Vec::new(),
        }
    }
}

/// Runs the verifier under test (the binary Cargo built for this check) with the cleared environment.
fn run(setup: &Setup<'_>, scratch: &Scratch) -> Run {
    run_binary(Path::new(PROGRAM), setup, scratch)
}

/// Runs the given copy of the verifier with the cleared environment.
fn run_binary(program: &Path, setup: &Setup<'_>, scratch: &Scratch) -> Run {
    let mut command = Command::new(program);
    command
        .args(&setup.arguments)
        .env_clear()
        .env(
            "PATH",
            setup
                .search_path
                .clone()
                .map(PathBuf::into_os_string)
                .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
        )
        .env("TMPDIR", scratch.tmp());
    if let Some(tree) = setup.tree {
        command.env("BENCH_VERIFY_REPO_ROOT", tree);
    }
    for (name, value) in &setup.extra {
        command.env(name, value);
    }
    Run::from(output_when_started(&mut command))
}

/// `ETXTBSY` (errno 26): a program this test has just written can still be open for writing in a process another test
/// thread forked a moment ago. Only the **start** is retried, a bounded number of times and only for this errno.
const TEXT_FILE_BUSY: i32 = 26;
const START_ATTEMPTS: u32 = 100;

fn output_when_started(command: &mut Command) -> Output {
    for attempt in 1..=START_ATTEMPTS {
        match command.output() {
            Err(error)
                if error.raw_os_error() == Some(TEXT_FILE_BUSY) && attempt < START_ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other.expect("the program starts"),
        }
    }
    unreachable!("the last attempt returns")
}

/// Every entry of a tree: kind, contents, permission bits, inode and modification time (directories by name).
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    collect(root, root, &mut entries);
    entries
}

fn collect(root: &Path, directory: &Path, entries: &mut BTreeMap<String, String>) {
    for entry in std::fs::read_dir(directory).expect("read directory") {
        let path = entry.expect("entry").path();
        let metadata = std::fs::symlink_metadata(&path).expect("metadata");
        let name = path
            .strip_prefix(root)
            .expect("below the root")
            .to_string_lossy()
            .into_owned();
        let description = if metadata.is_dir() {
            format!("dir mode={:o}", metadata.mode())
        } else if metadata.file_type().is_symlink() {
            format!("link to {:?}", std::fs::read_link(&path).expect("link"))
        } else {
            let bytes = std::fs::read(&path).expect("read file");
            let sum = bytes.iter().fold(0_u64, |sum, byte| {
                sum.wrapping_mul(1_099_511_628_211)
                    .wrapping_add(u64::from(*byte))
            });
            format!(
                "file mode={:o} ino={} mtime={}.{} len={} sum={sum}",
                metadata.mode(),
                metadata.ino(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                bytes.len()
            )
        };
        entries.insert(name, description);
        if metadata.is_dir() {
            collect(root, &path, entries);
        }
    }
}

fn first_lines(text: &str, count: usize) -> String {
    text.split_inclusive('\n').take(count).collect()
}

#[test]
fn the_deployed_tree_passes_80_checks_with_the_original_output_and_changes_nothing_in_it() {
    let scratch = Scratch::new("pass");
    let tree = private_tree(&scratch);
    let before = snapshot(&tree);
    let result = run(&Setup::on(&tree), &scratch);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stderr, "");
    assert_eq!(result.passed(), 80, "{}", result.stdout);
    assert!(result.failures().is_empty());
    assert_eq!(
        result.stdout, NORMAL_STDOUT,
        "byte for byte as the original printed it"
    );
    scratch.assert_temporary_directory_removed();
    assert_eq!(
        snapshot(&tree),
        before,
        "the program wrote into the deployed tree"
    );
}

#[test]
fn unrelated_arguments_are_ignored() {
    let scratch = Scratch::new("ignored");
    let tree = private_tree(&scratch);
    let mut setup = Setup::on(&tree);
    setup.arguments = vec!["--help", "positional", "--unknown"];
    let result = run(&setup, &scratch);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stderr, "");
    assert_eq!(result.stdout, NORMAL_STDOUT);
    scratch.assert_temporary_directory_removed();
}

#[test]
fn the_verbose_flag_among_other_arguments_adds_the_sweep_output_after_the_first_check() {
    let scratch = Scratch::new("verbose");
    let tree = private_tree(&scratch);
    let mut setup = Setup::on(&tree);
    setup.arguments = vec!["--help", "-v", "x"];
    let result = run(&setup, &scratch);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    let first = first_lines(NORMAL_STDOUT, 1);
    let rest = &NORMAL_STDOUT[first.len()..];
    assert!(result.stdout.starts_with(&first), "{}", result.stdout);
    assert!(result.stdout.ends_with(rest), "{}", result.stdout);
    let extra = &result.stdout[first.len()..result.stdout.len() - rest.len()];
    assert!(
        extra.contains("[sweep_bench]") && extra.ends_with('\n'),
        "the first sweep's output tails: {extra:?}"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_git_is_named_before_any_fixture_and_an_unknown_repository_after_it() {
    let scratch = Scratch::new("named");
    let tree = private_tree(&scratch);
    let mut setup = Setup::on(&tree);
    setup.search_path = Some(scratch.empty_path());
    let result = run(&setup, &scratch);
    assert_eq!(result.status, Some(1));
    assert_eq!(result.stdout, "");
    assert_eq!(
        result.stderr,
        "[selftest_sweep_repeats] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)\n"
    );
    scratch.assert_temporary_directory_removed();
    // No repository named, and the binary is not inside an installation: this port's own exit status 2. The binary
    // Cargo built may itself sit in an installed tree (then it rightly finds that tree), so the program is copied as a
    // new file into a directory of the scratch area whose name is not `release`: the installed layout is false by
    // construction, whatever `TMPDIR` of the test process is.
    let isolated = scratch.root.join("isolated-program").join(PROGRAM_NAME);
    copy_detached(Path::new(PROGRAM), &isolated);
    assert_ne!(
        isolated.parent().and_then(Path::file_name),
        Some(OsStr::new("release")),
        "the isolated copy must not look like an installed release program"
    );
    let unknown = run_binary(
        &isolated,
        &Setup {
            tree: None,
            search_path: None,
            extra: Vec::new(),
            arguments: Vec::new(),
        },
        &scratch,
    );
    assert_eq!(unknown.status, Some(2));
    assert_eq!(unknown.stdout, "");
    assert!(
        unknown
            .stderr
            .starts_with("[selftest_sweep_repeats] ERROR BENCH_VERIFY_REPO_ROOT"),
        "{}",
        unknown.stderr
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn an_installed_layout_program_finds_its_repository_without_the_override() {
    // The control of the unknown-repository half: the same program as a new file in the release directory of a tree
    // (`<tree>/.claude/skills/adversarial-benchmark/rust/target/release/`) finds that tree by itself and passes.
    let scratch = Scratch::new("installed");
    let tree = private_tree(&scratch);
    let installed = tree.join(RELEASE).join(PROGRAM_NAME);
    copy_detached(Path::new(PROGRAM), &installed);
    let result = run_binary(
        &installed,
        &Setup {
            tree: None,
            search_path: None,
            extra: Vec::new(),
            arguments: Vec::new(),
        },
        &scratch,
    );
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stderr, "");
    assert_eq!(result.stdout, NORMAL_STDOUT);
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_deployed_script_is_the_copys_file_not_found_before_any_check_and_leaves_no_fixture() {
    let scratch = Scratch::new("missing");
    let tree = private_tree(&scratch);
    std::fs::remove_file(tree.join(SWEEP_SCRIPT)).expect("remove the deployed sweep script");
    let result = run(&Setup::on(&tree), &scratch);
    assert_eq!(result.status, Some(1));
    assert_eq!(
        result.stdout, "",
        "nothing is checked before the copy fails"
    );
    assert!(
        result
            .stderr
            .starts_with("Traceback (most recent call last):\n"),
        "{}",
        result.stderr
    );
    assert!(
        result.stderr.ends_with(&format!(
            "FileNotFoundError: [Errno 2] No such file or directory: '{}'\n",
            tree.join(SWEEP_SCRIPT).display()
        )),
        "{}",
        result.stderr
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_changed_deployed_log_line_fails_only_the_check_that_reads_it() {
    let scratch = Scratch::new("log-line");
    let tree = private_tree(&scratch);
    // The line is printed by the native program the script calls; the deployed log line changes on its way out.
    interpose_bench_shell(
        &tree,
        &scratch.root,
        "set -o pipefail\n\"$real\" \"$@\" | sed -e 's/^\\[sweep_bench\\] bench_mode=/[sweep_bench] mode=/'",
    );
    let result = run(&Setup::on(&tree), &scratch);
    assert_eq!(result.status, Some(1), "{}", result.stdout);
    assert_eq!(result.passed(), 79, "{}", result.stdout);
    let name = "N3 sweep_bench 로그에 판정이 보인다(부르기만 하고 판정하지 않는다)";
    let failed = result.failures();
    assert_eq!(failed.len(), 1, "{}", result.stdout);
    assert!(
        failed[0].starts_with(&format!("  [FAIL] {name} — ")),
        "{}",
        failed[0]
    );
    assert!(
        failed[0].chars().count()
            <= "  [FAIL] ".chars().count() + name.chars().count() + " — ".chars().count() + 700,
        "the detail is the last 700 characters"
    );
    assert_eq!(
        result.stderr,
        format!("[selftest_sweep_repeats] FAIL 1건: ['{name}']\n")
    );
    scratch.assert_temporary_directory_removed();
}

/// Python stand-ins on `PATH` (`python3`, `python`) that leave a marker and fail: any interpreter start is visible.
fn poisoned_interpreters(scratch: &Scratch) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = scratch.root.join("poisoned-bin");
    std::fs::create_dir_all(&bin).expect("stub directory");
    let marker = scratch.root.join("python-started.marker");
    for name in ["python3", "python"] {
        let stub = bin.join(name);
        std::fs::write(
            &stub,
            format!("#!/bin/sh\necho \"$0 $*\" >> '{}'\nexit 97\n", marker.display()),
        )
        .expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }
    (bin, marker)
}

#[test]
fn the_hint_reader_check_needs_no_reader_py_and_starts_no_interpreter() {
    // plan_26111615 pass 3: the reader is the hint-publisher Rust library. Without its `.py` and with every Python
    // interpreter on `PATH` poisoned, the run is the normal 80-check pass, byte for byte.
    let scratch = Scratch::new("no-python");
    let tree = private_tree(&scratch);
    // absent already when the run has no `.py` at all
    let _ = std::fs::remove_file(tree.join(READER));
    let (bin, marker) = poisoned_interpreters(&scratch);
    let mut setup = Setup::on(&tree);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut search = std::ffi::OsString::from(bin.as_os_str());
    search.push(":");
    search.push(&inherited);
    setup.search_path = Some(PathBuf::from(search));
    let result = run(&setup, &scratch);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stdout, NORMAL_STDOUT);
    assert!(
        !marker.exists(),
        "an interpreter was started: {}",
        std::fs::read_to_string(&marker).unwrap_or_default()
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_conformance_program_fails_only_the_conformance_check() {
    let scratch = Scratch::new("conformance");
    let tree = private_tree(&scratch);
    std::fs::remove_file(tree.join(RELEASE).join("conformance_full_superset_lite"))
        .expect("remove the deployed conformance program");
    let result = run(&Setup::on(&tree), &scratch);
    assert_eq!(result.status, Some(1), "{}", result.stdout);
    assert_eq!(result.passed(), 79, "{}", result.stdout);
    let name = "G conformance(게이트 포함 단언 포함) 초록";
    assert_eq!(result.failures(), [format!("  [FAIL] {name} — ")]);
    assert_eq!(
        result.stderr,
        format!("[selftest_sweep_repeats] FAIL 1건: ['{name}']\n")
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn the_fresh_directory_claim_race_runs_both_calls_at_once_and_exactly_one_claim_succeeds() {
    let scratch = Scratch::new("race");
    let tree = private_tree(&scratch);
    let log = scratch.root.join("claims.log");
    // The deployed sweep script records when each call starts, and the native path program (through a forwarding script)
    // when it reaches the claim (both in wall-clock seconds); this changes nothing they decide.
    mutate(
        &tree,
        SWEEP_SCRIPT,
        "#!/usr/bin/env bash\n# sweep_bench.sh — client-load",
        &format!(
            "#!/usr/bin/env bash\nprintf 'start %s %s\\n' \"$EPOCHREALTIME\" \"$*\" >> '{}'\n# sweep_bench.sh — client-load",
            log.display()
        ),
    );
    interpose_bench_shell(
        &tree,
        &scratch.root,
        &format!(
            "if [ \"$1\" = sweep-path ] && [ \"$6\" = claim ]; then printf 'claim %s %s\\n' \"$4\" \"$EPOCHREALTIME\" >> '{}'; fi\nexec \"$real\" \"$@\"",
            log.display()
        ),
    );
    let result = run(&Setup::on(&tree), &scratch);
    assert_eq!(
        result.status,
        Some(0),
        "{}\n{}",
        result.stdout,
        result.stderr
    );
    assert_eq!(
        result.stdout, NORMAL_STDOUT,
        "the instrumentation decides nothing"
    );
    let text = std::fs::read_to_string(&log).expect("the starts and claims were recorded");
    let seconds = |line: &str, word: &str| -> Option<f64> {
        line.strip_prefix(word)?.split(' ').next()?.parse().ok()
    };
    let race_starts: Vec<f64> = text
        .lines()
        .filter(|line| line.starts_with("start ") && line.contains("sweep_cfg-ba_race"))
        .map(|line| seconds(line, "start ").expect("a start time"))
        .collect();
    let race_claims: Vec<f64> = text
        .lines()
        .filter(|line| line.starts_with("claim ") && line.contains("sweep_cfg-ba_race"))
        .map(|line| {
            line.rsplit(' ')
                .next()
                .expect("a claim time")
                .parse()
                .expect("number")
        })
        .collect();
    assert_eq!(race_starts.len(), 2, "two racing calls: {text}");
    assert!(race_claims.len() >= 2, "both calls reach the claim: {text}");
    let latest_start = race_starts.iter().copied().fold(f64::MIN, f64::max);
    let earliest_claim = race_claims.iter().copied().fold(f64::MAX, f64::min);
    assert!(
        latest_start < earliest_claim,
        "the second call started only after the first had claimed: starts {race_starts:?}, claims {race_claims:?}"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn an_inherited_owner_root_override_hides_the_sandbox_campaign_as_in_the_original() {
    let scratch = Scratch::new("inherited");
    let tree = private_tree(&scratch);
    let mut setup = Setup::on(&tree);
    setup.extra = vec![("BENCH_PRESET_REPO_ROOT", tree.as_os_str())];
    let result = run(&setup, &scratch);
    assert_eq!(result.status, Some(1), "{}", result.stdout);
    let failed: Vec<&str> = result
        .failures()
        .iter()
        .map(|line| {
            line.trim_start_matches("  [FAIL] ")
                .split(" — ")
                .next()
                .expect("name")
        })
        .collect();
    let starts: Vec<&str> = ["N1 index.repetition", "N6 재조립", "P1★", "A2★"].to_vec();
    assert_eq!(failed.len(), starts.len(), "{}", result.stdout);
    for (name, start) in failed.iter().zip(starts) {
        assert!(name.starts_with(start), "{name} / {start}");
    }
    assert!(
        result
            .stderr
            .starts_with("[selftest_sweep_repeats] FAIL 4건: ["),
        "{}",
        result.stderr
    );
    scratch.assert_temporary_directory_removed();
}
