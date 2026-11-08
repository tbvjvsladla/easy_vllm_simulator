//! The native topology helper (`topology_parity`, the `role-contract` predicate behind the owner CLI) on a focused sample of
//! the saved observations of the original (`R43_TOPOLOGY_REFERENCE`: one directory per observed row with `execution.json`,
//! `stdout.bin`, `stderr.bin`; read-only, nothing here executes the original) plus helper-interface checks and the resolver's
//! default-versus-explicit routing. The saved bytes are the oracle; the sample is named, it is not the whole matrix.

use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pystr::PyStr;
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

#[test]
fn the_resolver_runs_the_native_helper_by_default_and_the_python_owner_only_when_asked() {
    use sweep_stop::resolve_topology::{resolve, ResolveContext, CONFIGURATION_ERROR};
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("topology_route_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner_dir = root.join(".claude/policies/runtime");
    std::fs::create_dir_all(&owner_dir).expect("owner dir");
    std::fs::create_dir_all(root.join("output/single")).expect("manifest dir");
    std::fs::write(
        root.join("output/single/manifest.yaml"),
        "topology: single\n",
    )
    .expect("manifest");
    // a stand-in Python owner that proves the explicit route ran it (it answers a RED verdict with a marker)
    std::fs::write(owner_dir.join("topology_parity.py"), "import json,sys\nprint(json.dumps({'verdict':'RED','topology':None,'reasons':['EXPLICIT-ROUTE-RAN']}))\nsys.exit(5)\n").expect("owner");
    let repo = PyValue::str(root.to_str().expect("UTF-8"));
    let reasons_of = |why: &PyStr| why.to_utf8().expect("text");
    // explicit interpreter argument (python3 named explicitly is an explicit value) -> the stand-in ran
    let native = ResolveContext::native().with_native_helper(PathBuf::from(HELPER));
    let explicit = resolve(&repo, &PyValue::str("python3"), &native).expect("resolve");
    assert!(
        reasons_of(&explicit.why).contains("EXPLICIT-ROUTE-RAN"),
        "{}",
        reasons_of(&explicit.why)
    );
    // falsy argument + native context: the DEFAULT is the native helper under the project policy: the stand-in is not the reviewed
    // owner, so the source-version compatibility route runs the checkout's own original program under the configured python3
    for falsy in [PyValue::None, PyValue::str("")] {
        let by_default = resolve(&repo, &falsy, &native).expect("resolve");
        assert!(
            reasons_of(&by_default.why).contains("EXPLICIT-ROUTE-RAN"),
            "{}",
            reasons_of(&by_default.why)
        );
    }
    // the private strict-native mode (an explicit choice): a different source is a concrete unmet condition, never a current-source answer
    let strict = ResolveContext::native()
        .with_native_helper(PathBuf::from(HELPER))
        .with_strict_native();
    let unmet = resolve(&repo, &PyValue::None, &strict).expect("resolve");
    assert!(
        !reasons_of(&unmet.why).contains("EXPLICIT-ROUTE-RAN"),
        "{}",
        reasons_of(&unmet.why)
    );
    assert!(
        reasons_of(&unmet.why).contains("source version compatibility is required"),
        "{}",
        reasons_of(&unmet.why)
    );
    // an explicit context interpreter keeps its meaning (the stand-in runs)
    let legacy = ResolveContext::new(PyStr::new("python3"));
    let via_context = resolve(&repo, &PyValue::None, &legacy).expect("resolve");
    assert!(reasons_of(&via_context.why).contains("EXPLICIT-ROUTE-RAN"));
    // a missing helper is a named configuration error, not a Python fallback
    let missing = ResolveContext::native().with_native_helper(root.join("no_such_helper"));
    let failure = resolve(&repo, &PyValue::None, &missing).expect_err("named configuration error");
    assert_eq!(failure.class, CONFIGURATION_ERROR);
    let _ = std::fs::remove_dir_all(&root);
}

// ---- source-version routes, output failure, process group (H2 repair); W-owned literal fixtures only -------------------

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
fn a_reviewed_source_runs_native_and_a_different_one_runs_the_original_program_on_the_compat_route()
{
    let root = fixture_checkout("routes");
    let module = root.join(".claude/skills/terraforming_node/scripts/campaign_init.py");
    std::fs::create_dir_all(module.parent().expect("parent")).expect("dir");
    // the reviewed canonical campaign_init.py (the exact bytes the build compiled in): native route, no Python child
    std::fs::write(&module, role_contract::source_identity::REVIEWED_CAMPAIGN)
        .expect("reviewed copy");
    let sidecar = root.join("route-native.json");
    let native = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
            "--format",
            "value",
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(
        native.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(
        record.contains("\"route\": \"NativeCurrentSource\"")
            && record.contains("\"python_children\": 0"),
        "{record}"
    );
    // a different program: a stand-in ORIGINAL owner that prints a marker and exits 7 (the route runs it, never the native answer)
    std::fs::write(&module, b"raise SystemExit(7)\n").expect("different module");
    let owner = root.join("original_owner.py");
    std::fs::write(&owner, "import sys\nprint('ORIGINAL-RAN', sys.argv[1:])\nsys.stderr.write('stderr-line\\n')\nsys.exit(7)\n").expect("owner");
    let sidecar = root.join("route-legacy.json");
    let legacy = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(legacy.status.code(), Some(7));
    assert_eq!(
        String::from_utf8_lossy(&legacy.stdout),
        format!(
            "ORIGINAL-RAN ['evaluate', '--repo', '{}']\n",
            root.display()
        )
    );
    assert_eq!(
        String::from_utf8_lossy(&legacy.stderr),
        "stderr-line\n",
        "no route/warning bytes in the business stderr"
    );
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(
        record.contains("\"route\": \"LegacySourceVariant\"")
            && record.contains("\"python_children\": 1")
            && record.contains("\"child_pid\": "),
        "{record}"
    );
    assert!(record.contains("\"child_status\": 7"), "{record}");
    // without an explicit interpreter the different source is a concrete unmet condition, never the current-source answer
    let unmet = helper(
        &[
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(unmet.status.code(), Some(2));
    assert!(unmet.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&unmet.stderr).contains("source version compatibility is required"),
        "{}",
        String::from_utf8_lossy(&unmet.stderr)
    );
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
fn the_real_project_default_runs_a_changed_campaign_source_on_the_compat_route_without_any_new_environment_setting(
) {
    use sweep_stop::resolve_topology::{resolve, ResolveContext, COMPAT_PYTHON_ENV, PYTHON_ENV};
    if std::env::var_os(PYTHON_ENV).is_some_and(|v| !v.is_empty())
        || std::env::var_os(COMPAT_PYTHON_ENV).is_some_and(|v| !v.is_empty())
    {
        eprintln!(
            "SKIPPED: the check needs both interpreter variables unset (the project default)"
        );
        return;
    }
    if !Command::new("python3")
        .args(["-c", "import yaml"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        eprintln!("SKIPPED: python3 with PyYAML is not available for the compatibility route");
        return;
    }
    let root = fixture_checkout("default_compat");
    let owner = root.join(".claude/policies/runtime/topology_parity.py");
    std::fs::create_dir_all(owner.parent().expect("parent")).expect("dir");
    // the reviewed canonical owner (byte-identical) and a campaign module that is a DIFFERENT program (empty: no functions)
    std::fs::write(&owner, role_contract::source_identity::REVIEWED_OWNER).expect("owner");
    let module = root.join(".claude/skills/terraforming_node/scripts/campaign_init.py");
    std::fs::create_dir_all(module.parent().expect("parent")).expect("dir");
    std::fs::write(&module, b"").expect("empty module");
    // the project DEFAULT: from_environment() with no new configuration; the helper is only the sibling-binary seam of the check
    let context = ResolveContext::from_environment().with_native_helper(PathBuf::from(HELPER));
    assert!(context.is_native());
    let outcome = resolve(
        &PyValue::str(root.to_str().expect("UTF-8")),
        &PyValue::None,
        &context,
    )
    .expect("resolve");
    let why = outcome.why.to_utf8().expect("text");
    // the ORIGINAL program ran (its own AttributeError for the empty module), not a native current-source answer
    assert_eq!(outcome.code, 5, "{why}");
    assert!(
        why.contains("has no attribute 'active_campaign_id'"),
        "{why}"
    );
    // the same checkout with the REVIEWED campaign module runs natively (no Python): a plain PASS
    std::fs::write(&module, role_contract::source_identity::REVIEWED_CAMPAIGN)
        .expect("reviewed module");
    let native = resolve(
        &PyValue::str(root.to_str().expect("UTF-8")),
        &PyValue::None,
        &context,
    )
    .expect("resolve");
    assert_eq!(
        (native.code, native.topology),
        (0, PyValue::str("single")),
        "{}",
        native.why.to_utf8().expect("text")
    );
}

// ---- H3: logical owner, bounded compat child, output failure kinds, signal propagation, invalid profile ---------------

fn changed_checkout(label: &str) -> PathBuf {
    let root = fixture_checkout(label);
    let module = root.join(".claude/skills/terraforming_node/scripts/campaign_init.py");
    std::fs::create_dir_all(module.parent().expect("parent")).expect("dir");
    std::fs::write(&module, b"# a different program\n").expect("different module");
    root
}

fn alive(pid: &str) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map(|stat| !stat.contains(") Z "))
        .unwrap_or(false)
}

#[test]
fn the_logical_source_owner_is_never_run_and_a_missing_real_owner_is_unmet() {
    let root = changed_checkout("n5");
    let marker = root.join("logical-ran");
    let logical = root.join("logical_owner.py");
    std::fs::write(
        &logical,
        format!(
            "open({:?}, 'w').write('ran')\n",
            marker.to_str().expect("UTF-8")
        ),
    )
    .expect("logical owner");
    let repo = root.to_str().expect("UTF-8");
    // the logical metadata path alone never stands in for the real program
    let out = helper(
        &[
            "--source-owner",
            logical.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "evaluate",
            "--repo",
            repo,
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("source version compatibility is required"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!marker.exists(), "the logical owner must not have run");
    // a real owner that is named but does not exist: not native, not run, a concrete unmet condition (sidecar on request)
    let sidecar = root.join("unmet.json");
    let missing = root.join("no_such_owner.py");
    let out = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "--compat-owner",
            missing.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "evaluate",
            "--repo",
            repo,
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot be served"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(std::fs::read_to_string(&sidecar)
        .expect("sidecar")
        .contains("UnmetCompatibility"));
}

#[test]
fn a_hanging_compat_child_is_killed_with_its_descendants_at_the_private_limit() {
    let root = changed_checkout("n6");
    let pidfile = root.join("nested.pid");
    let owner = root.join("hanging_owner.py");
    std::fs::write(&owner, format!("import subprocess, time\np = subprocess.Popen(['sleep', '60'])\nopen({:?}, 'w').write(str(p.pid))\ntime.sleep(60)\n", pidfile.to_str().expect("UTF-8"))).expect("owner");
    let sidecar = root.join("timeout.json");
    let started = std::time::Instant::now();
    let out = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "--compat-deadline-ms",
            "800",
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "the private limit ended the call"
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "no business result");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("did not complete"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(
        record.contains("\"control_failure\": \"TimeoutExpired\"")
            && record.contains("\"complete\": false")
            && !record.contains("\"child_pid\": null"),
        "{record}"
    );
    let nested = std::fs::read_to_string(&pidfile).expect("nested pid");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        !alive(nested.trim()),
        "the descendant of the compat child must be gone"
    );
}

#[test]
fn an_immediate_stdout_write_failure_is_the_traceback_and_status_1() {
    let root = changed_checkout("n7");
    let owner = root.join("big_owner.py");
    std::fs::write(&owner, "import sys\nsys.stdout.write('x' * 20000)\n").expect("owner");
    let Ok(full) = std::fs::OpenOptions::new().write(true).open("/dev/full") else {
        eprintln!("SKIPPED: /dev/full is not available");
        return;
    };
    let out = helper(
        &[
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::from(full),
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Traceback (most recent call last):"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_compat_child_killed_by_a_signal_is_propagated_and_recorded() {
    use std::os::unix::process::ExitStatusExt;
    let root = changed_checkout("p3a");
    let owner = root.join("signal_owner.py");
    std::fs::write(
        &owner,
        "import os, signal\nprint('before', flush=True)\nos.kill(os.getpid(), signal.SIGTERM)\n",
    )
    .expect("owner");
    let sidecar = root.join("signal.json");
    let out = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(
        out.status.signal(),
        Some(15),
        "the caller sees the child's signal as Popen's -15: {:?}",
        out.status
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "before\n",
        "the output before the signal is delivered"
    );
    let record = std::fs::read_to_string(&sidecar).expect("sidecar");
    assert!(
        record.contains("\"child_signal\": 15") && record.contains("\"child_status\": -15"),
        "{record}"
    );
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

// ---- H4 / N10: nested cancellation, resolver -> helper -> compat Python -> descendants (W-owned disposables only) --------
//
// The ACTUAL resolver (`resolve`) runs the ACTUAL built helper, which runs a configured stand-in owner (the checkout's own,
// non-reviewed `topology_parity.py`) under `python3`; the stand-in starts a nested `sleep` and records its PIDs/groups.
// Limits are injected through the check-only seam so no case waits 60 s.

struct Nested {
    pythonpid: i32,
    helper_pid: i32,
    group: i32,
    sleeper_pid: i32,
    sleeper_group: i32,
}

fn nested_checkout(label: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("n10_nested")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner_dir = root.join(".claude/policies/runtime");
    std::fs::create_dir_all(&owner_dir).expect("owner dir");
    std::fs::create_dir_all(root.join("output/single")).expect("manifest dir");
    std::fs::write(
        root.join("output/single/manifest.yaml"),
        "topology: single\n",
    )
    .expect("manifest");
    let record = root.join("nested.json");
    let script = "import json, os, subprocess, time\n\
        p = subprocess.Popen(['sleep', '120'])\n\
        rec = {'python_pid': os.getpid(), 'helper_pid': os.getppid(), 'group': os.getpgrp(), 'sleeper_pid': p.pid, 'sleeper_group': os.getpgid(p.pid)}\n\
        open('@REC@.tmp', 'w').write(json.dumps(rec))\n\
        os.rename('@REC@.tmp', '@REC@')\n\
        time.sleep(120)\n"
        .replace("@REC@", record.to_str().expect("UTF-8"));
    std::fs::write(owner_dir.join("topology_parity.py"), script).expect("stand-in owner");
    (root, record)
}

fn read_nested(record: &Path) -> Nested {
    let text = std::fs::read_to_string(record).expect("the stand-in recorded its processes");
    let number = |key: &str| -> i32 {
        let at = text.find(&format!("\"{key}\": ")).expect(key) + key.len() + 4;
        text[at..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .expect("number")
    };
    Nested {
        pythonpid: number("python_pid"),
        helper_pid: number("helper_pid"),
        group: number("group"),
        sleeper_pid: number("sleeper_pid"),
        sleeper_group: number("sleeper_group"),
    }
}

/// Live (non-zombie) members of a process group.
fn group_members(group: i32) -> Vec<i32> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").expect("/proc").flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(tail) = stat.rsplit_once(')').map(|(_, rest)| rest.trim_start()) else {
            continue;
        };
        let mut fields = tail.split_whitespace();
        let (state, _ppid, pgrp) = (fields.next(), fields.next(), fields.next());
        if state != Some("Z") && pgrp.and_then(|g| g.parse::<i32>().ok()) == Some(group) {
            found.push(pid);
        }
    }
    found
}

fn nothing_of_it_is_alive(n: &Nested) -> bool {
    [n.pythonpid, n.helper_pid, n.sleeper_pid, n.group]
        .iter()
        .all(|pid| !alive(&pid.to_string()))
        && group_members(n.group).is_empty()
}

/// Waits (bounded) until everything the stand-in started is gone; returns the observed wait.
fn wait_cleaned(n: &Nested, within: std::time::Duration) -> Option<std::time::Duration> {
    let start = std::time::Instant::now();
    while start.elapsed() < within {
        if nothing_of_it_is_alive(n) {
            return Some(start.elapsed());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    nothing_of_it_is_alive(n).then(|| start.elapsed())
}

fn write_record(label: &str, root: &Path, body: String) {
    let _ = std::fs::write(root.join(format!("{label}.record.json")), body);
}

/// The resolver, the helper and the stand-in all ran for real and the resolver never fabricated a business answer.
fn assert_control_incomplete(res: &sweep_stop::resolve_topology::Resolution) {
    assert_eq!(res.code, 5, "unresolved, never a business verdict");
    assert!(
        !res.topology.is_truthy(),
        "no topology may be claimed from an incomplete control"
    );
}

#[test]
fn n10_the_inner_deadline_below_the_outer_cleans_the_whole_compat_subtree_before_the_resolver_acts()
{
    use sweep_stop::resolve_topology::{resolve, ResolveContext};
    let (root, record) = nested_checkout("inner");
    let repo = PyValue::str(root.to_str().expect("UTF-8"));
    // outer 20 s, inner 1 s: the helper's own cleanup runs, the resolver never has to kill the helper
    let context = ResolveContext::native()
        .with_native_helper(PathBuf::from(HELPER))
        .with_limits_for_check(20_000, 1_000);
    let started = std::time::Instant::now();
    let res = resolve(&repo, &PyValue::None, &context).expect("resolve");
    let elapsed = started.elapsed();
    let n = read_nested(&record);
    let cleaned = wait_cleaned(&n, std::time::Duration::from_secs(3));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let followup_alive = !nothing_of_it_is_alive(&n);
    write_record(
        "inner",
        &root,
        format!(
            "{{\"case\": \"inner_before_outer\", \"python_pid\": {}, \"helper_pid\": {}, \"group\": {}, \"sleeper_pid\": {}, \"sleeper_group\": {}, \"elapsed_ms\": {}, \"resolution_code\": {}, \"cleaned_after_ms\": {:?}, \"followup_alive\": {}}}\n",
            n.pythonpid, n.helper_pid, n.group, n.sleeper_pid, n.sleeper_group, elapsed.as_millis(), res.code, cleaned.map(|d| d.as_millis()), followup_alive
        ),
    );
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "the inner deadline (1 s) ended the call long before the 20 s outer limit: {elapsed:?}"
    );
    assert_control_incomplete(&res);
    let why = res.why.to_utf8().expect("text");
    assert!(
        why.contains("rc=2") && why.contains("timed out after 1 seconds"),
        "the helper's own control failure (status 2), not the resolver's outer timeout: {why}"
    );
    assert_ne!(
        n.group, n.helper_pid,
        "the compat group is the watchdog's, not the helper's"
    );
    assert_eq!(
        n.sleeper_group, n.group,
        "the nested sleeper is in the compat group"
    );
    assert!(
        cleaned.is_some(),
        "python, helper, watchdog and the nested sleeper are gone"
    );
    assert!(!followup_alive, "and they stay gone");
}

#[test]
fn n10_a_short_outer_limit_that_kills_the_helper_still_cleans_the_compat_descendants() {
    use sweep_stop::resolve_topology::{resolve, ResolveContext};
    let (root, record) = nested_checkout("outer");
    let repo = PyValue::str(root.to_str().expect("UTF-8"));
    // outer 1.5 s, inner 40 s: only the resolver's group kill of the HELPER fires; the watchdog must do the rest
    let context = ResolveContext::native()
        .with_native_helper(PathBuf::from(HELPER))
        .with_limits_for_check(1_500, 40_000);
    let started = std::time::Instant::now();
    let res = resolve(&repo, &PyValue::None, &context).expect("resolve");
    let elapsed = started.elapsed();
    let n = read_nested(&record);
    let alive_at_return = !nothing_of_it_is_alive(&n);
    let cleaned = wait_cleaned(&n, std::time::Duration::from_secs(5));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let followup_alive = !nothing_of_it_is_alive(&n);
    write_record(
        "outer",
        &root,
        format!(
            "{{\"case\": \"outer_kills_helper\", \"python_pid\": {}, \"helper_pid\": {}, \"group\": {}, \"sleeper_pid\": {}, \"sleeper_group\": {}, \"elapsed_ms\": {}, \"resolution_code\": {}, \"alive_at_resolver_return\": {}, \"cleaned_after_ms\": {:?}, \"followup_alive\": {}}}\n",
            n.pythonpid, n.helper_pid, n.group, n.sleeper_pid, n.sleeper_group, elapsed.as_millis(), res.code, alive_at_return, cleaned.map(|d| d.as_millis()), followup_alive
        ),
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(1_400)
            && elapsed < std::time::Duration::from_secs(10),
        "the OUTER limit ended the call: {elapsed:?}"
    );
    assert_control_incomplete(&res);
    assert_eq!(n.sleeper_group, n.group);
    assert!(
        cleaned.is_some(),
        "the watchdog killed the compat group after the helper was killed"
    );
    assert!(!followup_alive, "and they stay gone");
}

#[test]
fn n10_a_forced_kill_of_the_helper_itself_still_cleans_the_compat_descendants() {
    use sweep_stop::resolve_topology::{resolve, ResolveContext};
    let (root, record) = nested_checkout("sigkill");
    let repo = PyValue::str(root.to_str().expect("UTF-8"));
    // both limits far away: only the watchdog can end the subtree
    let context = ResolveContext::native()
        .with_native_helper(PathBuf::from(HELPER))
        .with_limits_for_check(30_000, 40_000);
    let (res, killed, n, elapsed) = std::thread::scope(|scope| {
        let started = std::time::Instant::now();
        let call = scope.spawn(|| resolve(&repo, &PyValue::None, &context).expect("resolve"));
        let wait = std::time::Instant::now();
        while !record.exists() && wait.elapsed() < std::time::Duration::from_secs(15) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let n = read_nested(&record);
        let killed = Command::new("kill")
            .args(["-9", &n.helper_pid.to_string()])
            .status()
            .expect("kill")
            .success();
        (
            call.join().expect("resolver thread"),
            killed,
            n,
            started.elapsed(),
        )
    });
    let cleaned = wait_cleaned(&n, std::time::Duration::from_secs(5));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let followup_alive = !nothing_of_it_is_alive(&n);
    write_record(
        "sigkill",
        &root,
        format!(
            "{{\"case\": \"helper_sigkill\", \"python_pid\": {}, \"helper_pid\": {}, \"group\": {}, \"sleeper_pid\": {}, \"sleeper_group\": {}, \"elapsed_ms\": {}, \"kill_status_ok\": {}, \"resolution_code\": {}, \"cleaned_after_ms\": {:?}, \"followup_alive\": {}}}\n",
            n.pythonpid, n.helper_pid, n.group, n.sleeper_pid, n.sleeper_group, elapsed.as_millis(), killed, res.code, cleaned.map(|d| d.as_millis()), followup_alive
        ),
    );
    assert!(killed, "the helper was killed with SIGKILL");
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "no limit had to expire: {elapsed:?}"
    );
    assert_control_incomplete(&res);
    assert!(
        cleaned.is_some(),
        "python and the nested sleeper died with the helper"
    );
    assert!(!followup_alive, "and they stay gone");
}

#[test]
fn n10_the_canonical_native_route_starts_no_python_and_no_supervision_child() {
    // a reviewed source: the helper answers natively; its sidecar names no watchdog and no Python child
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

#[test]
fn n10_a_normal_compat_completion_kills_a_detached_leftover_and_reaps_the_watchdog() {
    let root = changed_checkout("n10_normal");
    let record = root.join("leftover.pid");
    let owner = root.join("detaching_owner.py");
    // a descendant that detaches from the pipes (so the capture can finish) and outlives its parent
    std::fs::write(
        &owner,
        format!(
            "import subprocess\np = subprocess.Popen(['sleep', '120'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\nopen({:?}, 'w').write(str(p.pid))\nprint('done')\n",
            record.to_str().expect("UTF-8")
        ),
    )
    .expect("owner");
    let sidecar = root.join("normal.json");
    let out = helper(
        &[
            "--route-sidecar",
            sidecar.to_str().expect("UTF-8"),
            "--compat-owner",
            owner.to_str().expect("UTF-8"),
            "--compat-python",
            "python3",
            "evaluate",
            "--repo",
            root.to_str().expect("UTF-8"),
        ],
        &[],
        Stdio::piped(),
    );
    assert_eq!(out.status.code(), Some(0), "the compat child's own status");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "done\n");
    let leftover = std::fs::read_to_string(&record).expect("leftover pid");
    let text = std::fs::read_to_string(&sidecar).expect("sidecar");
    let at = text
        .find("\"watchdog_pid\": ")
        .expect("watchdog_pid recorded")
        + 16;
    let watchdog: String = text[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        !alive(leftover.trim()),
        "the leftover descendant was killed with the compat group"
    );
    assert!(
        !alive(&watchdog),
        "the watchdog is gone (killed and reaped)"
    );
    assert!(
        group_members(watchdog.parse().expect("pid")).is_empty(),
        "no member of the compat group is left"
    );
}
