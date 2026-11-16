//! The native topology helper (`topology_parity`, the `role-contract` predicate behind the owner CLI) on a focused sample of
//! the saved observations of the original (`R43_TOPOLOGY_REFERENCE`: one directory per observed row with `execution.json`,
//! `stdout.bin`, `stderr.bin`; read-only, nothing here executes the original) plus helper-interface checks and the resolver's
//! routing (always the native helper: the Python owner and source-version compatibility routes are retired, plan_26111615
//! pass 3). The saved bytes are the oracle; the sample is named, it is not the whole matrix.

use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::PyValue;

const HELPER: &str = env!("CARGO_BIN_EXE_topology_parity");

/// Saved rows replayed by name (crashes, usage, help, passes, branch and layer edges).
const SAMPLE: &[&str] = &[
    "A03-pass-single-4legs",
    "A01-unknown-branch-no-legs",
    "A02-everything-red",
    "A07-layer-conflict-and-manifest-unreadable",
    "C19-control-variables-string",
    "C18-control-variables-not-dict",
    "C23-nodes-not-list-dict",
    "C24-nodes-entry-not-dict",
    "C27-active-invalid-utf8",
    "C28-active-perm-000",
    "M18-terraforming-list",
    "M19-terraforming-string",
    "X01-expect-single",
    "X03-expect-invalid-argparse-choices",
    "X04-format-value-pass",
    "X05-format-value-red",
    "X06-format-invalid",
    "X07-no-args",
    "X08-help",
    "X09-evaluate-help",
    "X10-unknown-option",
    "X11-repo-missing-value",
    "X12-repo-abbrev",
    "X13-unknown-subcommand",
    "X20-repo-non-utf8-argv",
    "H-ALL-LOCALE-STRICT",
    "G06-detached-head",
    "G14-no-git",
    "LY14-sort-order-mix",
    "C26-active-whitespace-crlf",
    "H-ALL",
    "H-ALL-FMTVALUE",
    "LY06-one-malformed",
    "LY10-nonascii-and-space-names",
    "LY11-newline-in-name",
    "LY13-case-variant",
    "M09-topology-types",
    "M14-top-level-list",
    "M16-top-level-falsy-scalar",
    "C09-declaration-not-json-yaml-syntax",
    "C11-declaration-invalid-utf8",
    "C32-active-absolute-path",
    "C34-active-nul-and-dot",
    "G09-branch-nonascii",
];

fn dict_get(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(dict) => dict
            .get(key)
            .cloned()
            .unwrap_or_else(|| panic!("missing key {key}")),
        other => panic!("not a dict: {other:?}"),
    }
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn strings(value: &PyValue) -> Vec<String> {
    match value {
        PyValue::List(items) => items
            .iter()
            .map(|i| match i {
                PyValue::Str(t) => t.to_utf8().expect("text"),
                o => panic!("{o:?}"),
            })
            .collect(),
        o => panic!("{o:?}"),
    }
}

/// Runs one saved row through the helper with the row's own cwd and environment; `None` when the row is not saved here.
/// What a replay returns: the helper's status/stdout/stderr and the saved row's.
struct Replay {
    status: i32,
    out: Vec<u8>,
    err: Vec<u8>,
    saved_status: i32,
    saved_out: Vec<u8>,
    saved_err: Vec<u8>,
}

fn replay(reference: &Path, id: &str) -> Option<Replay> {
    let dir = reference.join("cases").join(id);
    let execution = std::fs::read_to_string(dir.join("execution.json")).ok()?;
    let value = sweep_stop::pyjson::loads(&execution).expect("JSON");
    let argv: Vec<Vec<u8>> = strings(&dict_get(&value, "argv_fsencode_hex"))
        .iter()
        .map(|h| hex(h))
        .collect();
    // python3 -B OWNER evaluate ... -> helper --source-owner OWNER evaluate ...
    let owner = argv[2].clone();
    let rest: Vec<Vec<u8>> = argv[3..].to_vec();
    let cwd = match dict_get(&value, "cwd") {
        PyValue::Str(t) => t.to_utf8().expect("cwd"),
        o => panic!("{o:?}"),
    };
    let mut command = Command::new(HELPER);
    command
        .arg("--source-owner")
        .arg(std::ffi::OsStr::from_bytes(&owner));
    for arg in &rest {
        command.arg(std::ffi::OsStr::from_bytes(arg));
    }
    command.env_clear().current_dir(&cwd).stdin(Stdio::null());
    if let PyValue::Dict(env) = dict_get(&value, "environment") {
        for (name, v) in env.iter() {
            if let PyValue::Str(text) = v {
                command.env(
                    name.to_utf8().expect("name"),
                    text.to_utf8().expect("value"),
                );
            }
        }
    }
    let out = command.output().expect("helper");
    let saved_status = match dict_get(&value, "exit_code") {
        PyValue::Int(n) => n.to_string().parse::<i32>().expect("status"),
        o => panic!("{o:?}"),
    };
    Some(Replay {
        status: out.status.code().expect("exit code"),
        out: out.stdout,
        err: out.stderr,
        saved_status,
        saved_out: std::fs::read(dir.join("stdout.bin")).expect("stdout.bin"),
        saved_err: std::fs::read(dir.join("stderr.bin")).expect("stderr.bin"),
    })
}

#[test]
fn the_helper_reproduces_the_named_saved_observations() {
    let Some(reference) = std::env::var_os("R43_TOPOLOGY_REFERENCE") else {
        eprintln!("SKIPPED: R43_TOPOLOGY_REFERENCE is not set");
        return;
    };
    let reference = PathBuf::from(reference);
    let mut compared = 0;
    let mut differ = Vec::new();
    for id in SAMPLE.iter().copied() {
        let Some(Replay {
            status,
            out,
            err,
            saved_status,
            saved_out,
            saved_err,
        }) = replay(&reference, id)
        else {
            differ.push(format!("{id}: not saved"));
            continue;
        };
        compared += 1;
        if status != saved_status || out != saved_out || err != saved_err {
            differ.push(format!(
                "{id}: status {status} (saved {saved_status}); stdout {}/{} bytes equal={}; stderr {}/{} bytes equal={}\n--- got stderr:\n{}\n--- saved stderr:\n{}",
                out.len(), saved_out.len(), out == saved_out, err.len(), saved_err.len(), err == saved_err,
                String::from_utf8_lossy(&err[..err.len().min(900)]), String::from_utf8_lossy(&saved_err[..saved_err.len().min(900)])
            ));
        }
    }
    eprintln!("compared {compared} saved rows, {} differ", differ.len());
    assert!(differ.is_empty(), "{}", differ.join("\n=====\n"));
}

#[test]
fn the_helper_runs_its_own_self_test_in_a_private_temporary_directory() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("topology_selftest_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("tmp");
    let out = Command::new(HELPER)
        .arg("--self-test")
        .env_clear()
        .env("LC_ALL", "C.UTF-8")
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", &tmp)
        .output()
        .expect("helper");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "[topology_parity] self-test PASS (6 헤더 케이스 + 열거 1)\n"
    );
    // the throwaway directories are gone again
    assert_eq!(std::fs::read_dir(&tmp).expect("tmp").count(), 0);
    let _ = std::fs::remove_dir_all(&tmp);
}

// ---- output failure, process group; W-owned literal fixtures only ------------------------------------------------------

fn fixture_checkout(label: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("topology_routes")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("git")
            .status
            .success());
    };
    git(&["init", "-q"]);
    git(&["symbolic-ref", "HEAD", "refs/heads/single-node"]);
    for (rel, bytes) in [
        (
            ".claude/rules/strategy.topology.md",
            "**topology: single** layer\n".as_bytes().to_vec(),
        ),
        (
            "output/single/manifest.yaml",
            b"topology: single\n".to_vec(),
        ),
    ] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, bytes).expect("write");
    }
    git(&["add", "-A"]);
    root
}

fn helper(args: &[&str], env: &[(&str, &str)], stdout: Stdio) -> std::process::Output {
    let mut command = Command::new(HELPER);
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("helper")
}

#[test]
fn a_failed_delivery_of_the_report_is_status_120_not_the_business_status() {
    let root = fixture_checkout("full");
    let Ok(full) = std::fs::OpenOptions::new().write(true).open("/dev/full") else {
        eprintln!("SKIPPED: /dev/full is not available");
        return;
    };
    let out = helper(
        &[
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
            "--format",
            "json",
        ],
        &[],
        Stdio::from(full),
    );
    assert_eq!(
        out.status.code(),
        Some(120),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_text_profile_decides_the_report_encoding_and_an_unsupported_one_is_named() {
    let root = fixture_checkout("profiles");
    let repo = root.to_str().expect("UTF-8");
    // an unset locale (UTF-8 mode on): the Korean reasons are written
    let default = helper(
        &[
            "evaluate", "--repo", repo, "--format", "json", "--expect", "multi",
        ],
        &[("LC_ALL", "")],
        Stdio::piped(),
    );
    assert!(String::from_utf8_lossy(&default.stdout).contains("EXPECT_MISMATCH"));
    // the strict-C profile: the same report cannot be encoded: the original's UnicodeEncodeError, exit 1
    let strict = helper(
        &[
            "evaluate", "--repo", repo, "--format", "json", "--expect", "multi",
        ],
        &[
            ("LC_ALL", "C"),
            ("PYTHONUTF8", "0"),
            ("PYTHONCOERCECLOCALE", "0"),
        ],
        Stdio::piped(),
    );
    assert_eq!(strict.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&strict.stderr)
        .contains("UnicodeEncodeError: 'ascii' codec can't encode character"));
    // a charset that is not modelled: a concrete private error
    let odd = helper(
        &["evaluate", "--repo", repo],
        &[("LC_ALL", "de_DE.ISO-8859-1")],
        Stdio::piped(),
    );
    assert_eq!(odd.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&odd.stderr).contains("unsupported text profile"));
}

#[test]
fn the_group_kill_reaches_a_nested_child_and_the_plain_kill_does_not() {
    use std::ffi::OsString;
    use std::time::Duration;
    use sweep_stop::captured_process::{run_captured_in, ChildSettings};
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("topology_group")
        .join(format!("{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let alive = |pid: &str| {
        std::path::Path::new(&format!("/proc/{pid}"))
            .join("stat")
            .exists()
            && !std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .unwrap_or_default()
                .contains(") Z ")
    };
    for (group, label) in [(true, "group"), (false, "plain")] {
        let pidfile = dir.join(format!("{label}.pid"));
        let script = format!("sleep 30 & echo $! > {}; wait", pidfile.display());
        let words: Vec<OsString> = vec!["/bin/sh".into(), "-c".into(), script.into()];
        let settings = ChildSettings {
            own_process_group: group,
            ..ChildSettings::default()
        };
        let Err(failure) =
            run_captured_in(&words, &[], Duration::from_millis(800), "0.8", &settings)
        else {
            panic!("the deadline must end the call")
        };
        assert_eq!(failure.class, "TimeoutExpired");
        let pid = std::fs::read_to_string(&pidfile)
            .expect("pid")
            .trim()
            .to_string();
        std::thread::sleep(Duration::from_millis(300));
        if group {
            assert!(
                !alive(&pid),
                "the nested child must be gone after the group kill"
            );
        } else {
            assert!(
                alive(&pid),
                "negative control: without a group the nested child survives the plain kill"
            );
            let _ = Command::new("kill").arg("-9").arg(&pid).status();
        }
    }
}

#[test]
fn an_invalid_pythonutf8_value_fails_the_startup_profile() {
    let root = fixture_checkout("utf8");
    let out = helper(
        &["evaluate", "--repo", root.to_str().expect("UTF-8")],
        &[("PYTHONUTF8", "2")],
        Stdio::piped(),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("PYTHONUTF8"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- plan_26111615 pass 3: the native helper is the only judge -------------------------------------------------------------

/// A Python stand-in owner that would prove it ran (a RED verdict with a marker) if anything started it.
const MARKER_OWNER: &str = "import json,sys\nprint(json.dumps({'verdict':'RED','topology':None,'reasons':['PYTHON-OWNER-RAN']}))\nsys.exit(5)\n";

#[test]
fn the_resolver_runs_only_the_native_helper_and_names_a_request_for_the_retired_interpreter_route() {
    use sweep_stop::resolve_topology::{resolve, ResolveContext, CONFIGURATION_ERROR};
    let root = fixture_checkout("route_native_only");
    let owner_dir = root.join(".claude/policies/runtime");
    std::fs::create_dir_all(&owner_dir).expect("owner dir");
    let repo = PyValue::str(root.to_str().expect("UTF-8"));
    let native = ResolveContext::native().with_native_helper(PathBuf::from(HELPER));
    // no owner file at all: the native helper still judges (the reviewed fixture checkout passes as single)
    let without = resolve(&repo, &PyValue::None, &native).expect("resolve");
    assert_eq!(without.code, 0, "{}", without.why.to_utf8().unwrap_or_default());
    assert!(matches!(&without.topology, PyValue::Str(t) if t.equals("single")));
    // a stand-in owner in the checkout changes nothing: it is never run
    std::fs::write(owner_dir.join("topology_parity.py"), MARKER_OWNER).expect("owner");
    for falsy in [PyValue::None, PyValue::str(""), PyValue::int(0), PyValue::Bool(false)] {
        let judged = resolve(&repo, &falsy, &native).expect("resolve");
        assert_eq!(judged.code, 0);
        assert!(!judged.why.to_utf8().unwrap_or_default().contains("PYTHON-OWNER-RAN"));
    }
    // a truthy interpreter argument asks for the retired route: a named configuration error, nothing runs
    let refused = resolve(&repo, &PyValue::str("python3"), &native).expect_err("named configuration error");
    assert_eq!(refused.class, CONFIGURATION_ERROR);
    assert!(refused.message.to_utf8().unwrap_or_default().contains("retired"));
    // a missing helper is a named configuration error, not a Python fallback
    let missing = ResolveContext::native().with_native_helper(root.join("no_such_helper"));
    let failure = resolve(&repo, &PyValue::None, &missing).expect_err("named configuration error");
    assert_eq!(failure.class, CONFIGURATION_ERROR);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_helper_answers_natively_whatever_the_owner_file_holds_and_refuses_the_retired_flags() {
    let root = fixture_checkout("helper_native_only");
    let repo = root.to_str().expect("UTF-8");
    let reference = helper(&["evaluate", "--repo", repo, "--format", "json"], &[], Stdio::piped());
    assert_eq!(reference.status.code(), Some(0), "{}", String::from_utf8_lossy(&reference.stderr));
    // a changed owner program named as the logical source changes neither the route nor the answer
    let owner = root.join("changed_owner.py");
    std::fs::write(&owner, MARKER_OWNER).expect("owner");
    let sidecar = root.join("route.json");
    let changed = helper(
        &[
            "--source-owner",
            owner.to_str().expect("UTF-8"),
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "evaluate",
            "--repo",
            repo,
            "--format",
            "json",
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(changed.status.code(), Some(0));
    assert_eq!(changed.stdout, reference.stdout);
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(record.contains("\"route\": \"NativeCurrentSource\"") && record.contains("\"python_children\": 0"), "{record}");
    // the flags of the retired compatibility route are refused by name (status 2), never ignored
    for flag in ["--compat-owner", "--compat-python", "--compat-deadline-ms"] {
        let refused = helper(&[flag, "x", "evaluate", "--repo", repo], &[], Stdio::piped());
        assert_eq!(refused.status.code(), Some(2), "{flag}");
        assert!(refused.stdout.is_empty());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("retired"), "{flag}");
    }
}

#[test]
fn n10_the_canonical_native_route_starts_no_python_and_no_supervision_child() {
    // the helper answers natively; its sidecar names no watchdog and no Python child
    let root = fixture_checkout("n10_native");
    let sidecar = root.join("native.json");
    let out = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(
        record.contains("\"route\": \"NativeCurrentSource\"")
            && record.contains("\"python_children\": 0")
            && !record.contains("watchdog_pid"),
        "{record} / status {:?}",
        out.status.code()
    );
}
