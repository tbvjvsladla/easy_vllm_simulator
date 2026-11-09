//! The two measured R49 v5 stream failures, as own checks derived from the SAVED accepted Original raw (never replayed):
//! R49C38 `--caps` with descriptor 1 CLOSED at process start (the original: `sys.stdout` is None, `json.dump` fails at `fp.write(chunk)` with
//! `AttributeError`, exit 1, 652 stderr bytes) and R49C16 `--self-test` under a layout without the schema on ONE merged pipe (the original flushes
//! the buffered `[PASS]` line BEFORE the unhandled-exception traceback). Golden = the saved bytes with the original layout root replaced by a token.
//! Also: the caller's actual `/dev/null` is NOT a closed descriptor, and the public Python -> Bash -> ELF route gives the same bytes.
//! Rust's runtime re-opens a closed descriptor 1 as `/dev/null`; the program therefore reads a load-time record of the descriptors the caller left
//! closed (`startup_descriptors`, a B-side boundary), and these checks exercise the REAL executable with real closed / `/dev/null` descriptors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const GOLDEN: &str = include_str!("data/turn_budget/stream_fixes.json");

fn elf() -> &'static str {
    env!("CARGO_BIN_EXE_turn_budget")
}

/// A minimal reader for the golden (the crate has no JSON dependency for checks: the golden is read through the program's own JSON reader).
mod serde_like {
    use agent_runtime_support::pyjson::loads_text;
    use agent_runtime_support::pystr::PyStr;
    use agent_runtime_support::pyvalue::PyValue;
    pub struct Value(PyValue);
    pub fn parse(text: &str) -> Value {
        Value(loads_text(&PyStr::new(text)).expect("golden JSON"))
    }
    impl Value {
        pub fn get(&self, key: &str) -> Value {
            match self.0.peel() {
                PyValue::Dict(map) => {
                    Value(map.get(key).unwrap_or_else(|| panic!("no {key}")).clone())
                }
                _ => panic!("not an object"),
            }
        }
        pub fn string(&self) -> String {
            match self.0.peel() {
                PyValue::Str(text) => text.to_utf8().expect("utf-8"),
                _ => panic!("not a string"),
            }
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tb-stream-fixes-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn repo_root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.."))
        .expect("repo root")
}

/// A layout like the original's: `<root>/.claude/skills/terraforming_node/scripts/turn_budget.py` as the lexical owner path, the real schema
/// present (LAY_REAL) or absent (LAY_NOSCHEMA).
fn layout(name: &str, with_schema: bool) -> (PathBuf, String) {
    let root = scratch(name);
    if with_schema {
        let target = root.join(".claude/schemas");
        std::fs::create_dir_all(&target).expect("schemas");
        std::fs::copy(
            repo_root().join(".claude/schemas/agent-control-request.schema.json"),
            target.join("agent-control-request.schema.json"),
        )
        .expect("schema copy");
    }
    let owner = root.join(".claude/skills/terraforming_node/scripts/turn_budget.py");
    let owner = owner.to_str().expect("utf-8").to_string();
    (root, owner)
}

/// The environment profile of the closed originals (explicit, nothing inherited).
fn profile(command: &mut Command) -> &mut Command {
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8")
        .env("LANG", "C.UTF-8")
        .env("TZ", "UTC")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("COLUMNS", "80")
}

/// `bash -c 'exec "$@" <redirections>' _ program args...`: the program starts with exactly those descriptors.
fn start(
    redirections: &str,
    cwd: &Path,
    owner_env: Option<&str>,
    extra_env: &[(&str, &str)],
    program: &[&str],
) -> Output {
    let mut command = Command::new("bash");
    profile(&mut command)
        .arg("-c")
        .arg(format!("exec \"$@\" {redirections}"))
        .arg("_")
        .args(program)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(owner) = owner_env {
        command.env("TURN_BUDGET_OWNER_PATH", owner);
    }
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command.output().expect("spawn")
}

fn expect_c38(root: &Path) -> Vec<u8> {
    let row = serde_like::parse(GOLDEN)
        .get("rows")
        .get("R49C38")
        .get("stderr_text")
        .string();
    row.replace("$LAYOUT", root.to_str().expect("utf-8"))
        .into_bytes()
}

#[test]
fn closed_stdout_gives_the_originals_attribute_error_traceback_and_exit_1() {
    let (root, owner) = layout("c38-direct", true);
    let output = start(">&-", &root, Some(&owner), &[], &[elf(), "--caps"]);
    let want = expect_c38(&root);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    // the saved original has 652 stderr bytes under ITS layout root; the byte count here differs only by the root path length
    assert_eq!(output.stderr.len(), want.len());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&want)
    );
    assert_eq!(output.stderr, want);
}

#[test]
fn the_callers_dev_null_is_not_a_closed_descriptor() {
    let (root, owner) = layout("c38-null", true);
    let output = start(">/dev/null", &root, Some(&owner), &[], &[elf(), "--caps"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // and an ordinary pipe still gets the document
    let piped = start("", &root, Some(&owner), &[], &[elf(), "--caps"]);
    assert_eq!(piped.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&piped.stdout).starts_with("{\n"),
        "{}",
        String::from_utf8_lossy(&piped.stdout)
    );
    assert!(piped.stderr.is_empty());
}

#[test]
fn a_closed_stdin_or_stderr_alone_does_not_make_stdout_absent() {
    let (root, owner) = layout("c38-others", true);
    for redirection in ["<&-", "2>&-"] {
        let output = start(redirection, &root, Some(&owner), &[], &[elf(), "--caps"]);
        assert_eq!(output.status.code(), Some(0), "{redirection}");
        assert!(
            String::from_utf8_lossy(&output.stdout).starts_with("{\n"),
            "{redirection}"
        );
    }
}

#[test]
fn closed_stdout_with_a_printing_only_command_is_silent_like_print_to_none() {
    // `--self-test` prints through `print`, which does nothing (and fails nothing) when `sys.stdout` is None
    let (root, owner) = layout("c38-selftest", true);
    let closed = start(">&-", &root, Some(&owner), &[], &[elf(), "--self-test"]);
    let null = start(
        ">/dev/null",
        &root,
        Some(&owner),
        &[],
        &[elf(), "--self-test"],
    );
    assert_eq!(closed.status.code(), null.status.code());
    assert_eq!(
        closed.stderr, null.stderr,
        "no traceback, nothing on stderr: print to None is silent"
    );
}

#[test]
fn self_test_failure_on_one_merged_pipe_flushes_stdout_before_the_traceback() {
    let (root, owner) = layout("c16", false);
    let output = start("2>&1", &root, Some(&owner), &[], &[elf(), "--self-test"]);
    let want = serde_like::parse(GOLDEN)
        .get("rows")
        .get("R49C16")
        .get("merged_text")
        .string()
        .replace("$LAYOUT", root.to_str().expect("utf-8"));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "stderr is the merged pipe");
    assert_eq!(
        output.stdout.len(),
        want.len(),
        "the saved merged capture has 1406 bytes"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), want);
    assert!(
        String::from_utf8_lossy(&output.stdout).starts_with("  [PASS]"),
        "the buffered PASS line comes first"
    );
}

#[test]
fn separate_pipes_keep_the_same_bytes_in_each_stream() {
    // with separate pipes the order is not observable; the streams carry the same two parts as the merged capture
    let (root, owner) = layout("c16-sep", false);
    let output = start("", &root, Some(&owner), &[], &[elf(), "--self-test"]);
    let merged = serde_like::parse(GOLDEN)
        .get("rows")
        .get("R49C16")
        .get("merged_text")
        .string()
        .replace("$LAYOUT", root.to_str().expect("utf-8"));
    let (out, err) = (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    );
    assert_eq!(format!("{out}{err}"), merged);
    assert!(out.starts_with("  [PASS]") && err.starts_with("Traceback"));
}

#[test]
fn the_public_python_bash_elf_route_gives_the_same_closed_stdout_traceback_and_merged_order() {
    let adapter = std::fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../terraforming_node/scripts/turn_budget.py"),
    )
    .expect("adapter");
    let root = repo_root();
    let adapter_text = adapter.to_str().expect("utf-8").to_string();
    // C38 through the public adapter
    let output = start(
        ">&-",
        &root,
        None,
        &[("TURN_BUDGET_BIN", elf())],
        &["python3", "-I", &adapter_text, "--caps"],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&expect_c38(&root))
    );
    // and the caller's /dev/null is still success through the same route
    let null = start(
        ">/dev/null",
        &root,
        None,
        &[("TURN_BUDGET_BIN", elf())],
        &["python3", "-I", &adapter_text, "--caps"],
    );
    assert_eq!(
        null.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&null.stderr)
    );
    assert!(null.stderr.is_empty());
}
