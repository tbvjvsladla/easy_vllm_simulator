//! P2 of `scan_node` (`role_contract::scan_node_probe`) over a SCRIPTED host: every outside effect is a declared literal of this check
//! (commands, `/sys` entries, sockets, resolver answers), so the decisions - order of probes, parsing, folding of failures, key order of
//! the facts - are exercised without a machine. Nothing here is measured on hardware; the real host is covered by the `sweep-stop`
//! checks (own fake commands, own loopback listener). Expected values follow the original's source text and the closed P23 rows.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use role_contract::scan_node::Failure;
use role_contract::scan_node_probe::{self as probe, Capture, Host};
use std::cell::RefCell;
use std::collections::HashMap;

type Script = Box<dyn Fn(&[Vec<u8>], Option<&[u8]>) -> Capture>;

#[derive(Default)]
struct Scripted {
    commands: HashMap<String, Script>,
    sys: HashMap<Vec<u8>, Vec<u8>>,
    dirs: HashMap<Vec<u8>, Vec<Vec<u8>>>,
    resolver: HashMap<String, Vec<String>>,
    machine: String,
    started: RefCell<Vec<String>>,
    stderr: RefCell<Vec<String>>,
    errors: RefCell<Vec<(String, String)>>,
}

fn out(code: i32, stdout: &str) -> Capture {
    Capture::Exited {
        code,
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

impl Scripted {
    fn command(
        mut self,
        name: &str,
        script: impl Fn(&[Vec<u8>], Option<&[u8]>) -> Capture + 'static,
    ) -> Self {
        self.commands.insert(name.to_string(), Box::new(script));
        self
    }
}

impl Host for Scripted {
    fn which(&self, program: &[u8]) -> bool {
        self.commands
            .contains_key(String::from_utf8_lossy(program).as_ref())
    }
    fn capture(&self, argv: &[Vec<u8>], stdin: Option<&[u8]>, _timeout: Option<f64>) -> Capture {
        let words: Vec<String> = argv
            .iter()
            .map(|word| String::from_utf8_lossy(word).into_owned())
            .collect();
        self.started.borrow_mut().push(words.join(" "));
        match self.commands.get(&words[0]) {
            Some(script) => script(argv, stdin),
            None => Capture::Failed {
                class: "FileNotFoundError".into(),
                message: format!("[Errno 2] No such file or directory: '{}'", words[0]),
            },
        }
    }
    fn list_dir(&self, path: &[u8]) -> Option<Vec<Vec<u8>>> {
        self.dirs.get(path).cloned()
    }
    fn read_file(&self, path: &[u8]) -> Option<Vec<u8>> {
        self.sys.get(path).cloned()
    }
    fn tcp_connect(&self, _host: &str, _port: u16, _timeout: Option<f64>) -> bool {
        false
    }
    fn resolve(&self, name: &str) -> Option<Vec<String>> {
        self.resolver.get(name).cloned()
    }
    fn https_head_ok(&self, _authority: &str, _timeout: Option<f64>) -> bool {
        false
    }
    fn machine(&self) -> String {
        self.machine.clone()
    }
    fn stderr_line(&self, text: &PyStr) {
        self.stderr
            .borrow_mut()
            .push(text.to_utf8().expect("utf-8"));
    }
    fn peer_error(&self, what: &str, cause: &PyStr) {
        self.errors
            .borrow_mut()
            .push((what.into(), cause.to_utf8().expect("utf-8")));
    }
}

fn shown(value: &PyValue) -> String {
    value
        .dumps_unicode()
        .expect("dump")
        .to_utf8()
        .expect("utf-8")
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

const GIDS: &str = "DEV\tPORT\tINDEX\tGID\t\t\tIPv4\t\tVER\tDEV\n---\t----\t-----\t---\nrocep1s0f1\t1\t0\tfe80::1\t\t\t\t\tv1\tenp1s0f1np1\nroceP2p1s0f1\t1\t3\t0000:ffff:cb00:710b\t203.0.113.20\tv2\tenP2p1s0f1np1\nrocep1s0f1\t1\t3\t0000:ffff:cb00:710a\t203.0.113.10\tv2\tenp1s0f1np1\nrocep1s0f1\t1\t1\tfe80::1\t\t\t\t\t\tv2\tenp1s0f1np1\n";

fn roce_host(table: &'static str) -> Scripted {
    let mut host = Scripted::default().command("show_gids", move |_, _| out(0, table));
    host.dirs.insert(
        b"/sys/class/infiniband".to_vec(),
        vec![
            b"rocep1s0f1".to_vec(),
            b"roceP2p1s0f0".to_vec(),
            b"rocep1s0f0".to_vec(),
        ],
    );
    host.sys.insert(
        b"/sys/class/net/enp1s0f1np1/mtu".to_vec(),
        b" 9000\n".to_vec(),
    );
    host
}

#[test]
fn interconnect_orders_by_lowest_ipv4_dedups_hcas_and_reads_the_bootstrap_interface_mtu() {
    let host = roce_host(GIDS);
    let found = probe::detect_interconnect(&host).expect("detect");
    assert_eq!(
        shown(&found),
        r#"{"type": "RoCE v2", "hca_devices": ["rocep1s0f1", "roceP2p1s0f1"], "gid_index": 3, "socket_iface": "enp1s0f1np1", "mtu": 9000, "bandwidth_gbps": null, "platform_preset": null, "_sysfs_hcas": ["roceP2p1s0f0", "rocep1s0f0", "rocep1s0f1"], "_roce_v2_gids": [{"hca": "roceP2p1s0f1", "port": "1", "gid_index": 3, "ipv4": "203.0.113.20", "iface": "enP2p1s0f1np1"}, {"hca": "rocep1s0f1", "port": "1", "gid_index": 3, "ipv4": "203.0.113.10", "iface": "enp1s0f1np1"}]}"#
    );
}

#[test]
fn interconnect_gid_index_is_single_sorted_list_or_invalid_and_without_v2_rows_it_is_generic_ethernet(
) {
    let mixed = "a 1 5 g 203.0.113.10 v2 e1\nb 1 3 g 203.0.113.20 v2 e2\n";
    let found = probe::detect_interconnect(&roce_host(mixed)).expect("detect");
    assert!(
        shown(&found).contains(r#""gid_index": [3, 5]"#),
        "{}",
        shown(&found)
    );
    let bad = "a 1 x g 203.0.113.10 v2 e1\nb 1 3 g 203.0.113.20 v2 e2\n";
    assert!(
        shown(&probe::detect_interconnect(&roce_host(bad)).expect("detect"))
            .contains(r#""gid_index": "invalid""#)
    );
    let none =
        roce_host("DEV PORT INDEX GID IPv4 VER DEV\nrocep1s0f1 1 0 fe80::1 v1 enp1s0f1np1\n");
    assert_eq!(
        shown(&probe::detect_interconnect(&none).expect("detect")),
        r#"{"type": "generic-ethernet", "hca_devices": [], "gid_index": null, "socket_iface": null, "mtu": null, "bandwidth_gbps": null, "platform_preset": null, "_sysfs_hcas": ["roceP2p1s0f0", "rocep1s0f0", "rocep1s0f1"], "_roce_v2_gids": []}"#
    );
}

#[test]
fn mtu_is_none_when_unreadable_undecodable_or_not_a_number() {
    for (content, expected) in [
        (Some(b"9000\n".to_vec()), "9000"),
        (Some(b"jumbo\n".to_vec()), "null"),
        (Some(vec![0xff, 0xfe]), "null"),
        (None, "null"),
    ] {
        let mut host = roce_host(GIDS);
        match content {
            Some(bytes) => host
                .sys
                .insert(b"/sys/class/net/enp1s0f1np1/mtu".to_vec(), bytes),
            None => host
                .sys
                .remove(b"/sys/class/net/enp1s0f1np1/mtu".as_slice()),
        };
        let found = probe::detect_interconnect(&host).expect("detect");
        assert!(
            shown(&found).contains(&format!(r#""mtu": {expected}"#)),
            "{expected}: {}",
            shown(&found)
        );
    }
}

fn gpu_host() -> Scripted {
    let mut host = Scripted::default()
        .command("nvcc", |_, _| out(0, "nvcc: NVIDIA (R) Cuda compiler driver\nCuda compilation tools, release 13.2, V13.2.51\n"))
        .command("nvidia-smi", |argv, _| {
            let query = String::from_utf8_lossy(&argv[1]).into_owned();
            if query == "-L" {
                return out(0, "GPU 0: NVIDIA GB10 (UUID: GPU-x)\n");
            }
            if query.contains("name") {
                return out(0, "NVIDIA GB10\n");
            }
            if query.contains("driver_version") {
                return out(0, "580.95.05\n");
            }
            out(0, "[N/A], 12.1\n")
        });
    host.machine = "aarch64".into();
    host
}

#[test]
fn local_hw_runs_nvcc_first_then_the_dictionary_in_key_order_with_the_originals_facts() {
    let host = gpu_host();
    let hw = probe::collect_local_hw(&host);
    assert_eq!(
        shown(&hw),
        r#"{"cpu_arch": "aarch64", "gpu_model": "NVIDIA GB10", "gpus_per_node": 1, "cuda": "13.2", "driver": "580.95.05"}"#
    );
    let order: Vec<String> = host
        .started
        .borrow()
        .iter()
        .map(|line| {
            line.split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(
        order,
        [
            "nvcc --version",
            "nvidia-smi --query-gpu=name",
            "nvidia-smi -L",
            "nvidia-smi --query-gpu=driver_version"
        ]
    );
    assert_eq!(shown(&probe::scan_cuda_version(&host)), r#""132""#);
    assert_eq!(
        shown(&probe::scan_gpu_memory(&host).expect("memory")),
        r#"{"gpu_memory_model": "unified", "gpu_vram_mib": null, "compute_capability": "12.1"}"#
    );
}

#[test]
fn missing_failing_or_undecodable_tools_fold_to_none_like_the_original() {
    let none = Scripted::default();
    assert_eq!(shown(&probe::scan_cuda_version(&none)), "null");
    assert_eq!(shown(&probe::scan_gpus_per_node(&none)), "null");
    assert_eq!(shown(&probe::scan_gpu_model(&none)), "null");
    let broken = Scripted::default()
        .command("nvidia-smi", |_, _| out(9, "GPU 0: x\n"))
        .command("nvcc", |_, _| Capture::Exited {
            code: 0,
            stdout: vec![0xff, 0xfe],
            stderr: vec![],
        });
    assert_eq!(
        shown(&probe::scan_gpus_per_node(&broken)),
        "null",
        "non-zero exit folds to empty"
    );
    assert_eq!(
        shown(&probe::scan_cuda_version(&broken)),
        "null",
        "undecodable output folds to empty"
    );
    let two = Scripted::default().command("nvidia-smi", |_, _| {
        out(0, "  GPU 0: a\nnoise\n\tGPU 1: b\nGPU2: no space\n")
    });
    assert_eq!(
        shown(&probe::scan_gpus_per_node(&two)),
        "2",
        "lines are stripped, `GPU ` with the space"
    );
}

#[test]
fn cuda_regex_is_unicode_and_leftmost() {
    let host = Scripted::default().command("nvcc", |_, _| {
        out(0, "release x\nrelease\u{2003}١٣.٢ then release 9.9\n")
    });
    assert_eq!(
        shown(&probe::scan_cuda_version(&host)),
        "\"١٣٢\"",
        "\\s and \\d are Unicode and the digits keep their characters"
    );
}

#[test]
fn iface_address_needs_a_word_boundary_and_a_zero_exit() {
    let line = |text: &'static str| Scripted::default().command("ip", move |_, _| out(0, text));
    let ok = line("2: enp1s0f1np1    inet 203.0.113.10/24 brd 203.0.113.255\n");
    assert_eq!(
        shown(&probe::iface_ipv4(&ok, &s("enp1s0f1np1")).expect("ip")),
        r#""203.0.113.10""#
    );
    assert_eq!(
        shown(&probe::iface_ipv4(&line("2: x    xinet 1.2.3.4/24\n"), &s("x")).expect("ip")),
        "null",
        "\\binet"
    );
    assert_eq!(
        shown(
            &probe::iface_ipv4(
                &Scripted::default().command("ip", |_, _| out(1, "inet 1.2.3.4/24")),
                &s("x")
            )
            .expect("ip")
        ),
        "null"
    );
    assert_eq!(
        shown(&probe::iface_ipv4(&Scripted::default(), &s("x")).expect("ip")),
        "null",
        "ip absent: OSError folded"
    );
    assert_eq!(
        shown(&probe::iface_ipv4(&Scripted::default(), &PyValue::None).expect("ip")),
        "null"
    );
    assert_eq!(
        shown(&probe::iface_ipv4(&Scripted::default(), &s("")).expect("ip")),
        "null"
    );
}

fn ssh_host(script: impl Fn(&[Vec<u8>], Option<&[u8]>) -> Capture + 'static) -> Scripted {
    Scripted::default().command("ssh", script)
}

#[test]
fn peer_hw_parses_key_values_and_folds_every_failure_to_the_empty_dictionary_with_a_diagnostic() {
    let reply = "ARCH=aarch64\nGPUS=1\nMODEL=NVIDIA GB10\nDRIVER=580.95.05\nCUDA=\nGPUMEM=[N/A], 12.1\nJUNK\nK=a=b\n";
    let host = ssh_host(move |argv, stdin| {
        assert_eq!(
            argv.iter()
                .map(|word| String::from_utf8_lossy(word).into_owned())
                .collect::<Vec<_>>(),
            [
                "ssh",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=8",
                "user@h",
                "bash",
                "-ls"
            ]
        );
        assert_eq!(stdin, Some(probe::PEER_HW_PROBE.as_bytes()));
        out(0, reply)
    });
    let hw = probe::collect_peer_hw(&host, &s("user@h"), &PyValue::None).expect("hw");
    assert_eq!(
        shown(&hw),
        r#"{"cpu_arch": "aarch64", "gpu_model": "NVIDIA GB10", "gpus_per_node": 1, "cuda": null, "driver": "580.95.05", "gpu_memory_model": "unified", "gpu_vram_mib": null, "compute_capability": "12.1"}"#
    );
    assert!(host.stderr.borrow().is_empty());
    let timed = ssh_host(|_, _| Capture::TimedOut);
    assert_eq!(
        shown(&probe::collect_peer_hw(&timed, &s("user@h"), &PyValue::None).expect("hw")),
        "{}"
    );
    assert_eq!(
        timed.errors.borrow().clone(),
        [("HW".to_string(), "네트워크 미도달/타임아웃".to_string())]
    );
    assert_eq!(timed.stderr.borrow()[1], "[scan]   ssh stderr: TimeoutExpired: Command '['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=8', 'user@h', 'bash', '-ls']' timed out after 25 seconds");
    let empty = ssh_host(|_, _| out(0, "  \n"));
    assert_eq!(
        shown(&probe::collect_peer_hw(&empty, &s("user@h"), &PyValue::None).expect("hw")),
        "{}"
    );
    assert!(
        empty.stderr.borrow().is_empty(),
        "an empty answer is folded without a diagnostic"
    );
    let undecodable = ssh_host(|_, _| Capture::Exited {
        code: 0,
        stdout: vec![0xff],
        stderr: vec![],
    });
    probe::collect_peer_hw(&undecodable, &s("user@h"), &PyValue::None).expect("hw");
    assert!(undecodable.stderr.borrow()[1].contains("UnicodeDecodeError: 'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"));
    let custom = ssh_host(|argv, _| {
        assert_eq!(String::from_utf8_lossy(&argv[1]), "-i");
        out(0, "ARCH=x\nGPUS=²\n")
    });
    let odd = probe::collect_peer_hw(&custom, &s("u@h"), &PyValue::List(vec![s("-i"), s("k")]));
    assert!(
        matches!(odd, Err(Failure::Unsupported(_))),
        "str.isdigit() of a non-decimal numeric character is not modelled (as in P1)"
    );
}

#[test]
fn peer_egress_and_model_env_follow_the_originals_probe_text_and_folding() {
    let host = ssh_host(|argv, stdin| {
        assert_eq!(String::from_utf8_lossy(&argv[argv.len() - 1]), "-ls");
        let probe = String::from_utf8_lossy(stdin.expect("input")).into_owned();
        assert_eq!(probe, "(curl -sS -o /dev/null -w '%{http_code}' --max-time 5 https://huggingface.co 2>/dev/null || echo 000); echo; (timeout 5 bash -c '</dev/tcp/8.8.8.8/443' 2>/dev/null && echo OK || echo FAIL)");
        out(0, "  200 \n\nOK\n")
    });
    let egress = probe::collect_peer_egress(
        &host,
        &s("u@h"),
        &s("huggingface.co"),
        &s("8.8.8.8"),
        &PyValue::int(443),
        &PyValue::int(5),
        &PyValue::None,
    )
    .expect("egress");
    assert_eq!(
        shown(&egress),
        r#"{"egress": "online", "hf_reachable": true, "gateway_reachable": true}"#
    );
    for (reply, expected) in [
        ("499\nOK\n", true),
        ("500\nOK\n", false),
        ("199\nOK\n", false),
        ("200\nFAIL\n", false),
        ("abc\nOK\n", false),
        ("200\n", false),
    ] {
        let host = ssh_host(move |_, _| out(0, reply));
        let found = probe::collect_peer_egress(
            &host,
            &s("u@h"),
            &s("huggingface.co"),
            &s("8.8.8.8"),
            &PyValue::int(443),
            &PyValue::int(5),
            &PyValue::None,
        )
        .expect("egress");
        assert_eq!(
            shown(&found).contains(r#""egress": "online""#),
            expected,
            "{reply:?}"
        );
    }
    let env = ssh_host(|_, stdin| {
        assert_eq!(String::from_utf8_lossy(stdin.expect("input")), "[ -d /nas/models ] && echo NAS=1 || echo NAS=0\ngrep -Eq \"^[[:space:]]*HF_TOKEN=.*[^[:space:]\\\"']\" '/h/w s'/envs/.env 2>/dev/null && echo TOK=1 || echo TOK=0\n");
        out(0, "NAS=0\nTOK=1\nother\n")
    });
    let found = probe::collect_peer_model_env(
        &env,
        &s("u@h"),
        &s("/nas/models"),
        &s("/h/w s"),
        &PyValue::None,
    )
    .expect("env");
    assert_eq!(
        shown(&found),
        r#"{"nas_reachable": false, "hf_token_present": true, "probed": true, "source": "measured:peer-ssh"}"#
    );
    let nothing = probe::collect_peer_model_env(
        &Scripted::default(),
        &s("u@h"),
        &PyValue::None,
        &s(""),
        &PyValue::None,
    )
    .expect("env");
    assert_eq!(
        shown(&nothing),
        r#"{"nas_reachable": null, "hf_token_present": null, "probed": false}"#
    );
}

#[test]
fn loopback_resolution_takes_the_first_loopback_address_in_resolver_order() {
    let mut host = Scripted::default();
    host.resolver.insert(
        "name".into(),
        vec!["203.0.113.1".into(), "127.0.1.1".into(), "::1".into()],
    );
    host.resolver
        .insert("v6".into(), vec!["2001:db8::1".into(), "::1".into()]);
    host.resolver.insert(
        "far".into(),
        vec!["203.0.113.1".into(), "::ffff:127.0.0.1".into()],
    );
    assert_eq!(
        shown(&probe::resolves_to_loopback(&host, &s("name")).expect("r")),
        r#""127.0.1.1""#
    );
    assert_eq!(
        shown(&probe::resolves_to_loopback(&host, &s("v6")).expect("r")),
        r#""::1""#
    );
    assert_eq!(
        shown(&probe::resolves_to_loopback(&host, &s("far")).expect("r")),
        "null"
    );
    assert_eq!(
        shown(&probe::resolves_to_loopback(&host, &s("unknown")).expect("r")),
        "null",
        "resolver failure is folded"
    );
    assert_eq!(
        shown(&probe::resolves_to_loopback(&host, &s("")).expect("r")),
        "null"
    );
    assert!(matches!(
        probe::resolves_to_loopback(&host, &s("bad\0host")),
        Err(Failure::Unsupported(_))
    ));
}

#[test]
fn git_branch_and_the_run_helper_follow_the_originals_empty_string_contract() {
    let host = Scripted::default().command("git", |_, _| out(0, "  single-node \n"));
    assert_eq!(shown(&probe::git_branch(&host)), r#""single-node""#);
    assert_eq!(shown(&probe::git_branch(&Scripted::default())), "null");
    let empty = probe::run(&Scripted::default(), &s("nvcc"), &PyValue::int(10)).expect("run");
    assert_eq!(shown(&empty), r#""""#, "a str command is looked up by its FIRST CHARACTER (cmd[0]) exactly as the closed rows S50K01-05 measured");
}

#[test]
fn an_undecodable_ip_stream_is_unsupported_whatever_the_return_code_never_a_silent_none() {
    // `subprocess.run(text=True)` decodes stdout AND stderr before the return code is read: UnicodeDecodeError, not `None`.
    let bad_stderr_nonzero = Scripted::default().command("ip", |_, _| Capture::Exited {
        code: 1,
        stdout: Vec::new(),
        stderr: vec![0xff, 0xfe],
    });
    let bad_stdout_nonzero = Scripted::default().command("ip", |_, _| Capture::Exited {
        code: 2,
        stdout: vec![0xc3, 0x28],
        stderr: Vec::new(),
    });
    let bad_stderr_zero = Scripted::default().command("ip", |_, _| Capture::Exited {
        code: 0,
        stdout: b"2: x    inet 192.0.2.4/24\n".to_vec(),
        stderr: vec![0xff],
    });
    for (name, host) in [
        ("stderr, rc 1", bad_stderr_nonzero),
        ("stdout, rc 2", bad_stdout_nonzero),
        ("stderr, rc 0", bad_stderr_zero),
    ] {
        match probe::iface_ipv4(&host, &s("x")) {
            Err(Failure::Unsupported(reason)) => {
                assert!(reason.contains("UnicodeDecodeError"), "{name}: {reason}");
            }
            other => panic!(
                "{name}: expected an explicit unsupported boundary, got {:?}",
                other.map(|v| shown(&v))
            ),
        }
    }
    // a clean non-zero exit is still the original's `None`
    let clean_failure = Scripted::default().command("ip", |_, _| out(1, ""));
    assert_eq!(
        shown(&probe::iface_ipv4(&clean_failure, &s("x")).expect("ip")),
        "null"
    );
}
