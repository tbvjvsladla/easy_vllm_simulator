//! `preload_ram_gate.py`: the deterministic available-RAM gate that runs before a checkpoint is loaded.
//!
//! The gate refuses a load when `ceil(checkpoint_bytes / tp / MiB) + floor_mib` exceeds `MemAvailable`, after one
//! cache-dropping attempt through the installed sudoers helper. This module is the language-level core: the five public
//! functions of the original (`mem_available_mib`, `checkpoint_bytes_for`, `offloadable_bytes_for`, `try_drop_caches`,
//! `gate`) over Python-typed values, with every operating-system effect behind a trait:
//!
//! * [`MeminfoSource`] opens the text `mem_available_mib` reads (`/proc/meminfo` in production);
//! * [`DropSystem`] answers `os.path.exists(DROP_HELPER)` and runs the helper command;
//! * [`Provider`] is what `gate` calls: the available memory and one cache drop;
//! * [`Host`] adds the two sinks the gate writes to: standard error (the default log) and the caller's own log callable.
//!
//! The checkpoint and offload sizes are the model owner's (`model_config`), called here as the original calls
//! `parse_model_config`; nothing of that logic is copied. Values are never validated ahead of the point where the original
//! first uses them, so a malformed argument fails at the same step, after the same effects, with the same exception.

use crate::model_config;
use crate::model_dyn::{int_floor_div, Warnings};
use crate::model_files::{open_binary, os_failure};
use crate::pyexpr::{compare, int_div_power_of_two, int_of, Res};
use crate::rank_recipes::percent_d;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyops::{percent_fixed, Compare};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::One;
use std::io::Read;

/// One mebibyte.
pub const MIB: u32 = 1024 * 1024;
/// The sudoers helper that drops the page cache.
pub const DROP_HELPER: &str = "/usr/local/sbin/vllm-drop-caches";
/// The file `mem_available_mib` reads.
pub const MEMINFO_PATH: &str = "/proc/meminfo";
/// The default additive safety margin of `gate` (`floor_mib=10240`).
pub const DEFAULT_FLOOR_MIB: i64 = 10240;
/// How long the helper may run before it is killed.
pub const HELPER_TIMEOUT_SECONDS: u64 = 30;
/// What the default log writes before each message.
const LOG_PREFIX: &str = "[preload-gate] ";
/// The raw read size of Python's text layer; a decoding error names its position inside the chunk that held it.
const CHUNK_BYTES: usize = 8192;

// ── the operating-system seams ───────────────────────────────────────────────────────────────────

/// Where the memory figures are read from: `open(path, encoding="ascii")`, positioned at the start of the file.
pub trait MeminfoSource {
    fn open(&mut self) -> Res<Box<dyn Read>>;
}

/// The command `try_drop_caches` runs: `subprocess.run(argv, stdout=DEVNULL, stderr=DEVNULL, timeout=...)`. Standard
/// output and standard error are always the null device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperRequest {
    pub argv: [&'static str; 3],
    pub timeout_seconds: u64,
}

/// The request `try_drop_caches` makes: `sudo -n <helper>` with a thirty-second limit.
pub const DROP_REQUEST: HelperRequest = HelperRequest {
    argv: ["sudo", "-n", DROP_HELPER],
    timeout_seconds: HELPER_TIMEOUT_SECONDS,
};

/// How a helper run ended: with an exit status (a negative one is a signal), or with any exception of the original's
/// `except Exception` (not found, not permitted, timed out, any other `OSError`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperRun {
    Exited(i64),
    Failed,
}

/// The existence test and the run of the cache-dropping helper.
pub trait DropSystem {
    /// `os.path.exists(DROP_HELPER)`.
    fn helper_exists(&mut self) -> bool;
    fn run_helper(&mut self, request: &HelperRequest) -> HelperRun;
}

/// What `gate` asks of the machine, at the level the original's own callers replace it: the available memory in MiB (an
/// integer, or `None` when the file has no such line) and one attempt to drop the caches.
pub trait Provider {
    fn mem_available_mib(&mut self) -> Res<PyValue>;
    fn try_drop_caches(&mut self) -> Res<bool>;
}

/// A [`Provider`] and the places `gate` writes to.
pub trait Host: Provider {
    /// `sys.stderr.write(text)`: the complete text, newline included.
    fn write_stderr(&mut self, text: &PyStr) -> Res<()>;
    /// The caller's own `log(message)` callable. Only reached when the caller supplied a real callable.
    fn call_log(&mut self, message: &PyStr) -> Res<()>;
}

// ── mem_available_mib ────────────────────────────────────────────────────────────────────────────

/// The lines of an ASCII text file read the way Python's text layer reads one: raw chunks decoded as they arrive (a byte
/// above 127 is a `UnicodeDecodeError` naming its position in its chunk, and nothing of that chunk is used), `\r\n` and `\r`
/// read as `\n`, and a line handed over as soon as its end has arrived.
struct AsciiLines {
    reader: Box<dyn Read>,
    buffer: Vec<u8>,
    scanned: usize,
    carriage_held: bool,
    finished: bool,
}

impl AsciiLines {
    fn new(reader: Box<dyn Read>) -> Self {
        AsciiLines {
            reader,
            buffer: Vec::new(),
            scanned: 0,
            carriage_held: false,
            finished: false,
        }
    }

    fn next_line(&mut self) -> Res<Option<Vec<u8>>> {
        loop {
            let end = self.buffer[self.scanned..]
                .iter()
                .position(|&byte| byte == b'\n');
            if let Some(offset) = end {
                let rest = self.buffer.split_off(self.scanned + offset + 1);
                self.scanned = 0;
                return Ok(Some(std::mem::replace(&mut self.buffer, rest)));
            }
            self.scanned = self.buffer.len();
            if self.finished {
                self.scanned = 0;
                return Ok(if self.buffer.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.buffer))
                });
            }
            self.read_chunk()?;
        }
    }

    fn read_chunk(&mut self) -> Res<()> {
        let mut chunk = [0u8; CHUNK_BYTES];
        let count = loop {
            match self.reader.read(&mut chunk) {
                Ok(count) => break count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(os_failure(&error, None)),
            }
        };
        if count == 0 {
            self.finished = true;
            if self.carriage_held {
                self.carriage_held = false;
                self.buffer.push(b'\n');
            }
            return Ok(());
        }
        let chunk = &chunk[..count];
        if let Some(position) = chunk.iter().position(|&byte| byte >= 0x80) {
            return Err(Failure::new(
                "UnicodeDecodeError",
                &format!(
                    "'ascii' codec can't decode byte 0x{:02x} in position {position}: ordinal not in range(128)",
                    chunk[position]
                ),
            ));
        }
        for &byte in chunk {
            if self.carriage_held {
                self.carriage_held = false;
                self.buffer.push(b'\n');
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\r' {
                self.carriage_held = true;
            } else {
                self.buffer.push(byte);
            }
        }
        Ok(())
    }
}

/// Whether Python's `str.split()` treats an ASCII byte as a separator.
fn is_ascii_separator(byte: u8) -> bool {
    matches!(byte, 0x09..=0x0d | 0x1c..=0x20)
}

/// `mem_available_mib()`: the `MemAvailable` figure in MiB (`kB // 1024`, rounded down), `None` when no line starts with
/// `MemAvailable:`. A file that cannot be opened or decoded, or a line without a usable integer, is the exception Python raises;
/// reading stops at the first matching line.
pub fn mem_available_mib(source: &mut dyn MeminfoSource) -> Res<PyValue> {
    let mut lines = AsciiLines::new(source.open()?);
    while let Some(line) = lines.next_line()? {
        if !line.starts_with(b"MemAvailable:") {
            continue;
        }
        let token = line
            .split(|&byte| is_ascii_separator(byte))
            .filter(|piece| !piece.is_empty())
            .nth(1)
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))?;
        let text = String::from_utf8_lossy(token);
        let kilobytes = int_of(&PyValue::Str(PyStr::new(&text)))?;
        return Ok(PyValue::Int(int_floor_div(
            &kilobytes,
            &BigInt::from(1024),
        )?));
    }
    Ok(PyValue::None)
}

/// `open(path, "rb")` of a text file: a directory is the `IsADirectoryError` Python's `open` raises (the open itself succeeds;
/// the file object's own check of the descriptor refuses it).
pub fn open_text_file(path: &PyStr) -> Res<std::fs::File> {
    let file = open_binary(path)?;
    match file.metadata() {
        Ok(metadata) if metadata.is_dir() => Err(os_failure(
            &std::io::Error::from_raw_os_error(21),
            Some(path),
        )),
        _ => Ok(file),
    }
}

// ── try_drop_caches ──────────────────────────────────────────────────────────────────────────────

/// `try_drop_caches()`: `True` only when the helper exists and its run exits with status 0. A missing helper, a non-zero
/// status and every exception of the run are `False`; nothing is logged here.
pub fn try_drop_caches(system: &mut dyn DropSystem) -> bool {
    if !system.helper_exists() {
        return false;
    }
    system.run_helper(&DROP_REQUEST) == HelperRun::Exited(0)
}

// ── the sizes the model owner derives ────────────────────────────────────────────────────────────

fn optional_integer(value: Option<BigInt>) -> PyValue {
    value.map_or(PyValue::None, PyValue::Int)
}

/// `warnings if warnings is not None else []`: the caller's object, or a list nobody sees.
fn with_warnings<T>(
    warnings: Option<&mut PyValue>,
    body: impl FnOnce(&mut Warnings<'_>) -> Res<T>,
) -> Res<T> {
    let mut private = PyValue::List(Vec::new());
    let target = match warnings {
        Some(given) if !matches!(given.peel(), PyValue::None) => given,
        _ => &mut private,
    };
    body(&mut Warnings::of(target))
}

/// `checkpoint_bytes_for(host_path, warnings=None)`: the total bytes of the checkpoint files, the model owner's
/// `_native_weight_bytes`; `warnings` is the caller's list (written to in place) or `None`.
pub fn checkpoint_bytes_for(host_path: &PyValue, warnings: Option<&mut PyValue>) -> Res<PyValue> {
    with_warnings(warnings, |sink| {
        model_config::native_weight_bytes(host_path, sink).map(optional_integer)
    })
}

/// `offloadable_bytes_for(host_path, mechanism, warnings=None)`: the bytes an NVMe mmap keeps out of residency, the model
/// owner's `offloadable_weight_bytes` (`None` = not derivable, `0` = no such tensor).
pub fn offloadable_bytes_for(
    host_path: &PyValue,
    mechanism: &PyValue,
    warnings: Option<&mut PyValue>,
) -> Res<PyValue> {
    with_warnings(warnings, |sink| {
        model_config::offloadable_weight_bytes(host_path, mechanism, sink).map(optional_integer)
    })
}

// ── gate ─────────────────────────────────────────────────────────────────────────────────────────

/// What `log or (lambda m: print(...))` came to for the caller's `log`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogChoice {
    /// A falsy `log`: the default, one line on standard error.
    Default,
    /// A callable the caller holds: each message is handed to it.
    Callback,
    /// A truthy object that is not callable: calling it is the `TypeError` naming its type.
    NotCallable(String),
}

impl LogChoice {
    /// The choice for a `log` that is plain data (no data is ever callable): falsy gives the default.
    pub fn of_data(value: &PyValue) -> LogChoice {
        if value.is_truthy() {
            LogChoice::NotCallable(value.type_name().to_string())
        } else {
            LogChoice::Default
        }
    }
}

/// The arguments of `gate`, as the caller gave them (nothing is converted before the gate itself converts it).
pub struct GateArguments<'a> {
    pub checkpoint_bytes: &'a PyValue,
    pub tp: &'a PyValue,
    pub floor_mib: &'a PyValue,
    pub auto_drop: &'a PyValue,
    pub log: LogChoice,
}

fn log(host: &mut dyn Host, choice: &LogChoice, message: &PyStr) -> Res<()> {
    match choice {
        LogChoice::Default => {
            let mut line = PyStr::new(LOG_PREFIX);
            line.push_py(message);
            line.push_str("\n");
            host.write_stderr(&line)
        }
        LogChoice::Callback => host.call_log(message),
        LogChoice::NotCallable(type_name) => Err(Failure::new(
            "TypeError",
            &format!("'{type_name}' object is not callable"),
        )),
    }
}

fn message(parts: &[&str]) -> PyStr {
    PyStr::new(&parts.concat())
}

fn result(
    ok: bool,
    skipped: bool,
    required: PyValue,
    before: PyValue,
    after: PyValue,
    dropped: bool,
) -> PyDict {
    let mut row = PyDict::new();
    row.set("ok", PyValue::Bool(ok));
    row.set("skipped", PyValue::Bool(skipped));
    row.set("required_mib", required);
    row.set("avail_before_mib", before);
    row.set("avail_after_mib", after);
    row.set("dropped", PyValue::Bool(dropped));
    row
}

/// `checkpoint_bytes / 2**30`, Python's true division (one rounding; `OverflowError` when an integer's quotient is not a
/// float).
fn gibibytes(checkpoint: &PyValue) -> Res<f64> {
    match checkpoint.peel() {
        PyValue::Float(bytes) => Ok(bytes / 1_073_741_824.0),
        PyValue::Bool(flag) => int_div_power_of_two(&BigInt::from(u8::from(*flag)), 30),
        PyValue::Int(bytes) => int_div_power_of_two(bytes, 30),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "unsupported operand type(s) for /: '{}' and 'int'",
                other.type_name()
            ),
        )),
    }
}

/// `gate(checkpoint_bytes, tp=1, floor_mib=10240, auto_drop=True, log=None)`.
///
/// The order is the original's. A checkpoint size that is falsy or not above zero skips the gate: the skip is logged first and the
/// available memory read after, and `tp` and `floor_mib` are never looked at. Otherwise `tp` is at least 1, the requirement
/// `ceil(checkpoint / tp / MiB) + floor` is computed exactly, memory is read, a shortfall with `auto_drop` truthy logs and
/// tries one cache drop (logging again, after a re-measure when it worked), and the verdict follows with its log line. The
/// raw `floor_mib` and checkpoint are formatted as the original formats them, so a string floor is its `%d` `TypeError`.
pub fn gate(host: &mut dyn Host, arguments: &GateArguments<'_>) -> Res<PyDict> {
    let choice = &arguments.log;
    let checkpoint = arguments.checkpoint_bytes.peel();
    let skipped =
        !checkpoint.is_truthy() || compare(Compare::LessEqual, checkpoint, &PyValue::int(0))?;
    if skipped {
        log(
            host,
            choice,
            &message(&[
                "⚠ 체크포인트 크기 미상(index total_size 부재) — 게이트 생략(음성정직·false-block 금지)",
            ]),
        )?;
        let before = host.mem_available_mib()?;
        return Ok(result(
            true,
            true,
            PyValue::None,
            before,
            PyValue::None,
            false,
        ));
    }
    let tp = int_of(arguments.tp.peel())?.max(BigInt::one());
    let floor = arguments.floor_mib.peel();
    let ceiling = {
        let negated = -int_of(checkpoint)?;
        let per_node = int_floor_div(&negated, &tp)?;
        -int_floor_div(&per_node, &BigInt::from(MIB))?
    };
    let required = ceiling + int_of(floor)?;
    let before = host.mem_available_mib()?;
    let mut available = before.clone();
    let mut dropped = false;
    let required_value = PyValue::Int(required.clone());
    let short = !matches!(available.peel(), PyValue::None)
        && compare(Compare::Less, available.peel(), &required_value)?;
    if short && arguments.auto_drop.is_truthy() {
        log(
            host,
            choice,
            &message(&[
                "부족(MemAvailable=",
                &percent_d(&available)?,
                "MiB < required=",
                &required.to_string(),
                "MiB) → drop-caches 시도",
            ]),
        )?;
        dropped = host.try_drop_caches()?;
        if dropped {
            available = host.mem_available_mib()?;
            log(
                host,
                choice,
                &message(&[
                    "drop-caches 후 MemAvailable=",
                    &percent_d(&available)?,
                    "MiB",
                ]),
            )?;
        } else {
            log(
                host,
                choice,
                &message(&["drop-caches 헬퍼 미가용(미설치/무권한) — install_host_safety.sh 참조"]),
            )?;
        }
    }
    let ok = !matches!(available.peel(), PyValue::None)
        && compare(Compare::GreaterEqual, available.peel(), &required_value)?;
    if !ok {
        let gibibytes = gibibytes(checkpoint)?;
        let shown = available.display_text();
        let mut line = PyStr::new("거부: MemAvailable=");
        line.push_py(&shown);
        line.push_str("MiB < required=");
        line.push_str(&required.to_string());
        line.push_str("MiB (ckpt=");
        line.push_str(
            &percent_fixed(&PyValue::Float(gibibytes), 2).map_err(crate::pyexpr::exception)?,
        );
        line.push_str("GiB ÷ tp=");
        line.push_str(&tp.to_string());
        line.push_str(" + floor=");
        line.push_str(&percent_d(floor)?);
        line.push_str(
            "MiB) — 잔존 컨테이너/페이지캐시 정리 후 재시도 (헌법 호스트 안전체계 따름정리)",
        );
        log(host, choice, &line)?;
    }
    Ok(result(
        ok,
        false,
        required_value,
        before,
        available,
        dropped,
    ))
}
