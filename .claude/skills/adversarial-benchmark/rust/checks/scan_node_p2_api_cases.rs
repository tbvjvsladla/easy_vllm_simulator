//! The P2 probes AND the P3 command line of `scan_node` through the REAL executable (the private bridge for the probes, the public
//! command line for the CLI rows).
//!
//! The P2 probes through the REAL private bridge of the `scan_node` executable, against OWN fake commands and an OWN
//! loopback listener. Expected values are the facts the CLOSED saved P23 rows measured (`checks/data/scan_node/p23_golden.json`, data
//! only: nothing of the Original, of Root's control or of the saved fake wrappers is run); the fake commands are written by this
//! file from the declared outputs in `p23_scenarios.json`. Rows that read this machine (the `/sys` listing, the interface MTU, the
//! CPU architecture) are compared with what this check reads from the same machine, labelled HOST_OBSERVED.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("data/scan_node/p23_golden.json");
const SCENARIOS: &str = include_str!("data/scan_node/p23_scenarios.json");
const CLI_GOLDEN: &str = include_str!("data/scan_node/p23_cli_golden.json");

fn parse(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("json")
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("no key {key}")),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn text_of(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().expect("utf-8"),
        other => panic!("not a string: {}", other.repr()),
    }
}

fn items(value: &PyValue) -> Vec<PyValue> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
        other => panic!("not a list: {}", other.repr()),
    }
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// One fake command (a `/bin/sh` script) of the declared scenario entries: first entry whose `match` equals the arguments; every
/// invocation logs its argv and standard input under `log`.
fn fake_script(entries: &[PyValue], log: &Path) -> String {
    let mut body = String::from("#!/bin/sh\n");
    body += &format!(
        "LOG={}\nSTAMP=$(/bin/date +%s%N)\n",
        shell_quote(&log.to_string_lossy())
    );
    body += "{ printf '%s' \"${0##*/}\"; for a in \"$@\"; do printf '\\0%s' \"$a\"; done; } > \"$LOG/$STAMP.$$.argv\"\n/bin/cat > \"$LOG/$STAMP.$$.stdin\"\n";
    for entry in entries {
        let matcher = items(member(entry, "match"));
        let condition = if matcher.len() == 1 && text_of(&matcher[0]) == "*" {
            "true".to_string()
        } else {
            let mut parts = vec![format!("[ \"$#\" -eq {} ]", matcher.len())];
            for (at, word) in matcher.iter().enumerate() {
                let word = text_of(word);
                if word != "*" {
                    parts.push(format!("[ \"${}\" = {} ]", at + 1, shell_quote(&word)));
                }
            }
            parts.join(" && ")
        };
        body += &format!("if {condition}; then\n");
        if let Some(PyValue::Int(seconds)) = member_opt(entry, "sleep_s") {
            body += &format!("  /bin/sleep {seconds}\n");
        }
        if let Some(PyValue::Str(stdout)) = member_opt(entry, "stdout") {
            body += &format!(
                "  printf '%s' {}\n",
                shell_quote(&stdout.to_utf8().expect("utf-8"))
            );
        }
        if let Some(PyValue::Str(hex)) = member_opt(entry, "stdout_hex") {
            let octal: String = hex
                .to_utf8()
                .expect("utf-8")
                .split_whitespace()
                .map(|byte| format!("\\{:03o}", u8::from_str_radix(byte, 16).expect("hex")))
                .collect();
            body += &format!("  printf '{octal}'\n");
        }
        if let Some(PyValue::Str(stderr)) = member_opt(entry, "stderr") {
            body += &format!(
                "  printf '%s' {} >&2\n",
                shell_quote(&stderr.to_utf8().expect("utf-8"))
            );
        }
        let rc = match member_opt(entry, "rc") {
            Some(PyValue::Int(number)) => number.to_string(),
            _ => "0".into(),
        };
        body += &format!("  exit {rc}\nfi\n");
    }
    body += "printf 'FAKE: no scenario entry' >&2\nexit 97\n";
    body
}

fn member_opt(value: &PyValue, key: &str) -> Option<PyValue> {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).map(|found| found.peel().clone()),
        _ => None,
    }
}

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root = std::env::temp_dir().join(format!("scan-node-p2-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        Scratch { root }
    }

    /// The PATH directory of a scenario: its fake commands written as own scripts.
    fn scenario(&self, name: &str, log: &Path) -> PathBuf {
        let scenarios = parse(SCENARIOS);
        let dir = self.root.join("bin").join(name);
        std::fs::create_dir_all(&dir).expect("bin dir");
        let entry = member(member(&scenarios, "dirs"), name);
        if let PyValue::Dict(commands) = member(entry, "commands").peel() {
            for (command, entries) in commands.iter() {
                let path = dir.join(command.to_utf8().expect("name"));
                std::fs::write(&path, fake_script(&items(entries), log)).expect("script");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
            }
        }
        dir
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// JSON value -> wire of the bridge protocol.
fn wire(value: &PyValue) -> String {
    match value.peel() {
        PyValue::None => r#"{"t":"none"}"#.into(),
        PyValue::Bool(flag) => format!(r#"{{"t":"bool","v":{flag}}}"#),
        PyValue::Int(number) => {
            let text = number.to_string();
            match text.strip_prefix('-') {
                Some(rest) => format!(
                    r#"{{"t":"int","hex":"-{:x}"}}"#,
                    rest.parse::<u128>().expect("small int")
                ),
                None => format!(
                    r#"{{"t":"int","hex":"+{:x}"}}"#,
                    text.parse::<u128>().expect("small int")
                ),
            }
        }
        PyValue::Str(text) => format!(
            r#"{{"t":"str","v":{}}}"#,
            PyValue::Str(text.clone()).dumps_ascii().expect("json")
        ),
        PyValue::List(entries) => format!(
            r#"{{"t":"list","v":[{}]}}"#,
            entries.iter().map(wire).collect::<Vec<_>>().join(",")
        ),
        PyValue::Dict(map) if map.len() == 1 && map.get("tuple").is_some() => {
            format!(
                r#"{{"t":"tuple","v":[{}]}}"#,
                items(member(value, "tuple"))
                    .iter()
                    .map(wire)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        other => panic!("not a wire value: {}", other.repr()),
    }
}

/// Wire of the answer -> plain value (the shape of the golden).
fn plain(value: &PyValue) -> PyValue {
    let kind = text_of(member(value, "t"));
    match kind.as_str() {
        "none" => PyValue::None,
        "bool" | "str" => member(value, "v").clone(),
        "int" => {
            let word = text_of(member(value, "hex"));
            let (negative, digits) = match word.strip_prefix('-') {
                Some(rest) => (true, rest.to_string()),
                None => (false, word.trim_start_matches('+').to_string()),
            };
            let number = i128::from_str_radix(&digits, 16).expect("hex");
            PyValue::int(if negative { -number } else { number } as i64)
        }
        "list" | "tuple" => PyValue::List(items(member(value, "v")).iter().map(plain).collect()),
        "dict" => {
            let mut out = agent_runtime_support::pyvalue::PyDict::new();
            for pair in items(member(value, "v")) {
                let parts = items(&pair);
                out.set(&text_of(member(&parts[0], "v")), plain(&parts[1]));
            }
            PyValue::Dict(out)
        }
        other => panic!("unknown wire type {other}"),
    }
}

struct Answer {
    status: String,
    value: Option<PyValue>,
    effects_stderr: Vec<String>,
    peer_errors: Vec<(String, String)>,
    raw: PyValue,
}

impl Answer {
    /// The effects the importer must perform: the diagnostic lines and the `PEER_SSH_LAST_ERROR` assignments, in order.
    fn effects(&self) -> (&[String], &[(String, String)]) {
        (&self.effects_stderr, &self.peer_errors)
    }
}

fn call(op: &str, params: &[(&str, String)], path: &str) -> Answer {
    let request = format!(
        r#"{{"protocol":"scan-node-bridge/1","op":{},"params":{{{}}}}}"#,
        PyValue::str(op).dumps_ascii().expect("json"),
        params
            .iter()
            .map(|(name, value)| format!("\"{name}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_scan_node"))
        .env_clear()
        .env("PATH", path)
        .env("SCAN_NODE_BRIDGE", "1")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn scan_node");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(request.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("output");
    assert!(
        output.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let raw = parse(std::str::from_utf8(&output.stdout).expect("ascii").trim());
    let status = text_of(member(&raw, "status"));
    let value = (status == "returned").then(|| plain(member(&raw, "value")));
    let (mut lines, mut peers) = (Vec::new(), Vec::new());
    if let Some(effects) = member_opt(&raw, "effects") {
        lines = items(member(&effects, "stderr"))
            .iter()
            .map(|line| text_of(member(line, "v")))
            .collect();
        peers = items(member(&effects, "peer_errors"))
            .iter()
            .map(|pair| {
                let parts = items(pair);
                (
                    text_of(member(&parts[0], "v")),
                    text_of(member(&parts[1], "v")),
                )
            })
            .collect();
    }
    Answer {
        status,
        value,
        effects_stderr: lines,
        peer_errors: peers,
        raw,
    }
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn machine() -> String {
    let output = Command::new("uname").arg("-m").output().expect("uname");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_string()
}

fn sysfs_hcas() -> PyValue {
    let mut names: Vec<String> = std::fs::read_dir("/sys/class/infiniband")
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    PyValue::List(names.iter().map(|name| PyValue::str(name)).collect())
}

fn host_mtu(iface: &str) -> PyValue {
    std::fs::read_to_string(format!("/sys/class/net/{iface}/mtu"))
        .ok()
        .and_then(|text| text.trim().parse::<i64>().ok())
        .map_or(PyValue::None, PyValue::int)
}

/// HOST_OBSERVED fields of a golden value are replaced by what this machine reports.
fn host_substitute(op: &str, expected: PyValue) -> PyValue {
    let PyValue::Dict(mut map) = expected.peel().clone() else {
        return expected;
    };
    match op {
        "detect_interconnect" => {
            if let Some(PyValue::Str(iface)) = map.get("socket_iface").cloned() {
                map.set("mtu", host_mtu(&iface.to_utf8().expect("name")));
            }
            map.set("_sysfs_hcas", sysfs_hcas());
        }
        "collect_local_hw" => map.set("cpu_arch", PyValue::str(&machine())),
        _ => {}
    }
    PyValue::Dict(map)
}

fn render(value: &PyValue) -> String {
    value
        .dumps_unicode()
        .expect("dump")
        .to_utf8()
        .expect("utf-8")
}

#[test]
fn every_saved_p2_api_row_is_reproduced_by_the_native_probes_on_own_fakes() {
    let golden = parse(GOLDEN);
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let listener_port = listener.local_addr().expect("addr").port();
    let accept = std::thread::spawn(move || {
        listener.set_nonblocking(false).ok();
        while let Ok((stream, _)) = listener.accept() {
            drop(stream);
        }
    });
    let dead_port = closed_port();
    let mut checked = Vec::new();
    for row in items(member(&golden, "rows")) {
        if text_of(member(&row, "kind")) != "api" {
            continue;
        }
        let id = text_of(member(&row, "id"));
        let operation = text_of(member(&row, "operation"));
        let scratch = Scratch::new(&id);
        let log = scratch.root.join("log");
        std::fs::create_dir_all(&log).expect("log");
        let dirs: Vec<String> = items(member(&row, "env_path_dirs"))
            .iter()
            .map(text_of)
            .collect();
        let path = dirs
            .iter()
            .map(|dir| scratch.scenario(dir, &log).to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(":");
        // the parameter names of the original's functions, bound to the saved arguments (declared defaults where the row gave none)
        let names: &[&str] = match operation.as_str() {
            "_run" => &["cmd", "timeout"],
            "peer_reachable" => &["ip", "port", "timeout"],
            "scan_egress" => &["hf_host", "gw_host", "gw_port", "timeout"],
            "_iface_ipv4" => &["iface"],
            "collect_peer_hw" => &["ssh_target", "ssh_opts"],
            "collect_peer_model_env" => {
                &["ssh_target", "nas_path", "hf_token_env_file", "ssh_opts"]
            }
            "collect_peer_egress" => &[
                "ssh_target",
                "hf_host",
                "gw_host",
                "gw_port",
                "timeout",
                "ssh_opts",
            ],
            "_resolves_to_loopback" => &["name"],
            _ => &[],
        };
        let defaults: &[&str] = match operation.as_str() {
            "_run" => &["", r#"{"t":"int","hex":"+a"}"#],
            "peer_reachable" => &[
                "",
                r#"{"t":"int","hex":"+16"}"#,
                r#"{"t":"int","hex":"+5"}"#,
            ],
            "scan_egress" => &[
                r#"{"t":"str","v":"huggingface.co"}"#,
                r#"{"t":"str","v":"8.8.8.8"}"#,
                r#"{"t":"int","hex":"+1bb"}"#,
                r#"{"t":"int","hex":"+5"}"#,
            ],
            "collect_peer_hw" => &["", r#"{"t":"none"}"#],
            "collect_peer_model_env" => &["", "", "", r#"{"t":"none"}"#],
            "collect_peer_egress" => &[
                "",
                r#"{"t":"str","v":"huggingface.co"}"#,
                r#"{"t":"str","v":"8.8.8.8"}"#,
                r#"{"t":"int","hex":"+1bb"}"#,
                r#"{"t":"int","hex":"+5"}"#,
                r#"{"t":"none"}"#,
            ],
            _ => &[],
        };
        let args = items(member(&row, "args"));
        let mut params: Vec<(&str, String)> = Vec::new();
        for (at, name) in names.iter().enumerate() {
            let value = if operation == "_run" && at == 0 {
                // the saved row passed the command as the list [..] elements of `args` (the proposal's argument shape: a STRING command)
                wire(&args[0])
            } else if let Some(given) = args.get(at) {
                // the fixture placeholders are replaced by the ports of this check's own listener
                match given.peel() {
                    PyValue::Str(word) if word.equals("<FX:closed_port>") => {
                        format!(r#"{{"t":"int","hex":"+{dead_port:x}"}}"#)
                    }
                    PyValue::Str(word) if word.equals("<FX:listener_port>") => {
                        format!(r#"{{"t":"int","hex":"+{listener_port:x}"}}"#)
                    }
                    _ => wire(given),
                }
            } else {
                defaults[at].to_string()
            };
            params.push((name, value));
        }
        if let PyValue::Dict(kwargs) = member(&row, "kwargs").peel() {
            for (name, value) in kwargs.iter() {
                let name = name.to_utf8().expect("name");
                let at = names
                    .iter()
                    .position(|own| *own == name)
                    .expect("kwarg name");
                params[at].1 = wire(value);
            }
        }
        if operation == "_run" {
            // the saved row calls `_run(cmd, timeout)` with the saved two positional values: cmd = args[0], timeout = args[1]
            if let Some(second) = args.get(1) {
                params[1].1 = wire(second);
            }
        }
        let answer = call(&operation, &params, &path);
        assert_eq!(
            answer.status,
            "returned",
            "{id} {operation}: {}",
            render(&answer.raw)
        );
        let expected = host_substitute(&operation, member(&row, "return").clone());
        assert_eq!(
            render(answer.value.as_ref().expect("value")),
            render(&expected),
            "{id} {operation}"
        );
        // the commands the original started, in order, with the bytes it fed them
        let mut logged: Vec<(String, Vec<u8>)> = Vec::new();
        let mut files: Vec<_> = std::fs::read_dir(&log)
            .expect("log dir")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        files.sort();
        for file in files
            .iter()
            .filter(|f| f.extension().is_some_and(|ext| ext == "argv"))
        {
            let argv = std::fs::read(file).expect("argv");
            let parts: Vec<String> = argv
                .split(|&b| b == 0)
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect();
            let stdin = std::fs::read(file.with_extension("stdin")).unwrap_or_default();
            logged.push((
                render(&PyValue::List(
                    parts.iter().map(|part| PyValue::str(part)).collect(),
                )),
                stdin,
            ));
        }
        let saved = items(member(&row, "fake_invocations"));
        let saved_argv: Vec<String> = saved
            .iter()
            .map(|inv| render(member(inv, "argv")))
            .collect();
        let saved_stdin: Vec<Vec<u8>> = saved
            .iter()
            .map(|inv| match member(inv, "stdin_hex").peel() {
                PyValue::Str(hex) => {
                    let hex = hex.to_utf8().expect("hex");
                    (0..hex.len())
                        .step_by(2)
                        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
                        .collect()
                }
                _ => Vec::new(),
            })
            .collect();
        assert_eq!(
            logged
                .iter()
                .map(|(argv, _)| argv.clone())
                .collect::<Vec<_>>(),
            saved_argv,
            "{id}: commands started, in order"
        );
        assert_eq!(
            logged
                .iter()
                .map(|(_, stdin)| stdin.clone())
                .collect::<Vec<_>>(),
            saved_stdin,
            "{id}: bytes fed on standard input"
        );
        checked.push(id);
    }
    drop(accept);
    assert_eq!(checked.len(), 43, "the 43 saved P2 API rows");
}

const HOSTKEY_CAUSE: &str =
    "host-key 미등록/불일치 — 메인 known_hosts 에 서브 키를 등록하라(ssh-keyscan)";
const PERM_CAUSE: &str = "키 인증 거부 — 서브 authorized_keys 에 메인 공개키를 넣어라";
const REMOTE_TOOL_CAUSE: &str = "원격 도구 부재(nvidia-smi/uname 등) — PATH 또는 미설치";
const UNREACHED_CAUSE: &str = "네트워크 미도달/타임아웃";

fn s(text: &str) -> String {
    format!(
        r#"{{"t":"str","v":{}}}"#,
        PyValue::str(text).dumps_ascii().expect("json")
    )
}

/// The diagnostics of a failed SSH probe are effects of the importer's own process: the native side returns them as data, in the
/// original's order. The expected texts are SOURCE_DERIVED (the original printed them; the closed rows do not record stderr).
#[test]
fn failed_ssh_probes_return_their_diagnostics_and_peer_errors_as_effects() {
    let cases: [(&str, &str, &str, Vec<String>); 5] = [
        ("SC_SSH_HOSTKEY", "HW", HOSTKEY_CAUSE, vec!["[scan]   ssh stderr: Host key verification failed.".into()]),
        ("SC_SSH_PERM", "HW", PERM_CAUSE, vec!["[scan]   ssh stderr: user@203.0.113.11: Permission denied (publickey).".into()]),
        ("SC_SSH_127", "HW", REMOTE_TOOL_CAUSE, vec!["[scan]   ssh stderr: bash: nvidia-smi: command not found".into()]),
        ("SC_NONE", "HW", UNREACHED_CAUSE, vec!["[scan]   ssh stderr: FileNotFoundError: [Errno 2] No such file or directory: 'ssh'".into()]),
        ("SC_SSH_HOSTKEY", "egress", HOSTKEY_CAUSE, vec!["[scan]   ssh stderr: Host key verification failed.".into()]),
    ];
    for (scenario, what, cause, detail) in cases {
        let scratch = Scratch::new(&format!("diag-{scenario}-{what}"));
        let log = scratch.root.join("log");
        std::fs::create_dir_all(&log).expect("log");
        let path = scratch
            .scenario(scenario, &log)
            .to_string_lossy()
            .into_owned();
        let answer = if what == "HW" {
            call(
                "collect_peer_hw",
                &[
                    ("ssh_target", s("user@203.0.113.11")),
                    ("ssh_opts", r#"{"t":"none"}"#.into()),
                ],
                &path,
            )
        } else {
            call(
                "collect_peer_egress",
                &[
                    ("ssh_target", s("user@203.0.113.11")),
                    ("hf_host", s("huggingface.co")),
                    ("gw_host", s("8.8.8.8")),
                    ("gw_port", r#"{"t":"int","hex":"+1bb"}"#.into()),
                    ("timeout", r#"{"t":"int","hex":"+5"}"#.into()),
                    ("ssh_opts", r#"{"t":"none"}"#.into()),
                ],
                &path,
            )
        };
        assert_eq!(answer.status, "returned");
        let (lines, peers) = answer.effects();
        let mut expected = vec![format!("[scan] 서브 {what} 프로브 실패: {cause}")];
        expected.extend(detail);
        assert_eq!(lines, &expected[..], "{scenario}/{what}");
        assert_eq!(
            peers,
            &[(what.to_string(), cause.to_string())][..],
            "{scenario}/{what}"
        );
    }
}

#[test]
fn a_successful_probe_has_no_effects_and_a_model_env_failure_is_named_model_env() {
    let scratch = Scratch::new("model-env");
    let log = scratch.root.join("log");
    std::fs::create_dir_all(&log).expect("log");
    let ok = scratch
        .scenario("SC_SSH_MODELENV_OK", &log)
        .to_string_lossy()
        .into_owned();
    let answer = call(
        "collect_peer_model_env",
        &[
            ("ssh_target", s("user@h")),
            ("nas_path", s("/nas/it's here")),
            ("hf_token_env_file", r#"{"t":"none"}"#.into()),
            (
                "ssh_opts",
                r#"{"t":"list","v":[{"t":"str","v":"-o"},{"t":"str","v":"BatchMode=yes"}]}"#.into(),
            ),
        ],
        &ok,
    );
    assert_eq!(answer.effects(), (&[][..], &[][..]));
    let logged = std::fs::read_dir(&log)
        .expect("log")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    let stdin = logged
        .iter()
        .find(|file| file.extension().is_some_and(|ext| ext == "stdin"))
        .expect("stdin log");
    // shlex.quote of a path with a quote, fed on standard input, argv with the caller's own options
    assert_eq!(
        std::fs::read_to_string(stdin).expect("stdin"),
        "[ -d '/nas/it'\"'\"'s here' ] && echo NAS=1 || echo NAS=0\n"
    );
    let argv = logged
        .iter()
        .find(|file| file.extension().is_some_and(|ext| ext == "argv"))
        .expect("argv log");
    assert_eq!(
        std::fs::read(argv).expect("argv"),
        b"ssh\0-o\0BatchMode=yes\0user@h\0bash\0-ls".to_vec()
    );
    let hostkey = scratch
        .scenario("SC_SSH_HOSTKEY", &log)
        .to_string_lossy()
        .into_owned();
    let failed = call(
        "collect_peer_model_env",
        &[
            ("ssh_target", s("user@h")),
            ("nas_path", s("/n")),
            ("hf_token_env_file", r#"{"t":"none"}"#.into()),
            ("ssh_opts", r#"{"t":"none"}"#.into()),
        ],
        &hostkey,
    );
    assert_eq!(
        failed.effects().1,
        &[("model-env".to_string(), HOSTKEY_CAUSE.to_string())][..]
    );
}

#[test]
fn shapes_the_port_does_not_carry_are_explicit_unsupported_never_guessed() {
    let none = || r#"{"t":"none"}"#.to_string();
    // a host name with a NUL: CPython hands the truncated name to the resolver (the unmeasured S50L03): named, not run
    let nul = call(
        "peer_reachable",
        &[
            ("ip", s("bad host\0")),
            ("port", r#"{"t":"int","hex":"+16"}"#.into()),
            ("timeout", r#"{"t":"int","hex":"+1"}"#.into()),
        ],
        "/nonexistent",
    );
    assert_eq!(nul.status, "unsupported");
    assert!(text_of(member(&nul.raw, "reason")).contains("NUL"));
    // a non-ASCII name (IDNA in CPython), a non-str command element, a non-int port
    for (op, params) in [
        ("_resolves_to_loopback", vec![("name", s("호스트"))]),
        (
            "_run",
            vec![
                (
                    "cmd",
                    r#"{"t":"list","v":[{"t":"int","hex":"+1"}]}"#.to_string(),
                ),
                ("timeout", none()),
            ],
        ),
        (
            "peer_reachable",
            vec![
                ("ip", s("127.0.0.1")),
                ("port", s("22")),
                ("timeout", none()),
            ],
        ),
    ] {
        assert_eq!(
            call(op, &params, "/nonexistent").status,
            "unsupported",
            "{op}"
        );
    }
    // the shapes CPython answers itself are answered the same way: IndexError of an empty command
    let empty = call(
        "_run",
        &[
            ("cmd", r#"{"t":"list","v":[]}"#.into()),
            ("timeout", r#"{"t":"int","hex":"+a"}"#.into()),
        ],
        "/nonexistent",
    );
    assert_eq!(empty.status, "raised");
    assert_eq!(
        text_of(member(member(&empty.raw, "exc"), "cls")),
        "IndexError"
    );
}

#[test]
fn run_starts_a_listed_command_kills_a_slow_one_and_folds_bad_output() {
    let scratch = Scratch::new("run");
    let log = scratch.root.join("log");
    std::fs::create_dir_all(&log).expect("log");
    // own commands: a slow one (killed at the timeout -> ""), a failing one (rc 1 -> ""), garbage output (not UTF-8 -> ""), CRLF output (universal newlines)
    let dir = scratch.root.join("bin");
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, body) in [
        ("slow", "#!/bin/sh\nexec /bin/sleep 5\n"),
        ("slowgroup", "#!/bin/sh\n/bin/sleep 5\nprintf late\n"),
        ("fail", "#!/bin/sh\nprintf x\nexit 1\n"),
        ("garbage", "#!/bin/sh\nprintf '\\377\\376'\n"),
        ("crlf", "#!/bin/sh\nprintf 'a\\r\\nb\\rc\\n'\n"),
        ("ok", "#!/bin/sh\nprintf 'out:%s' \"$*\"\n"),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let path = dir.to_string_lossy().into_owned();
    let run = |argv: &[&str], timeout: &str| {
        let list = argv
            .iter()
            .map(|word| s(word))
            .collect::<Vec<_>>()
            .join(",");
        call(
            "_run",
            &[
                ("cmd", format!(r#"{{"t":"list","v":[{list}]}}"#)),
                ("timeout", timeout.to_string()),
            ],
            &path,
        )
    };
    let started = std::time::Instant::now();
    let slow = run(&["slow"], r#"{"t":"int","hex":"+1"}"#);
    assert!(
        started.elapsed().as_secs_f64() < 4.5,
        "the slow command is killed at its timeout"
    );
    assert_eq!(render(slow.value.as_ref().expect("value")), "\"\"");
    // a command that leaves a grandchild holding the pipes: killed at the timeout, the drain is bounded (documented deviation: CPython waits for EOF)
    let started = std::time::Instant::now();
    let group = run(&["slowgroup"], r#"{"t":"int","hex":"+1"}"#);
    assert!(started.elapsed().as_secs_f64() < 4.5, "bounded drain");
    assert_eq!(render(group.value.as_ref().expect("value")), "\"\"");
    for name in ["fail", "garbage", "nonexistent-command"] {
        assert_eq!(
            render(
                run(&[name], r#"{"t":"int","hex":"+a"}"#)
                    .value
                    .as_ref()
                    .expect("value")
            ),
            "\"\"",
            "{name}"
        );
    }
    assert_eq!(
        render(
            run(&["crlf"], r#"{"t":"int","hex":"+a"}"#)
                .value
                .as_ref()
                .expect("value")
        ),
        "\"a\\nb\\nc\\n\""
    );
    assert_eq!(
        render(
            run(&["ok", "a b", ""], r#"{"t":"none"}"#)
                .value
                .as_ref()
                .expect("value")
        ),
        "\"out:a b \""
    );
}

// ───────────────────────────── P3: the command line, row by row ─────────────────────────────

fn put(value: &mut PyValue, path: &[&str], new: PyValue) {
    if let PyValue::Dict(map) = value {
        if path.len() == 1 {
            if map.contains_key(path[0]) {
                map.set(path[0], new);
            }
        } else if let Some(inner) = map.get_mut(path[0]) {
            put(inner, &path[1..], new);
        }
    }
}

/// The fields of a scan envelope that read THIS machine (HOST_OBSERVED): replaced by what the machine reports, on both sides.
fn host_fields(envelope: &mut PyValue) {
    let socket_iface =
        match member_opt(envelope, "interconnect").and_then(|ic| member_opt(&ic, "socket_iface")) {
            Some(PyValue::Str(iface)) => Some(iface.to_utf8().expect("name")),
            _ => None,
        };
    if let Some(iface) = socket_iface {
        put(envelope, &["interconnect", "mtu"], host_mtu(&iface));
    }
    put(envelope, &["scan_detail", "_sysfs_hcas"], sysfs_hcas());
    put(envelope, &["cpu_arch"], PyValue::str(&machine()));
    put(
        envelope,
        &["homogeneity", "local", "cpu_arch"],
        PyValue::str(&machine()),
    );
}

fn node_name() -> String {
    let output = Command::new("uname").arg("-n").output().expect("uname");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_string()
}

#[test]
fn every_saved_p3_command_line_row_is_reproduced_by_the_native_scanner() {
    let golden = parse(CLI_GOLDEN);
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let listener_port = listener.local_addr().expect("addr").port();
    let accept = std::thread::spawn(move || {
        while let Ok((stream, _)) = listener.accept() {
            drop(stream);
        }
    });
    let is_root = Command::new("id")
        .arg("-u")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
        .unwrap_or(false);
    let aarch64 = machine() == "aarch64";
    let node = node_name();
    let mut covered = Vec::new();
    for row in items(member(&golden, "rows")) {
        let id = text_of(member(&row, "id"));
        let host_row = member_opt(&row, "stdout_tokenized").is_some();
        if host_row && !aarch64 {
            eprintln!("{id}: host-observed row skipped on a {} machine (the saved row was measured on aarch64)", machine());
            continue;
        }
        if id == "S50C12" && is_root {
            eprintln!("{id}: skipped as root (a mode-000 file stays readable)");
            continue;
        }
        let scratch = Scratch::new(&format!("cli-{id}"));
        let case = scratch.root.join("case");
        std::fs::create_dir_all(&case).expect("case dir");
        let case_text = case.to_string_lossy().into_owned();
        if id == "S50C12" {
            let file = case.join("manifest-mode000.yaml");
            std::fs::write(&file, "topology: single\n").expect("write");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        }
        let log = scratch.root.join("log");
        std::fs::create_dir_all(&log).expect("log");
        let path = items(member(&row, "path_dirs"))
            .iter()
            .map(|dir| {
                scratch
                    .scenario(&text_of(dir), &log)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>()
            .join(":");
        let args: Vec<String> = items(member(&row, "args"))
            .iter()
            .map(|arg| {
                text_of(arg)
                    .replace("<CASE>", &case_text)
                    .replace("52082", &listener_port.to_string())
            })
            .collect();
        let mut command = Command::new(env!("CARGO_BIN_EXE_scan_node"));
        command
            .env_clear()
            .env("PATH", path)
            .env("LOGNAME", "tester")
            .current_dir(&case)
            .args(&args)
            .stdin(Stdio::null());
        if let PyValue::Dict(env) = member(&row, "env").peel() {
            for (key, value) in env.iter() {
                command.env(key.to_utf8().expect("key"), text_of(value));
            }
        }
        let output = command.output().expect("run scan_node");
        let first_exit = match member(&row, "first_exit").peel() {
            PyValue::Int(code) => code.to_string(),
            other => panic!("exit {}", other.repr()),
        };
        assert_eq!(
            output.status.code().map(|code| code.to_string()),
            Some(first_exit),
            "{id}: first exit"
        );
        let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
        let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
        if !host_row {
            let want_out = text_of(member(&row, "stdout")).replace("<CASE>", &case_text);
            let want_err = text_of(member(&row, "stderr")).replace("<CASE>", &case_text);
            assert_eq!(
                stdout, want_out,
                "{id}: stdout is the saved original's bytes"
            );
            assert_eq!(
                stderr, want_err,
                "{id}: stderr is the saved original's bytes"
            );
        } else {
            let tokenize = |text: &str| {
                text.replace(&case_text, "<CASE>")
                    .replace(&format!("\"{node}\""), "\"<NODE>\"")
                    .replace("\"ssh_user\": \"tester\"", "\"ssh_user\": \"<USER>\"")
                    .replace(&listener_port.to_string(), "52082")
            };
            let mut mine = parse(&tokenize(&stdout));
            let mut saved = parse(&text_of(member(&row, "stdout_tokenized")));
            host_fields(&mut mine);
            host_fields(&mut saved);
            let (mine_text, saved_text) = (render(&mine), render(&saved));
            if mine_text != saved_text {
                let at = mine_text
                    .chars()
                    .zip(saved_text.chars())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                let window = |text: &str| {
                    text.chars()
                        .skip(at.saturating_sub(120))
                        .take(300)
                        .collect::<String>()
                };
                panic!(
                    "{id}: the scan envelope differs at {at}:\n mine : {}\n saved: {}",
                    window(&mine_text),
                    window(&saved_text)
                );
            }
            assert_eq!(
                tokenize(&stderr),
                text_of(member(&row, "stderr_tokenized")),
                "{id}: stderr"
            );
        }
        covered.push(id);
    }
    drop(accept);
    if aarch64 {
        assert_eq!(
            covered.len(),
            if is_root { 12 } else { 13 },
            "covered rows: {covered:?}"
        );
    }
}

// ───────── own CLI branches the closed rows do not measure (SOURCE_DERIVED expectations, own fakes, no network) ─────────

fn own_commands(scratch: &Scratch, files: &[(&str, &str)]) -> String {
    let dir = scratch.root.join("own-bin");
    std::fs::create_dir_all(&dir).expect("dir");
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    dir.to_string_lossy().into_owned()
}

fn scanner(scratch: &Scratch, path: &str, args: &[&str]) -> (i32, String, String) {
    let case = scratch.root.join("case");
    std::fs::create_dir_all(case.join("output/single")).expect("case");
    std::fs::write(
        case.join("output/single/manifest.yaml"),
        "topology: single\nmodel_source: managed\nnas_model_path: /nonexistent\n",
    )
    .expect("manifest");
    let output = Command::new(env!("CARGO_BIN_EXE_scan_node"))
        .env_clear()
        .env("PATH", format!("{path}:/usr/bin:/bin"))
        .env("LOGNAME", "tester")
        .env("TZ", "UTC")
        .env("LC_ALL", "C.UTF-8")
        .current_dir(&case)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("utf-8"),
        String::from_utf8(output.stderr).expect("utf-8"),
    )
}

const OWN_GPU: [(&str, &str); 4] = [
    ("git", "printf 'single-node\\n'\n"),
    ("nvcc", "printf 'Cuda compilation tools, release 13.2, V\\n'\n"),
    ("nvidia-smi", "case \"$1\" in -L) printf 'GPU 0: x\\n';; *name*) printf 'NVIDIA GB10\\n';; *driver*) printf '580.95.05\\n';; *) printf '[N/A], 12.1\\n';; esac\n"),
    ("ssh", "/bin/cat >/dev/null\nprintf 'ARCH=aarch64\\nGPUS=1\\nMODEL=NVIDIA GB10\\nDRIVER=580.95.05\\nCUDA=13.2\\nGPUMEM=[N/A], 12.1\\n'\n"),
];

#[test]
fn single_with_a_peer_ssh_registers_the_sub_and_judges_it_by_observation_not_homogeneity() {
    let scratch = Scratch::new("cli-single-peer");
    let path = own_commands(&scratch, &OWN_GPU);
    let (code, stdout, _) = scanner(
        &scratch,
        &path,
        &["--topology", "single", "--peer-ssh", "user@203.0.113.11"],
    );
    assert_eq!(code, 0, "gate ok, sub observed");
    let envelope = parse(&stdout);
    assert_eq!(text_of(member(member(&envelope, "gate"), "status")), "ok");
    let nodes = items(member(&envelope, "nodes"));
    assert_eq!(nodes.len(), 2);
    let sub = &nodes[1];
    assert_eq!(
        render(sub),
        r#"{"role": "sub", "node_id": "sub", "host": "203.0.113.11", "hostname": "203.0.113.11", "ssh_user": "user"}"#
    );
    let homogeneity = member(&envelope, "homogeneity");
    assert_eq!(
        text_of(member(homogeneity, "verified_rule")),
        "single:peer-observed(gpus>=1)"
    );
    assert!(matches!(
        member(homogeneity, "verified").peel(),
        PyValue::Bool(true)
    ));
    // the key order of the envelope: gate was assigned before homogeneity in the original
    let keys: Vec<String> = match envelope.peel() {
        PyValue::Dict(map) => map
            .iter()
            .map(|(key, _)| key.to_utf8().expect("key"))
            .collect(),
        _ => unreachable!(),
    };
    let at = |name: &str| {
        keys.iter()
            .position(|key| key == name)
            .unwrap_or_else(|| panic!("{name} in {keys:?}"))
    };
    assert!(
        at("nodes") < at("consistency_assertion")
            && at("consistency_assertion") < at("gate")
            && at("gate") < at("homogeneity")
    );
}

#[test]
fn an_unobserved_sub_blocks_the_gate_with_the_originals_text_and_the_probes_diagnostic_goes_to_stderr(
) {
    let scratch = Scratch::new("cli-single-peer-down");
    let mut files = OWN_GPU.to_vec();
    files[3] = (
        "ssh",
        "printf 'Host key verification failed.\\n' >&2\nexit 255\n",
    );
    let path = own_commands(&scratch, &files);
    let (code, stdout, stderr) = scanner(
        &scratch,
        &path,
        &["--topology", "single", "--peer-ssh", "user@203.0.113.11"],
    );
    assert_eq!(code, 2);
    let envelope = parse(&stdout);
    let gate = member(&envelope, "gate");
    assert_eq!(text_of(member(gate, "status")), "blocked");
    assert!(text_of(member(gate, "note")).starts_with("서브 HW 동질성 미충족(fail-closed)"));
    let blocks = items(member(member(&envelope, "homogeneity"), "blocks"));
    assert_eq!(
        text_of(&blocks[0]),
        "서브 HW 수집 실패 또는 GPU 0 — 관측되지 않은 노드에 위임하지 않는다(fail-closed)"
    );
    assert!(
        stderr.contains("[scan] 서브 HW 프로브 실패: host-key 미등록/불일치")
            && stderr.contains("[scan]   ssh stderr: Host key verification failed."),
        "{stderr}"
    );
}

#[test]
fn the_sub_manifest_guards_run_in_the_originals_order_and_its_yaml_tail_is_a_named_boundary() {
    let scratch = Scratch::new("cli-sub-manifest");
    let path = own_commands(&scratch, &OWN_GPU);
    let out = scratch.root.join("out/sub.yaml");
    let out_text = out.to_string_lossy().into_owned();
    // without --peer-ssh: the first guard
    let (code, stdout, stderr) = scanner(
        &scratch,
        &path,
        &["--topology", "single", "--emit-sub-manifest", &out_text],
    );
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(stderr, "[scan] FAIL: --emit-sub-manifest 는 명시 topology와 --peer-ssh가 함께 있어야 한다(서브 실측이 전제).\n");
    // observed sub but no model source: the model_source guard
    let (code, _, stderr) = scanner(
        &scratch,
        &path,
        &[
            "--topology",
            "single",
            "--peer-ssh",
            "user@203.0.113.11",
            "--emit-sub-manifest",
            &out_text,
        ],
    );
    assert_eq!(code, 2);
    assert!(
        stderr.starts_with("[scan] FAIL: 서브 manifest 에 model_source 가 필요하다"),
        "{stderr}"
    );
    // all guards passed: the PyYAML/contract tail is not modelled - named, status 3, nothing written
    let (code, stdout, stderr) = scanner(
        &scratch,
        &path,
        &[
            "--topology",
            "single",
            "--peer-ssh",
            "user@203.0.113.11",
            "--model-source",
            "managed",
            "--emit-sub-manifest",
            &out_text,
        ],
    );
    assert_eq!((code, stdout.as_str()), (3, ""));
    assert!(
        stderr.contains("--emit-sub-manifest writes the sub manifest with PyYAML")
            && stderr.contains("nothing was written"),
        "{stderr}"
    );
    assert!(!out.exists(), "the boundary writes nothing");
}
