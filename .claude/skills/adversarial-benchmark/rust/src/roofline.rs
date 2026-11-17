//! The decode-performance suspicion threshold: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/roofline.py`.
//!
//! `R_fp` is the forward passes per second a memory-bandwidth-bound decode could reach at 100 % utilisation,
//! `R_token = accept_len × R_fp` the token rate with speculative decoding, `expected_achievable` the MBU-corrected
//! acceptance line. The numbers are derived from a model directory (config + safetensors headers, never the
//! weights) and the hardware facts in a manifest. Every input problem is either a fatal [`Stop::Die`] (the
//! original's `_die`, exit 2) or an uncaught Python exception ([`Stop::Raise`], exit 1); nothing is silently
//! replaced by a default except where the original does so and says so in `notes` / `tp_source`.
//!
//! `accept_len` provenance (the closed vocabulary below) is owned here because this module emits the field; the
//! verdict module and the judge launcher import it.

use crate::pyjson::{decode_file_text, decode_utf8_strict, loads_text, LoadError};
use crate::pynumber::{float_call, float_from_int, int_call, round_to_places, PyException};
use crate::pyops::{compare, num_of, Compare};
use crate::pystr::{is_decimal_digit, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};
use std::path::PathBuf;

pub const GIB: u64 = 1024 * 1024 * 1024;

pub const ACCEPT_LEN_MEASURED: &str = "measured";
pub const ACCEPT_LEN_DECLARED: &str = "declared";
pub const ACCEPT_LEN_DECLARED_ABSENT: &str = "declared-absent";
pub const ACCEPT_LEN_ABSENT: &str = "absent";
pub const ACCEPT_LEN_SOURCES: [&str; 4] = [
    ACCEPT_LEN_MEASURED,
    ACCEPT_LEN_DECLARED,
    ACCEPT_LEN_DECLARED_ABSENT,
    ACCEPT_LEN_ABSENT,
];
/// The sources that carry a value; only these may build `R_token`.
pub const ACCEPT_LEN_VALUE_SOURCES: [&str; 2] = [ACCEPT_LEN_MEASURED, ACCEPT_LEN_DECLARED];

/// Peak memory bandwidth in GB/s (10^9 bytes/s) by case-insensitive substring of the GPU model, in lookup order.
pub const GPU_PEAK_BW_GBPS: [(&str, f64); 10] = [
    ("gb10", 273.0),
    ("rtx pro 6000", 1792.0),
    ("rtx 6000 ada", 960.0),
    ("rtx 4090", 1008.0),
    ("rtx 5090", 1792.0),
    ("h100", 3350.0),
    ("h200", 4800.0),
    ("a100", 2039.0),
    ("l40", 864.0),
    ("l40s", 864.0),
];

/// Why a run ended without a result.
#[derive(Debug)]
pub enum Stop {
    /// `_die`: `[roofline] ERROR: …` on stderr, exit 2.
    Die(PyStr),
    /// An exception nothing catches: traceback, exit 1.
    Raise(PyException),
}

impl From<PyException> for Stop {
    fn from(error: PyException) -> Self {
        Stop::Raise(error)
    }
}

fn die(text: PyStr) -> Stop {
    Stop::Die(text)
}

fn cat(parts: &[&PyStr]) -> PyStr {
    let mut out = PyStr::default();
    for part in parts {
        out.push_py(part);
    }
    out
}

fn lit(text: &str) -> PyStr {
    PyStr::new(text)
}

// ── accept_len ──────────────────────────────────────────────────────────────────

/// The state of an `accept_len` value (`accept_len_state`): valid means finite and at least 1.
#[derive(Clone, Debug, PartialEq)]
pub enum AcceptLenState {
    Unset,
    Invalid(PyValue),
    Valid(f64),
}

/// `accept_len_state(raw)`: the single predicate. A `bool` is not a number here; `OverflowError` escapes.
pub fn accept_len_state(raw: &PyValue) -> Result<AcceptLenState, PyException> {
    if matches!(raw, PyValue::None) {
        return Ok(AcceptLenState::Unset);
    }
    if matches!(raw, PyValue::Bool(_)) {
        return Ok(AcceptLenState::Invalid(raw.clone()));
    }
    let value = match float_call(raw) {
        Ok(value) => value,
        Err(error) if error.is_type_or_value_error() => {
            return Ok(AcceptLenState::Invalid(raw.clone()))
        }
        Err(error) => return Err(error),
    };
    if value.is_nan() || value.is_infinite() || value < 1.0 {
        return Ok(AcceptLenState::Invalid(raw.clone()));
    }
    Ok(AcceptLenState::Valid(value))
}

/// `resolve_accept_len(value, source)`: the combination contract of the two options. A contradiction is a
/// fatal error, never silently repaired.
pub fn resolve_accept_len(value: Option<f64>, source: Option<&str>) -> Result<(f64, String), Stop> {
    if let Some(value) = value {
        let source = source.unwrap_or(ACCEPT_LEN_DECLARED);
        if !ACCEPT_LEN_VALUE_SOURCES.contains(&source) {
            return Err(die(lit(&format!(
                "--accept-len {} 과 --accept-len-source {source} 는 모순이다 — {source} 는 값이 없다는 뜻이다",
                PyValue::Float(value).repr()
            ))));
        }
        return match accept_len_state(&PyValue::Float(value))? {
            AcceptLenState::Valid(valid) => Ok((valid, source.to_string())),
            _ => Err(die(lit(&format!(
                "--accept-len={} 는 유효한 평균 수용길이가 아니다(유한 ∧ ≥1) — 조용히 1.0 으로 눕히지 않는다(accept_len 은 R_token 과 expected_achievable 양쪽에 곱해진다)",
                PyValue::Float(value).repr()
            )))),
        };
    }
    let source = source.unwrap_or(ACCEPT_LEN_ABSENT);
    if ACCEPT_LEN_VALUE_SOURCES.contains(&source) {
        return Err(die(lit(&format!(
            "--accept-len-source {source} 는 값을 주장하는데 --accept-len 이 없다"
        ))));
    }
    Ok((1.0, source.to_string()))
}

// ── paths ───────────────────────────────────────────────────────────────────────

fn starts_with_points(text: &[u32], prefix: &[u32]) -> bool {
    text.starts_with(prefix)
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn rstrip_slashes(text: &str) -> Vec<u32> {
    let mut chars = points(text);
    while chars.last() == Some(&u32::from('/')) {
        chars.pop();
    }
    chars
}

/// `os.path.join(a, b)` for strings.
pub fn path_join(first: &PyStr, second: &PyStr) -> PyStr {
    let slash = u32::from('/');
    let second_points = second.code_points();
    if second_points.first() == Some(&slash) {
        return second.clone();
    }
    let mut out = first.code_points().to_vec();
    if !out.is_empty() && out.last() != Some(&slash) {
        out.push(slash);
    }
    out.extend_from_slice(second_points);
    PyStr::from_code_points(out)
}

/// `_resolve_host_path`: container paths (`/app/models/…`, `/app/quant_models/…`) become host paths.
pub fn resolve_host_path(
    model_path: &PyStr,
    nas_host_root: &PyStr,
    quant_host_root: Option<&PyStr>,
) -> Result<PyStr, Stop> {
    let slash = u32::from('/');
    let model = model_path.code_points();
    let under = |root: &[u32]| {
        model == root || {
            let mut prefix = root.to_vec();
            prefix.push(slash);
            starts_with_points(model, &prefix)
        }
    };
    let quant_prefix = rstrip_slashes("/app/quant_models");
    if under(&quant_prefix) {
        let Some(quant_root) = quant_host_root.filter(|root| !root.is_empty()) else {
            return Err(die(cat(&[
                &lit("manifest quant_model_path 미전달 — /app/quant_models 매핑 불가(추측 금지): "),
                model_path,
            ])));
        };
        let relative = lstrip_slashes(&model[quant_prefix.len()..]);
        return Ok(path_join(quant_root, &PyStr::from_code_points(relative)));
    }
    let nas_prefix = rstrip_slashes("/app/models");
    if under(&nas_prefix) {
        let relative = lstrip_slashes(&model[nas_prefix.len()..]);
        return Ok(path_join(nas_host_root, &PyStr::from_code_points(relative)));
    }
    Ok(model_path.clone())
}

fn lstrip_slashes(text: &[u32]) -> Vec<u32> {
    let start = text
        .iter()
        .position(|&c| c != u32::from('/'))
        .unwrap_or(text.len());
    text[start..].to_vec()
}

fn fs_path(path: &PyStr) -> Result<PathBuf, PyException> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = path
        .to_surrogateescape_bytes()
        .map_err(|failure| PyException::new("UnicodeEncodeError", failure.message()))?;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

fn is_dir(path: &PyStr) -> bool {
    fs_path(path).map(|p| p.is_dir()).unwrap_or(false)
}

fn is_file(path: &PyStr) -> bool {
    fs_path(path).map(|p| p.is_file()).unwrap_or(false)
}

/// The exception `open()` raises for an I/O failure: `[Errno N] strerror: 'path'` with the errno's subclass.
pub(crate) fn open_error(error: &std::io::Error, path: &PyStr) -> PyException {
    let class = match error.raw_os_error() {
        Some(2) => "FileNotFoundError",
        Some(13) => "PermissionError",
        Some(21) => "IsADirectoryError",
        Some(20) => "NotADirectoryError",
        _ => "OSError",
    };
    PyException::new(class, crate::cli::os_error_text(error, path))
}

fn read_file(path: &PyStr) -> Result<Vec<u8>, PyException> {
    std::fs::read(fs_path(path)?).map_err(|error| open_error(&error, path))
}

fn load_error(error: LoadError) -> PyException {
    match error {
        LoadError::Decode { .. } => {
            PyException::new("json.decoder.JSONDecodeError", error.to_string())
        }
        LoadError::IntegerTooLong { .. } => PyException::new("ValueError", error.to_string()),
        LoadError::Encoding(message) => PyException::new("UnicodeDecodeError", message),
        LoadError::TooDeep { .. } => {
            let text = error.to_string();
            PyException::new(
                "RecursionError",
                text.strip_prefix("RecursionError: ")
                    .unwrap_or(&text)
                    .to_string(),
            )
        }
    }
}

/// `json.load(open(path, encoding="utf-8"))`.
pub(crate) fn json_file(path: &PyStr) -> Result<PyValue, PyException> {
    let bytes = read_file(path)?;
    let text = decode_file_text(&bytes).map_err(load_error)?;
    loads_text(&text).map_err(load_error)
}

// ── model files ─────────────────────────────────────────────────────────────────

/// A safetensors header: `{name: {data_offsets: [start, end]}}` read from the first bytes only.
fn safetensors_header(path: &PyStr) -> Result<PyValue, PyException> {
    use std::io::Read;
    // Only the header is read - weight shards run to many gigabytes.
    let mut file = std::fs::File::open(fs_path(path)?).map_err(|error| open_error(&error, path))?;
    let mut first = Vec::new();
    (&mut file)
        .take(8)
        .read_to_end(&mut first)
        .map_err(|error| open_error(&error, path))?;
    if first.len() < 8 {
        return Err(PyException::new(
            "struct.error",
            "unpack requires a buffer of 8 bytes",
        ));
    }
    let length = u64::from_le_bytes(first[..8].try_into().expect("8 bytes"));
    // `f.read(n)` first allocates n bytes (`PyBytes_FromStringAndSize`): an n beyond ssize_t is an OverflowError,
    // an allocation that fails is a MemoryError. Whether n bytes can be had depends on the machine, so the same
    // question is asked of this one: reserve (not touch) n bytes and see. Only what the file holds is then read.
    read_request(length, |bytes| {
        Vec::<u8>::new().try_reserve_exact(bytes).is_ok()
    })?;
    let mut header = Vec::new();
    file.take(length)
        .read_to_end(&mut header)
        .map_err(|error| open_error(&error, path))?;
    let text = decode_utf8_strict(&header).map_err(load_error)?;
    loads_text(&text).map_err(load_error)
}

/// The checks `f.read(n)` makes before reading: `n` must fit a C `ssize_t`, and `n` bytes must be allocatable
/// (`can_allocate` answers that; it is injected so the decision is testable without exhausting memory).
pub(crate) fn read_request(
    n: u64,
    can_allocate: impl FnOnce(usize) -> bool,
) -> Result<(), PyException> {
    if n > i64::MAX as u64 {
        return Err(PyException::new(
            "OverflowError",
            "Python int too large to convert to C ssize_t",
        ));
    }
    match usize::try_from(n) {
        Ok(bytes) if can_allocate(bytes) => Ok(()),
        _ => Err(PyException::new("MemoryError", "")),
    }
}

fn is_mtp(name: &[u32]) -> bool {
    let dot = u32::from('.');
    let contains = |needle: &str| {
        let needle = points(needle);
        name.windows(needle.len())
            .any(|window| window == needle.as_slice())
    };
    let mtp = points("mtp");
    let mut at = 0;
    while at + mtp.len() < name.len() {
        if name[at..].starts_with(&mtp) && (at == 0 || name[at - 1] == dot) {
            let after = name[at + mtp.len()];
            if after == dot || after == u32::from('_') {
                return true;
            }
        }
        at += 1;
    }
    if contains(".mtp_block.") {
        return true;
    }
    let layers = points("model.layers.");
    let tail = points(".mtp");
    let mut start = 0;
    while start + layers.len() <= name.len() {
        if name[start..].starts_with(&layers) {
            let mut index = start + layers.len();
            let digits_from = index;
            while index < name.len() && is_decimal_digit(name[index]) {
                index += 1;
            }
            if index > digits_from && name[index..].starts_with(&tail) {
                return true;
            }
        }
        start += 1;
    }
    false
}

fn lookup_kind(name: &[u32]) -> Option<&'static str> {
    let contains = |needle: &str| {
        let needle = points(needle);
        name.windows(needle.len())
            .any(|window| window == needle.as_slice())
    };
    if contains(".ple.ple_embedding.") || contains(".ngram_embedding.") {
        return Some("ple");
    }
    // `\.engram\.embed\.(weight|scale)$` - `$` also matches before one trailing newline.
    let trimmed = name.strip_suffix(&[u32::from('\n')]).unwrap_or(name);
    for suffix in [".engram.embed.weight", ".engram.embed.scale"] {
        if name.ends_with(&points(suffix)) || trimmed.ends_with(&points(suffix)) {
            return Some("engram");
        }
    }
    None
}

/// `int(off[1]) - int(off[0])` for a header entry with a truthy `data_offsets` of length 2.
fn tensor_bytes(offsets: &PyValue) -> Result<Option<BigInt>, PyException> {
    let length = match offsets {
        PyValue::List(items) => items.len(),
        PyValue::Str(text) => text.len(),
        PyValue::Dict(map) => map.len(),
        other => {
            return Err(PyException::new(
                "TypeError",
                format!("object of type '{}' has no len()", other.type_name()),
            ))
        }
    };
    if length != 2 {
        return Ok(None);
    }
    let element = |index: usize| -> Result<PyValue, PyException> {
        match offsets {
            PyValue::List(items) => Ok(items[index].clone()),
            PyValue::Str(text) => Ok(PyValue::Str(PyStr::from_code_points(vec![
                text.code_points()[index],
            ]))),
            _ => Err(PyException::new("KeyError", index.to_string())),
        }
    };
    let end = int_call(&element(1)?)?;
    let start = int_call(&element(0)?)?;
    Ok(Some(end - start))
}

fn dict_items<'a>(value: &'a PyValue, method: &str) -> Result<&'a PyDict, PyException> {
    match value {
        PyValue::Dict(map) => Ok(map),
        other => Err(PyException::new(
            "AttributeError",
            format!("'{}' object has no attribute '{method}'", other.type_name()),
        )),
    }
}

/// `sorted(set(weight_map.values()))` as shard file names.
fn shard_names(weight_map: &PyDict) -> Result<Vec<PyStr>, PyException> {
    let mut names: Vec<PyStr> = Vec::new();
    let mut others: Vec<&PyValue> = Vec::new();
    for (_, value) in weight_map.iter() {
        match value {
            PyValue::Str(name) => {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            PyValue::List(_) | PyValue::Dict(_) => {
                return Err(PyException::new(
                    "TypeError",
                    format!("unhashable type: '{}'", value.type_name()),
                ))
            }
            other => others.push(other),
        }
    }
    if let Some(first) = others.first() {
        if !names.is_empty() || others.len() > 1 {
            let other_type = names
                .first()
                .map_or_else(|| others[1].type_name(), |_| "str");
            return Err(PyException::new(
                "TypeError",
                format!(
                    "'<' not supported between instances of '{other_type}' and '{}'",
                    first.type_name()
                ),
            ));
        }
        return Err(PyException::new(
            "TypeError",
            format!(
                "join() argument must be str, bytes, or os.PathLike object, not '{}'",
                first.type_name()
            ),
        ));
    }
    names.sort_by(|a, b| a.code_points().cmp(b.code_points()));
    Ok(names)
}

/// Active weight bytes per forward pass of a mixture-of-experts model, with the pieces that explain it.
pub struct MoeBytes {
    pub active: BigInt,
    pub dense: BigInt,
    pub routed: BigInt,
    pub experts_per_token: BigInt,
    pub experts: BigInt,
    pub lookup: BigInt,
    pub lookup_by_kind: Vec<(&'static str, BigInt)>,
}

fn add_kind(kinds: &mut Vec<(&'static str, BigInt)>, kind: &'static str, bytes: &BigInt) {
    match kinds.iter_mut().find(|(name, _)| *name == kind) {
        Some((_, total)) => *total += bytes,
        None => kinds.push((kind, bytes.clone())),
    }
}

/// `_active_bytes_moe`: categorise every tensor of the index (or single file) into per-token traffic.
pub fn active_bytes_moe(
    host_dir: &PyStr,
    num_experts: &PyValue,
    experts_per_token: &PyValue,
) -> Result<MoeBytes, Stop> {
    let index_path = path_join(host_dir, &lit("model.safetensors.index.json"));
    let shards: Vec<PyStr> = if is_file(&index_path) {
        let blob = json_file(&index_path)?;
        // `.get("weight_map", {})`: only an absent key falls back to {}; an explicit null stays None.
        let weight_map = match &blob {
            PyValue::Dict(map) => map
                .get("weight_map")
                .cloned()
                .unwrap_or_else(|| PyValue::Dict(PyDict::new())),
            other => crate::verdict::get(other, "weight_map")?.clone(),
        };
        shard_names(dict_items(&weight_map, "values")?)?
    } else {
        let single = path_join(host_dir, &lit("model.safetensors"));
        if is_file(&single) {
            vec![lit("model.safetensors")]
        } else {
            return Err(die(cat(&[
                &lit("safetensors index/단일파일 없음: "),
                host_dir,
            ])));
        }
    };

    let (mut dense, mut routed, mut lookup) = (BigInt::zero(), BigInt::zero(), BigInt::zero());
    let mut kinds: Vec<(&'static str, BigInt)> = Vec::new();
    for shard in &shards {
        let header = safetensors_header(&path_join(host_dir, shard))?;
        for (name, meta) in dict_items(&header, "items")?.iter() {
            let name_points = name.code_points();
            if name.equals("__metadata__") {
                continue;
            }
            let PyValue::Dict(meta) = meta else { continue };
            let offsets = meta.get("data_offsets").cloned().unwrap_or(PyValue::None);
            if !offsets.is_truthy() {
                continue;
            }
            let Some(bytes) = tensor_bytes(&offsets)? else {
                continue;
            };
            if is_mtp(name_points) {
                continue;
            }
            if let Some(kind) = lookup_kind(name_points) {
                lookup += &bytes;
                add_kind(&mut kinds, kind, &bytes);
                continue;
            }
            let routed_name = name_points
                .windows(9)
                .any(|window| window == points(".experts.").as_slice())
                && !name_points
                    .windows(13)
                    .any(|window| window == points("shared_expert").as_slice());
            if routed_name {
                routed += &bytes;
            } else {
                dense += &bytes;
            }
        }
    }
    if !num_experts.is_truthy() || compare(Compare::LessEqual, num_experts, &PyValue::int(0))? {
        return Err(die(lit("MoE 인데 num_experts 불명 — config 확인")));
    }
    let k = int_call(&if experts_per_token.is_truthy() {
        experts_per_token.clone()
    } else {
        PyValue::int(0)
    })?;
    let n = int_call(num_experts)?;
    if !k.is_positive() {
        return Err(die(lit("MoE 인데 top-k(num_experts_per_tok 류) 불명 — config 키를 확인하고 roofline.sh 의 pick 목록에 등재하라")));
    }
    let ratio = true_divide(&k, &n)?;
    let routed_float = float_from_int(&routed)?;
    let active_routed = crate::pynumber::int_from_float(routed_float * ratio)?;
    Ok(MoeBytes {
        active: &dense + &active_routed,
        dense,
        routed,
        experts_per_token: k,
        experts: n,
        lookup,
        lookup_by_kind: kinds,
    })
}

/// `_total_bytes_dense`: the sum of tensor bytes over the shards named by the index (the header offsets are the
/// authority; `metadata.total_size` is only cross-checked and a mismatch is announced on stderr).
pub fn total_bytes_dense(host_dir: &PyStr, stderr: &mut String) -> Result<BigInt, Stop> {
    let index_path = path_join(host_dir, &lit("model.safetensors.index.json"));
    let mut shard_list: Option<Vec<PyStr>> = None;
    let mut total_size = PyValue::None;
    if is_file(&index_path) {
        let blob = json_file(&index_path)?;
        let weight_map = crate::verdict::get(&blob, "weight_map")?;
        if let PyValue::Dict(map) = weight_map {
            if !map.is_empty() {
                let candidates = shard_names(map)?;
                if candidates
                    .iter()
                    .all(|name| is_file(&path_join(host_dir, name)))
                {
                    shard_list = Some(candidates);
                }
            }
        }
        let metadata = match &blob {
            PyValue::Dict(map) => map
                .get("metadata")
                .cloned()
                .unwrap_or_else(|| PyValue::Dict(PyDict::new())),
            other => other.clone(),
        };
        total_size = crate::verdict::get(&metadata, "total_size")?.clone();
    }
    let names: Vec<PyStr> = match shard_list {
        Some(list) => list,
        None => {
            let mut listed: Vec<PyStr> = std::fs::read_dir(fs_path(host_dir)?)
                .map_err(|error| open_error(&error, host_dir))?
                .filter_map(Result::ok)
                .map(|entry| {
                    use std::os::unix::ffi::OsStrExt;
                    PyStr::from_surrogateescape(entry.file_name().as_bytes())
                })
                .collect();
            listed.sort_by(|a, b| a.code_points().cmp(b.code_points()));
            listed
        }
    };
    let mut total = BigInt::zero();
    for name in names {
        if !name.code_points().ends_with(&points(".safetensors")) {
            continue;
        }
        let header = safetensors_header(&path_join(host_dir, &name))?;
        for (key, meta) in dict_items(&header, "items")?.iter() {
            if key.equals("__metadata__") {
                continue;
            }
            let PyValue::Dict(meta) = meta else { continue };
            let offsets = meta.get("data_offsets").cloned().unwrap_or(PyValue::None);
            if !offsets.is_truthy() {
                continue;
            }
            if let Some(bytes) = tensor_bytes(&offsets)? {
                total += bytes;
            }
        }
    }
    if !total.is_positive() {
        return Err(die(cat(&[&lit("가중치 바이트 산출 실패: "), host_dir])));
    }
    if total_size.is_truthy() {
        let declared = float_call(&total_size)?;
        let measured = float_from_int(&total)?;
        let ratio = declared / measured;
        if !(0.9..=1.1).contains(&ratio) {
            let gib = PyValue::int(GIB);
            let declared_gib = crate::pyops::divide(&total_size, &gib)?;
            let measured_gib = true_divide(&total, &BigInt::from(GIB))?;
            stderr.push_str(&format!(
                "[roofline] ⚠ index metadata.total_size={} GiB 가 실제 텐서 합 {} GiB 의 {}배 — 발행자 오기재. 실측 헤더 합을 채택한다.\n",
                crate::pynumber::format_fixed(declared_gib, 2),
                crate::pynumber::format_fixed(measured_gib, 2),
                crate::pynumber::format_fixed(ratio, 2)
            ));
        }
    }
    Ok(total)
}

/// Python `int / int` (true division): correctly rounded, `ZeroDivisionError` on a zero divisor.
pub fn true_divide(numerator: &BigInt, denominator: &BigInt) -> Result<f64, PyException> {
    if denominator.is_zero() {
        return Err(PyException::new("ZeroDivisionError", "division by zero"));
    }
    let negative = (numerator.sign() == Sign::Minus) != (denominator.sign() == Sign::Minus);
    let (a, b) = (numerator.abs(), denominator.abs());
    if a.is_zero() {
        return Ok(if negative { -0.0 } else { 0.0 });
    }
    // Scale so the integer quotient has at least 66 significant bits, keep a sticky bit for the remainder.
    let shift = 66i64 - (a.bits() as i64 - b.bits() as i64);
    let (scaled_a, scaled_b) = if shift >= 0 {
        (a << shift as usize, b)
    } else {
        (a, b << (-shift) as usize)
    };
    let quotient = &scaled_a / &scaled_b;
    let sticky = !(&scaled_a % &scaled_b).is_zero();
    let mut integer = quotient;
    if sticky {
        integer = (integer << 1usize) | BigInt::from(1u8);
    } else {
        integer <<= 1usize;
    }
    let mantissa = integer.to_f64().expect("fits a double");
    let exponent = -(shift + 1);
    let value = scale_by_power_of_two(mantissa, exponent);
    if value.is_infinite() {
        return Err(PyException::new(
            "OverflowError",
            "integer division result too large for a float",
        ));
    }
    Ok(if negative { -value } else { value })
}

fn scale_by_power_of_two(mut value: f64, mut exponent: i64) -> f64 {
    while exponent > 1000 {
        value *= 2f64.powi(1000);
        exponent -= 1000;
    }
    while exponent < -1000 {
        value *= 2f64.powi(-1000);
        exponent += 1000;
    }
    value * 2f64.powi(exponent as i32)
}

// ── the run ─────────────────────────────────────────────────────────────────────

/// The command-line values of `roofline.py` after `argparse` conversion.
#[derive(Clone, Debug)]
pub struct RooflineOptions {
    pub model_path: PyStr,
    pub nas_root: PyStr,
    pub quant_root: Option<PyStr>,
    pub manifest: Option<PyStr>,
    pub tp: Option<BigInt>,
    pub accept_len: Option<f64>,
    pub accept_len_source: Option<String>,
    pub accept_len_evidence: Option<PyStr>,
    pub bandwidth_gbps: Option<f64>,
    pub interconnect_gbps: Option<f64>,
    pub act_dtype_bytes: BigInt,
    pub comm_latency_us_per_layer: f64,
    pub realistic_fraction: f64,
}

impl RooflineOptions {
    pub fn new(model_path: PyStr) -> Self {
        RooflineOptions {
            model_path,
            nas_root: lit("/mnt/models"),
            quant_root: None,
            manifest: None,
            tp: None,
            accept_len: None,
            accept_len_source: None,
            accept_len_evidence: None,
            bandwidth_gbps: None,
            interconnect_gbps: None,
            act_dtype_bytes: BigInt::from(2),
            comm_latency_us_per_layer: 0.0,
            realistic_fraction: 0.35,
        }
    }
}

/// What `roofline.py` produced before it finished or stopped: the stderr text written on the way and the result.
pub struct RooflineRun {
    pub stderr: String,
    pub result: Result<PyDict, Stop>,
}

/// Runs the whole computation (the original's `main` after argument parsing).
pub fn run(options: &RooflineOptions) -> RooflineRun {
    let mut stderr = String::new();
    let result = compute(options, &mut stderr);
    RooflineRun { stderr, result }
}

/// `yaml.safe_load(open(path, encoding="utf-8")) or {}`.
fn load_manifest(path: &PyStr) -> Result<PyValue, PyException> {
    let bytes = read_file(path)?;
    let document = crate::pyyaml::safe_load(&bytes, path).map_err(|error| {
        PyException::new(
            error.class,
            error
                .message
                .to_utf8()
                .unwrap_or_else(|| error.message.repr()),
        )
    })?;
    Ok(if document.is_truthy() {
        document
    } else {
        PyValue::Dict(PyDict::new())
    })
}

fn config_pick(tc: &PyDict, cfg: &PyDict, keys: &[&str]) -> PyValue {
    for key in keys {
        if let Some(value) = tc.get(key) {
            if !matches!(value, PyValue::None) {
                return value.clone();
            }
        }
        if let Some(value) = cfg.get(key) {
            if !matches!(value, PyValue::None) {
                return value.clone();
            }
        }
    }
    PyValue::None
}

fn compute(options: &RooflineOptions, stderr: &mut String) -> Result<PyDict, Stop> {
    let (accept, accept_source) =
        resolve_accept_len(options.accept_len, options.accept_len_source.as_deref())?;

    let host_dir = resolve_host_path(
        &options.model_path,
        &options.nas_root,
        options.quant_root.as_ref(),
    )?;
    if !is_dir(&host_dir) {
        return Err(die(cat(&[
            &lit("모델 디렉토리 없음: "),
            &host_dir,
            &lit(" (다운로드 금지·중단)"),
        ])));
    }

    let config_path = path_join(&host_dir, &lit("config.json"));
    if !is_file(&config_path) {
        return Err(die(cat(&[
            &lit("config.json 없음: "),
            &config_path,
            &lit(" (모델 부재 — 다운로드 금지)"),
        ])));
    }
    let cfg_value = json_file(&config_path)?;
    let cfg = dict_items(&cfg_value, "get")?;
    let empty = PyDict::new();
    // The nested language-model config through the config owner's one resolver (plan_26111708 B L9): `text_config` alone
    // missed an omni/VL config that nests it as `llm_config` (MoE undetected → full weights counted as active bytes). An
    // ambiguous config (two candidates) stops here instead of silently reading one.
    let tc = match recipe_runtime::model_config::text_config_of(cfg) {
        Ok(found) => found.nested.unwrap_or(&empty),
        Err(failure) => return Err(die(cat(&[&lit("config.json 텍스트 config 해소 불가: "), &failure.line()]))),
    };
    let hidden = config_pick(tc, cfg, &["hidden_size"]);
    let n_layers = config_pick(tc, cfg, &["num_hidden_layers"]);
    let num_experts = config_pick(
        tc,
        cfg,
        &["num_experts", "n_routed_experts", "num_local_experts"],
    );
    let experts_per_token = config_pick(
        tc,
        cfg,
        &[
            "num_experts_per_tok",
            "num_experts_per_token",
            "moe_topk",
            "top_k_experts",
            "experts_top_k",
        ],
    );
    let is_moe = num_experts.is_truthy();
    let mut notes: Vec<PyValue> = Vec::new();

    // --- hardware (manifest) ---
    let mut gpu_model = PyValue::None;
    let mut gpus_per_node = BigInt::from(1);
    let mut manifest_interconnect = PyValue::None;
    let mut manifest_topology = PyValue::None;
    let manifest_given = options.manifest.as_ref().filter(|path| !path.is_empty());
    let manifest_present = manifest_given.filter(|path| is_file(path));
    if let Some(manifest) = manifest_present {
        let man = load_manifest(manifest).map_err(|error| {
            die(cat(&[
                &lit("manifest 파싱 실패("),
                manifest,
                &lit("): "),
                &lit(error.class),
                &lit(": "),
                &lit(&error.message),
                &lit(" — 기대치 산출의 HW 입력이 없다(fail-loud · audit_26090515 B4)"),
            ]))
        })?;
        gpu_model = crate::verdict::get(&man, "gpu_model")?.clone();
        let raw_gpus = crate::verdict::get(&man, "gpus_per_node")?.clone();
        let parsed = match int_call(&raw_gpus) {
            Ok(number) => Some(number),
            Err(error) if error.is_type_or_value_error() => None,
            Err(error) => return Err(error.into()),
        };
        match parsed {
            Some(number) if number >= BigInt::from(1) => gpus_per_node = number,
            _ => {
                return Err(die(cat(&[
                    &lit("manifest("),
                    manifest,
                    &lit(") 의 gpus_per_node 부재/비정수("),
                    &lit(&raw_gpus.repr()),
                    &lit(") — TP 는 HW 사실이지 기본값이 아니다. terraforming_node 스캔으로 채우세요(fail-loud · 침묵 폴백 ✗ · audit_26090515 B4)"),
                ])))
            }
        }
        manifest_topology = crate::verdict::get(&man, "topology")?.clone();
        let interconnect = crate::verdict::get(&man, "interconnect")?.clone();
        let interconnect = if interconnect.is_truthy() {
            interconnect
        } else {
            PyValue::Dict(PyDict::new())
        };
        manifest_interconnect = crate::verdict::get(&interconnect, "bandwidth_gbps")?.clone();
    }

    // --- tensor parallel size and where it came from ---
    let mut tp: Option<BigInt> = options.tp.clone();
    let mut tp_source: Option<String> = options.tp.as_ref().map(|_| "explicit(--tp)".to_string());
    if tp.is_none() {
        if let Some(manifest) = manifest_present {
            match derive_tp(manifest, &manifest_topology, &gpus_per_node) {
                Ok((derived, source)) => {
                    tp = derived;
                    tp_source = source;
                }
                Err(error) => {
                    tp = None;
                    notes.push(PyValue::Str(lit(&format!(
                        "tp manifest 파싱 실패({}: {}) → tp 도출 불가",
                        error.class, error.message
                    ))));
                    tp_source = Some("fallback(manifest 파싱 실패)".to_string());
                }
            }
        }
    }
    let tp = match tp {
        Some(tp) => tp,
        None => {
            let source = tp_source.clone().unwrap_or_else(|| {
                if manifest_given.is_none() {
                    "fallback(manifest 부재/미지정)".to_string()
                } else {
                    "fallback(manifest 에 tp 근거 없음)".to_string()
                }
            });
            tp_source = Some(source.clone());
            notes.push(PyValue::Str(lit(&format!(
                "tp 미지정 → 1 가정 [{source}] — TP 과소평가는 기대치를 낮춰 느린 서빙을 통과시킬 수 있다"
            ))));
            BigInt::from(1)
        }
    };

    let (bandwidth_gbps, bandwidth_source) = bandwidth(&gpu_model, options.bandwidth_gbps)?;
    let interconnect_gbps: PyValue = match options.interconnect_gbps {
        Some(value) if value != 0.0 => PyValue::Float(value),
        _ => manifest_interconnect,
    };

    // --- active bytes per forward pass ---
    let (active_bytes, lookup_bytes, lookup_by_kind): (
        BigInt,
        BigInt,
        Vec<(&'static str, BigInt)>,
    );
    if is_moe {
        let moe = active_bytes_moe(&host_dir, &num_experts, &experts_per_token)?;
        notes.push(PyValue::Str(lit(&format!(
            "MoE active = dense {}GiB + ({}/{})×routed {}GiB",
            crate::pynumber::format_fixed(true_divide(&moe.dense, &BigInt::from(GIB))?, 2),
            moe.experts_per_token,
            moe.experts,
            crate::pynumber::format_fixed(true_divide(&moe.routed, &BigInt::from(GIB))?, 2)
        ))));
        active_bytes = moe.active;
        lookup_bytes = moe.lookup;
        lookup_by_kind = moe.lookup_by_kind;
    } else {
        active_bytes = total_bytes_dense(&host_dir, stderr)?;
        notes.push(PyValue::Str(lit("dense 모델 — active = 전 가중치")));
        lookup_bytes = BigInt::zero();
        lookup_by_kind = Vec::new();
    }
    if lookup_bytes.is_positive() {
        let mut kinds: Vec<&str> = lookup_by_kind.iter().map(|(kind, _)| *kind).collect();
        kinds.sort_unstable();
        notes.push(PyValue::Str(lit(&format!(
            "sparse lookup 테이블 {}GiB 는 active 제외(행 gather; 종류={}) — 전체 테이블을 합산하면 active 가 {}GiB 로 과대계산된다",
            crate::pynumber::format_fixed(true_divide(&lookup_bytes, &BigInt::from(GIB))?, 2),
            kinds.join(","),
            crate::pynumber::format_fixed(true_divide(&(&active_bytes + &lookup_bytes), &BigInt::from(GIB))?, 2)
        ))));
    }
    let kind_bytes = |kind: &str| {
        lookup_by_kind
            .iter()
            .find(|(name, _)| *name == kind)
            .map_or_else(BigInt::zero, |(_, bytes)| bytes.clone())
    };
    let ple_bytes = kind_bytes("ple");
    let engram_bytes = kind_bytes("engram");

    let per_node_read = true_divide(&active_bytes, &tp)?;
    let bandwidth_bytes = bandwidth_gbps * 1e9;
    let t_weight = float_division(per_node_read, bandwidth_bytes)?;

    // --- communication (multi-node tensor parallelism) ---
    let (t_comm, _comm_bytes);
    if tp > BigInt::from(1)
        && hidden.is_truthy()
        && n_layers.is_truthy()
        && interconnect_gbps.is_truthy()
    {
        let tp_f = float_from_int(&tp)?;
        let tp_minus_one = float_from_int(&(&tp - 1))?;
        let mut comm = 2.0 * tp_minus_one;
        comm = float_division(comm, tp_f)?;
        comm *= float_call(&hidden)?;
        comm *= float_from_int(&options.act_dtype_bytes)?;
        let layers = float_call(&n_layers)?;
        comm *= layers;
        let ic_bytes = float_call(&interconnect_gbps)? * 1e9 / 8.0;
        t_comm =
            float_division(comm, ic_bytes)? + options.comm_latency_us_per_layer * layers * 1e-6;
        _comm_bytes = comm;
    } else {
        _comm_bytes = 0.0;
        t_comm = 0.0;
        if tp > BigInt::from(1) && !interconnect_gbps.is_truthy() {
            notes.push(PyValue::Str(lit(
                "tp>1 인데 interconnect 대역폭 불명 → comm 무시(과소추정 주의)",
            )));
        }
    }

    let denominator = t_weight + t_comm;
    if denominator <= 0.0 {
        return Err(die(lit("루프라인 분모 0 — 입력 확인")));
    }
    let r_fp = 1.0 / denominator;
    let r_token = accept * r_fp;
    let expected = options.realistic_fraction * r_token;
    let comm_bound = t_comm > t_weight;

    let float = PyValue::Float;
    let mut out = PyDict::new();
    out.set("model_path", PyValue::Str(options.model_path.clone()));
    out.set("is_moe", PyValue::Bool(is_moe));
    out.set("tp", PyValue::Int(tp.clone()));
    out.set(
        "tp_source",
        PyValue::Str(lit(&tp_source.unwrap_or_default())),
    );
    out.set("gpu_model", gpu_model);
    out.set("bandwidth_gbps", float(bandwidth_gbps));
    out.set("bandwidth_source", PyValue::Str(lit(&bandwidth_source)));
    out.set("interconnect_gbps", interconnect_gbps);
    out.set("active_bytes", PyValue::Int(active_bytes.clone()));
    out.set(
        "active_gib",
        float(round_to_places(
            true_divide(&active_bytes, &BigInt::from(GIB))?,
            3,
        )),
    );
    out.set("ple_excluded_bytes", PyValue::Int(ple_bytes.clone()));
    out.set(
        "ple_excluded_gib",
        float(round_to_places(
            true_divide(&ple_bytes, &BigInt::from(GIB))?,
            3,
        )),
    );
    out.set("engram_excluded_bytes", PyValue::Int(engram_bytes.clone()));
    out.set(
        "engram_excluded_gib",
        float(round_to_places(
            true_divide(&engram_bytes, &BigInt::from(GIB))?,
            3,
        )),
    );
    out.set("sparse_lookup_excluded_bytes", PyValue::Int(lookup_bytes));
    out.set(
        "sparse_lookup_excluded_source",
        PyValue::Str(lit(if is_moe {
            "measured:safetensors header data_offsets; row-lookup tensor name classification"
        } else {
            "not-applicable:dense total-weight accounting"
        })),
    );
    out.set(
        "per_node_read_gib",
        float(round_to_places(per_node_read / GIB as f64, 3)),
    );
    out.set("t_weight_ms", float(round_to_places(t_weight * 1e3, 4)));
    out.set("t_comm_ms", float(round_to_places(t_comm * 1e3, 4)));
    out.set("comm_bound", PyValue::Bool(comm_bound));
    out.set("accept_len", float(accept));
    out.set("accept_len_source", PyValue::Str(lit(&accept_source)));
    out.set(
        "accept_len_evidence",
        options
            .accept_len_evidence
            .clone()
            .map_or(PyValue::None, PyValue::Str),
    );
    out.set("R_fp", float(round_to_places(r_fp, 2)));
    out.set("R_token", float(round_to_places(r_token, 2)));
    out.set("realistic_fraction", float(options.realistic_fraction));
    out.set("expected_achievable", float(round_to_places(expected, 2)));
    out.set("notes", PyValue::List(notes));
    out.set(
        "caveat",
        PyValue::Str(lit("R 은 의심 임계(낙관적 천장). 측정 M·외부 레퍼런스 E 가 진실. spec on 이면 R_token 과, off 면 R_fp 와 비교.")),
    );
    Ok(out)
}

/// `float / float` with Python's `ZeroDivisionError`.
fn float_division(numerator: f64, denominator: f64) -> Result<f64, PyException> {
    if denominator == 0.0 {
        Err(PyException::new(
            "ZeroDivisionError",
            "float division by zero",
        ))
    } else {
        Ok(numerator / denominator)
    }
}

/// The manifest-derived tensor-parallel size: single-topology manifests count only their own node.
fn derive_tp(
    manifest: &PyStr,
    topology: &PyValue,
    gpus_per_node: &BigInt,
) -> Result<(Option<BigInt>, Option<String>), PyException> {
    let man = load_manifest(manifest)?;
    let single = match topology {
        PyValue::Str(text) => text.code_points().starts_with(&points("single")),
        other if !other.is_truthy() => false,
        other => {
            return Err(PyException::new(
                "AttributeError",
                format!(
                    "'{}' object has no attribute 'startswith'",
                    other.type_name()
                ),
            ))
        }
    };
    let one = BigInt::from(1);
    let per_node = std::cmp::max(one.clone(), gpus_per_node.clone());
    if single {
        return Ok((
            Some(per_node),
            Some("manifest(single: gpus_per_node)".to_string()),
        ));
    }
    let nodes = crate::verdict::get(&man, "nodes")?;
    if !nodes.is_truthy() {
        return Ok((None, None));
    }
    let count = match nodes {
        PyValue::List(items) => items.len(),
        PyValue::Str(text) => text.len(),
        PyValue::Dict(map) => map.len(),
        other => {
            return Err(PyException::new(
                "TypeError",
                format!("object of type '{}' has no len()", other.type_name()),
            ))
        }
    };
    let nodes = std::cmp::max(one, BigInt::from(count));
    Ok((
        Some(nodes * per_node),
        Some("manifest(nodes×gpus_per_node)".to_string()),
    ))
}

/// `_bandwidth_gbps(gpu_model, override)`.
fn bandwidth(gpu_model: &PyValue, override_value: Option<f64>) -> Result<(f64, String), Stop> {
    if let Some(value) = override_value.filter(|value| *value != 0.0) {
        return Ok((value, "override".to_string()));
    }
    let model = if gpu_model.is_truthy() {
        match gpu_model {
            PyValue::Str(text) => lowercase(text),
            other => {
                return Err(PyException::new(
                    "AttributeError",
                    format!("'{}' object has no attribute 'lower'", other.type_name()),
                )
                .into())
            }
        }
    } else {
        String::new()
    };
    for (key, bandwidth) in GPU_PEAK_BW_GBPS {
        if model.contains(key) {
            return Ok((bandwidth, format!("lookup:{key}")));
        }
    }
    Err(die(lit(&format!(
        "gpu_model '{}' 대역폭 미상 — --bandwidth-gbps 로 명시(추측 금지)",
        gpu_model.display_text().to_utf8().unwrap_or_default()
    ))))
}

/// `str.lower()` for the characters that matter to the substring lookup (ASCII keys): Unicode simple lowering
/// is applied so that e.g. a Kelvin sign (K) lowers to `k` exactly as in Python.
fn lowercase(text: &PyStr) -> String {
    text.code_points()
        .iter()
        .filter_map(|&point| char::from_u32(point))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether `value` is a number that Python would accept where the original calls `float()` on a count
/// (used by the API layer's conversions).
pub fn is_real_number(value: &PyValue) -> bool {
    num_of(value).is_some()
}

#[cfg(test)]
mod request_tests {
    use super::*;

    #[test]
    fn read_request_models_ssize_t_and_allocation_failure() {
        assert!(read_request(0, |_| true).is_ok());
        assert!(
            read_request(1 << 62, |_| true).is_ok(),
            "an allocation the machine grants is not refused by a cap"
        );
        assert_eq!(
            read_request(1 << 62, |_| false).unwrap_err().class,
            "MemoryError"
        );
        assert_eq!(
            read_request(1 << 62, |_| false).unwrap_err().to_string(),
            "MemoryError"
        );
        assert_eq!(
            read_request(u64::MAX, |_| true).unwrap_err().class,
            "OverflowError"
        );
        assert_eq!(
            read_request(i64::MAX as u64 + 1, |_| true)
                .unwrap_err()
                .class,
            "OverflowError"
        );
        assert_eq!(
            read_request(i64::MAX as u64, |_| false).unwrap_err().class,
            "MemoryError"
        );
    }

    #[test]
    fn the_real_reservation_probe_refuses_an_absurd_request_without_touching_memory() {
        assert_eq!(
            read_request(1 << 62, |bytes| Vec::<u8>::new()
                .try_reserve_exact(bytes)
                .is_ok())
            .unwrap_err()
            .class,
            "MemoryError"
        );
    }
}
