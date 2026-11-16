//! Own regression of the independent P23 review fixes (D1 empty options, D2 typed Local failures, M5 child environment). Own fake commands in own
//! scratch directories, the real `scan_node` executable of this build, no Original, no network, no real host scan, no `--self-test`. Expected values
//! are the Python truthiness / exception rules of the original SOURCE (SOURCE_DERIVED), not measured Original output.

use role_contract::scan_node::Failure;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::scan_node_cli::user_from;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root =
            std::env::temp_dir().join(format!("scan-node-fix-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        Scratch { root }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn commands(scratch: &Scratch, ssh_log: &Path) -> String {
    let dir = scratch.root.join("own-bin");
    std::fs::create_dir_all(&dir).expect("dir");
    let ssh = format!(
        "printf 'ARGS=%s | OWN=%s VAL=%s BR=%s CE=%s CO=%s CV=%s CB=%s\\n' \"$*\" \"${{SCAN_NODE_OWNER_PATH-<unset>}}\" \"${{SCAN_NODE_VALIDATE_ONLY-<unset>}}\" \"${{SCAN_NODE_BRIDGE-<unset>}}\" \"${{SCAN_NODE_CALLER_ENV-<unset>}}\" \"${{SCAN_NODE_CALLER_OWNER_PATH-<unset>}}\" \"${{SCAN_NODE_CALLER_VALIDATE_ONLY-<unset>}}\" \"${{SCAN_NODE_CALLER_BRIDGE-<unset>}}\" >> {}\n/bin/cat >/dev/null\nprintf 'ARCH=aarch64\\nGPUS=1\\nMODEL=NVIDIA GB10\\nDRIVER=580.95.05\\nCUDA=13.2\\nGPUMEM=[N/A], 12.1\\n'\n",
        ssh_log.display()
    );
    let files: [(&str, String); 4] = [
        ("git", "printf 'single-node\\n'\n".into()),
        ("nvcc", "printf 'Cuda compilation tools, release 13.2, V\\n'\n".into()),
        ("nvidia-smi", "case \"$1\" in -L) printf 'GPU 0: x\\n';; *name*) printf 'NVIDIA GB10\\n';; *driver*) printf '580.95.05\\n';; *) printf '[N/A], 12.1\\n';; esac\n".into()),
        ("ssh", ssh),
    ];
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    dir.to_string_lossy().into_owned()
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
    ssh_lines: Vec<String>,
}

fn prepare_case(scratch: &Scratch) -> PathBuf {
    let case = scratch.root.join("case");
    std::fs::create_dir_all(case.join("output/single")).expect("case");
    std::fs::write(
        case.join("output/single/manifest.yaml"),
        "topology: single\nmodel_source: managed\nnas_model_path: /nonexistent\n",
    )
    .expect("manifest");
    case
}

/// `launcher`: start through the real launcher script (with `SCAN_NODE_BIN`), else the executable directly. `gone_cwd`: the working directory
/// is removed before the program starts (own directory only).
fn run_with(
    scratch: &Scratch,
    tag: &str,
    args: &[&str],
    env: &[(&str, &str)],
    launcher: bool,
    gone_cwd: bool,
) -> Run {
    let case = prepare_case(scratch);
    let log = scratch.root.join(format!("ssh-{tag}.log"));
    let _ = std::fs::remove_file(&log);
    let path = commands(scratch, &log);
    let elf = env!("CARGO_BIN_EXE_scan_node");
    let launcher_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../terraforming_node/scripts/scan_node.sh");
    let mut command = if gone_cwd {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("mkdir gone && cd gone && rmdir ../gone && exec \"$0\" \"$@\"")
            .arg(elf);
        command
    } else if launcher {
        let mut command = Command::new("/bin/bash");
        command.arg(&launcher_path);
        command
    } else {
        Command::new(elf)
    };
    command
        .env_clear()
        .env("PATH", format!("{path}:/usr/bin:/bin"))
        .env("LOGNAME", "tester")
        .env("TZ", "UTC")
        .env("LC_ALL", "C.UTF-8")
        .env("SCAN_NODE_BIN", elf)
        .current_dir(&case)
        .args(args)
        .stdin(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).expect("utf-8"),
        stderr: String::from_utf8(output.stderr).expect("utf-8"),
        ssh_lines: std::fs::read_to_string(&log)
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default(),
    }
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

#[test]
fn an_empty_option_value_behaves_like_the_option_that_was_not_given_python_truthiness_and_starts_no_ssh(
) {
    let scratch = Scratch::new("d1");
    let peer = "user@203.0.113.11";
    let cases: [(&str, Vec<&str>, Vec<&str>); 6] = [
        (
            "single peer_ssh empty",
            vec!["--topology", "single", "--peer-ssh", ""],
            vec!["--topology", "single"],
        ),
        (
            "single peer_ip and peer_ssh empty",
            vec!["--topology", "single", "--peer-ip", "", "--peer-ssh", ""],
            vec!["--topology", "single"],
        ),
        (
            "multi peer_ip empty",
            vec!["--topology", "multi", "--peer-ip", ""],
            vec!["--topology", "multi"],
        ),
        (
            "multi peer_ip and peer_ssh empty",
            vec!["--topology", "multi", "--peer-ip", "", "--peer-ssh", ""],
            vec!["--topology", "multi"],
        ),
        (
            "single sub_work_dir empty beside a real peer",
            vec![
                "--topology",
                "single",
                "--peer-ssh",
                peer,
                "--sub-work-dir",
                "",
            ],
            vec!["--topology", "single", "--peer-ssh", peer],
        ),
        (
            "single emit_sub_manifest empty",
            vec!["--topology", "single", "--emit-sub-manifest", ""],
            vec!["--topology", "single"],
        ),
    ];
    for (n, (label, empty, absent)) in cases.iter().enumerate() {
        let with_empty = run_with(&scratch, &format!("e{n}"), empty, &[], false, false);
        let without = run_with(&scratch, &format!("a{n}"), absent, &[], false, false);
        assert_ne!(
            with_empty.code, 3,
            "{label}: an empty value must not end as an unsupported shape: {}",
            with_empty.stderr
        );
        assert_eq!(with_empty.code, without.code, "{label}: exit status");
        assert_eq!(with_empty.stderr, without.stderr, "{label}: stderr");
        assert_eq!(
            with_empty.stdout, without.stdout,
            "{label}: stdout must equal the run without the option"
        );
        assert_eq!(
            with_empty.ssh_lines.len(),
            without.ssh_lines.len(),
            "{label}: ssh starts"
        );
        if !empty.contains(&peer) {
            assert!(
                with_empty.ssh_lines.is_empty(),
                "{label}: an empty peer target started ssh: {:?}",
                with_empty.ssh_lines
            );
            let envelope = compact(&with_empty.stdout);
            assert!(
                !envelope.contains("\"node_id\":\"sub\""),
                "{label}: a bogus sub entry"
            );
            assert!(
                !envelope.contains("\"peer_check\""),
                "{label}: a peer_check for an empty peer_ip"
            );
        } else {
            assert!(
                !with_empty.ssh_lines.is_empty(),
                "{label}: the real peer is still probed"
            );
            assert!(
                compact(&with_empty.stdout).contains("\"node_id\":\"sub\""),
                "{label}: sub entry"
            );
            assert!(
                !compact(&with_empty.stdout).contains("\"work_dir\":\"\""),
                "{label}: an empty sub work_dir was added"
            );
        }
    }
}

#[test]
fn a_removed_working_directory_ends_like_the_originals_uncaught_oserror_with_no_normal_output() {
    let scratch = Scratch::new("d2cwd");
    let abs = |name: &str, text: &str| {
        let path = scratch.root.join(name);
        std::fs::write(&path, text).expect("file");
        path.to_string_lossy().into_owned()
    };
    let compose = abs("compose.yaml", "services: {}\n");
    let env_file = abs("interconnect.env", "");
    let manifest = abs(
        "manifest.yaml",
        "topology: single\nmodel_source: managed\nnas_model_path: /nonexistent\n",
    );
    let run = run_with(
        &scratch,
        "gone",
        &[
            "--topology",
            "single",
            "--compose",
            &compose,
            "--env-interconnect",
            &env_file,
            "--manifest",
            &manifest,
        ],
        &[],
        false,
        true,
    );
    assert_eq!(
        run.code, 1,
        "stdout={:?} stderr={:?}",
        run.stdout, run.stderr
    );
    assert_eq!(run.stdout, "", "no normal envelope");
    assert!(
        run.stderr.contains("Traceback (most recent call last):"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr
            .trim_end()
            .ends_with("FileNotFoundError: [Errno 2] No such file or directory"),
        "{}",
        run.stderr
    );
    assert!(!run.stderr.contains("work_dir") && !run.stdout.contains("null"));
}

fn unsupported_text(result: Result<role_contract_pystr::PyStr, Failure>) -> String {
    match result {
        Err(Failure::Unsupported(reason)) => reason,
        Err(Failure::Raised(raised)) => panic!("raised {}", raised.class),
        Ok(value) => panic!("unexpected user {:?}", value.to_utf8()),
    }
}

use agent_runtime_support::pystr as role_contract_pystr;

#[test]
fn getuser_follows_the_originals_order_and_a_miss_is_an_explicit_boundary_never_a_null() {
    let no_env = |_: &str| None;
    let name = |value: &str| value.to_owned();
    let passwd = || {
        Ok(name(
            "root:x:0:0::/root:/bin/sh\nalice:x:4242:4242::/nonexistent:/bin/sh\n",
        ))
    };
    // an environment value wins; an empty one is skipped like Python's `if user:`
    let env = |key: &str| match key {
        "LOGNAME" => Some(std::ffi::OsString::new()),
        "USER" => Some(std::ffi::OsString::from("from-env")),
        _ => None,
    };
    assert_eq!(
        user_from(env, passwd, 4242)
            .expect("env")
            .to_utf8()
            .expect("utf-8"),
        "from-env"
    );
    assert_eq!(
        user_from(no_env, passwd, 4242)
            .expect("passwd")
            .to_utf8()
            .expect("utf-8"),
        "alice"
    );
    let missing = unsupported_text(user_from(no_env, passwd, 31337));
    assert!(
        missing.contains("no entry for uid 31337") && missing.contains("not modelled"),
        "{missing}"
    );
    let unreadable = unsupported_text(user_from(
        no_env,
        || Err(std::io::Error::from_raw_os_error(2)),
        4242,
    ));
    assert!(unreadable.contains("cannot be read"), "{unreadable}");
}

fn child_line(run: &Run) -> String {
    assert_eq!(
        run.ssh_lines.len(),
        3.min(run.ssh_lines.len().max(1)).max(run.ssh_lines.len()),
        "ssh lines"
    );
    let line = run.ssh_lines.first().cloned().unwrap_or_default();
    line.split(" | ").nth(1).unwrap_or_default().to_owned()
}

#[test]
fn probe_children_see_the_callers_environment_not_the_plumbing_the_adapter_and_launcher_add() {
    let scratch = Scratch::new("m5");
    let args = ["--topology", "single", "--peer-ssh", "user@203.0.113.11"];
    // (a) the executable started directly: nothing to undo, the user's own values pass through untouched
    let direct = run_with(
        &scratch,
        "m5a",
        &args,
        &[
            ("SCAN_NODE_OWNER_PATH", "/user/own"),
            ("SCAN_NODE_VALIDATE_ONLY", ""),
        ],
        false,
        false,
    );
    assert_eq!(
        child_line(&direct),
        "OWN=/user/own VAL= BR=<unset> CE=<unset> CO=<unset> CV=<unset> CB=<unset>"
    );
    // (b) a transport recorded the caller's values (explicit user value, empty value) and replaced the plumbing: the child gets the caller's
    let recorded = run_with(
        &scratch,
        "m5b",
        &args,
        &[
            ("SCAN_NODE_CALLER_ENV", "1"),
            ("SCAN_NODE_OWNER_PATH", "/adapter/owner"),
            ("SCAN_NODE_CALLER_OWNER_PATH", "/user/own2"),
            ("SCAN_NODE_CALLER_VALIDATE_ONLY", ""),
        ],
        false,
        false,
    );
    assert_eq!(
        child_line(&recorded),
        "OWN=/user/own2 VAL= BR=<unset> CE=<unset> CO=<unset> CV=<unset> CB=<unset>"
    );
    // (c) a transport whose caller had none of them: the child sees none
    let none = run_with(
        &scratch,
        "m5c",
        &args,
        &[
            ("SCAN_NODE_CALLER_ENV", "1"),
            ("SCAN_NODE_OWNER_PATH", "/adapter/owner"),
        ],
        false,
        false,
    );
    assert_eq!(
        child_line(&none),
        "OWN=<unset> VAL=<unset> BR=<unset> CE=<unset> CO=<unset> CV=<unset> CB=<unset>"
    );
    // (d) through the real launcher script: its own default owner path never reaches the child, the user's values do
    let plain = run_with(&scratch, "m5d", &args, &[], true, false);
    assert_eq!(
        child_line(&plain),
        "OWN=<unset> VAL=<unset> BR=<unset> CE=<unset> CO=<unset> CV=<unset> CB=<unset>"
    );
    let user = run_with(
        &scratch,
        "m5e",
        &args,
        &[
            ("SCAN_NODE_OWNER_PATH", "/user/own3"),
            ("SCAN_NODE_VALIDATE_ONLY", "0"),
        ],
        true,
        false,
    );
    assert_eq!(
        child_line(&user),
        "OWN=/user/own3 VAL=0 BR=<unset> CE=<unset> CO=<unset> CV=<unset> CB=<unset>"
    );
}

