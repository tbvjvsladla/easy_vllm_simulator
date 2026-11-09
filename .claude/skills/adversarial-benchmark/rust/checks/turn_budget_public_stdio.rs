//! The public `turn_budget.py` command line when a standard descriptor is closed, and its forwarding to the launcher. Synthetic checks
//! (no saved row, no original call): in a Python that starts with fd 1 and/or 2 closed, `sys.stdout`/`sys.stderr` is `None`, and the
//! adapter must skip an absent stream instead of failing before the exec, while a present stream's flush error must still propagate.
//! The exec itself is the unchanged `os.execve("/bin/bash", [bash, launcher, *argv], env)`: argv, environment and PID reach the program
//! (the route observation records the program's own pid, which must be the pid of the process the check started).

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn scripts() -> PathBuf {
    // lexically normalised like the adapter's `os.path.abspath` (the package path holds no symbolic link)
    std::fs::canonicalize(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../terraforming_node/scripts"),
    )
    .expect("scripts dir")
}

fn elf() -> &'static str {
    env!("CARGO_BIN_EXE_turn_budget")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turn-budget-stdio-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// Runs `bash -c 'exec <program> ... <closures>'` so the program starts with the given descriptors closed.
fn run_closed(
    program_and_args: &[&str],
    closures: &str,
    observer: Option<&PathBuf>,
) -> (Output, u32) {
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(format!("exec \"$@\" {closures}"))
        .arg("_")
        .args(program_and_args)
        .env("TURN_BUDGET_BIN", elf())
        .env_remove("TURN_BUDGET_BRIDGE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match observer {
        Some(path) => command.env("TURN_BUDGET_ROUTE_OBSERVER", path),
        None => command.env_remove("TURN_BUDGET_ROUTE_OBSERVER"),
    };
    let child = command.spawn().expect("spawn");
    let pid = child.id();
    (child.wait_with_output().expect("wait"), pid)
}

fn python_script() -> String {
    scripts()
        .join("turn_budget.py")
        .to_str()
        .expect("utf-8")
        .to_string()
}

fn launcher() -> String {
    scripts()
        .join("turn_budget.sh")
        .to_str()
        .expect("utf-8")
        .to_string()
}

#[test]
fn closed_standard_descriptors_do_not_stop_the_public_forwarding() {
    for closures in [">&-", "2>&-", ">&- 2>&-"] {
        let dir = scratch("closed");
        let observer = dir.join("route.jsonl");
        // through the public Python file ...
        let (public, pid) = run_closed(
            &["python3", "-I", &python_script(), "--help"],
            closures,
            Some(&observer),
        );
        // ... and straight through the launcher under the same closure: the adapter must add nothing and lose nothing
        let (direct, _) = run_closed(&["bash", &launcher(), "--help"], closures, None);
        let stderr = String::from_utf8_lossy(&public.stderr);
        assert!(
            !stderr.contains("Traceback") && !stderr.contains("AttributeError"),
            "{closures}: {stderr}"
        );
        assert_eq!(
            public.status.code(),
            direct.status.code(),
            "{closures}: exit status differs from the launcher's"
        );
        assert_eq!(
            public.stderr, direct.stderr,
            "{closures}: stderr differs from the launcher's"
        );
        assert_eq!(
            public.stdout, direct.stdout,
            "{closures}: stdout differs from the launcher's"
        );
        // the program ran in the process the check started (python -> bash -> ELF by exec: one pid)
        let lines: Vec<String> = std::fs::read_to_string(&observer)
            .expect("route observation")
            .lines()
            .map(str::to_string)
            .collect();
        assert!(
            !lines.is_empty(),
            "{closures}: the program wrote no route observation"
        );
        for line in &lines {
            assert!(
                line.contains(&format!("\"pid\": {pid},")),
                "{closures}: pid {pid} not in {line}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn open_descriptors_forward_argv_bytes_status_and_pid() {
    let dir = scratch("open");
    let observer = dir.join("route.jsonl");
    for args in [vec!["--help"], vec!["--caps"], vec!["--no-such-option"]] {
        let mut argv = vec!["python3", "-I"];
        let script = python_script();
        argv.push(&script);
        argv.extend(args.iter().copied());
        let (public, pid) = run_closed(&argv, "", Some(&observer));
        let launcher = launcher();
        let mut direct_argv = vec!["bash", launcher.as_str()];
        direct_argv.extend(args.iter().copied());
        let (direct, _) = run_closed(&direct_argv, "", None);
        assert_eq!(public.status.code(), direct.status.code(), "{args:?}");
        assert_eq!(public.stdout, direct.stdout, "{args:?}");
        assert_eq!(public.stderr, direct.stderr, "{args:?}");
        let text = std::fs::read_to_string(&observer).expect("route observation");
        assert!(
            text.contains(&format!("\"pid\": {pid},")),
            "{args:?}: {text}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The synthetic harness: loads the adapter, replaces only `os.execve` by a recorder (the real exec is checked above) and drives
/// `main()` with absent and failing streams.
const HARNESS: &str = r#"
import importlib.util, os, sys, json
spec = importlib.util.spec_from_file_location("turn_budget", sys.argv[1])
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
real_stdout, real_stderr = sys.stdout, sys.stderr
calls = []
os.execve = lambda path, argv, env: calls.append((path, list(argv), dict(env)))
class Failing:
    def flush(self): raise OSError(28, "No space left on device")
results = {}
def attempt(name, out, err):
    calls.clear(); sys.argv = ["turn_budget.py", "--help", "x y"]
    sys.stdout, sys.stderr = out, err
    try:
        returned = module.main(); error = None
    except BaseException as exc:
        returned, error = None, "%s:%s" % (type(exc).__name__, exc)
    finally:
        sys.stdout, sys.stderr = real_stdout, real_stderr
    results[name] = {"returned": returned, "error": error, "execs": len(calls),
                     "argv": calls[0][1][2:] if calls else None, "launcher": calls[0][1][1] if calls else None,
                     "path": calls[0][0] if calls else None,
                     "owner": calls[0][2].get("TURN_BUDGET_OWNER_PATH") if calls else None,
                     "bridge_env_removed": ("TURN_BUDGET_BRIDGE" not in calls[0][2]) if calls else None}
attempt("both_absent", None, None)
attempt("stdout_absent", None, real_stderr)
attempt("stderr_absent", real_stdout, None)
attempt("stdout_flush_fails", Failing(), real_stderr)
attempt("stderr_flush_fails", None, Failing())
# _self_test: absent stdout is skipped, a failing one propagates
module.subprocess.call = lambda argv, env=None: 0
sys.stdout = None
try: results["self_test_absent"] = {"returned": module._self_test()}
finally: sys.stdout = real_stdout
sys.stdout = Failing()
try:
    module._self_test(); results["self_test_failing"] = {"error": None}
except OSError as exc:
    results["self_test_failing"] = {"error": "OSError:%s" % exc}
finally: sys.stdout = real_stdout
real_stderr.write(json.dumps(results)); real_stderr.flush()
"#;

#[test]
fn an_absent_stream_is_skipped_and_a_failing_stream_still_fails() {
    let output = Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(HARNESS)
        .arg(python_script())
        .output()
        .expect("python3");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results = String::from_utf8(output.stderr).expect("utf-8");
    let launcher = launcher();
    let owner = python_script();
    for name in ["both_absent", "stdout_absent", "stderr_absent"] {
        let needle = format!("\"{name}\": {{\"returned\": 1, \"error\": null, \"execs\": 1, \"argv\": [\"--help\", \"x y\"], \"launcher\": \"{launcher}\", \"path\": \"/bin/bash\", \"owner\": \"{owner}\", \"bridge_env_removed\": true}}");
        assert!(results.contains(&needle), "{name}: {results}");
    }
    for name in ["stdout_flush_fails", "stderr_flush_fails"] {
        let needle = format!("\"{name}\": {{\"returned\": null, \"error\": \"OSError:[Errno 28] No space left on device\", \"execs\": 0");
        assert!(
            results.contains(&needle),
            "{name} must fail before the exec: {results}"
        );
    }
    assert!(
        results.contains("\"self_test_absent\": {\"returned\": 0}"),
        "{results}"
    );
    assert!(
        results.contains(
            "\"self_test_failing\": {\"error\": \"OSError:[Errno 28] No space left on device\"}"
        ),
        "{results}"
    );
}
