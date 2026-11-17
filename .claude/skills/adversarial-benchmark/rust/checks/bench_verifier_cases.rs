//! `selftest_judge_bench`, `selftest_lite_report`, `selftest_lite_gate` and `selftest_sweep_meta` as programs. Every
//! scenario runs the real binary on a deployed tree, which copies the deployed files into its own temporary git
//! checkout and runs the shell scripts there:
//!
//! * the passing runs of the deployed tree (every check line, the closing line, the expected diagnostics);
//! * mutated deployed files in a private tree the program is pointed at (publication suppression removed, the lite
//!   default flipped, the publisher forced to succeed, the reader's contract title changed, the embedded program's
//!   marker removed): the verifiers must fail exactly the checks that guard those facts — the program copies and runs
//!   what it is given, and its checks can fail;
//! * the owner bridge: a reader that appends a declared marker when it is imported (the verifier must import the real
//!   module once for its own binding and the real evidence publisher must import it again at each call — five lines,
//!   not one), and an unusable interpreter (a named failure before any check);
//! * an unknown context (named before any fixture), a missing `git` (named first), `-v` and unrelated arguments;
//! * the temporary directory is gone after every run, the failing ones included.
//!
//! The programs run with a cleared environment that names only `PATH`, `TMPDIR`, the repository and (where a scenario
//! says so) the interpreter of the owner bridge.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const JUDGE: &str = env!("CARGO_BIN_EXE_selftest_judge_bench");
const LITE_REPORT: &str = env!("CARGO_BIN_EXE_selftest_lite_report");
const LITE_GATE: &str = env!("CARGO_BIN_EXE_selftest_lite_gate");
const SWEEP_META: &str = env!("CARGO_BIN_EXE_selftest_sweep_meta");

const SCRIPTS: &str = ".claude/skills/adversarial-benchmark/scripts";
const RELEASE: &str = ".claude/skills/adversarial-benchmark/rust/target/release";
const READER: &str = ".claude/skills/hint-publisher/scripts/render_bench_section.sh";

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
/// be empty again when the program ends) and `repo/` a private tree.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "bench-verifier-check-{label}-{}-{serial}",
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

/// Copies `source` to `destination` as a **new file** (new inode, the source's bytes and permission bits).
///
/// An existing name at the destination is unlinked first. That removes only the name: it never opens the old file for
/// writing, so a destination that is a hard link to the source (or to a deployed program) cannot truncate the shared
/// inode. A plain `std::fs::copy` over such a name does exactly that, and did once destroy deployed programs.
fn copy_detached(source: &Path, destination: &Path) {
    use std::os::unix::fs::MetadataExt;
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
        let name_text = name.to_string_lossy();
        if matches!(
            name_text.as_ref(),
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

/// The release programs of the deployed tree, copied into the private tree as new files. They are never linked: a
/// link shares the inode of the deployed program, and anything that opens the copy for writing (a second copy onto
/// the same name truncates it) would destroy the deployed program itself.
fn copy_release_programs(source: &Path, target: &Path) {
    let from = source.join(RELEASE);
    std::fs::create_dir_all(target.join(RELEASE)).expect("release directory");
    for entry in std::fs::read_dir(&from).expect("release programs") {
        let entry = entry.expect("entry");
        if entry.file_type().expect("file type").is_file() {
            let destination = target.join(RELEASE).join(entry.file_name());
            copy_detached(&entry.path(), &destination);
        }
    }
}

/// A private tree with everything the four programs read: the benchmark scripts and their owners, the release
/// programs, and the campaign template the schema is read from.
fn private_tree(scratch: &Scratch) -> PathBuf {
    let source = canonical_root();
    let target = scratch.repo();
    // Every private tree is built into a directory that does not exist yet.
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
    // The pin record is read only by programs these checks do not run; copied when the tree has it.
    let pin = ".claude/skills/adversarial-benchmark/bench_tool_pin.json";
    if source.join(pin).is_file() {
        copy_detached(&source.join(pin), &target.join(pin));
    }
    copy_release_programs(&source, &target);
    target
}

/// Replaces `from` with `to` in a file of a private tree; the replacement must change something.
fn mutate(tree: &Path, relative: &str, from: &str, to: &str) {
    let path = tree.join(relative);
    let text = std::fs::read_to_string(&path).expect("read mutated file");
    assert!(text.contains(from), "{relative} no longer holds {from:?}");
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

    fn failed(&self) -> usize {
        self.stdout
            .lines()
            .filter(|line| line.starts_with("  [FAIL] "))
            .count()
    }
}

/// Runs a verifier with the cleared environment. `tree` = `BENCH_VERIFY_REPO_ROOT` (none = not set).
fn run(
    program: &str,
    tree: Option<&Path>,
    scratch: &Scratch,
    extra: &[(&str, &OsStr)],
    arguments: &[&str],
) -> Run {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("TMPDIR", scratch.tmp());
    if let Some(tree) = tree {
        command.env("BENCH_VERIFY_REPO_ROOT", tree);
    }
    for (name, value) in extra {
        command.env(name, value);
    }
    Run::from(output_when_started(&mut command))
}

/// `ETXTBSY` (errno 26): a program this test has just written can still be open for writing in a process another test
/// thread forked a moment ago. Only the **start** is retried, a bounded number of times and only for this errno; the
/// status and output of a program that started are never retried.
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

fn assert_clean_pass(run: &Run, passed: usize, closing: &str, scratch: &Scratch) {
    assert_eq!(run.status, Some(0), "stderr: {}", run.stderr);
    assert_eq!(run.failed(), 0, "{}", run.stdout);
    assert_eq!(run.passed(), passed, "{}", run.stdout);
    assert!(
        run.stdout
            .lines()
            .last()
            .is_some_and(|line| line.starts_with(closing)),
        "{}",
        run.stdout
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn the_deployed_tree_passes_every_check_and_leaves_no_fixture() {
    let root = canonical_root();
    let scratch = Scratch::new("pass");
    let judge = run(JUDGE, Some(&root), &scratch, &[], &[]);
    assert_clean_pass(
        &judge,
        30,
        "[judge_bench --self-test] OK — J1~J12",
        &scratch,
    );
    assert_eq!(judge.stderr, "");
    let report = run(LITE_REPORT, Some(&root), &scratch, &[], &[]);
    assert_clean_pass(&report, 28, "[selftest_lite_report] PASS", &scratch);
    assert!(
        report.stdout.contains("[sweep_bench] sweep_index.json → "),
        "the producer's output is replayed"
    );
    let gate = run(LITE_GATE, Some(&root), &scratch, &[], &[]);
    assert_clean_pass(&gate, 25, "[selftest_lite_gate] PASS", &scratch);
    assert_eq!(gate.stderr, "");
    let meta = run(SWEEP_META, Some(&root), &scratch, &[], &[]);
    assert_eq!(meta.status, Some(0), "{}", meta.stderr);
    assert_eq!(
        meta.stdout
            .lines()
            .filter(|line| line.starts_with("[sweep_bench] sweep_index.json → "))
            .count(),
        23
    );
    assert!(
        meta.stdout
            .ends_with("(측정>선언 · 기본값≠미상 · 함정 3종 차단 · 주석≠선언 · 부재≠off)\n"),
        "{}",
        meta.stdout
    );
    assert!(
        meta.stderr
            .lines()
            .all(|line| line.starts_with("[sweep_bench] ⚠")),
        "{}",
        meta.stderr
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn unrelated_arguments_are_ignored_and_the_verbose_flag_adds_lines() {
    let root = canonical_root();
    let scratch = Scratch::new("arguments");
    let help = run(JUDGE, Some(&root), &scratch, &[], &["--help", "x"]);
    assert_clean_pass(&help, 30, "[judge_bench --self-test] OK", &scratch);
    let verbose = run(SWEEP_META, Some(&root), &scratch, &[], &["-v"]);
    assert_eq!(verbose.status, Some(0), "{}", verbose.stderr);
    assert!(verbose
        .stdout
        .contains("  ok   C1 yaml-only — 선언만 있으면 선언을 싣고 출처를 밝힌다\n"));
    assert!(verbose
        .stdout
        .contains("  ..   S1 JSON 선언(+행말 주석) → on · k=3 → on/3\n"));
    assert!(verbose.stdout.contains(
        "  ..   W1 ★포인터 → warm JSON 경로를 넘긴다(포인터 문서 자체 ✗) → ['--accept-len-src', '"
    ));
    assert!(verbose.stdout.contains("  ok   실 config RC1 승자 셀 MTP k=3(JSON 문자열 · 곁에 레버 주석 줄) · spec_declared=on\n"));
    let quiet = run(SWEEP_META, Some(&root), &scratch, &[], &["--verbose"]);
    assert!(
        !quiet.stdout.contains("  ok   C1"),
        "only -v asks for the lines"
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn an_unknown_context_is_named_before_any_fixture_and_git_is_named_first() {
    let scratch = Scratch::new("context");
    // The executable is copied outside any `rust/target/release` ancestor and no repository is given.
    for (program, name) in [
        (JUDGE, "selftest_judge_bench"),
        (LITE_REPORT, "selftest_lite_report"),
        (LITE_GATE, "selftest_lite_gate"),
        (SWEEP_META, "selftest_sweep_meta"),
    ] {
        let elsewhere = scratch.root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("directory");
        let copy = elsewhere.join(name);
        copy_detached(Path::new(program), &copy);
        let outcome = run(copy.to_str().expect("path"), None, &scratch, &[], &[]);
        assert_eq!(outcome.status, Some(2), "{name}");
        assert_eq!(outcome.stdout, "", "{name}");
        assert!(
            outcome
                .stderr
                .starts_with(&format!("[{name}] ERROR BENCH_VERIFY_REPO_ROOT")),
            "{}",
            outcome.stderr
        );
        scratch.assert_temporary_directory_removed();
    }
    // With no `git` the lite programs name that first, even with the repository unknown.
    for (program, named) in [
        (LITE_REPORT, "[selftest_lite_report] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)\n"),
        (LITE_GATE, "[selftest_lite_gate] FAIL git 이 없다 — 격리 저장소를 만들 수 없다\n"),
    ] {
        let mut command = Command::new(program);
        command.env_clear().env("PATH", "").env("TMPDIR", scratch.tmp());
        let outcome = Run::from(output_when_started(&mut command));
        assert_eq!((outcome.status, outcome.stdout.as_str(), outcome.stderr.as_str()), (Some(1), "", named));
    }
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_judge_whose_suppression_message_changed_fails_exactly_the_suppression_checks() {
    let scratch = Scratch::new("judge-mutation");
    let tree = private_tree(&scratch);
    mutate(
        &tree,
        &format!("{SCRIPTS}/judge_bench.sh"),
        "인증서 발행 억제",
        "발행 억제",
    );
    let outcome = run(JUDGE, Some(&tree), &scratch, &[], &[]);
    assert_eq!(outcome.status, Some(1), "{}", outcome.stdout);
    assert_eq!(
        (outcome.passed(), outcome.failed()),
        (28, 2),
        "{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stderr
            .starts_with("[judge_bench --self-test] FAIL 2 건: ['J6 --out-dir"),
        "{}",
        outcome.stderr
    );
    assert!(
        outcome.stderr.contains("'J6b --no-publish"),
        "{}",
        outcome.stderr
    );
    // The detail of a failed check follows its name after a single space (the judge's own style).
    assert!(outcome
        .stdout
        .lines()
        .any(|line| line.starts_with("  [FAIL] J6b --no-publish — 제자리 판정 · 발행 억제 (0, ")));
    scratch.assert_temporary_directory_removed();
}

/// The first executable named `name` on the inherited `PATH`.
fn program_on_path(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("NOT_RUN: no {name} on PATH"))
}

/// A test clock for the two `date` forms `lite_bench.sh` reads (`date -u +%FT%TZ` for the measurement time and
/// `date -u +%Y%m%dT%H%M%SZ` for the output directory); every other `date` call goes to the real one. The returned
/// `PATH` is the inherited one with the proxy directory in front, so the toolchain found there stays reachable. With
/// `advancing` the second measurement-time request answers one second later, which is what a real clock does when the
/// first two lite runs straddle a second boundary; otherwise every request answers the same instant.
fn date_proxy_path(scratch: &Scratch, advancing: bool) -> OsString {
    use std::os::unix::fs::PermissionsExt;
    let directory = scratch.root.join("clock");
    std::fs::create_dir_all(&directory).expect("proxy directory");
    let counter = scratch.root.join("clock-requests");
    let measured = if advancing {
        format!(
            "n=$(cat '{0}' 2>/dev/null || echo 0); n=$((n+1)); echo $n > '{0}'; if [ \"$n\" = 2 ]; then echo 2026-11-05T00:00:01Z; else echo 2026-11-05T00:00:00Z; fi",
            counter.display()
        )
    } else {
        "echo 2026-11-05T00:00:00Z".to_string()
    };
    let script = format!(
        "#!/bin/bash\ncase \"$*\" in\n  '-u +%FT%TZ') {measured} ;;\n  '-u +%Y%m%dT%H%M%SZ') echo 20261105T000000Z ;;\n  *) exec '{}' \"$@\" ;;\nesac\n",
        program_on_path("date").display()
    );
    let proxy = directory.join("date");
    std::fs::write(&proxy, script).expect("proxy script");
    std::fs::set_permissions(&proxy, std::fs::Permissions::from_mode(0o755)).expect("executable");
    let mut parts = vec![directory];
    parts.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(parts).expect("PATH")
}

/// `selftest_lite_report` on a private tree whose `lite_bench.sh` publishes by default, run under the test clock.
fn lite_default_publish(label: &str, advancing: bool) -> (Scratch, Run) {
    let scratch = Scratch::new(label);
    let tree = private_tree(&scratch);
    mutate(
        &tree,
        &format!("{SCRIPTS}/lite_bench.sh"),
        "PUBLISH_REPORT=0",
        "PUBLISH_REPORT=1",
    );
    let path = date_proxy_path(&scratch, advancing);
    let outcome = run(
        LITE_REPORT,
        Some(&tree),
        &scratch,
        &[("PATH", path.as_os_str())],
        &[],
    );
    (scratch, outcome)
}

#[test]
fn a_lite_script_that_publishes_by_default_fails_the_default_path_checks_and_the_run_continues() {
    // The exact count holds for one measurement instant: L1 and L2 name their reports by it, and two reports of the same
    // measurement are one report. The input clock is therefore stated here instead of read from the wall clock.
    let (scratch, outcome) = lite_default_publish("lite-default", false);
    assert_eq!(outcome.status, Some(1), "{}", outcome.stdout);
    assert!(
        outcome
            .stderr
            .starts_with("[selftest_lite_report] FAIL 3 건: ['L1 기본(플래그 없음)"),
        "{}",
        outcome.stderr
    );
    assert_eq!(outcome.failed(), 3);
    assert_eq!(outcome.passed(), 25, "the checks after a failure still run");
    // A failed check shows its name, an em dash and the tail of the detail's text.
    assert!(outcome.stdout.lines().any(|line| line.starts_with(
        "  [FAIL] L1 기본(플래그 없음) → exit 0 · 리포트 미발행 · 미발행을 이름으로 알린다 — (0, "
    )));
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_measurement_instant_that_advances_between_the_first_two_lite_runs_shows_the_second_report() {
    // The same mutated tree under a clock whose second measurement request is one second later: L1 and L2 no longer
    // describe one measurement, the second report exists, and ten checks fail instead of three. This is the documented
    // reason the control above fixes its clock; it must stay visible.
    let (scratch, outcome) = lite_default_publish("lite-advancing", true);
    assert_eq!(outcome.status, Some(1), "{}", outcome.stdout);
    assert!(
        outcome
            .stderr
            .starts_with("[selftest_lite_report] FAIL 10 건:"),
        "{}",
        outcome.stderr
    );
    assert_eq!(outcome.failed(), 10);
    assert_eq!(outcome.passed(), 18);
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_publisher_that_always_succeeds_fails_the_gate_negative_controls() {
    let scratch = Scratch::new("gate-publisher");
    let tree = private_tree(&scratch);
    std::fs::write(
        tree.join(SCRIPTS).join("publish_benchmark_record.sh"),
        "#!/bin/bash\nexit 0\n",
    )
    .expect("replace the publisher launcher");
    let outcome = run(LITE_GATE, Some(&tree), &scratch, &[], &[]);
    assert_eq!(outcome.status, Some(1), "{}", outcome.stdout);
    for name in ["K5★ 판정 부재 raw", "K5★ 어휘 밖 raw", "K5★ 불통과 raw"] {
        assert!(outcome.stderr.contains(name), "{name}: {}", outcome.stderr);
    }
    assert!(outcome.failed() >= 3, "{}", outcome.stdout);
    scratch.assert_temporary_directory_removed();
}

const PRODUCER_CALL: &str = "bash \"$SDIR/assemble_sweep_meta.sh\"";
const PRODUCER_RELEASE: &str =
    ".claude/skills/adversarial-benchmark/rust/target/release/assemble_sweep_meta";

/// What the meta check and the report check say when the producer binding fails: the meta check stops before any
/// check, the report check passes its first nineteen checks and stops where it loads the meta source (L8).
fn assert_both_stop_on_the_binding(tree: &Path, scratch: &Scratch, message: &str) {
    let meta = run(SWEEP_META, Some(tree), scratch, &[], &[]);
    assert_eq!(
        (meta.status, meta.stdout.as_str(), meta.stderr.as_str()),
        (Some(1), "", message)
    );
    let report = run(LITE_REPORT, Some(tree), scratch, &[], &[]);
    assert_eq!(report.status, Some(1));
    assert_eq!(
        report.stderr, message,
        "the report check reaches L8 and binds to the producer there"
    );
    assert_eq!(report.passed(), 19, "{}", report.stdout);
    assert!(report.stdout.ends_with("  [PASS] L7 lite_bench 는 플래그가 있을 때만 render_report --lite-only 를 부른다(기본 PUBLISH_REPORT=0)\n"));
}

#[test]
fn a_producer_the_script_no_longer_calls_stops_the_meta_check_before_any_check_and_the_report_check_after_its_first_nineteen_passes(
) {
    let scratch = Scratch::new("no-caller");
    let tree = private_tree(&scratch);
    mutate(
        &tree,
        &format!("{SCRIPTS}/sweep_bench.sh"),
        PRODUCER_CALL,
        "true",
    );
    assert_both_stop_on_the_binding(
        &tree,
        &scratch,
        &format!(
            "[selftest_sweep_meta] sweep_bench.sh 가 index 조립 생성부를 호출하지 않는다({PRODUCER_CALL} 부재) — 중단\n"
        ),
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn an_old_python_program_left_in_the_script_is_a_binding_failure_even_when_the_call_remains() {
    let scratch = Scratch::new("dead-program");
    let tree = private_tree(&scratch);
    append_to(
        &tree,
        &format!("{SCRIPTS}/sweep_bench.sh"),
        "\npython3 - <<'PY'\nimport json\njson.dump({}, open('sweep_index.json', 'w'))\nPY\n",
    );
    assert_both_stop_on_the_binding(
        &tree,
        &scratch,
        "[selftest_sweep_meta] sweep_bench.sh 에 index 를 조립하는 옛 python heredoc 이 남아 있다(호출 뒤의 죽은 생성부) — 중단\n",
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_missing_launcher_is_a_binding_failure() {
    let scratch = Scratch::new("no-launcher");
    let tree = private_tree(&scratch);
    std::fs::remove_file(tree.join(format!("{SCRIPTS}/assemble_sweep_meta.sh"))).expect("remove");
    assert_both_stop_on_the_binding(
        &tree,
        &scratch,
        "[selftest_sweep_meta] assemble_sweep_meta.sh 런처가 설치돼 있지 않다 — 중단\n",
    );
    scratch.assert_temporary_directory_removed();
}

#[test]
fn a_producer_program_that_is_missing_or_does_not_start_is_a_binding_failure_with_the_launchers_reason(
) {
    let scratch = Scratch::new("no-program");
    let tree = private_tree(&scratch);
    std::fs::remove_file(tree.join(PRODUCER_RELEASE)).expect("remove the program");
    let meta = run(SWEEP_META, Some(&tree), &scratch, &[], &[]);
    assert_eq!((meta.status, meta.stdout.as_str()), (Some(1), ""));
    assert!(
        meta.stderr
            .starts_with("[selftest_sweep_meta] native index 생성부가 기동하지 않는다(rc=2): "),
        "{}",
        meta.stderr
    );
    assert!(
        meta.stderr.contains("Rust 실행파일이 없다/실행 불가"),
        "{}",
        meta.stderr
    );
    // A program that exits quietly without the start-up answer is no producer either.
    let silent = scratch.root.join("silent_producer");
    std::fs::write(&silent, "#!/bin/sh\nexit 0\n").expect("write");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&silent, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }
    let overridden = run(
        SWEEP_META,
        Some(&tree),
        &scratch,
        &[("ASSEMBLE_SWEEP_META_BIN", silent.as_os_str())],
        &[],
    );
    assert_eq!(
        (overridden.status, overridden.stdout.as_str()),
        (Some(1), "")
    );
    assert!(
        overridden
            .stderr
            .starts_with("[selftest_sweep_meta] native index 생성부가 기동하지 않는다(rc=0): "),
        "{}",
        overridden.stderr
    );
    scratch.assert_temporary_directory_removed();
}

/// Appends source text to a file of a private tree (the way the declared owner mutations of the checks are made).
fn append_to(tree: &Path, relative: &str, addition: &str) {
    let path = tree.join(relative);
    let mut text = std::fs::read_to_string(&path).expect("read mutated file");
    text.push_str(addition);
    std::fs::write(&path, text).expect("write mutated file");
}

#[test]
fn the_lite_verifiers_need_no_owner_py_and_start_no_interpreter() {
    // plan_26111615 pass 3: the hint reader, the evidence publisher and the completion gate are Rust libraries. Without
    // the reader's `.py` and with every Python interpreter on `PATH` poisoned, both programs pass as on the deployed tree.
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("no-python");
    let tree = private_tree(&scratch);
    // absent already when the run has no `.py` at all
    let _ = std::fs::remove_file(tree.join(READER));
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
    let mut search = std::ffi::OsString::from(bin.as_os_str());
    search.push(":");
    search.push(std::env::var_os("PATH").unwrap_or_default());
    let report = run(LITE_REPORT, Some(&tree), &scratch, &[("PATH", search.as_os_str())], &[]);
    assert_clean_pass(&report, 28, "[selftest_lite_report] PASS", &scratch);
    let gate = run(LITE_GATE, Some(&tree), &scratch, &[("PATH", search.as_os_str())], &[]);
    assert_clean_pass(&gate, 25, "[selftest_lite_gate] PASS", &scratch);
    assert!(
        !marker.exists(),
        "an interpreter was started: {}",
        std::fs::read_to_string(&marker).unwrap_or_default()
    );
}

#[test]
fn copying_over_an_existing_name_never_writes_through_a_shared_inode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let scratch = Scratch::new("shared-inode");
    let directory = scratch.root.join("files");
    std::fs::create_dir_all(&directory).expect("directory");
    let source = directory.join("program");
    std::fs::write(&source, b"deployed program bytes").expect("source");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let alias = directory.join("alias");
    let second_alias = directory.join("second-alias");
    std::fs::hard_link(&source, &alias).expect("alias");
    std::fs::hard_link(&source, &second_alias).expect("second alias");
    let inode = std::fs::metadata(&source).expect("metadata").ino();

    // Over a name that is a hard link of the source, and again over the name that copy made.
    copy_detached(&source, &alias);
    copy_detached(&source, &alias);
    assert_eq!(std::fs::metadata(&source).expect("metadata").ino(), inode);
    assert_eq!(
        std::fs::read(&source).expect("read"),
        b"deployed program bytes"
    );
    assert_eq!(
        std::fs::metadata(&source).expect("metadata").nlink(),
        2,
        "the source and its other alias still share the inode; the copy left it"
    );
    // Over the source's own name: the name gets a new file, the inode the other alias holds is untouched.
    copy_detached(&alias, &source);
    assert_eq!(
        std::fs::metadata(&second_alias).expect("metadata").ino(),
        inode
    );
    assert_eq!(
        std::fs::read(&second_alias).expect("read"),
        b"deployed program bytes"
    );
    assert_ne!(std::fs::metadata(&source).expect("metadata").ino(), inode);
    for copy in [&alias, &source] {
        let metadata = std::fs::metadata(copy).expect("metadata");
        assert_ne!(
            metadata.ino(),
            inode,
            "{} was written as a new file",
            copy.display()
        );
        assert_eq!(
            std::fs::read(copy).expect("read"),
            b"deployed program bytes"
        );
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o755,
            "the permission bits are the source's"
        );
    }
}

