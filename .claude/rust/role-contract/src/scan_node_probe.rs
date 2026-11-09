//! The second scan-node migration phase (P2): the hardware probes, the interconnect detection and the SSH/network collection of
//! `scan_node.py`, over Python-compatible values ([`PyValue`]) exactly as the original's callers hand them in.
//!
//! **Purity.** This crate neither runs a process nor opens a socket. Every outside effect the original had - `shutil.which`,
//! `subprocess.run`, `os.listdir`, `open()`, `socket.create_connection`, `socket.getaddrinfo`, `urllib` HEAD, `platform.machine()` and
//! the two `print(..., file=sys.stderr)` diagnostics - is one method of the [`Host`] trait, handed in by the caller: the real one is
//! `adversarial-benchmark/rust` `scan_node_host`, the checks of this file use a scripted one. All the *decisions* (which command
//! runs in which order, how its output is parsed, which text becomes which fact, how a failure is folded) are here and are the
//! original's.
//!
//! **Python behaviour reproduced.** Text is `str` (code points): `str.strip`/`split`/`splitlines` use the Unicode white space and
//! line-boundary sets, `\d`/`\s`/`\b` of the regexes are Unicode, `str.isdigit` of a non-decimal numeric character is reported as
//! an explicit unsupported shape (as in P1), the text of a subprocess is decoded as strict UTF-8 with universal newlines (a decoding
//! failure is the `UnicodeDecodeError` the original's `except Exception` folds), and the diagnostic of a failed SSH probe carries the
//! same `Class: message` text a Python exception would (`TimeoutExpired`, `FileNotFoundError`, ...).
//!
//! **Not modelled (explicit `unsupported`, never guessed).** An argument of a type the measured calls do not use (a non-`str`
//! command element, a non-integer port, ...); a host name with a NUL or a non-ASCII character (CPython hands the truncated or
//! IDNA-encoded name to the resolver: an unmeasured shape of that kind); the text encoding of a non-UTF-8 locale.

use crate::scan_node::{
    as_str, dict_of, digits_only, list_of_texts, parse_gpu_memory_line, raise, sorted_values, text,
    unsupported, Res,
};
use agent_runtime_support::pyjson::decode_file_text;
use agent_runtime_support::pynumber::int_from_str;
use agent_runtime_support::pyops::{compare, Compare};
use agent_runtime_support::pysort::py_eq;
use agent_runtime_support::pystr::{
    decimal_digit_value, is_decimal_digit, is_python_space, is_python_word, PyStr,
};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// How one captured process ended (`subprocess.run(..., capture_output=True)`): the three outcomes the original distinguishes.
#[derive(Clone, Debug, PartialEq)]
pub enum Capture {
    /// The process ran and ended. `code` is Python's `returncode` (negative for a signal).
    Exited {
        code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// `subprocess.TimeoutExpired`: the process was killed.
    TimedOut,
    /// An `OSError` of the spawn (`FileNotFoundError`, `PermissionError`, ...): its class name and `str()`.
    Failed { class: String, message: String },
}

/// The outside the scan reads. See the module documentation; a method has exactly the effect its Python counterpart had.
pub trait Host {
    /// `shutil.which(program) is not None`.
    fn which(&self, program: &[u8]) -> bool;
    /// `subprocess.run(argv, input=stdin, capture_output=True, timeout=timeout)` without the text decoding.
    fn capture(&self, argv: &[Vec<u8>], stdin: Option<&[u8]>, timeout: Option<f64>) -> Capture;
    /// `os.listdir(path)` (unsorted) or `None` for an `OSError`.
    fn list_dir(&self, path: &[u8]) -> Option<Vec<Vec<u8>>>;
    /// The bytes of a file or `None` for an `OSError`.
    fn read_file(&self, path: &[u8]) -> Option<Vec<u8>>;
    /// `socket.create_connection((host, port), timeout)` succeeded (every `OSError`, resolver failures included, is `false`).
    fn tcp_connect(&self, host: &str, port: u16, timeout: Option<f64>) -> bool;
    /// The addresses `socket.getaddrinfo(name, None)` lists, in order, as text (`None` for an `OSError`).
    fn resolve(&self, name: &str) -> Option<Vec<String>>;
    /// `urllib.request.urlopen(Request("https://<authority>", method="HEAD"), timeout)` answered with a 2xx after redirects.
    fn https_head_ok(&self, authority: &str, timeout: Option<f64>) -> bool;
    /// `platform.machine()`.
    fn machine(&self) -> String;
    /// `print(text, file=sys.stderr)`: the text without its newline.
    fn stderr_line(&self, text: &PyStr);
    /// `PEER_SSH_LAST_ERROR[what] = cause`.
    fn peer_error(&self, what: &str, cause: &PyStr);
}

// ── small text helpers (Unicode, as `str` behaves) ─────────────────────────────────────────

fn pts(value: &PyStr) -> &[u32] {
    value.code_points()
}

fn from(points: &[u32]) -> PyStr {
    PyStr::from_code_points(points.to_vec())
}

fn ascii(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn starts_with(points: &[u32], prefix: &str) -> bool {
    let prefix = ascii(prefix);
    points.len() >= prefix.len() && points[..prefix.len()] == prefix[..]
}

fn contains(points: &[u32], needle: &str) -> bool {
    let needle = ascii(needle);
    needle.is_empty()
        || points
            .windows(needle.len())
            .any(|window| window == &needle[..])
}

fn trim(points: &[u32]) -> &[u32] {
    let start = points
        .iter()
        .position(|&p| !is_python_space(p))
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|&p| !is_python_space(p))
        .map_or(start, |at| at + 1);
    &points[start..end]
}

/// `str.splitlines()` (the nine line boundaries of Python, `\r\n` as one).
pub fn splitlines(points: &[u32]) -> Vec<Vec<u32>> {
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut at = 0;
    while at < points.len() {
        let point = points[at];
        let boundary = matches!(
            point,
            0x0A | 0x0B | 0x0C | 0x0D | 0x1C | 0x1D | 0x1E | 0x85 | 0x2028 | 0x2029
        );
        if boundary {
            lines.push(std::mem::take(&mut current));
            if point == 0x0D && points.get(at + 1) == Some(&0x0A) {
                at += 1;
            }
        } else {
            current.push(point);
        }
        at += 1;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// `str.split()` on white space.
pub fn split_ws(points: &[u32]) -> Vec<Vec<u32>> {
    points
        .split(|&p| is_python_space(p))
        .filter(|piece| !piece.is_empty())
        .map(<[u32]>::to_vec)
        .collect()
}

fn digits_run(points: &[u32], from_at: usize) -> usize {
    points[from_at..]
        .iter()
        .take_while(|&&p| is_decimal_digit(p))
        .count()
}

/// `re.search(r"release\s+(\d+)\.(\d+)", text)`: the two digit groups of the leftmost match.
fn release_groups(points: &[u32]) -> Option<(Vec<u32>, Vec<u32>)> {
    let word = ascii("release");
    let mut at = 0;
    while at + word.len() <= points.len() {
        if points[at..at + word.len()] == word[..] {
            let mut cursor = at + word.len();
            let spaces = points[cursor..]
                .iter()
                .take_while(|&&p| is_python_space(p))
                .count();
            if spaces > 0 {
                cursor += spaces;
                let major = digits_run(points, cursor);
                if major > 0 && points.get(cursor + major) == Some(&u32::from(b'.')) {
                    let minor_at = cursor + major + 1;
                    let minor = digits_run(points, minor_at);
                    if minor > 0 {
                        return Some((
                            points[cursor..cursor + major].to_vec(),
                            points[minor_at..minor_at + minor].to_vec(),
                        ));
                    }
                }
            }
        }
        at += 1;
    }
    None
}

/// `re.search(r"\binet\s+(\d+\.\d+\.\d+\.\d+)/", text)`: the address of the leftmost match.
fn inet_address(points: &[u32]) -> Option<Vec<u32>> {
    let word = ascii("inet");
    let mut at = 0;
    while at + word.len() <= points.len() {
        let boundary = at == 0 || !is_python_word(points[at - 1]);
        if boundary && points[at..at + word.len()] == word[..] {
            let mut cursor = at + word.len();
            let spaces = points[cursor..]
                .iter()
                .take_while(|&&p| is_python_space(p))
                .count();
            if spaces > 0 {
                cursor += spaces;
                let begin = cursor;
                let mut ok = true;
                for part in 0..4 {
                    let run = digits_run(points, cursor);
                    if run == 0 {
                        ok = false;
                        break;
                    }
                    cursor += run;
                    if part < 3 {
                        if points.get(cursor) != Some(&u32::from(b'.')) {
                            ok = false;
                            break;
                        }
                        cursor += 1;
                    }
                }
                if ok && points.get(cursor) == Some(&u32::from(b'/')) {
                    return Some(points[begin..cursor].to_vec());
                }
            }
        }
        at += 1;
    }
    None
}

/// `IPV4_RE.match(token)`: `^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$` (a token has no line end, so `$` is the end).
fn is_ipv4_token(token: &[u32]) -> bool {
    let parts: Vec<&[u32]> = token.split(|&p| p == u32::from(b'.')).collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|part| (1..=3).contains(&part.len()) && part.iter().all(|&p| is_decimal_digit(p)))
}

fn ipv4_key(address: &PyStr) -> [u32; 4] {
    let mut key = [0u32; 4];
    for (slot, part) in pts(address)
        .split(|&p| p == u32::from(b'.'))
        .take(4)
        .enumerate()
    {
        key[slot] = part.iter().fold(0u32, |acc, &p| {
            acc * 10 + decimal_digit_value(p).unwrap_or(0)
        });
    }
    key
}

// ── strings and arguments of the original, as the callers pass them ─────────────────────────

fn bytes_of(value: &PyStr) -> Res<Vec<u8>> {
    value.to_surrogateescape_bytes().map_err(|_| {
        unsupported(
            "an argument with a lone surrogate that os.fsencode would refuse is not modelled",
        )
    })
}

fn utf8(text: &str) -> PyStr {
    PyStr::new(text)
}

/// The number a `timeout=` argument carries: `None`, an `int` or a `float`. Anything else is not a measured shape.
fn timeout_of(value: &PyValue, what: &str) -> Res<Option<f64>> {
    match value.peel() {
        PyValue::None => Ok(None),
        PyValue::Int(number) => Ok(Some(big_to_f64(number))),
        PyValue::Float(number) => Ok(Some(*number)),
        PyValue::Bool(flag) => Ok(Some(f64::from(u8::from(*flag)))),
        other => Err(unsupported(&format!(
            "{what} of type {} is not modelled",
            other.type_name()
        ))),
    }
}

fn big_to_f64(number: &impl std::fmt::Display) -> f64 {
    number.to_string().parse::<f64>().unwrap_or(f64::INFINITY)
}

fn strings_of(value: &PyValue, what: &str) -> Res<Vec<PyStr>> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items
            .iter()
            .map(|item| {
                as_str(item).cloned().ok_or_else(|| {
                    unsupported(&format!("{what} with a non-str element is not modelled"))
                })
            })
            .collect(),
        _ => Err(unsupported(&format!(
            "{what} that is not a list of str is not modelled"
        ))),
    }
}

/// The text of a captured stream the way `text=True` returns it, or the `UnicodeDecodeError` text.
fn decode(bytes: &[u8]) -> Result<PyStr, String> {
    decode_file_text(bytes).map_err(|error| error.to_string())
}

// ── subprocess.run as the original uses it ─────────────────────────────────────────────────

/// A finished probe: `returncode`, `stdout`, `stderr` as text.
struct Ran {
    code: i32,
    stdout: PyStr,
    stderr: PyStr,
}

/// What `subprocess.run(argv, ..., timeout)` raised, as `(class, message)` for the diagnostic and for the folding `except`s.
struct RunFailure {
    class: String,
    message: String,
}

fn repr_command(argv: &[Vec<u8>]) -> String {
    let items: Vec<String> = argv
        .iter()
        .map(|arg| PyStr::from_surrogateescape(arg).repr())
        .collect();
    format!("[{}]", items.join(", "))
}

fn seconds_text(timeout: f64) -> String {
    if timeout.fract() == 0.0 && timeout.abs() < 1e15 {
        // an `int` timeout prints as an int; the original's literals are ints (`25`, `5 * 2 + 10`)
        format!("{}", timeout as i64)
    } else {
        format!("{timeout}")
    }
}

fn run_text(
    host: &dyn Host,
    argv: &[Vec<u8>],
    stdin: Option<&str>,
    timeout: Option<f64>,
) -> Result<Ran, RunFailure> {
    match host.capture(argv, stdin.map(str::as_bytes), timeout) {
        Capture::TimedOut => Err(RunFailure {
            class: "TimeoutExpired".into(),
            message: format!(
                "Command '{}' timed out after {} seconds",
                repr_command(argv),
                seconds_text(timeout.unwrap_or(0.0))
            ),
        }),
        Capture::Failed { class, message } => Err(RunFailure { class, message }),
        Capture::Exited {
            code,
            stdout,
            stderr,
        } => {
            let (out, err) = (decode(&stdout), decode(&stderr));
            match (out, err) {
                (Ok(stdout), Ok(stderr)) => Ok(Ran {
                    code,
                    stdout,
                    stderr,
                }),
                (Err(message), _) | (_, Err(message)) => Err(RunFailure {
                    class: "UnicodeDecodeError".into(),
                    message,
                }),
            }
        }
    }
}

/// `_run(cmd, timeout)` over already-typed arguments: stdout of a successful command, `""` for everything else.
fn run_quiet(host: &dyn Host, argv: &[&str], timeout: f64) -> PyStr {
    let first = argv[0].as_bytes();
    if !host.which(first) {
        return utf8("");
    }
    let vector: Vec<Vec<u8>> = argv.iter().map(|arg| arg.as_bytes().to_vec()).collect();
    match run_text(host, &vector, None, Some(timeout)) {
        Ok(ran) if ran.code == 0 => ran.stdout,
        _ => utf8(""),
    }
}

/// `_run(cmd, timeout=10)` with the caller's own values (`str` or list of `str` for the command).
pub fn run(host: &dyn Host, cmd: &PyValue, timeout: &PyValue) -> Res<PyValue> {
    let (first, vector): (PyStr, Vec<PyStr>) = match cmd.peel() {
        PyValue::Str(whole) => {
            let Some(&head) = pts(whole).first() else {
                return Err(raise("IndexError", "string index out of range"));
            };
            (PyStr::from_code_points(vec![head]), vec![whole.clone()])
        }
        PyValue::List(_) | PyValue::Tuple(_) => {
            let items = strings_of(cmd, "a command")?;
            let Some(first) = items.first() else {
                let kind = if matches!(cmd.peel(), PyValue::List(_)) {
                    "list"
                } else {
                    "tuple"
                };
                return Err(raise("IndexError", &format!("{kind} index out of range")));
            };
            (first.clone(), items)
        }
        other => {
            return Err(unsupported(&format!(
                "a command of type {} is not modelled",
                other.type_name()
            )))
        }
    };
    if !host.which(&bytes_of(&first)?) {
        return Ok(text(""));
    }
    let timeout = timeout_of(timeout, "a timeout")?;
    let argv: Vec<Vec<u8>> = vector.iter().map(bytes_of).collect::<Res<_>>()?;
    match run_text(host, &argv, None, timeout) {
        Ok(ran) if ran.code == 0 => Ok(PyValue::Str(ran.stdout)),
        _ => Ok(text("")),
    }
}

// ── the probes ──────────────────────────────────────────────────────────────────────────

pub fn scan_cpu_arch(host: &dyn Host) -> PyValue {
    PyValue::Str(utf8(&host.machine()))
}

fn cuda_text(output: &PyStr) -> Option<(PyStr, PyStr)> {
    release_groups(pts(output)).map(|(major, minor)| (from(&major), from(&minor)))
}

pub fn scan_cuda_version(host: &dyn Host) -> PyValue {
    match cuda_text(&run_quiet(host, &["nvcc", "--version"], 10.0)) {
        Some((major, minor)) => {
            let mut joined = major;
            joined.push_py(&minor);
            PyValue::Str(joined)
        }
        None => PyValue::None,
    }
}

pub fn scan_gpus_per_node(host: &dyn Host) -> PyValue {
    let output = run_quiet(host, &["nvidia-smi", "-L"], 10.0);
    if output.is_empty() {
        return PyValue::None;
    }
    let count = splitlines(pts(&output))
        .iter()
        .filter(|line| starts_with(trim(line), "GPU "))
        .count();
    PyValue::int(count as i64)
}

fn first_query_line(host: &dyn Host, query: &str) -> Option<PyStr> {
    let output = run_quiet(host, &["nvidia-smi", query, "--format=csv,noheader"], 10.0);
    splitlines(trim(pts(&output)))
        .first()
        .map(|line| from(trim(line)))
}

pub fn scan_gpu_model(host: &dyn Host) -> PyValue {
    first_query_line(host, "--query-gpu=name").map_or(PyValue::None, PyValue::Str)
}

pub fn scan_driver_version(host: &dyn Host) -> PyValue {
    first_query_line(host, "--query-gpu=driver_version").map_or(PyValue::None, PyValue::Str)
}

pub fn scan_gpu_memory(host: &dyn Host) -> Res<PyValue> {
    let output = run_quiet(
        host,
        &[
            "nvidia-smi",
            "--query-gpu=memory.total,compute_cap",
            "--format=csv,noheader,nounits",
        ],
        10.0,
    );
    let line = splitlines(trim(pts(&output)))
        .first()
        .map_or_else(|| utf8(""), |line| from(line));
    parse_gpu_memory_line(&PyValue::Str(line))
}

/// One `show_gids` row of a RoCE v2 routable GID (`{hca, port, gid_index, ipv4, iface}`).
struct GidRow {
    hca: PyStr,
    port: PyStr,
    gid_index: GidIndex,
    ipv4: PyStr,
    iface: PyStr,
}

enum GidIndex {
    Number(PyValue),
    Text(PyStr),
}

fn gid_rows(host: &dyn Host) -> Res<Vec<GidRow>> {
    let output = run_quiet(host, &["show_gids"], 10.0);
    let mut rows = Vec::new();
    for line in splitlines(pts(&output)) {
        let tokens = split_ws(&line);
        if tokens.len() < 5 || tokens[0] == ascii("DEV") || tokens[0] == ascii("---") {
            continue;
        }
        if !tokens.iter().any(|token| *token == ascii("v2")) {
            continue;
        }
        let Some(ipv4) = tokens.iter().find(|token| is_ipv4_token(token)) else {
            continue;
        };
        let index_text = from(&tokens[2]);
        let gid_index = if digits_only(&index_text)? {
            GidIndex::Number(PyValue::Int(int_from_str(&index_text)?))
        } else {
            GidIndex::Text(index_text)
        };
        rows.push(GidRow {
            hca: from(&tokens[0]),
            port: from(&tokens[1]),
            gid_index,
            ipv4: from(ipv4),
            iface: from(&tokens[tokens.len() - 1]),
        });
    }
    Ok(rows)
}

fn gid_value(index: &GidIndex) -> PyValue {
    match index {
        GidIndex::Number(number) => number.clone(),
        GidIndex::Text(word) => PyValue::Str(word.clone()),
    }
}

fn gid_row_value(row: &GidRow) -> PyValue {
    dict_of(vec![
        ("hca", PyValue::Str(row.hca.clone())),
        ("port", PyValue::Str(row.port.clone())),
        ("gid_index", gid_value(&row.gid_index)),
        ("ipv4", PyValue::Str(row.ipv4.clone())),
        ("iface", PyValue::Str(row.iface.clone())),
    ])
}

pub fn scan_roce_v2_gids(host: &dyn Host) -> Res<PyValue> {
    Ok(PyValue::List(
        gid_rows(host)?.iter().map(gid_row_value).collect(),
    ))
}

/// `sorted(os.listdir("/sys/class/infiniband"))` (code point order of the names), `[]` for an `OSError`.
fn infiniband_names(host: &dyn Host) -> Vec<PyStr> {
    let mut names: Vec<PyStr> = host
        .list_dir(b"/sys/class/infiniband")
        .unwrap_or_default()
        .iter()
        .map(|name| PyStr::from_surrogateescape(name))
        .collect();
    names.sort_by(|a, b| pts(a).cmp(pts(b)));
    names
}

pub fn scan_infiniband_hcas(host: &dyn Host) -> PyValue {
    list_of_texts(&infiniband_names(host))
}

/// `_read_iface_mtu(iface)` with the caller's own value: falsy -> `None`, a `str` -> the measured `mtu`, any other type is not modelled.
pub fn read_iface_mtu_value(host: &dyn Host, iface: &PyValue) -> Res<PyValue> {
    if !iface.is_truthy() {
        return Ok(PyValue::None);
    }
    match as_str(iface) {
        Some(name) => Ok(read_iface_mtu(host, name)),
        None => Err(unsupported(&format!(
            "an interface of type {} is not modelled",
            iface.type_name()
        ))),
    }
}

/// `_read_iface_mtu(iface)`: the measured `mtu` of the interface, `None` when it cannot be read or parsed.
fn read_iface_mtu(host: &dyn Host, iface: &PyStr) -> PyValue {
    if iface.is_empty() {
        return PyValue::None;
    }
    let mut path = PyStr::new("/sys/class/net/");
    path.push_py(iface);
    path.push_str("/mtu");
    let Ok(bytes) = path.to_surrogateescape_bytes() else {
        return PyValue::None;
    };
    let Some(content) = host.read_file(&bytes) else {
        return PyValue::None;
    };
    let Ok(decoded) = decode_file_text(&content) else {
        return PyValue::None;
    };
    match int_from_str(&decoded.strip()) {
        Ok(number) => PyValue::Int(number),
        Err(_) => PyValue::None,
    }
}

pub fn detect_interconnect(host: &dyn Host) -> Res<PyValue> {
    let hcas_all = infiniband_names(host);
    let rows = gid_rows(host)?;
    if rows.is_empty() {
        return Ok(dict_of(vec![
            ("type", text("generic-ethernet")),
            ("hca_devices", PyValue::List(Vec::new())),
            ("gid_index", PyValue::None),
            ("socket_iface", PyValue::None),
            ("mtu", PyValue::None),
            ("bandwidth_gbps", PyValue::None),
            ("platform_preset", PyValue::None),
            ("_sysfs_hcas", list_of_texts(&hcas_all)),
            ("_roce_v2_gids", PyValue::List(Vec::new())),
        ]));
    }
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&at| ipv4_key(&rows[at].ipv4));
    let mut hca_devices: Vec<PyStr> = Vec::new();
    for &at in &order {
        if !hca_devices
            .iter()
            .any(|known| pts(known) == pts(&rows[at].hca))
        {
            hca_devices.push(rows[at].hca.clone());
        }
    }
    let numbers: Option<Vec<PyValue>> = rows
        .iter()
        .map(|row| match &row.gid_index {
            GidIndex::Number(number) => Some(number.clone()),
            GidIndex::Text(_) => None,
        })
        .collect();
    let gid_index = match numbers {
        Some(values) => {
            let mut ordered: Vec<PyValue> = Vec::new();
            for value in sorted_values(values)? {
                if ordered.last().is_none_or(|last| !py_eq(last, &value)) {
                    ordered.push(value);
                }
            }
            if ordered.len() == 1 {
                ordered.remove(0)
            } else {
                PyValue::List(ordered)
            }
        }
        None => text("invalid"),
    };
    let bootstrap = &rows[order[0]];
    Ok(dict_of(vec![
        ("type", text("RoCE v2")),
        ("hca_devices", list_of_texts(&hca_devices)),
        ("gid_index", gid_index),
        ("socket_iface", PyValue::Str(bootstrap.iface.clone())),
        ("mtu", read_iface_mtu(host, &bootstrap.iface)),
        ("bandwidth_gbps", PyValue::None),
        ("platform_preset", PyValue::None),
        ("_sysfs_hcas", list_of_texts(&hcas_all)),
        (
            "_roce_v2_gids",
            PyValue::List(rows.iter().map(gid_row_value).collect()),
        ),
    ]))
}

pub fn collect_local_hw(host: &dyn Host) -> PyValue {
    // the original evaluates the `nvcc` probe first, then the dictionary literal in key order
    let cuda = release_groups(pts(&run_quiet(host, &["nvcc", "--version"], 10.0))).map_or(
        PyValue::None,
        |(major, minor)| {
            let mut joined = from(&major);
            joined.push_str(".");
            joined.push_py(&from(&minor));
            PyValue::Str(joined)
        },
    );
    let cpu_arch = scan_cpu_arch(host);
    let gpu_model = scan_gpu_model(host);
    let gpus = scan_gpus_per_node(host);
    let driver = scan_driver_version(host);
    dict_of(vec![
        ("cpu_arch", cpu_arch),
        ("gpu_model", gpu_model),
        ("gpus_per_node", gpus),
        ("cuda", cuda),
        ("driver", driver),
    ])
}

pub fn git_branch(host: &dyn Host) -> PyValue {
    let output = run_quiet(host, &["git", "rev-parse", "--abbrev-ref", "HEAD"], 10.0);
    let line = output.strip();
    if line.is_empty() {
        PyValue::None
    } else {
        PyValue::Str(line)
    }
}

/// `_iface_ipv4(iface)`: the first IPv4 of an interface from `ip -o -4 addr show`, `None` for any failure the original folds.
pub fn iface_ipv4(host: &dyn Host, iface: &PyValue) -> Res<PyValue> {
    if !iface.is_truthy() {
        return Ok(PyValue::None);
    }
    let Some(name) = as_str(iface) else {
        return Err(unsupported(&format!(
            "an interface of type {} is not modelled",
            iface.type_name()
        )));
    };
    let mut argv: Vec<Vec<u8>> = ["ip", "-o", "-4", "addr", "show"]
        .iter()
        .map(|part| part.as_bytes().to_vec())
        .collect();
    argv.push(bytes_of(name)?);
    match host.capture(&argv, None, Some(5.0)) {
        Capture::TimedOut | Capture::Failed { .. } => Ok(PyValue::None),
        Capture::Exited {
            code,
            stdout,
            stderr,
        } => {
            // `subprocess.run(text=True)` decodes both streams before the return code is looked at: an undecodable stream raises
            // `UnicodeDecodeError` whatever the code was, so it is never folded into `None`.
            let (Ok(out), Ok(_)) = (decode(&stdout), decode(&stderr)) else {
                return Err(unsupported("an undecodable `ip` output (a UnicodeDecodeError the original does not fold) is not modelled"));
            };
            if code != 0 {
                return Ok(PyValue::None);
            }
            Ok(inet_address(pts(&out))
                .map_or(PyValue::None, |address| PyValue::Str(from(&address))))
        }
    }
}

// ── SSH collection ──────────────────────────────────────────────────────────────────────

pub const PEER_HW_PROBE: &str = "printf 'ARCH=%s\\n' \"$(uname -m)\"; printf 'GPUS=%s\\n' \"$(nvidia-smi -L 2>/dev/null | grep -c '^GPU ')\"; printf 'MODEL=%s\\n' \"$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)\"; printf 'DRIVER=%s\\n' \"$(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null | head -1)\"; printf 'CUDA=%s\\n' \"$(nvcc --version 2>/dev/null | grep -oE 'release [0-9]+\\.[0-9]+' | grep -oE '[0-9]+\\.[0-9]+')\"; printf 'GPUMEM=%s\\n' \"$(nvidia-smi --query-gpu=memory.total,compute_cap --format=csv,noheader,nounits 2>/dev/null | head -1)\"";

const EGRESS_PEER_PROBE: [&str; 5] = [
    "(curl -sS -o /dev/null -w '%{http_code}' --max-time ",
    " https://",
    " 2>/dev/null || echo 000); echo; (timeout ",
    " bash -c '</dev/tcp/",
    "' 2>/dev/null && echo OK || echo FAIL)",
];

/// `_peer_ssh_diag(what, target, rc, stderr)`: the folded cause, kept in `PEER_SSH_LAST_ERROR` and printed with the stderr tail.
fn peer_ssh_diag(host: &dyn Host, what: &str, rc: Option<i32>, stderr: &PyStr) {
    let detail = stderr.strip();
    let d = pts(&detail);
    let cause = if contains(d, "Host key verification failed")
        || contains(d, "REMOTE HOST IDENTIFICATION")
    {
        "host-key 미등록/불일치 — 메인 known_hosts 에 서브 키를 등록하라(ssh-keyscan)".to_string()
    } else if contains(d, "Permission denied") {
        "키 인증 거부 — 서브 authorized_keys 에 메인 공개키를 넣어라".to_string()
    } else if contains(d, "Could not resolve hostname") || contains(d, "Name or service not known")
    {
        "이름 해소 실패 — 주소/DNS 확인".to_string()
    } else if contains(d, "Connection timed out") || contains(d, "No route to host") || rc.is_none()
    {
        "네트워크 미도달/타임아웃".to_string()
    } else if rc == Some(127) || contains(d, "command not found") {
        "원격 도구 부재(nvidia-smi/uname 등) — PATH 또는 미설치".to_string()
    } else {
        format!(
            "미분류(rc={})",
            rc.map_or("None".to_string(), |code| code.to_string())
        )
    };
    host.peer_error(what, &utf8(&cause));
    host.stderr_line(&utf8(&format!("[scan] 서브 {what} 프로브 실패: {cause}")));
    if !d.is_empty() {
        let tail = if d.len() > 500 {
            &d[d.len() - 500..]
        } else {
            d
        };
        let mut line = PyStr::new("[scan]   ssh stderr: ");
        line.push_py(&from(tail));
        host.stderr_line(&line);
    }
}

fn failure_text(failure: &RunFailure) -> PyStr {
    utf8(&format!("{}: {}", failure.class, failure.message))
}

fn default_options() -> Vec<Vec<u8>> {
    ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
        .iter()
        .map(|part| part.as_bytes().to_vec())
        .collect()
}

/// `ssh_opts or [...]`: the caller's list when it is a non-empty list of `str`, the defaults for every falsy value.
fn ssh_options(value: &PyValue) -> Res<Vec<Vec<u8>>> {
    if !value.is_truthy() {
        return Ok(default_options());
    }
    strings_of(value, "ssh options")?
        .iter()
        .map(bytes_of)
        .collect()
}

fn ssh_argv(options: &[Vec<u8>], target: &PyStr) -> Res<Vec<Vec<u8>>> {
    let mut argv = vec![b"ssh".to_vec()];
    argv.extend(options.iter().cloned());
    argv.push(bytes_of(target)?);
    argv.push(b"bash".to_vec());
    argv.push(b"-ls".to_vec());
    Ok(argv)
}

fn target_of(value: &PyValue) -> Res<PyStr> {
    as_str(value).cloned().ok_or_else(|| {
        unsupported(&format!(
            "an ssh target of type {} is not modelled",
            value.type_name()
        ))
    })
}

/// One ssh round: stdout text of a successful run, `""` with the diagnostic for every failure (what the original does thrice).
fn ssh_round(host: &dyn Host, what: &str, argv: &[Vec<u8>], input: &str, timeout: f64) -> PyStr {
    match run_text(host, argv, Some(input), Some(timeout)) {
        Ok(ran) if ran.code == 0 => ran.stdout,
        Ok(ran) => {
            peer_ssh_diag(host, what, Some(ran.code), &ran.stderr);
            utf8("")
        }
        Err(failure) => {
            peer_ssh_diag(host, what, None, &failure_text(&failure));
            utf8("")
        }
    }
}

pub fn collect_peer_hw(host: &dyn Host, ssh_target: &PyValue, ssh_opts: &PyValue) -> Res<PyValue> {
    let options = ssh_options(ssh_opts)?;
    let target = target_of(ssh_target)?;
    let argv = ssh_argv(&options, &target)?;
    let output = ssh_round(host, "HW", &argv, PEER_HW_PROBE, 25.0);
    if trim(pts(&output)).is_empty() {
        return Ok(PyValue::Dict(PyDict::new()));
    }
    let mut kv: Vec<(PyStr, PyStr)> = Vec::new();
    for line in splitlines(pts(&output)) {
        if let Some(at) = line.iter().position(|&p| p == u32::from(b'=')) {
            let key = from(trim(&line[..at]));
            let value = from(trim(&line[at + 1..]));
            match kv.iter_mut().find(|(known, _)| pts(known) == pts(&key)) {
                Some(slot) => slot.1 = value,
                None => kv.push((key, value)),
            }
        }
    }
    let get = |name: &str| {
        kv.iter()
            .find(|(key, _)| pts(key) == &ascii(name)[..])
            .map(|(_, value)| value.clone())
    };
    let empty_none = |value: Option<PyStr>| match value {
        Some(found) if !found.is_empty() => PyValue::Str(found),
        _ => PyValue::None,
    };
    let gpus_text = get("GPUS").unwrap_or_else(|| utf8(""));
    let gpus = if digits_only(&gpus_text)? {
        PyValue::Int(int_from_str(&gpus_text)?)
    } else {
        PyValue::None
    };
    let mut result = PyDict::new();
    result.set("cpu_arch", empty_none(get("ARCH")));
    result.set("gpu_model", empty_none(get("MODEL")));
    result.set("gpus_per_node", gpus);
    result.set("cuda", empty_none(get("CUDA")));
    result.set("driver", empty_none(get("DRIVER")));
    let memory = parse_gpu_memory_line(&PyValue::Str(get("GPUMEM").unwrap_or_else(|| utf8(""))))?;
    if let PyValue::Dict(memory) = memory {
        for (key, value) in memory.iter() {
            result.insert(key.clone(), value.clone());
        }
    }
    Ok(PyValue::Dict(result))
}

/// `shlex.quote(text)`.
fn shell_quote(word: &PyStr) -> PyStr {
    let safe = |p: u32| {
        char::from_u32(p).is_some_and(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
    };
    if word.is_empty() {
        return utf8("''");
    }
    if pts(word).iter().all(|&p| safe(p)) {
        return word.clone();
    }
    let mut out = PyStr::new("'");
    for &p in pts(word) {
        if p == u32::from(b'\'') {
            out.push_str("'\"'\"'");
        } else {
            out.push_code_point(p);
        }
    }
    out.push_str("'");
    out
}

fn quoted_argument(value: &PyValue, what: &str) -> Res<Option<PyStr>> {
    if !value.is_truthy() {
        return Ok(None);
    }
    as_str(value)
        .map(|word| Some(shell_quote(word)))
        .ok_or_else(|| {
            unsupported(&format!(
                "{what} of type {} is not modelled",
                value.type_name()
            ))
        })
}

pub fn collect_peer_model_env(
    host: &dyn Host,
    ssh_target: &PyValue,
    nas_path: &PyValue,
    hf_token_env_file: &PyValue,
    ssh_opts: &PyValue,
) -> Res<PyValue> {
    let options = ssh_options(ssh_opts)?;
    let mut checks: Vec<String> = Vec::new();
    if let Some(quoted) = quoted_argument(nas_path, "a NAS path")? {
        checks.push(format!(
            "[ -d {} ] && echo NAS=1 || echo NAS=0",
            quoted
                .to_utf8()
                .ok_or_else(|| unsupported("a NAS path with a lone surrogate"))?
        ));
    }
    if let Some(quoted) = quoted_argument(hf_token_env_file, "a token file")? {
        checks.push(format!(
            "[ -f {} ] && echo TOK=1 || echo TOK=0",
            quoted
                .to_utf8()
                .ok_or_else(|| unsupported("a token file with a lone surrogate"))?
        ));
    }
    let unprobed = || {
        dict_of(vec![
            ("nas_reachable", PyValue::None),
            ("hf_token_present", PyValue::None),
            ("probed", PyValue::Bool(false)),
        ])
    };
    if checks.is_empty() {
        return Ok(unprobed());
    }
    let target = target_of(ssh_target)?;
    let argv = ssh_argv(&options, &target)?;
    let input = format!("{}\n", checks.join("\n"));
    let output = match run_text(host, &argv, Some(&input), Some(25.0)) {
        Ok(ran) if ran.code == 0 => ran.stdout,
        Ok(ran) => {
            peer_ssh_diag(host, "model-env", Some(ran.code), &ran.stderr);
            return Ok(unprobed());
        }
        Err(failure) => {
            peer_ssh_diag(host, "model-env", None, &failure_text(&failure));
            return Ok(unprobed());
        }
    };
    let mut kv: Vec<(Vec<u32>, Vec<u32>)> = Vec::new();
    for token in split_ws(pts(&output)) {
        if let Some(at) = token.iter().position(|&p| p == u32::from(b'=')) {
            let (key, value) = (token[..at].to_vec(), token[at + 1..].to_vec());
            match kv.iter_mut().find(|(known, _)| *known == key) {
                Some(slot) => slot.1 = value,
                None => kv.push((key, value)),
            }
        }
    }
    let flag = |name: &str| {
        kv.iter()
            .find(|(key, _)| *key == ascii(name))
            .map_or(PyValue::None, |(_, value)| {
                PyValue::Bool(*value == ascii("1"))
            })
    };
    Ok(dict_of(vec![
        ("nas_reachable", flag("NAS")),
        ("hf_token_present", flag("TOK")),
        ("probed", PyValue::Bool(true)),
        ("source", text("measured:peer-ssh")),
    ]))
}

fn plain_text(value: &PyValue, what: &str) -> Res<String> {
    match value.peel() {
        PyValue::Str(word) => word
            .to_utf8()
            .ok_or_else(|| unsupported(&format!("{what} with a lone surrogate is not modelled"))),
        PyValue::Int(number) => Ok(number.to_string()),
        other => Err(unsupported(&format!(
            "{what} of type {} is not modelled",
            other.type_name()
        ))),
    }
}

pub fn collect_peer_egress(
    host: &dyn Host,
    ssh_target: &PyValue,
    hf_host: &PyValue,
    gw_host: &PyValue,
    gw_port: &PyValue,
    timeout: &PyValue,
    ssh_opts: &PyValue,
) -> Res<PyValue> {
    let options = ssh_options(ssh_opts)?;
    let (hf, gw, port, seconds) = (
        plain_text(hf_host, "an HF host")?,
        plain_text(gw_host, "a gateway host")?,
        plain_text(gw_port, "a gateway port")?,
        plain_text(timeout, "an egress timeout")?,
    );
    let limit = match timeout.peel() {
        PyValue::Int(number) => big_to_f64(number) * 2.0 + 10.0,
        _ => return Err(unsupported("a non-int egress timeout is not modelled")),
    };
    let probe = format!(
        "{}{seconds}{}{hf}{}{seconds}{}{gw}/{port}{}",
        EGRESS_PEER_PROBE[0],
        EGRESS_PEER_PROBE[1],
        EGRESS_PEER_PROBE[2],
        EGRESS_PEER_PROBE[3],
        EGRESS_PEER_PROBE[4]
    );
    let target = target_of(ssh_target)?;
    let argv = ssh_argv(&options, &target)?;
    let output = ssh_round(host, "egress", &argv, &probe, limit);
    let lines: Vec<Vec<u32>> = splitlines(pts(&output))
        .iter()
        .map(|line| trim(line).to_vec())
        .filter(|line| !line.is_empty())
        .collect();
    let code = lines.first().cloned().unwrap_or_else(|| ascii("000"));
    let code_text = from(&code);
    let hf_ok = if digits_only(&code_text)? {
        let value = PyValue::Int(int_from_str(&code_text)?);
        compare(Compare::GreaterEqual, &value, &PyValue::int(200)).unwrap_or(false)
            && compare(Compare::Less, &value, &PyValue::int(500)).unwrap_or(false)
    } else {
        false
    };
    let gw_ok = lines.get(1).is_some_and(|line| *line == ascii("OK"));
    Ok(egress_value(hf_ok, gw_ok))
}

fn egress_value(hf_ok: bool, gw_ok: bool) -> PyValue {
    dict_of(vec![
        (
            "egress",
            text(if hf_ok && gw_ok {
                "online"
            } else {
                "restricted"
            }),
        ),
        ("hf_reachable", PyValue::Bool(hf_ok)),
        ("gateway_reachable", PyValue::Bool(gw_ok)),
    ])
}

// ── network ────────────────────────────────────────────────────────────────────────────

/// A host name or address the port hands to the resolver: ASCII without NUL (see the module documentation for what is not).
fn resolver_name(value: &PyValue, what: &str) -> Res<String> {
    let Some(word) = as_str(value) else {
        return Err(unsupported(&format!(
            "{what} of type {} is not modelled",
            value.type_name()
        )));
    };
    if pts(word).contains(&0) {
        return Err(unsupported(&format!("{what} with a NUL: CPython hands the truncated name to the C resolver (an unmeasured shape), not modelled")));
    }
    if pts(word).iter().any(|&p| p > 0x7f) {
        return Err(unsupported(&format!("{what} with a non-ASCII character: CPython IDNA-encodes it for the resolver, not modelled")));
    }
    word.to_utf8()
        .ok_or_else(|| unsupported(&format!("{what} with a lone surrogate is not modelled")))
}

fn socket_timeout(value: &PyValue) -> Res<Option<f64>> {
    let timeout = timeout_of(value, "a timeout")?;
    if timeout.is_some_and(|seconds| seconds.is_nan() || seconds < 0.0) {
        return Err(unsupported(
            "a negative or NaN socket timeout (ValueError in CPython) is not modelled",
        ));
    }
    Ok(timeout)
}

fn port_of(value: &PyValue) -> Res<u16> {
    match value.peel() {
        PyValue::Int(number) => u16::try_from(number.clone()).map_err(|_| {
            unsupported("a port outside 0-65535 (OverflowError in CPython) is not modelled")
        }),
        other => Err(unsupported(&format!(
            "a port of type {} is not modelled",
            other.type_name()
        ))),
    }
}

pub fn peer_reachable(
    host: &dyn Host,
    ip: &PyValue,
    port: &PyValue,
    timeout: &PyValue,
) -> Res<PyValue> {
    let name = resolver_name(ip, "a peer host")?;
    if name.is_empty() {
        return Err(unsupported("an empty peer host is not modelled"));
    }
    let port = port_of(port)?;
    let timeout = socket_timeout(timeout)?;
    Ok(PyValue::Bool(host.tcp_connect(&name, port, timeout)))
}

pub fn scan_egress(
    host: &dyn Host,
    hf_host: &PyValue,
    gw_host: &PyValue,
    gw_port: &PyValue,
    timeout: &PyValue,
) -> Res<PyValue> {
    let authority = resolver_name(hf_host, "an HF host")?;
    let gateway = resolver_name(gw_host, "a gateway host")?;
    if gateway.is_empty() {
        return Err(unsupported("an empty gateway host is not modelled"));
    }
    let port = port_of(gw_port)?;
    let timeout = socket_timeout(timeout)?;
    let hf_ok = host.https_head_ok(&authority, timeout);
    let gw_ok = host.tcp_connect(&gateway, port, timeout);
    Ok(egress_value(hf_ok, gw_ok))
}

pub fn resolves_to_loopback(host: &dyn Host, name: &PyValue) -> Res<PyValue> {
    if !name.is_truthy() {
        return Ok(PyValue::None);
    }
    let word = resolver_name(name, "a host name")?;
    let Some(addresses) = host.resolve(&word) else {
        return Ok(PyValue::None);
    };
    Ok(addresses
        .into_iter()
        .find(|address| address.starts_with("127.") || address == "::1")
        .map_or(PyValue::None, |address| PyValue::Str(utf8(&address))))
}
