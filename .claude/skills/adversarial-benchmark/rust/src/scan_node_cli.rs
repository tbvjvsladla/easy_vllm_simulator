//! Command line of `scan_node` (P3): `main()` of the original `scan_node.py` over the P1 business (`role_contract::scan_node*`) and
//! the P2 probes (`role_contract::scan_node_probe`), with the original's argparse interface ([`SCAN_NODE`]: usage, help and error text
//! byte-for-byte, the logical program name `scan_node.py`) and the original's order of effects: the argument parse, the
//! `--emit-manifest` interview gate, the interconnect scan, the local scan envelope, the node entries, the peer check, the
//! three-way assertion inputs, the egress axis, the gate, the sub-host homogeneity, the sub-manifest and manifest emission, the JSON
//! envelope and the emission block.
//!
//! **Boundaries (visible, never retried in Python).**
//! * `--self-test` is the original's own regression harness (it needs the original source): it is refused here with status 3 and a
//!   named message; `scan_node.py` runs it through the exact original owner (route `CompatOriginalCli`).
//! * The tail of `--emit-sub-manifest` after its three guards needs PyYAML (`yaml.safe_load`/`safe_dump` of the main and the sub
//!   manifest) and `manifest_contract.evaluate_contract`: not modelled; it ends with status 3 and a named message after the same
//!   guards the original applies first.
//! * An uncaught Python exception ends with the traceback marker and the final `Class: message` line only (frames are not
//!   reproduced); a shape the port does not carry (`Failure::Unsupported`) ends with status 3 and its reason.
//!
//! **Effects.** The probes are real (`scan_node_host`); in this module the host-local facts (`platform.node()`,
//! `getpass.getuser()`, `os.getcwd()`, `os.path.isdir/isfile`) come from [`Local`], so an own check can hand in fixed values.

use crate::argv::{parse_with, ActionDef, Conversion, Kind, Outcome, Spec};
use crate::cli::{on_large_stack, write_stderr, Streams, TRACEBACK_HEADER};
use crate::pystr::PyStr as BStr;
use crate::terminal;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use role_contract::scan_node::{self as scan, Failure, GateArgs, Res};
use role_contract::scan_node_files::{self as files, Files};
use role_contract::scan_node_manifest::{self as manifest, Clock};
use role_contract::scan_node_probe::{self as probe, Host};
use std::cell::RefCell;

/// Exit status of a boundary the port does not carry (the same status the bridge uses for a control failure).
pub const EXIT_BOUNDARY: i32 = 3;

pub mod args {
    pub const TOPOLOGY: usize = 1;
    pub const PEER_IP: usize = 2;
    pub const PEER_PORT: usize = 3;
    pub const PEER_SSH: usize = 4;
    pub const COMPOSE: usize = 5;
    pub const ENV_INTERCONNECT: usize = 6;
    pub const BANDWIDTH_GBPS: usize = 7;
    pub const PER_PORT_GBPS: usize = 8;
    pub const PER_PORT_FLOOR: usize = 9;
    pub const BW_FLOOR: usize = 10;
    pub const EMIT_MANIFEST: usize = 11;
    pub const MANIFEST: usize = 12;
    pub const CHECK_EGRESS: usize = 13;
    pub const SUB_WORK_DIR: usize = 14;
    pub const MODEL_SOURCE: usize = 15;
    pub const EMIT_SUB_MANIFEST: usize = 16;
    pub const SELF_TEST: usize = 17;
}

const fn action(
    name: &'static str,
    kind: Kind,
    conversion: Conversion,
    choices: &'static [&'static str],
) -> ActionDef {
    ActionDef {
        name,
        kind,
        required: false,
        conversion,
        choices,
    }
}

/// The original `scan_node.py` parser (registration order of `main()`).
pub const SCAN_NODE: Spec = Spec::with_layouts(
    "scan_node.py",
    &[
        action("-h/--help", Kind::Help, Conversion::Text, &[]),
        action(
            "--topology",
            Kind::Value,
            Conversion::Text,
            &["single", "multi", "auto"],
        ),
        action("--peer-ip", Kind::Value, Conversion::Text, &[]),
        action("--peer-port", Kind::Value, Conversion::Int, &[]),
        action("--peer-ssh", Kind::Value, Conversion::Text, &[]),
        action("--compose", Kind::Value, Conversion::Text, &[]),
        action("--env-interconnect", Kind::Value, Conversion::Text, &[]),
        action("--bandwidth-gbps", Kind::Value, Conversion::Float, &[]),
        action("--per-port-gbps", Kind::Value, Conversion::Float, &[]),
        action("--per-port-floor", Kind::Value, Conversion::Float, &[]),
        action("--bw-floor", Kind::Value, Conversion::Float, &[]),
        action("--emit-manifest", Kind::Flag, Conversion::Text, &[]),
        action("--manifest", Kind::Value, Conversion::Text, &[]),
        action("--check-egress", Kind::Flag, Conversion::Text, &[]),
        action("--sub-work-dir", Kind::Value, Conversion::Text, &[]),
        action(
            "--model-source",
            Kind::Value,
            Conversion::Text,
            &["managed", "ephemeral", "custom"],
        ),
        action("--emit-sub-manifest", Kind::Value, Conversion::Text, &[]),
        action("--self-test", Kind::Flag, Conversion::Text, &[]),
    ],
    &[
        ("-h", 0),
        ("--help", 0),
        ("--topology", args::TOPOLOGY),
        ("--peer-ip", args::PEER_IP),
        ("--peer-port", args::PEER_PORT),
        ("--peer-ssh", args::PEER_SSH),
        ("--compose", args::COMPOSE),
        ("--env-interconnect", args::ENV_INTERCONNECT),
        ("--bandwidth-gbps", args::BANDWIDTH_GBPS),
        ("--per-port-gbps", args::PER_PORT_GBPS),
        ("--per-port-floor", args::PER_PORT_FLOOR),
        ("--bw-floor", args::BW_FLOOR),
        ("--emit-manifest", args::EMIT_MANIFEST),
        ("--manifest", args::MANIFEST),
        ("--check-egress", args::CHECK_EGRESS),
        ("--sub-work-dir", args::SUB_WORK_DIR),
        ("--model-source", args::MODEL_SOURCE),
        ("--emit-sub-manifest", args::EMIT_SUB_MANIFEST),
        ("--self-test", args::SELF_TEST),
    ],
    crate::help_layouts_scan_node::LAYOUTS,
);

/// The facts of this machine and its file system that the original read with `platform`, `getpass`, `os`.
pub trait Local {
    /// `platform.node()` (`""` when unknown).
    fn node(&self) -> PyStr;
    /// `getpass.getuser()`: an uncaught failure of the original, so a failure is a typed result (never `None`/JSON null).
    fn user(&self) -> Res<PyStr>;
    /// `os.getcwd()`: raises `FileNotFoundError`/`PermissionError`/`OSError` in the original; a failure is a typed result.
    fn cwd(&self) -> Res<PyStr>;
    fn is_dir(&self, path: &PyStr) -> bool;
    fn is_file(&self, path: &PyStr) -> bool;
}

/// An OS failure as the exception class and `str()` text CPython gives it (`[Errno N] strerror`); frames/`args` tuple are not reproduced.
fn os_error(error: &std::io::Error) -> scan::Raised {
    let (class, text) = match error.raw_os_error() {
        Some(code) => {
            let full = error.to_string();
            let strerror = full
                .strip_suffix(&format!(" (os error {code})"))
                .unwrap_or(&full)
                .to_owned();
            let class = match code {
                2 => "FileNotFoundError",
                1 | 13 => "PermissionError",
                _ => "OSError",
            };
            (class, format!("[Errno {code}] {strerror}"))
        }
        None => ("OSError", error.to_string()),
    };
    scan::Raised::with_message(class, PyStr::new(&text))
}

/// `getpass.getuser()` over its inputs (the environment, the passwd text, the uid) so that every branch is testable without touching the
/// machine. The first non-empty of `LOGNAME`/`USER`/`LNAME`/`USERNAME` wins; then the original asks the passwd database through
/// `pwd.getpwuid` (NSS, which may know users /etc/passwd does not) and lets its `KeyError` escape. This port reads /etc/passwd only, so a
/// miss is an explicit unsupported boundary (a `KeyError` is never invented and the failure is never folded into a normal envelope).
pub fn user_from(
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
    passwd: impl FnOnce() -> std::io::Result<String>,
    uid: u32,
) -> Res<PyStr> {
    use std::os::unix::ffi::OsStrExt;
    for name in ["LOGNAME", "USER", "LNAME", "USERNAME"] {
        if let Some(value) = env(name).filter(|value| !value.is_empty()) {
            return Ok(PyStr::from_surrogateescape(value.as_bytes()));
        }
    }
    let table = passwd().map_err(|error| {
        scan::unsupported(&format!(
            "getpass.getuser(): none of LOGNAME/USER/LNAME/USERNAME is set and /etc/passwd cannot be read ({error}); the passwd database lookup of the original is not modelled"
        ))
    })?;
    table
        .lines()
        .find_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            (fields.len() > 2 && fields[2].parse::<u32>().ok() == Some(uid))
                .then(|| PyStr::new(fields[0]))
        })
        .ok_or_else(|| {
            scan::unsupported(&format!(
                "getpass.getuser(): none of LOGNAME/USER/LNAME/USERNAME is set and /etc/passwd has no entry for uid {uid}; the original asks the passwd database (NSS), which is not modelled here"
            ))
        })
}

/// The real machine.
pub struct StdLocal;

impl Local for StdLocal {
    fn node(&self) -> PyStr {
        PyStr::new(&rustix::system::uname().nodename().to_string_lossy())
    }

    fn user(&self) -> Res<PyStr> {
        user_from(
            |name| std::env::var_os(name),
            || std::fs::read_to_string("/etc/passwd"),
            rustix::process::getuid().as_raw(),
        )
    }

    fn cwd(&self) -> Res<PyStr> {
        use std::os::unix::ffi::OsStrExt;
        match std::env::current_dir() {
            Ok(dir) => Ok(PyStr::from_surrogateescape(dir.as_os_str().as_bytes())),
            Err(error) => Err(Failure::Raised(os_error(&error))),
        }
    }

    fn is_dir(&self, path: &PyStr) -> bool {
        path.to_surrogateescape_bytes().is_ok_and(|bytes| {
            use std::os::unix::ffi::OsStrExt;
            std::fs::metadata(std::ffi::OsStr::from_bytes(&bytes)).is_ok_and(|meta| meta.is_dir())
        })
    }

    fn is_file(&self, path: &PyStr) -> bool {
        path.to_surrogateescape_bytes().is_ok_and(|bytes| {
            use std::os::unix::ffi::OsStrExt;
            std::fs::metadata(std::ffi::OsStr::from_bytes(&bytes)).is_ok_and(|meta| meta.is_file())
        })
    }
}

/// Everything the program reads from outside, injectable for an own check.
pub struct World<'a> {
    pub host: &'a (dyn Host + Sync),
    pub files: &'a (dyn Files + Sync),
    pub clock: &'a (dyn Clock + Sync),
    pub local: &'a (dyn Local + Sync),
}

/// This crate's own string type (what the argparse port and the streams use) <-> the common package's (what the business uses).
fn q(text: &BStr) -> PyStr {
    PyStr::from_code_points(text.code_points().to_vec())
}

fn b(text: &PyStr) -> BStr {
    BStr::from_code_points(text.code_points().to_vec())
}

/// The result of the heavy part: the text of the two streams and the status.
struct Finished {
    stdout: Vec<PyStr>,
    stderr: PyStr,
    exit_code: i32,
}

/// A `Host` seen through the program's own stderr: the diagnostic lines of the probes join the program's stderr text in order.
struct Tee<'a> {
    inner: &'a (dyn Host + Sync),
    stderr: &'a RefCell<PyStr>,
}

impl Host for Tee<'_> {
    fn which(&self, program: &[u8]) -> bool {
        self.inner.which(program)
    }
    fn capture(
        &self,
        argv: &[Vec<u8>],
        stdin: Option<&[u8]>,
        timeout: Option<f64>,
    ) -> probe::Capture {
        self.inner.capture(argv, stdin, timeout)
    }
    fn list_dir(&self, path: &[u8]) -> Option<Vec<Vec<u8>>> {
        self.inner.list_dir(path)
    }
    fn read_file(&self, path: &[u8]) -> Option<Vec<u8>> {
        self.inner.read_file(path)
    }
    fn tcp_connect(&self, host: &str, port: u16, timeout: Option<f64>) -> bool {
        self.inner.tcp_connect(host, port, timeout)
    }
    fn resolve(&self, name: &str) -> Option<Vec<String>> {
        self.inner.resolve(name)
    }
    fn https_head_ok(&self, authority: &str, timeout: Option<f64>) -> bool {
        self.inner.https_head_ok(authority, timeout)
    }
    fn machine(&self) -> String {
        self.inner.machine()
    }
    fn stderr_line(&self, text: &PyStr) {
        let mut sink = self.stderr.borrow_mut();
        sink.push_py(text);
        sink.push_str("\n");
    }
    fn peer_error(&self, _what: &str, _cause: &PyStr) {}
}

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn opt_text(value: Option<&PyStr>) -> PyValue {
    value.map_or(PyValue::None, |own| PyValue::Str(own.clone()))
}

fn dict(entries: Vec<(&str, PyValue)>) -> PyDict {
    let mut out = PyDict::new();
    for (key, value) in entries {
        out.set(key, value);
    }
    out
}

fn get(value: &PyValue, key: &str) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

/// How the program ends on a `Failure`: the original's traceback ending, or the named boundary.
fn ends_with(failure: &Failure, stderr: &RefCell<PyStr>) -> i32 {
    let mut sink = stderr.borrow_mut();
    match failure {
        Failure::Raised(raised) => {
            sink.push_str(TRACEBACK_HEADER);
            sink.push_str(raised.class);
            sink.push_str(": ");
            sink.push_py(&raised.message);
            sink.push_str("\n");
            1
        }
        Failure::Unsupported(reason) => {
            sink.push_str(&format!(
                "[scan_node] ERROR unsupported shape (not modelled in this phase): {reason}\n"
            ));
            EXIT_BOUNDARY
        }
    }
}

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(arguments: &[BStr], io: &mut Streams<'_>, world: &World<'_>) -> i32 {
    let matches = match parse_with(&SCAN_NODE, arguments) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(SCAN_NODE.help(terminal::columns()));
            return io.finish(0);
        }
        Outcome::Error(message) => {
            let mut text = BStr::new(&format!(
                "{}{}: error: ",
                SCAN_NODE.usage(terminal::columns()),
                SCAN_NODE.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return io.finish(2);
        }
    };
    if matches.flag(args::SELF_TEST) {
        write_stderr(
            io,
            &BStr::new("[scan_node] ERROR --self-test is the original's own regression harness and is not native in this phase (scan_node.py runs it through the exact original owner)\n"),
        );
        return io.finish(EXIT_BOUNDARY);
    }
    let finished = {
        let host: &(dyn Host + Sync) = world.host;
        let parsed = Parsed::of(&matches);
        let (files, clock, local) = (world.files, world.clock, world.local);
        on_large_stack(move || process(&parsed, host, files, clock, local))
    };
    // The original writes every diagnostic (stderr) while it works and only then, at the very end, its output (stdout); no stderr text follows the
    // first stdout write (checked on the original main). Here both are collected, so the order of the original is restored by writing the
    // stderr text FIRST: on a terminal, a merged pipe and a large output that overruns the stdout block buffer alike. A failing `print` then
    // ends the program after the diagnostics, exactly as the original's traceback follows them.
    write_stderr(io, &b(&finished.stderr));
    for chunk in &finished.stdout {
        if let Err(failure) = io.write_plain(&b(chunk)) {
            return crate::cli::print_failed(io, &failure, "");
        }
    }
    io.finish(finished.exit_code)
}

/// The parsed command line as plain values (the part of the argparse namespace the program reads).
struct Parsed {
    topology: PyStr,
    peer_ip: Option<PyStr>,
    peer_port: PyValue,
    peer_ssh: Option<PyStr>,
    compose: PyStr,
    env_interconnect: Option<PyStr>,
    bandwidth: Option<f64>,
    per_port_gbps: Option<f64>,
    per_port_floor: f64,
    bw_floor: f64,
    emit_manifest: bool,
    manifest: Option<PyStr>,
    check_egress: bool,
    sub_work_dir: Option<PyStr>,
    model_source: Option<PyStr>,
    emit_sub_manifest: Option<PyStr>,
}

impl Parsed {
    // Python truthiness: argparse hands the original the string "" for `--opt ""`, and the original tests `if args.opt:`. The raw value stays in the
    // field (it is echoed by the original where it echoes `args.peer_ip`); every decision site reads these views.
    fn peer_ip_set(&self) -> Option<&PyStr> {
        self.peer_ip.as_ref().filter(|value| !value.is_empty())
    }
    fn peer_ssh_set(&self) -> Option<&PyStr> {
        self.peer_ssh.as_ref().filter(|value| !value.is_empty())
    }
    fn sub_work_dir_set(&self) -> Option<&PyStr> {
        self.sub_work_dir.as_ref().filter(|value| !value.is_empty())
    }
    fn emit_sub_manifest_set(&self) -> Option<&PyStr> {
        self.emit_sub_manifest
            .as_ref()
            .filter(|value| !value.is_empty())
    }

    fn of(matches: &crate::argv::Matches) -> Parsed {
        Parsed {
            topology: matches
                .value(args::TOPOLOGY)
                .map_or_else(|| PyStr::new("auto"), q),
            peer_ip: matches.value(args::PEER_IP).map(q),
            peer_port: matches
                .int(args::PEER_PORT)
                .map_or_else(|| PyValue::int(22), |number| PyValue::Int(number.clone())),
            peer_ssh: matches.value(args::PEER_SSH).map(q),
            compose: matches
                .value(args::COMPOSE)
                .map_or_else(|| PyStr::new("output/multi/docker-compose.yaml"), q),
            env_interconnect: matches.value(args::ENV_INTERCONNECT).map(q),
            bandwidth: matches.float(args::BANDWIDTH_GBPS),
            per_port_gbps: matches.float(args::PER_PORT_GBPS),
            per_port_floor: matches.float(args::PER_PORT_FLOOR).unwrap_or(100.0),
            bw_floor: matches.float(args::BW_FLOOR).unwrap_or(180.0),
            emit_manifest: matches.flag(args::EMIT_MANIFEST),
            manifest: matches.value(args::MANIFEST).map(q),
            check_egress: matches.flag(args::CHECK_EGRESS),
            sub_work_dir: matches.value(args::SUB_WORK_DIR).map(q),
            model_source: matches.value(args::MODEL_SOURCE).map(q),
            emit_sub_manifest: matches.value(args::EMIT_SUB_MANIFEST).map(q),
        }
    }
}

fn float_value(value: f64) -> PyValue {
    PyValue::Float(value)
}

fn opt_float(value: Option<f64>) -> PyValue {
    value.map_or(PyValue::None, float_value)
}

fn process(
    a: &Parsed,
    host: &(dyn Host + Sync),
    files: &(dyn Files + Sync),
    clock: &(dyn Clock + Sync),
    local: &(dyn Local + Sync),
) -> Finished {
    let stderr = RefCell::new(PyStr::default());
    let tee = Tee {
        inner: host,
        stderr: &stderr,
    };
    let mut stdout: Vec<PyStr> = Vec::new();
    let code = match scan_flow(a, &tee, files, clock, local, &stderr, &mut stdout) {
        Ok(code) => code,
        Err(failure) => ends_with(&failure, &stderr),
    };
    Finished {
        stdout,
        stderr: stderr.into_inner(),
        exit_code: code,
    }
}

fn say(stderr: &RefCell<PyStr>, line: &str) {
    stderr.borrow_mut().push_str(line);
}

#[allow(clippy::too_many_lines)]
fn scan_flow(
    a: &Parsed,
    host: &Tee<'_>,
    files: &dyn Files,
    clock: &dyn Clock,
    local: &dyn Local,
    stderr: &RefCell<PyStr>,
    stdout: &mut Vec<PyStr>,
) -> Res<i32> {
    let topology = PyValue::Str(a.topology.clone());
    // fail-closed interview gate before any probe
    let gate_code = scan::emit_gate(&PyValue::Bool(a.emit_manifest), &topology);
    if gate_code != 0 {
        say(stderr, "[scan] FAIL: --emit-manifest 에는 --topology single|multi 명시 필요 — 토폴로지는 terraforming_node 진입의 인터뷰로 선언한다(브랜치/스캔 추론으로 manifest 기입 금지).\n");
        return Ok(i32::try_from(gate_code).unwrap_or(1));
    }

    let mut ic = probe::detect_interconnect(host)?;
    if let Some(bandwidth) = a.bandwidth {
        if let PyValue::Dict(map) = &mut ic {
            map.set("bandwidth_gbps", float_value(bandwidth));
        }
    }
    let compose = PyValue::Str(a.compose.clone());
    let env_path = match a.env_interconnect.as_ref().filter(|path| !path.is_empty()) {
        Some(path) => PyValue::Str(path.clone()),
        None => scan::default_env_interconnect_path(&compose)?,
    };
    let result = build_local_scan_result(&ic, &compose, &topology, &env_path, host, files)?;

    let node = local.node();
    let main_node_host = if node.is_empty() {
        PyStr::new("localhost")
    } else {
        node.clone()
    };
    let mut main_node = dict(vec![
        ("role", text("main")),
        ("node_id", text("main")),
        ("host", PyValue::Str(main_node_host.clone())),
        (
            "hostname",
            if node.is_empty() {
                PyValue::None
            } else {
                PyValue::Str(node.clone())
            },
        ),
        ("ssh_user", PyValue::Str(local.user()?)),
        ("work_dir", PyValue::Str(local.cwd()?)),
    ]);
    let is_multi = a.topology.equals("multi");
    if is_multi {
        let socket_iface = get(&ic, "socket_iface");
        let main_ip = if socket_iface.is_truthy() {
            probe::iface_ipv4(host, &socket_iface)?
        } else {
            PyValue::None
        };
        if main_ip.is_truthy() {
            main_node.set("host", main_ip);
            main_node.set(
                "host_source",
                PyValue::Str(PyStr::new(&format!(
                    "interconnect-iface:{}",
                    socket_iface.display_text().to_utf8().unwrap_or_default()
                ))),
            );
        } else {
            let resolved =
                probe::resolves_to_loopback(host, &PyValue::Str(main_node_host.clone()))?;
            if resolved.is_truthy() {
                let mut message = PyStr::new("[scan] FAIL: multi 인데 main.host=");
                message.push_str(&main_node_host.repr());
                message.push_str(" 가 루프백(");
                message.push_py(&resolved.display_text());
                message.push_str(")으로 해소되고 인터커넥트 iface(");
                message.push_py(&socket_iface.display_text());
                message.push_str(") IPv4 도 얻지 못했다 — 이 값이 Ray head 주소가 되면 서브가 붙을 수 없다(분산 서빙 구조적 불가). manifest.nodes[main].host 에 서브가 도달 가능한 IP 를 직접 지정하라.\n");
                stderr.borrow_mut().push_py(&message);
                return Ok(2);
            }
            main_node.set(
                "host_source",
                text("hostname(루프백 아님 — iface IP 미해소)"),
            );
        }
    }
    let main_node = PyValue::Dict(main_node);
    let mut result_map = match result {
        PyValue::Dict(map) => map,
        other => {
            return Err(scan::unsupported(&format!(
                "a scan envelope of type {}",
                other.type_name()
            )))
        }
    };
    if is_multi && a.peer_ip_set().is_some() {
        let peer_ip = a.peer_ip_set().cloned().unwrap_or_default();
        let mut sub = dict(vec![
            ("role", text("sub")),
            ("node_id", text("sub")),
            ("host", PyValue::Str(peer_ip)),
        ]);
        if let Some(ssh) = a.peer_ssh_set() {
            let (user, host_part) = split_once_at(ssh);
            match host_part {
                Some(rest) => {
                    sub.set("ssh_user", PyValue::Str(user));
                    sub.set("hostname", PyValue::Str(rest));
                }
                None => sub.set("hostname", PyValue::Str(ssh.clone())),
            }
        }
        if let Some(work_dir) = a.sub_work_dir_set() {
            sub.set("work_dir", PyValue::Str(work_dir.clone()));
        }
        result_map.set(
            "nodes",
            PyValue::List(vec![main_node.clone(), PyValue::Dict(sub)]),
        );
    } else if a.topology.equals("single") {
        let mut nodes = vec![main_node.clone()];
        if let Some(ssh) = a.peer_ssh_set() {
            let (user, host_part) = split_once_at(ssh);
            let hostname = host_part.unwrap_or_default();
            let sub_host = match a.peer_ip_set() {
                Some(ip) => ip.clone(),
                None if !hostname.is_empty() => hostname.clone(),
                None => ssh.clone(),
            };
            let mut sub = dict(vec![
                ("role", text("sub")),
                ("node_id", text("sub")),
                ("host", PyValue::Str(sub_host)),
                (
                    "hostname",
                    if hostname.is_empty() {
                        PyValue::None
                    } else {
                        PyValue::Str(hostname)
                    },
                ),
                (
                    "ssh_user",
                    if user.is_empty() {
                        PyValue::None
                    } else {
                        PyValue::Str(user)
                    },
                ),
            ]);
            if let Some(work_dir) = a.sub_work_dir_set() {
                sub.set("work_dir", PyValue::Str(work_dir.clone()));
            }
            nodes.push(PyValue::Dict(sub));
        }
        result_map.set("nodes", PyValue::List(nodes));
    }
    if let Some(peer_ip) = a.peer_ip_set() {
        let reachable = probe::peer_reachable(
            host,
            &PyValue::Str(peer_ip.clone()),
            &a.peer_port,
            &PyValue::int(5),
        )?;
        result_map.set(
            "peer_check",
            PyValue::Dict(dict(vec![
                ("ip", PyValue::Str(peer_ip.clone())),
                ("port", a.peer_port.clone()),
                ("reachable", reachable),
            ])),
        );
    }

    // ── three-way assertion + alpha/gamma gate inputs ──
    let branch = probe::git_branch(host);
    let branch_topo = match branch.peel() {
        PyValue::Str(name) if name.equals("single-node") => text("single"),
        PyValue::Str(name) if name.equals("multi-node") => text("multi"),
        _ => PyValue::None,
    };
    let mani_path = match a.manifest.as_ref().filter(|path| !path.is_empty()) {
        Some(path) => PyValue::Str(path.clone()),
        None if branch_topo.is_truthy() => PyValue::Str(PyStr::new(&format!(
            "output/{}/manifest.yaml",
            branch_topo.display_text().to_utf8().unwrap_or_default()
        ))),
        None => text("manifest.yaml"),
    };
    let mut manifest_read_error = PyValue::None;
    let mani_topo = match files::read_manifest_topology(files, &mani_path) {
        Ok(found) => found,
        Err(Failure::Raised(raised)) if raised.class == "ManifestUnreadable" => {
            manifest_read_error = PyValue::Str(raised.message);
            PyValue::None
        }
        Err(other) => return Err(other),
    };
    let peer_reach = get(
        &get(&PyValue::Dict(result_map.clone()), "peer_check"),
        "reachable",
    );

    let (mut egress_self, mut egress_peer, mut model_env_result) =
        (PyValue::None, PyValue::None, PyValue::None);
    if let Some(source) = &a.model_source {
        if !result_map.contains_key("model_source") {
            result_map.set("model_source", PyValue::Str(source.clone()));
        }
    }
    let nas = match files::read_manifest_field(files, &mani_path, &text("nas_model_path")) {
        Ok(found) => found,
        Err(Failure::Raised(raised)) if raised.class == "ManifestUnreadable" => {
            manifest_read_error = PyValue::Str(raised.message);
            PyValue::None
        }
        Err(other) => return Err(other),
    };
    if nas.is_truthy() && !result_map.contains_key("nas_model_path") {
        result_map.set("nas_model_path", nas);
    }
    let peer_ssh_given = a.peer_ssh_set().is_some() && a.topology.equals("single")
        || a.peer_ssh_set().is_some() && a.topology.equals("multi");
    let peer_ssh_value = opt_text(a.peer_ssh_set());
    if a.check_egress {
        let default_timeout = PyValue::int(5);
        let egress = probe::scan_egress(
            host,
            &text("huggingface.co"),
            &text("8.8.8.8"),
            &PyValue::int(443),
            &default_timeout,
        )?;
        egress_self = get(&egress, "egress");
        let mut block = dict(vec![("self", egress)]);
        if peer_ssh_given {
            let peer = probe::collect_peer_egress(
                host,
                &peer_ssh_value,
                &text("huggingface.co"),
                &text("8.8.8.8"),
                &PyValue::int(443),
                &default_timeout,
                &PyValue::None,
            )?;
            egress_peer = get(&peer, "egress");
            block.set("peer", peer);
        }
        result_map.set("egress", PyValue::Dict(block));
        let model_source = match &a.model_source {
            Some(source) if !source.is_empty() => PyValue::Str(source.clone()),
            _ => match files::read_manifest_field(files, &mani_path, &text("model_source")) {
                Ok(found) => found,
                Err(Failure::Raised(raised)) if raised.class == "ManifestUnreadable" => {
                    manifest_read_error = PyValue::Str(raised.message);
                    opt_text(a.model_source.as_ref())
                }
                Err(other) => return Err(other),
            },
        };
        if model_source.is_truthy() {
            result_map.set("model_source", model_source.clone());
            let nas_path = get(&PyValue::Dict(result_map.clone()), "nas_model_path");
            if nas_path.is_truthy() {
                result_map.set("nas_model_path", nas_path.clone());
            }
            let managed =
                matches!(model_source.peel(), PyValue::Str(word) if word.equals("managed"));
            let free_form = matches!(model_source.peel(), PyValue::Str(word) if word.equals("ephemeral") || word.equals("custom"));
            let mut nas_reachable = match (managed, nas_path.peel()) {
                (true, PyValue::Str(path)) if !path.is_empty() => PyValue::Bool(local.is_dir(path)),
                _ => PyValue::None,
            };
            let mut nas_source = "measured:main-fs";
            let token_file =
                match files::read_manifest_field(files, &mani_path, &text("hf_token_env_file")) {
                    Ok(found) => found,
                    Err(Failure::Raised(raised)) if raised.class == "ManifestUnreadable" => {
                        PyValue::None
                    }
                    Err(other) => return Err(other),
                };
            let mut token_present = if free_form {
                match token_file.peel() {
                    PyValue::Str(path) if !path.is_empty() => PyValue::Bool(local.is_file(path)),
                    _ => PyValue::Bool(false),
                }
            } else {
                PyValue::None
            };
            let mut token_source = "measured:main-fs";
            let egress_for_model = if peer_ssh_given {
                let peer_env = probe::collect_peer_model_env(
                    host,
                    &peer_ssh_value,
                    &nas_path,
                    &token_file,
                    &PyValue::None,
                )?;
                result_map.set("model_env_peer", peer_env.clone());
                if get(&peer_env, "probed").is_truthy() {
                    if managed && !matches!(get(&peer_env, "nas_reachable").peel(), PyValue::None) {
                        nas_reachable = get(&peer_env, "nas_reachable");
                        nas_source = "measured:peer-ssh";
                    }
                    if free_form
                        && !matches!(get(&peer_env, "hf_token_present").peel(), PyValue::None)
                    {
                        token_present = get(&peer_env, "hf_token_present");
                        token_source = "measured:peer-ssh";
                    }
                }
                if egress_peer.is_truthy() {
                    egress_peer.clone()
                } else {
                    egress_self.clone()
                }
            } else {
                egress_self.clone()
            };
            let mut env_result = match scan::check_model_env(
                &model_source,
                &egress_for_model,
                &nas_reachable,
                &token_present,
            ) {
                PyValue::Dict(map) => map,
                other => {
                    return Err(scan::unsupported(&format!(
                        "a model-env verdict of type {}",
                        other.type_name()
                    )))
                }
            };
            env_result.set("nas_source", text(nas_source));
            env_result.set("hf_token_source", text(token_source));
            env_result.set(
                "egress_source",
                text(if peer_ssh_given && egress_peer.is_truthy() {
                    "peer"
                } else {
                    "self"
                }),
            );
            let env_value = PyValue::Dict(env_result);
            result_map.set("model_env", env_value.clone());
            model_env_result = env_value;
        }
    }

    if manifest_read_error.is_truthy() {
        stderr
            .borrow_mut()
            .push_str("[scan] FAIL(F10): manifest 가 존재하나 판독 불가 — ");
        stderr
            .borrow_mut()
            .push_py(&manifest_read_error.display_text());
        say(stderr, "\n");
        say(stderr, "[scan]        판독 불가를 '부재' 로 접으면 3자일치 게이트가 통째로 skip 된다(무증거 통과).\n");
    }
    let interconnect_present = get(&PyValue::Dict(result_map.clone()), "interconnect_present");
    let gate_args = GateArgs {
        declared: topology.clone(),
        ic_present: interconnect_present,
        peer_given: PyValue::Bool(a.peer_ip_set().is_some()),
        peer_reachable: peer_reach,
        bandwidth: get(&ic, "bandwidth_gbps"),
        bw_floor: float_value(a.bw_floor),
        per_port_gbps: opt_float(a.per_port_gbps),
        per_port_floor: float_value(a.per_port_floor),
        branch: branch.clone(),
        branch_topo,
        mani_topo,
        mani_path: mani_path.clone(),
        peer_ip: opt_text(a.peer_ip.as_ref()),
        egress_self,
        egress_peer: egress_peer.clone(),
        model_env_ok: if model_env_result.is_truthy() {
            get(&model_env_result, "ok")
        } else {
            PyValue::None
        },
        model_env_reason: if model_env_result.is_truthy() {
            get(&model_env_result, "reason")
        } else {
            PyValue::None
        },
        manifest_read_error: manifest_read_error.clone(),
    };
    let evaluated = scan::evaluate_gate(&gate_args)?;
    let PyValue::Tuple(parts) = evaluated.peel() else {
        return Err(scan::unsupported(
            "an evaluate_gate answer that is not a tuple",
        ));
    };
    let (assertion, gate, mut exit_code) = (
        parts[0].clone(),
        parts[1].clone(),
        parts[2]
            .as_plain_int()
            .and_then(|n| i32::try_from(n.clone()).ok())
            .unwrap_or(1),
    );
    result_map.set("consistency_assertion", assertion);
    let mut gate_map = match gate {
        PyValue::Dict(map) => map,
        other => {
            return Err(scan::unsupported(&format!(
                "a gate of type {}",
                other.type_name()
            )))
        }
    };
    // `result["gate"] = gate` is assigned HERE in the original (before the homogeneity key exists); the dictionary is mutated afterwards,
    // so the key keeps this position in the envelope and the later `set` replaces the value in place
    result_map.set("gate", PyValue::Dict(gate_map.clone()));

    // ── sub-host homogeneity ──
    if peer_ssh_given {
        let local_hw = probe::collect_local_hw(host);
        let peer_hw = probe::collect_peer_hw(host, &peer_ssh_value, &PyValue::None)?;
        let mut homo = match scan::assert_homogeneity(&local_hw, &peer_hw)? {
            PyValue::Dict(map) => map,
            other => {
                return Err(scan::unsupported(&format!(
                    "a homogeneity verdict of type {}",
                    other.type_name()
                )))
            }
        };
        if a.topology.equals("single") {
            let gpus = get(&peer_hw, "gpus_per_node");
            let observed = peer_hw.is_truthy()
                && matches!(gpus.peel(), PyValue::Int(_))
                && agent_runtime_support::pyops::compare(
                    agent_runtime_support::pyops::Compare::GreaterEqual,
                    gpus.peel(),
                    &PyValue::int(1),
                )
                .unwrap_or(false);
            let blocks = get(&PyValue::Dict(homo.clone()), "blocks");
            let mut warns = list_items(&get(&PyValue::Dict(homo.clone()), "warns"));
            for block in list_items(&blocks) {
                let mut line = PyStr::new("single 동질성 정보(블록 아님): ");
                line.push_py(&block.display_text());
                warns.push(PyValue::Str(line));
            }
            let detail = get(&PyValue::Dict(homo.clone()), "detail");
            homo = dict(vec![
                ("verified", PyValue::Bool(observed)),
                (
                    "blocks",
                    PyValue::List(if observed {
                        Vec::new()
                    } else {
                        vec![text("서브 HW 수집 실패 또는 GPU 0 — 관측되지 않은 노드에 위임하지 않는다(fail-closed)")]
                    }),
                ),
                ("warns", PyValue::List(warns)),
                (
                    "detail",
                    if detail.is_truthy() {
                        detail
                    } else {
                        PyValue::Dict(PyDict::new())
                    },
                ),
                ("verified_rule", text("single:peer-observed(gpus>=1)")),
            ]);
        } else {
            homo.set("verified_rule", text("multi:homogeneity-hard"));
        }
        let mut envelope = dict(vec![("local", local_hw), ("peer", peer_hw.clone())]);
        for (key, value) in homo.iter() {
            envelope.insert(key.clone(), value.clone());
        }
        result_map.set("homogeneity", PyValue::Dict(envelope));
        let blocks = list_items(&get(&PyValue::Dict(homo.clone()), "blocks"));
        if !blocks.is_empty() {
            gate_map.set("status", text("blocked"));
            let mut reasons = list_items(&get(&PyValue::Dict(gate_map.clone()), "reasons"));
            reasons.extend(blocks.clone());
            gate_map.set("reasons", PyValue::List(reasons));
            let mut note = PyStr::new("서브 HW 동질성 미충족(fail-closed) — 표기 드리프트(동일 GPU 다른 문자열) 등 오탐 판단 시 HITL 우회 = manifest nodes[sub].hw_verified 수동 기입이 권위(plan_26063021_14_37 D3) — ");
            let previous = get(&PyValue::Dict(gate_map.clone()), "note");
            if !matches!(previous.peel(), PyValue::None) {
                note.push_py(&previous.display_text());
            }
            gate_map.set("note", PyValue::Str(note));
            exit_code = 2;
        }
    }
    result_map.set("gate", PyValue::Dict(gate_map.clone()));

    // ── sub manifest (needs PyYAML + the contract: the tail is a visible boundary) ──
    if let Some(path) = a.emit_sub_manifest_set() {
        let homo = get(&PyValue::Dict(result_map.clone()), "homogeneity");
        if !(a.topology.equals("single") || a.topology.equals("multi"))
            || a.peer_ssh_set().is_none()
        {
            say(stderr, "[scan] FAIL: --emit-sub-manifest 는 명시 topology와 --peer-ssh가 함께 있어야 한다(서브 실측이 전제).\n");
            return Ok(2);
        }
        let gate_status = get(&PyValue::Dict(gate_map.clone()), "status");
        let status_ok = matches!(gate_status.peel(), PyValue::Str(word) if word.equals("ok"));
        if !status_ok || !get(&homo, "verified").is_truthy() {
            let reasons = {
                let blocks = list_items(&get(&homo, "blocks"));
                if blocks.is_empty() {
                    list_items(&get(&PyValue::Dict(gate_map.clone()), "reasons"))
                } else {
                    blocks
                }
            };
            let joined: Vec<String> = reasons
                .iter()
                .map(|reason| reason.display_text().to_utf8().unwrap_or_default())
                .collect();
            stderr.borrow_mut().push_str(&format!(
                "[scan] FAIL: 서브 manifest 발행 금지 — gate={} · 서브 관측={} ({})\n",
                gate_status.display_text().to_utf8().unwrap_or_default(),
                get(&homo, "verified")
                    .display_text()
                    .to_utf8()
                    .unwrap_or_default(),
                joined.join("; ")
            ));
            return Ok(2);
        }
        let model_source = get(&PyValue::Dict(result_map.clone()), "model_source");
        let known = matches!(model_source.peel(), PyValue::Str(word) if word.equals("managed") || word.equals("ephemeral") || word.equals("custom"));
        if !known {
            say(stderr, "[scan] FAIL: 서브 manifest 에 model_source 가 필요하다 — --model-source <managed|ephemeral|custom>(인터뷰 확정값).\n");
            return Ok(2);
        }
        let _ = path;
        say(
            stderr,
            "[scan_node] ERROR --emit-sub-manifest writes the sub manifest with PyYAML and the Flag contract of manifest_contract, which this phase does not model (the original's guards above all passed); nothing was written\n",
        );
        return Ok(EXIT_BOUNDARY);
    }

    if a.emit_manifest
        && matches!(get(&PyValue::Dict(gate_map.clone()), "status").peel(), PyValue::Str(word) if word.equals("ok"))
    {
        let incomplete = scan::emission_blockers(&PyValue::Dict(result_map.clone()))?;
        let missing = list_items(&incomplete);
        if !missing.is_empty() {
            gate_map.set("status", text("blocked"));
            let mut reasons = list_items(&get(&PyValue::Dict(gate_map.clone()), "reasons"));
            let joined: Vec<String> = missing
                .iter()
                .map(|item| item.display_text().to_utf8().unwrap_or_default())
                .collect();
            reasons.push(text(&format!(
                "완수 manifest 발행 금지: scan facts 불완전({})",
                joined.join(",")
            )));
            gate_map.set("reasons", PyValue::List(reasons));
            result_map.set("gate", PyValue::Dict(gate_map.clone()));
            exit_code = 2;
        }
    }

    let envelope = PyValue::Dict(result_map);
    let rendered = envelope
        .dumps_pretty()
        .map_err(|error| scan::raise("ValueError", &error.to_string()))?;
    stdout.push(rendered);
    stdout.push(PyStr::new("\n"));
    let gate_ok = matches!(get(&get(&envelope, "gate"), "status").peel(), PyValue::Str(word) if word.equals("ok"));
    if a.emit_manifest && gate_ok && (a.topology.equals("single") || a.topology.equals("multi")) {
        stdout.push(PyStr::new(
            "\n# ===== manifest 기입 블록 (검증 통과 — 사람 확인 후 manifest.yaml 반영) =====\n",
        ));
        let block = manifest::emit_manifest_block(&envelope, clock)?;
        stdout.push(block.display_text());
    }
    Ok(exit_code)
}

/// `value.partition("@")` as `(before, Some(after))` when the separator is present.
fn split_once_at(text: &PyStr) -> (PyStr, Option<PyStr>) {
    let points = text.code_points();
    match points.iter().position(|&point| point == u32::from(b'@')) {
        Some(at) => (
            PyStr::from_code_points(points[..at].to_vec()),
            Some(PyStr::from_code_points(points[at + 1..].to_vec())),
        ),
        None => (text.clone(), None),
    }
}

fn list_items(value: &PyValue) -> Vec<PyValue> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.clone(),
        _ => Vec::new(),
    }
}

/// `build_local_scan_result(ic, compose, topology, env_interconnect)`: the local scan envelope, calling the probes in the original's order.
fn build_local_scan_result(
    ic: &PyValue,
    compose: &PyValue,
    topology: &PyValue,
    env_interconnect: &PyValue,
    host: &dyn Host,
    files: &dyn Files,
) -> Res<PyValue> {
    let mut out = PyDict::new();
    out.set("cpu_arch", probe::scan_cpu_arch(host));
    out.set("cuda_version", probe::scan_cuda_version(host));
    out.set("gpus_per_node", probe::scan_gpus_per_node(host));
    out.set("gpu_model", probe::scan_gpu_model(host));
    out.set("driver_version", probe::scan_driver_version(host));
    if let PyValue::Dict(memory) = probe::scan_gpu_memory(host)? {
        for (key, value) in memory.iter() {
            out.insert(key.clone(), value.clone());
        }
    }
    let (mut public, mut detail) = (PyDict::new(), PyDict::new());
    if let PyValue::Dict(map) = ic.peel() {
        for (key, value) in map.iter() {
            if key.code_points().first() == Some(&u32::from(b'_')) {
                detail.insert(key.clone(), value.clone());
            } else {
                public.insert(key.clone(), value.clone());
            }
        }
    }
    out.set("interconnect", PyValue::Dict(public));
    out.set("scan_detail", PyValue::Dict(detail));
    out.set(
        "cross_validation",
        files::cross_validate(ic, compose, env_interconnect, files)?,
    );
    out.set("topology_declared", topology.clone());
    let kind = get(ic, "type");
    out.set(
        "interconnect_present",
        PyValue::Bool(
            !matches!(kind.peel(), PyValue::Str(word) if word.equals("generic-ethernet")),
        ),
    );
    Ok(PyValue::Dict(out))
}
