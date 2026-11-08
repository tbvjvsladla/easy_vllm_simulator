//! `crosscheck_model_card.py`: the cross-check of a model directory's bundled README (the HF model card), `config.json`,
//! `inference/requirements.txt` and the safetensors headers, plus the external parameter-total check against the public
//! Hugging Face API. Ported function by function from the original source, with its dynamic typing, its known quirks and its
//! messages. The regular expressions of the original are hand-written matchers over code points that reuse the common
//! Python-compatible word/space tables and the Unicode 15 lower-casing of the support package.

use crate::model_config::{count_mxfp4_params, fixed};
use crate::model_dyn::{
    compare_text, contains, dict_from_pairs, ends_with, float_of, int_of, int_over_float,
    int_over_int, is_os_error, is_value_error, rstrip_slashes, s, starts_with, KeyMap, Message,
};
use crate::model_files::{
    basename, exists, fspath, glob, is_dir, is_file, join, join_values, load_json_file,
    loads_bytes, lower, open_binary, read_text_replacing, read_up_to,
};
use crate::pyexpr::Res;
use crate::quant_table;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{as_get_target, ensure_hashable};
use agent_runtime_support::pynumber::round_to_places;
use agent_runtime_support::pyops::Compare;
use agent_runtime_support::pysort::py_eq;
use agent_runtime_support::pystr::{is_python_space, is_python_word, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::Zero;

/// `HF_API_BASE`: the public Hugging Face model API.
pub const HF_API_BASE: &str = "https://huggingface.co/api/models";

/// `KNOWN_SPECIAL_DEPS`: kernels and libraries a novel architecture may need installed, with why that matters.
pub const KNOWN_SPECIAL_DEPS: [(&str, &str); 12] = [
    (
        "deepgemm",
        "FP8 블록 GEMM / DeepSeek 희소어텐션(DSA) 커널 — 미설치 시 SparseAttnIndexer RuntimeError",
    ),
    ("tilelang", "타일 기반 커널 DSL(DeepSeek inference 커널)"),
    (
        "fast_hadamard_transform",
        "Hadamard 변환 커널(양자화/rotation)",
    ),
    ("flash_attn", "FlashAttention 커널"),
    ("flashinfer", "FlashInfer 어텐션/MoE 커널"),
    ("sgl_kernel", "SGLang 커널"),
    ("sgl-kernel", "SGLang 커널"),
    ("causal_conv1d", "Mamba/SSM 커널"),
    ("mamba_ssm", "Mamba SSM 커널"),
    ("triton", "Triton JIT 커널"),
    ("xformers", "xFormers 어텐션"),
    ("apex", "NVIDIA Apex(fused 커널/norm)"),
];

/// Precision classes of safetensors dtypes (the original's `_FP8`, `_SCALE`, `_HIGH`, `_INTPACK`).
pub const FP8_DTYPES: [&str; 4] = ["F8_E4M3", "F8_E5M2", "FLOAT8_E4M3FN", "FLOAT8_E5M2"];
pub const SCALE_DTYPES: [&str; 3] = ["F8_E8M0", "F8_UE8M0", "UE8M0"];
pub const HIGH_DTYPES: [&str; 6] = ["BF16", "F16", "F32", "FLOAT16", "BFLOAT16", "FLOAT32"];
pub const INTPACK_DTYPES: [&str; 6] = ["I8", "U8", "UINT8", "INT8", "I4", "U4"];

fn in_set(value: &PyValue, set: &[&str]) -> bool {
    match value.peel() {
        PyValue::Str(text) => set.iter().any(|name| text.equals(name)),
        _ => false,
    }
}

// ── the original's regular expressions, as matchers ─────────────────────────────────────────────

fn code(c: char) -> u32 {
    u32::from(c)
}

/// `sre_lower_unicode`: the first character of the lower-case mapping (the simple mapping the `re` module compares with).
fn fold(point: u32) -> u32 {
    if point < 128 {
        return u32::from((point as u8).to_ascii_lowercase());
    }
    match char::from_u32(point) {
        Some(c) => agent_runtime_support::pylower::python_lower(&c.to_string())
            .chars()
            .next()
            .map_or(point, u32::from),
        None => point,
    }
}

/// A literal pattern character against an input character under `re.IGNORECASE`: equal after lower-casing, plus the
/// extra equivalences `re` adds for the letters `i` and `s` (dotless i, long s).
fn ci_eq(input: u32, literal: char) -> bool {
    let wanted = fold(code(literal));
    let got = fold(input);
    got == wanted || (wanted == code('i') && got == 0x131) || (wanted == code('s') && got == 0x17F)
}

/// Whether the literal text matches at `at` under `re.IGNORECASE` (returns the end).
fn ci_literal(points: &[u32], at: usize, literal: &str) -> Option<usize> {
    let mut index = at;
    for c in literal.chars() {
        if index >= points.len() || !ci_eq(points[index], c) {
            return None;
        }
        index += 1;
    }
    Some(index)
}

fn skip_space(points: &[u32], mut at: usize) -> usize {
    while at < points.len() && is_python_space(points[at]) {
        at += 1;
    }
    at
}

fn word_at(points: &[u32], index: usize) -> bool {
    index < points.len() && is_python_word(points[index])
}

/// `\b` at `index`.
fn boundary(points: &[u32], index: usize) -> bool {
    let before = index > 0 && is_python_word(points[index - 1]);
    before != word_at(points, index)
}

/// `_PREC_KEYWORDS` at `at`: the first alternative that matches, in the original's order. Returns the end of the match.
fn precision_keyword_at(points: &[u32], at: usize) -> Option<usize> {
    // FP4\s*\+\s*FP8
    if let Some(after) = ci_literal(points, at, "FP4") {
        let plus = skip_space(points, after);
        if plus < points.len() && points[plus] == code('+') {
            if let Some(end) = ci_literal(points, skip_space(points, plus + 1), "FP8") {
                return Some(end);
            }
        }
    }
    for literal in ["NVFP4", "MXFP4"] {
        if let Some(end) = ci_literal(points, at, literal) {
            return Some(end);
        }
    }
    // FP8\s*Mixed
    if let Some(after) = ci_literal(points, at, "FP8") {
        if let Some(end) = ci_literal(points, skip_space(points, after), "Mixed") {
            return Some(end);
        }
    }
    for literal in [
        "FP8", "FP4", "BF16", "INT4", "INT8", "AWQ", "GPTQ", "W4A16", "W8A8",
    ] {
        if let Some(end) = ci_literal(points, at, literal) {
            return Some(end);
        }
    }
    None
}

/// `_PREC_KEYWORDS.search(text)` as a yes/no.
fn has_precision_keyword(text: &PyStr) -> bool {
    let points = text.code_points();
    (0..points.len()).any(|at| precision_keyword_at(points, at).is_some())
}

/// The texts `_PREC_KEYWORDS.finditer(text)` finds (non-overlapping, left to right).
fn precision_keywords(text: &PyStr) -> Vec<PyStr> {
    let points = text.code_points();
    let mut found = Vec::new();
    let mut at = 0;
    while at < points.len() {
        match precision_keyword_at(points, at) {
            Some(end) => {
                found.push(PyStr::from_code_points(points[at..end].to_vec()));
                at = if end > at { end } else { at + 1 };
            }
            None => at += 1,
        }
    }
    found
}

/// `re.sub(r"\W", "", text)`: only the word characters stay.
fn remove_non_word(text: &PyStr) -> PyStr {
    PyStr::from_code_points(
        text.code_points()
            .iter()
            .copied()
            .filter(|&c| is_python_word(c))
            .collect(),
    )
}

/// `re.search(r"FP4\s*\+\s*FP8[^\n]*?(expert[^\n]*?FP4[^\n]*)", card, re.I)`: `group(0)` of the leftmost match.
fn mixed_note_match(card: &PyStr) -> Option<PyStr> {
    let points = card.code_points();
    for start in 0..points.len() {
        let Some(after_fp4) = ci_literal(points, start, "FP4") else {
            continue;
        };
        let plus = skip_space(points, after_fp4);
        if plus >= points.len() || points[plus] != code('+') {
            continue;
        }
        let Some(after_fp8) = ci_literal(points, skip_space(points, plus + 1), "FP8") else {
            continue;
        };
        let line_end = points[after_fp8..]
            .iter()
            .position(|&c| c == 0x0A)
            .map_or(points.len(), |offset| after_fp8 + offset);
        // The lazy runs stay on the line: the first `expert` decides, a later `FP4` must follow it.
        let Some(expert) = (after_fp8..=line_end)
            .find(|&at| ci_literal(points, at, "expert").is_some_and(|end| end <= line_end))
        else {
            continue;
        };
        let after_expert = expert + "expert".len();
        let has_fp4 = (after_expert..=line_end)
            .any(|at| ci_literal(points, at, "FP4").is_some_and(|end| end <= line_end));
        if has_fp4 {
            return Some(PyStr::from_code_points(points[start..line_end].to_vec()));
        }
    }
    None
}

/// `re.search(r"\b1\s*M(illion)?\b|1,?048,?576|one million", text, re.I)` as a yes/no.
fn mentions_one_million(text: &PyStr) -> bool {
    let points = text.code_points();
    (0..points.len()).any(|at| {
        // \b1\s*M(illion)?\b
        let first = boundary(points, at) && points[at] == code('1') && {
            let m = skip_space(points, at + 1);
            ci_literal(points, m, "M").is_some_and(|after_m| {
                ci_literal(points, after_m, "illion").is_some_and(|end| boundary(points, end))
                    || boundary(points, after_m)
            })
        };
        // 1,?048,?576
        let second = points[at] == code('1') && {
            let mut index = at + 1;
            if points.get(index) == Some(&code(',')) {
                index += 1;
            }
            let digits = |mut index: usize, text: &str| -> Option<usize> {
                for c in text.chars() {
                    if points.get(index) != Some(&code(c)) {
                        return None;
                    }
                    index += 1;
                }
                Some(index)
            };
            digits(index, "048").is_some_and(|after| {
                let mut index = after;
                if points.get(index) == Some(&code(',')) {
                    index += 1;
                }
                digits(index, "576").is_some()
            })
        };
        first || second || ci_literal(points, at, "one million").is_some()
    })
}

/// `re.search(r"reasoning effort|thinking budget|<think>|reasoning mode", text, re.I)` as a yes/no.
fn mentions_reasoning(text: &PyStr) -> bool {
    let points = text.code_points();
    (0..points.len()).any(|at| {
        [
            "reasoning effort",
            "thinking budget",
            "<think>",
            "reasoning mode",
        ]
        .iter()
        .any(|literal| ci_literal(points, at, literal).is_some())
    })
}

/// `re.search(r"\b" + re.escape(keyword) + r"\b", text, re.I)` as a yes/no.
fn mentions_word(text: &PyStr, keyword: &str) -> bool {
    let points = text.code_points();
    (0..points.len()).any(|at| {
        boundary(points, at)
            && ci_literal(points, at, keyword).is_some_and(|end| boundary(points, end))
    })
}

fn contains_text(haystack: &PyStr, needle: &PyStr) -> bool {
    let needle = needle.code_points();
    if needle.is_empty() {
        return true;
    }
    haystack
        .code_points()
        .windows(needle.len())
        .any(|window| window == needle)
}

fn attribute_error(value: &PyValue, name: &str) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute '{name}'", value.type_name()),
    )
}

fn text_arg<'a>(value: &'a PyValue, method: &str) -> Res<&'a PyStr> {
    match value.peel() {
        PyValue::Str(text) => Ok(text),
        other => Err(attribute_error(other, method)),
    }
}

/// `os.path.isdir(x)` / `isfile(x)` of a value: a non-path is the `TypeError` of `os.stat`.
fn stat_path(value: &PyValue) -> Res<&PyStr> {
    match value.peel() {
        PyValue::Str(text) => Ok(text),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "stat: path should be string, bytes, os.PathLike or integer, not {}",
                other.type_name()
            ),
        )),
    }
}

fn item_of(dict: &PyValue, key: &str) -> Res<PyValue> {
    crate::pyexpr::item(dict, key)
}

fn caught_in_reader(failure: &Failure) -> bool {
    is_os_error(failure.class) || is_value_error(failure.class)
}

/// `_read_st_header(path)`: the header JSON of a safetensors file, read without any bound or handler.
pub fn read_st_header(path: &PyValue) -> Res<PyValue> {
    let path = fspath(path, false)?;
    let mut file = open_binary(path)?;
    let length = read_up_to(&mut file, 8)?;
    if length.len() != 8 {
        return Err(Failure::new("error", "unpack requires a buffer of 8 bytes"));
    }
    let n = u64::from_le_bytes(length.as_slice().try_into().expect("8 bytes"));
    if n > i64::MAX as u64 {
        return Err(Failure::new(
            "OverflowError",
            "cannot fit 'int' into an index-sized integer",
        ));
    }
    let bytes = read_up_to(&mut file, n)?;
    loads_bytes(&bytes)
}

fn sorted_texts(mut items: Vec<PyStr>) -> Vec<PyStr> {
    items.sort_by(compare_text);
    items
}

/// `_canonical_shards(model_dir)`: the shards the loader reads (the index's, when all exist), else every `*.safetensors`.
pub fn canonical_shards(model_dir: &PyValue) -> Res<Vec<PyStr>> {
    let index_path = join_values(model_dir, &PyValue::str("model.safetensors.index.json"))?;
    let dir = fspath(model_dir, true)?.clone();
    if is_file(&index_path) {
        let attempt = (|| -> Res<Option<Vec<PyStr>>> {
            let index = load_json_file(&index_path)?;
            let weight_map = as_get_target(&index)?
                .get("weight_map")
                .cloned()
                .unwrap_or(PyValue::None);
            match weight_map.peel() {
                PyValue::Dict(map) if !map.is_empty() => {
                    let mut set: Vec<PyStr> = Vec::new();
                    for (_, value) in map.iter() {
                        let path = join_values(&PyValue::Str(dir.clone()), value)?;
                        if !set.contains(&path) {
                            set.push(path);
                        }
                    }
                    let reference = sorted_texts(set);
                    Ok(reference.iter().all(is_file).then_some(reference))
                }
                _ => Ok(None),
            }
        })();
        match attempt {
            Ok(Some(reference)) => return Ok(reference),
            Ok(None) => {}
            Err(failure) if caught_in_reader(&failure) => {}
            Err(failure) => return Err(failure),
        }
    }
    Ok(sorted_texts(glob(&join(&dir, &s("*.safetensors")))))
}

fn number_add(left: &PyValue, right: &PyValue) -> Res<PyValue> {
    match (left.peel(), right.peel()) {
        (PyValue::Int(a), PyValue::Int(b)) => Ok(PyValue::Int(a + b)),
        _ => {
            let a = float_of(left)?;
            let b = float_of(right)?;
            Ok(PyValue::Float(a + b))
        }
    }
}

fn length_of(value: &PyValue) -> Res<usize> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.len()),
        PyValue::Str(text) => Ok(text.len()),
        PyValue::Dict(map) => Ok(map.len()),
        PyValue::Bytes(bytes) => Ok(bytes.len()),
        other => Err(Failure::new(
            "TypeError",
            &format!("object of type '{}' has no len()", other.type_name()),
        )),
    }
}

fn index_of(value: &PyValue, at: usize) -> Res<PyValue> {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items[at].clone()),
        PyValue::Str(text) => Ok(PyValue::Str(PyStr::from_code_points(vec![
            text.code_points()[at],
        ]))),
        PyValue::Bytes(bytes) => Ok(PyValue::int(BigInt::from(bytes[at]))),
        PyValue::Dict(_) => Err(Failure::new("KeyError", &at.to_string())),
        other => Err(Failure::new(
            "TypeError",
            &format!("'{}' object is not subscriptable", other.type_name()),
        )),
    }
}

/// `value / 2**30` for a number (an `int` divides exactly, a `float` as a float).
fn over_gib(value: &PyValue) -> Res<f64> {
    match value.peel() {
        PyValue::Float(number) => Ok(number / f64::from(1u32 << 30)),
        other => {
            let integer = int_of(other)?;
            int_over_int(&integer, &BigInt::from(1u64 << 30))
        }
    }
}

fn counter_dict(map: &KeyMap<PyValue>) -> Res<PyValue> {
    let pairs: Vec<(PyValue, PyValue)> = map
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    Ok(PyValue::Dict(dict_from_pairs(pairs)?))
}

/// `scan_dtypes(model_dir, mxfp4=None)`: dtype statistics of the canonical shards' headers, the expert/non-expert split and
/// the precision signature.
pub fn scan_dtypes(model_dir: &PyValue, mxfp4: &PyValue) -> Res<PyDict> {
    let shards = canonical_shards(model_dir)?;
    let mut out = PyDict::new();
    if shards.is_empty() {
        out.set("present", PyValue::Bool(false));
        return Ok(out);
    }
    let one = PyValue::int(1);
    let mut counts: KeyMap<PyValue> = KeyMap::new();
    let mut nbytes: KeyMap<PyValue> = KeyMap::new();
    let mut expert_dt: KeyMap<PyValue> = KeyMap::new();
    let mut other_dt: KeyMap<PyValue> = KeyMap::new();
    for shard in &shards {
        let header = match read_st_header(&PyValue::Str(shard.clone())) {
            Ok(header) => header,
            // `except Exception: continue`
            Err(_) => continue,
        };
        let PyValue::Dict(items) = header.peel() else {
            return Err(attribute_error(&header, "items"));
        };
        for (key, meta) in items.iter() {
            let PyValue::Dict(meta) = meta.peel() else {
                continue;
            };
            if key.equals("__metadata__") {
                continue;
            }
            let dtype = meta.get("dtype").cloned().unwrap_or(PyValue::None);
            let offsets = meta
                .get("data_offsets")
                .cloned()
                .unwrap_or_else(|| PyValue::List(vec![PyValue::int(0), PyValue::int(0)]));
            let nb = if length_of(&offsets)? == 2 {
                crate::model_dyn::sub(&index_of(&offsets, 1)?, &index_of(&offsets, 0)?)?
            } else {
                PyValue::int(0)
            };
            let bump = |slot: &mut PyValue, by: &PyValue| -> Res<()> {
                *slot = number_add(slot, by)?;
                Ok(())
            };
            bump(counts.entry(&dtype, || PyValue::int(0))?, &one)?;
            bump(nbytes.entry(&dtype, || PyValue::int(0))?, &nb)?;
            let lowered = lower(key);
            let is_expert = contains(&lowered, "expert") && !contains(&lowered, "shared");
            let target = if is_expert {
                &mut expert_dt
            } else {
                &mut other_dt
            };
            bump(target.entry(&dtype, || PyValue::int(0))?, &one)?;
        }
    }
    let mut sum = PyValue::int(0);
    for (_, value) in nbytes.iter() {
        sum = number_add(&sum, value)?;
    }
    let total = if sum.is_truthy() {
        sum
    } else {
        PyValue::int(1)
    };
    let has_intpack = counts
        .iter()
        .any(|(dtype, _)| in_set(dtype, &INTPACK_DTYPES));
    let has_scale = counts.iter().any(|(dtype, _)| in_set(dtype, &SCALE_DTYPES));
    let has_fp8 = counts.iter().any(|(dtype, _)| in_set(dtype, &FP8_DTYPES));
    let has_high = counts.iter().any(|(dtype, _)| in_set(dtype, &HIGH_DTYPES));
    let mxfp4_given = !matches!(mxfp4.peel(), PyValue::None);
    let fourbit = (has_intpack && has_scale) || mxfp4_given;
    let mut classes: Vec<PyValue> = Vec::new();
    if fourbit {
        classes.push(PyValue::str("4bit(MXFP4-pack: int8+microscale)"));
    }
    if has_fp8 {
        classes.push(PyValue::str("fp8"));
    }
    if has_high {
        classes.push(PyValue::str("bf16/fp16"));
    }
    out.set("present", PyValue::Bool(true));
    out.set("num_shards", PyValue::int(BigInt::from(shards.len())));
    out.set("tensor_counts", counter_dict(&counts)?);
    let mut gib: Vec<(PyValue, PyValue)> = Vec::new();
    for (key, bytes) in nbytes.iter() {
        gib.push((
            key.clone(),
            PyValue::Float(round_to_places(over_gib(bytes)?, 2)),
        ));
    }
    out.set("byte_gib", PyValue::Dict(dict_from_pairs(gib)?));
    out.set(
        "total_gib",
        PyValue::Float(round_to_places(over_gib(&total)?, 2)),
    );
    out.set("expert_dtypes", counter_dict(&expert_dt)?);
    out.set("nonexpert_dtypes", counter_dict(&other_dt)?);
    let mixed = classes.len() >= 2;
    out.set("precision_classes", PyValue::List(classes));
    out.set("is_mixed_precision", PyValue::Bool(mixed));
    let fourbit_experts = if mxfp4_given {
        item_of(mxfp4, "packed_expert_tensor_count")?.is_truthy()
    } else {
        fourbit
            && !expert_dt.is_empty()
            && expert_dt
                .iter()
                .any(|(dtype, _)| in_set(dtype, &INTPACK_DTYPES))
    };
    out.set("fourbit_experts", PyValue::Bool(fourbit_experts));
    if mxfp4_given {
        out.set("mxfp4_layout", mxfp4.clone());
    }
    Ok(out)
}

/// `config_precision(cfg)`: the config's declared precision as text.
pub fn config_precision(cfg: &PyValue) -> Res<PyStr> {
    let map = as_get_target(cfg)?;
    let qc = map
        .get("quantization_config")
        .cloned()
        .unwrap_or(PyValue::None);
    if let PyValue::Dict(quant) = qc.peel() {
        let pick = |key: &str| quant.get(key).cloned().unwrap_or(PyValue::None);
        let (method, algo) = (pick("quant_method"), pick("quant_algo"));
        let question = PyValue::str("?");
        let qm = crate::model_dyn::or_chain(&[&method, &algo, &question]).clone();
        let (format, weight_dtype) = (pick("fmt"), pick("weight_dtype"));
        let empty = PyValue::str("");
        let fmt = crate::model_dyn::or_chain(&[&format, &weight_dtype, &empty]).clone();
        let mut text = qm.display_text();
        if fmt.is_truthy() {
            let PyValue::Str(suffix) = fmt.peel() else {
                return Err(Failure::new(
                    "TypeError",
                    &format!(
                        "can only concatenate str (not \"{}\") to str",
                        fmt.type_name()
                    ),
                ));
            };
            text.push_str("/");
            text.push_py(suffix);
        }
        return Ok(text);
    }
    Ok(map
        .get("torch_dtype")
        .cloned()
        .unwrap_or_else(|| PyValue::str("?"))
        .display_text())
}

/// `card_precision(card, model_name)`: the precision cell of the model's table row (an exact name cell wins) and the
/// precision keywords of the card, with the FP4+FP8 definition sentence when there is one.
pub fn card_precision(card: &PyValue, model_name: &PyValue) -> Res<PyDict> {
    let mut out = PyDict::new();
    out.set("table_row", PyValue::None);
    out.set("keywords", PyValue::List(Vec::new()));
    let name = text_arg(model_name, "split")?;
    let base = {
        let slash = code('/');
        let points = name.code_points();
        let start = points
            .iter()
            .rposition(|&c| c == slash)
            .map_or(0, |at| at + 1);
        PyStr::from_code_points(points[start..].to_vec())
    };
    let nbase = lower(&remove_non_word(&base));
    let card = text_arg(card, "splitlines")?;
    let base_lower = lower(&base);
    let mut candidates: Vec<(u8, PyStr)> = Vec::new();
    for line in crate::model_files::splitlines(card) {
        if !contains_text(&line, &s("|")) || !contains_text(&lower(&line), &base_lower) {
            continue;
        }
        let trimmed = strip_bars(&line.strip());
        let cells: Vec<PyStr> = trimmed
            .code_points()
            .split(|&c| c == code('|'))
            .map(|piece| PyStr::from_code_points(piece.to_vec()).strip())
            .collect();
        let precs: Vec<&PyStr> = cells
            .iter()
            .filter(|cell| has_precision_keyword(cell))
            .collect();
        let Some(first) = precs.first() else {
            continue;
        };
        let exact = cells
            .iter()
            .any(|cell| lower(&remove_non_word(cell)) == nbase);
        candidates.push((if exact { 0 } else { 1 }, (*first).clone()));
    }
    if !candidates.is_empty() {
        candidates.sort_by_key(|(rank, _)| *rank);
        out.set("table_row", PyValue::Str(candidates[0].1.clone()));
    }
    let mut keywords: Vec<PyStr> = Vec::new();
    for keyword in precision_keywords(card) {
        if !keywords.contains(&keyword) {
            keywords.push(keyword);
        }
    }
    out.set(
        "keywords",
        PyValue::List(
            sorted_texts(keywords)
                .into_iter()
                .map(PyValue::Str)
                .collect(),
        ),
    );
    if let Some(note) = mixed_note_match(card) {
        let points: Vec<u32> = note.code_points().iter().copied().take(200).collect();
        out.set("mixed_note", PyValue::Str(PyStr::from_code_points(points)));
    }
    Ok(out)
}

/// `text.strip("|")`.
fn strip_bars(text: &PyStr) -> PyStr {
    let points = text.code_points();
    let bar = code('|');
    let start = points
        .iter()
        .position(|&c| c != bar)
        .unwrap_or(points.len());
    let end = points
        .iter()
        .rposition(|&c| c != bar)
        .map_or(start, |at| at + 1);
    PyStr::from_code_points(points[start..end.max(start)].to_vec())
}

/// `read_inference_deps(model_dir)`: the non-comment lines of `inference/requirements.txt` and `requirements.txt` with the
/// package name each one starts with.
pub fn read_inference_deps(model_dir: &PyValue) -> Res<Vec<PyValue>> {
    let mut deps = Vec::new();
    for relative in ["inference/requirements.txt", "requirements.txt"] {
        let path = join_values(model_dir, &PyValue::str(relative))?;
        if !is_file(&path) {
            continue;
        }
        let text = read_text_replacing(&path)?;
        for line in text.code_points().split(|&c| c == 0x0A) {
            let line = PyStr::from_code_points(line.to_vec()).strip();
            if line.is_empty() || line.code_points()[0] == code('#') {
                continue;
            }
            let delimiter = |c: u32| {
                [
                    code('<'),
                    code('>'),
                    code('='),
                    code('!'),
                    code('~'),
                    code('['),
                    code(' '),
                ]
                .contains(&c)
            };
            let end = line
                .code_points()
                .iter()
                .position(|&c| delimiter(c))
                .unwrap_or(line.len());
            let package = lower(&PyStr::from_code_points(line.code_points()[..end].to_vec()));
            let mut entry = PyDict::new();
            entry.set("src", PyValue::str(relative));
            entry.set("spec", PyValue::Str(line));
            entry.set("pkg", PyValue::Str(package));
            deps.push(PyValue::Dict(entry));
        }
    }
    Ok(deps)
}

fn unavailable(reason: &str) -> PyValue {
    let mut dict = PyDict::new();
    dict.set("ok", PyValue::Bool(false));
    dict.set("reason", PyValue::str(reason));
    PyValue::Dict(dict)
}

/// What the fetch of the external parameter total needs from the network layer, so that one call can be pointed at one base
/// without any global setting.
pub struct Origin<'a> {
    pub api_base: &'a str,
}

/// `fetch_hf_safetensors_total(repo_id, timeout=8.0)`: asks the HF API for the parameter total of a repository (no model
/// is downloaded). `{"ok": True, "total": int, "parameters": {...}}`, or `{"ok": False, "reason": ...}` when the lookup
/// fails (never a made-up value).
pub fn fetch_hf_safetensors_total(
    repo_id: &PyValue,
    timeout: &PyValue,
    origin: &Origin<'_>,
) -> Res<PyValue> {
    if !repo_id.is_truthy() {
        return Ok(unavailable("repo_id 미지정"));
    }
    let mut url = PyStr::new(origin.api_base);
    url.push_str("/");
    url.push_py(&repo_id.display_text());
    let body = match crate::hf_http::get_json_bytes(&url, timeout) {
        Ok(body) => body,
        Err(crate::hf_http::HttpFailure::Raised(failure)) => return Err(failure),
        Err(crate::hf_http::HttpFailure::Value(message)) => {
            let mut reason = s("응답 파싱 실패: ");
            reason.push_py(&message);
            let mut dict = PyDict::new();
            dict.set("ok", PyValue::Bool(false));
            dict.set("reason", PyValue::Str(reason));
            return Ok(PyValue::Dict(dict));
        }
        Err(crate::hf_http::HttpFailure::Status(code)) => {
            return Ok(unavailable(&format!(
                "HTTP {code}({})",
                url.to_utf8().unwrap_or_default()
            )));
        }
        Err(crate::hf_http::HttpFailure::Network(message)) => {
            let mut reason = s("네트워크 오류: ");
            reason.push_py(&message);
            reason.push_str("(");
            reason.push_py(&url);
            reason.push_str(")");
            let mut dict = PyDict::new();
            dict.set("ok", PyValue::Bool(false));
            dict.set("reason", PyValue::Str(reason));
            return Ok(PyValue::Dict(dict));
        }
    };
    let parsed = crate::model_files::decode_utf8(&body)
        .and_then(|text| crate::model_files::loads_str(&text));
    let data = match parsed {
        Ok(data) => data,
        Err(failure) if is_value_error(failure.class) => {
            let mut reason = s("응답 파싱 실패: ");
            reason.push_py(&failure.message);
            let mut dict = PyDict::new();
            dict.set("ok", PyValue::Bool(false));
            dict.set("reason", PyValue::Str(reason));
            return Ok(PyValue::Dict(dict));
        }
        Err(failure) => return Err(failure),
    };
    let st = as_get_target(&data)?
        .get("safetensors")
        .cloned()
        .unwrap_or(PyValue::None);
    let PyValue::Dict(st_map) = st.peel() else {
        return Ok(unavailable(
            "응답에 safetensors.total 필드 없음(비공개/게이트/구형 포맷 가능)",
        ));
    };
    let Some(total) = st_map.get("total") else {
        return Ok(unavailable(
            "응답에 safetensors.total 필드 없음(비공개/게이트/구형 포맷 가능)",
        ));
    };
    let total = int_of(total)?;
    let parameters = st_map.get("parameters").cloned().unwrap_or(PyValue::None);
    let parameters = if parameters.is_truthy() {
        parameters
    } else {
        PyValue::Dict(PyDict::new())
    };
    let mut dict = PyDict::new();
    dict.set("ok", PyValue::Bool(true));
    dict.set("total", PyValue::Int(total));
    dict.set("parameters", parameters);
    Ok(PyValue::Dict(dict))
}

/// `f"{value:,}"` for a number.
pub fn with_commas(value: &PyValue) -> Res<PyStr> {
    let group = |digits: &str| -> String {
        let (sign, digits) = match digits.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", digits),
        };
        let mut out = String::new();
        for (index, c) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        format!("{sign}{out}")
    };
    Ok(match value.peel() {
        PyValue::Int(number) => PyStr::new(&group(&number.to_string())),
        PyValue::Bool(flag) => PyStr::new(if *flag { "1" } else { "0" }),
        PyValue::Float(number) => {
            let text = PyValue::Float(*number).repr();
            if text.contains('e') || text.contains("inf") || text.contains("nan") {
                PyStr::new(&text)
            } else {
                let (integer, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
                PyStr::new(&format!("{}.{fraction}", group(integer)))
            }
        }
        other => {
            return Err(Failure::new(
                "ValueError",
                &format!(
                    "Cannot specify ',' with '{}'.",
                    if matches!(other, PyValue::Str(_)) {
                        "s"
                    } else {
                        "?"
                    }
                ),
            ))
        }
    })
}

/// The type of the fetch of the external total (the real HTTP lookup or a stand-in a self-test installs).
pub type Fetch<'a> = &'a dyn Fn(&PyValue) -> Res<PyValue>;

fn dict_get_value(dict: &PyValue, key: &str) -> Res<PyValue> {
    Ok(as_get_target(dict)?
        .get(key)
        .cloned()
        .unwrap_or(PyValue::None))
}

/// `crosscheck_external_vram(local_num_params, local_native_weight_bytes, repo_id, tolerance=0.05)`: the local parameter
/// count against the external total; beyond `tolerance` it is a MISMATCH, an unreachable lookup is UNAVAILABLE.
pub fn crosscheck_external_vram(
    local_num_params: &PyValue,
    repo_id: &PyValue,
    tolerance: &PyValue,
    fetch: Fetch<'_>,
) -> Res<PyDict> {
    let ext = if repo_id.is_truthy() {
        fetch(repo_id)?
    } else {
        unavailable("repo_id 미지정")
    };
    let mut out = PyDict::new();
    if !dict_get_value(&ext, "ok")?.is_truthy() {
        out.set("verdict", PyValue::str("UNAVAILABLE"));
        out.set("reason", dict_get_value(&ext, "reason")?);
        out.set("repo_id", repo_id.clone());
        out.set("local_num_params", local_num_params.clone());
        out.set("external_total", PyValue::None);
        return Ok(out);
    }
    let ext_total_value = item_of(&ext, "total")?;
    if matches!(local_num_params.peel(), PyValue::None) {
        let mut est_bytes = PyValue::None;
        let parameters = dict_get_value(&ext, "parameters")?;
        if parameters.is_truthy() {
            let mut sum = PyValue::int(0);
            let PyValue::Dict(parameters) = parameters.peel() else {
                return Err(attribute_error(&parameters, "items"));
            };
            for (dtype, count) in parameters.iter() {
                let bpw = match quant_table::dtype_bpw(&PyValue::Str(dtype.clone()))? {
                    PyValue::Tuple(parts) => parts[0].clone(),
                    other => other,
                };
                let product = crate::model_dyn::mul(&PyValue::Int(int_of(count)?), &bpw)?;
                sum = number_add(&sum, &product)?;
            }
            est_bytes = sum;
        }
        out.set("verdict", PyValue::str("OK"));
        out.set("mode", PyValue::str("ephemeral-estimate"));
        out.set("repo_id", repo_id.clone());
        out.set("external_total", ext_total_value);
        out.set("external_native_weight_bytes_est", est_bytes.clone());
        let gib = if est_bytes.is_truthy() {
            PyValue::Float(round_to_places(over_gib(&est_bytes)?, 2))
        } else {
            PyValue::None
        };
        out.set("external_native_weight_gib_est", gib);
        return Ok(out);
    }
    let difference = crate::model_dyn::sub(local_num_params, &ext_total_value)?;
    let diff = match difference.peel() {
        PyValue::Int(number) => PyValue::Int(num_traits::Signed::abs(number)),
        PyValue::Float(number) => PyValue::Float(number.abs()),
        other => other.clone(),
    };
    let rel = if ext_total_value.is_truthy() {
        match diff.peel() {
            PyValue::Float(number) => {
                let total = int_of(&ext_total_value)?;
                int_over_float(&total, 1.0).map(|total| number / total)?
            }
            other => int_over_int(&int_of(other)?, &int_of(&ext_total_value)?)?,
        }
    } else {
        1.0
    };
    let mismatch = crate::model_dyn::compare(Compare::Greater, &PyValue::Float(rel), tolerance)?;
    let verdict = if mismatch { "MISMATCH" } else { "OK" };
    out.set("verdict", PyValue::str(verdict));
    out.set("mode", PyValue::str("managed-doublecheck"));
    out.set("repo_id", repo_id.clone());
    out.set("local_num_params", local_num_params.clone());
    out.set("external_total", ext_total_value.clone());
    out.set("relative_diff", PyValue::Float(round_to_places(rel, 4)));
    out.set(
        "note",
        PyValue::Str(
            Message::new()
                .lit("로컬 실측(")
                .text(&with_commas(local_num_params)?)
                .lit(") vs 외부(")
                .text(&with_commas(&ext_total_value)?)
                .lit(") 오차 ")
                .text(&fixed(rel * 100.0, 1))
                .lit("% ")
                .lit(if verdict == "MISMATCH" {
                    "허용범위 초과 — 로컬 파일 재확인 필요"
                } else {
                    "(허용범위 내)"
                })
                .done(),
        ),
    );
    Ok(out)
}

fn flag(value: bool) -> PyValue {
    PyValue::Bool(value)
}

fn check(entries: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in entries {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

/// `crosscheck(model_dir, hf_repo_id=None, local_num_params=None, local_parse_error=None)`: the whole report.
pub fn crosscheck(
    model_dir: &PyValue,
    hf_repo_id: &PyValue,
    local_num_params: &PyValue,
    local_parse_error: &PyValue,
    fetch: Fetch<'_>,
) -> Res<PyDict> {
    let dir = stat_path(model_dir)?.clone();
    if !is_dir(&dir) {
        return Err(Failure::with_message(
            "FileNotFoundError",
            Message::new()
                .lit("모델 디렉토리 부재(다운로드 금지): ")
                .val(model_dir)
                .done(),
        ));
    }
    let cfg_path = join(&dir, &s("config.json"));
    if !is_file(&cfg_path) {
        return Err(Failure::with_message(
            "FileNotFoundError",
            Message::new()
                .lit("config.json 부재(다운로드 금지): ")
                .text(&cfg_path)
                .done(),
        ));
    }
    let cfg = load_json_file(&cfg_path)?;
    let card_path = ["README.md", "readme.md", "MODEL_CARD.md"]
        .iter()
        .map(|name| join(&dir, &s(name)))
        .find(is_file);
    let card = match &card_path {
        Some(path) => read_text_replacing(path)?,
        None => PyStr::default(),
    };
    let cprec = config_precision(&cfg)?;
    let cfg_map = as_get_target(&cfg)?;
    let quant = cfg_map
        .get("quantization_config")
        .cloned()
        .unwrap_or(PyValue::None);
    let quant_dict = matches!(quant.peel(), PyValue::Dict(_));
    let mut mxfp4 = PyValue::None;
    let mut layout_error: Option<PyStr> = None;
    if quant_dict
        && py_eq(
            &dict_get_value(&quant, "quant_method")?,
            &PyValue::str("mxfp4"),
        )
    {
        match count_mxfp4_params(model_dir) {
            Ok(counts) => mxfp4 = PyValue::Dict(counts),
            Err(failure) if is_value_error(failure.class) => layout_error = Some(failure.message),
            Err(failure) => return Err(failure),
        }
    }
    let dt: PyValue = if layout_error.is_none() {
        PyValue::Dict(scan_dtypes(model_dir, &mxfp4)?)
    } else {
        let mut absent = PyDict::new();
        absent.set("present", PyValue::Bool(false));
        PyValue::Dict(absent)
    };
    let deps = read_inference_deps(model_dir)?;
    let name = basename(&rstrip_slashes(&dir));

    let mut checks: Vec<PyValue> = Vec::new();
    let mut recs: Vec<PyValue> = Vec::new();
    if let Some(reason) = &layout_error {
        checks.push(check(vec![
            ("name", PyValue::str("mxfp4_layout")),
            ("verdict", PyValue::str("MISMATCH")),
            ("reason", PyValue::Str(reason.clone())),
        ]));
    }
    let dt_get = |key: &str| -> PyValue { dict_get_value(&dt, key).unwrap_or(PyValue::None) };
    let present = dt_get("present").is_truthy();

    // 1. precision
    let cardp = if card.is_empty() {
        let mut empty = PyDict::new();
        empty.set("table_row", PyValue::None);
        empty.set("keywords", PyValue::List(Vec::new()));
        empty
    } else {
        card_precision(&PyValue::Str(card.clone()), &PyValue::Str(name.clone()))?
    };
    let cp = |key: &str| cardp.get(key).cloned().unwrap_or(PyValue::None);
    let sig = if present {
        dt_get("precision_classes")
    } else {
        PyValue::List(Vec::new())
    };
    let mut verdict = "info";
    let mut note = PyStr::default();
    let table_row = cp("table_row");
    let card_says_mixed = cp("mixed_note").is_truthy()
        || (table_row.is_truthy()
            && matches!(table_row.peel(), PyValue::Str(text) if contains(text, "+")));
    if present {
        let coarse_single = (!contains(&cprec, "/") && {
            let lowered = lower(&cprec);
            ["fp8", "fp16", "bf16"]
                .iter()
                .any(|name| lowered.equals(name))
        }) || (quant_dict && !contains(&cprec, "+"));
        if dt_get("is_mixed_precision").is_truthy()
            && coarse_single
            && matches!(mxfp4.peel(), PyValue::None)
        {
            verdict = "MISMATCH";
            let shown = crate::model_dyn::or_chain(&[&table_row, &cp("mixed_note")]).clone();
            let mut message = Message::new()
                .lit("config quant='")
                .text(&cprec)
                .lit("'(coarse) 가 실제 혼합정밀도를 과소표기. 실측 dtype classes=")
                .text(&PyStr::new(&sig.repr()));
            if dt_get("fourbit_experts").is_truthy() {
                message = message.lit("; experts=4bit");
            }
            if table_row.is_truthy() || cp("mixed_note").is_truthy() {
                message = message.lit("; 카드='").val(&shown).lit("'");
            }
            note = message.done();
            recs.push(PyValue::str(
                "정밀도 = 실측 dtype·카드 기준으로 확정(config coarse 라벨 신뢰 금지). 메모리/적합성 재산정.",
            ));
        } else if card_says_mixed && !dt_get("is_mixed_precision").is_truthy() {
            verdict = "MISMATCH";
            note = Message::new()
                .lit("카드는 혼합정밀도('")
                .val(&table_row)
                .lit("') 인데 실측 dtype 단일(")
                .text(&PyStr::new(&sig.repr()))
                .lit(") — 다운로드본 변형 의심.")
                .done();
        } else {
            let shown = crate::model_dyn::or_chain(&[&table_row, &cp("keywords")]).clone();
            let mut message = Message::new()
                .lit("config='")
                .text(&cprec)
                .lit("' · 실측=")
                .text(&PyStr::new(&sig.repr()))
                .lit(" · 카드='")
                .val(&shown)
                .lit("'");
            if !matches!(mxfp4.peel(), PyValue::None) {
                message = message.lit(
                    "; 검증된 MXFP4 packed weights + 비패킹 부동소수 텐서(설명 가능한 혼합 저장)",
                );
            }
            note = message.done();
        }
    }
    let card_value = crate::model_dyn::or_chain(&[&table_row, &cp("keywords")]).clone();
    checks.push(check(vec![
        ("name", PyValue::str("precision")),
        ("config", PyValue::Str(cprec.clone())),
        ("dtype_evidence", sig.clone()),
        ("card", card_value),
        ("verdict", PyValue::str(verdict)),
        ("note", PyValue::Str(note)),
    ]));

    // 2. special dependencies
    let mut flagged: Vec<PyValue> = Vec::new();
    let known = |pkg: &PyStr| KNOWN_SPECIAL_DEPS.iter().find(|(name, _)| pkg.equals(name));
    for dep in &deps {
        let pkg = dict_get_value(dep, "pkg")?;
        if let PyValue::Str(pkg_text) = pkg.peel() {
            if let Some(&(_, why)) = known(pkg_text) {
                flagged.push(check(vec![
                    ("pkg", pkg.clone()),
                    ("spec", dict_get_value(dep, "spec")?),
                    ("src", dict_get_value(dep, "src")?),
                    ("why", PyValue::str(why)),
                ]));
            }
        }
    }
    for (keyword, why) in KNOWN_SPECIAL_DEPS {
        let already = flagged
            .iter()
            .any(|entry| matches!(dict_get_value(entry, "pkg"), Ok(PyValue::Str(pkg)) if pkg.equals(keyword)));
        if !card.is_empty() && mentions_word(&card, keyword) && !already {
            flagged.push(check(vec![
                ("pkg", PyValue::str(keyword)),
                ("spec", PyValue::str("(README 언급)")),
                ("src", PyValue::str("README.md")),
                ("why", PyValue::str(why)),
            ]));
        }
    }
    let any_flagged = !flagged.is_empty();
    if any_flagged {
        recs.push(PyValue::str(
            "novel-arch special-dep 발견 → serving 전 **vLLM dry-init(모델 클래스 import) + 해당 op 가용성** 확인(예: DSA SparseAttnIndexer→DeepGEMM). 분류(3+1+1 따름정리): **native lib/커널 = 빌드-바깥** → upstream-version-watch `build_patches/`(patch.py ✗ — 몽키패치로 native 설치 불가) · **Python 코드불일치 = `<model>_patch.py`**. 빌드-바깥은 recipe-explorer 발견·핸드오프만.",
        ));
    }
    checks.push(check(vec![
        ("name", PyValue::str("special_deps")),
        (
            "verdict",
            PyValue::str(if any_flagged { "WARN" } else { "info" }),
        ),
        ("flagged", PyValue::List(flagged)),
    ]));

    // 3-5. context, architecture, reasoning
    let mpe = cfg_map
        .get("max_position_embeddings")
        .cloned()
        .unwrap_or(PyValue::None);
    let card_ctx = if mentions_one_million(&card) {
        PyValue::str("1M")
    } else {
        PyValue::None
    };
    checks.push(check(vec![
        ("name", PyValue::str("context")),
        ("config_max_position_embeddings", mpe),
        ("card", card_ctx),
        ("verdict", PyValue::str("info")),
    ]));
    let card_lower = lower(&card);
    let mut card_keywords: Vec<PyStr> = Vec::new();
    for keyword in [
        "MoE",
        "Mixture-of-Experts",
        "Sparse Attention",
        "Multi-head Latent",
        "MTP",
        "Multi-Token",
    ] {
        if !card.is_empty() && contains_text(&card_lower, &lower(&s(keyword))) {
            card_keywords.push(s(keyword));
        }
    }
    checks.push(check(vec![
        ("name", PyValue::str("arch")),
        (
            "config",
            cfg_map
                .get("architectures")
                .cloned()
                .unwrap_or(PyValue::None),
        ),
        (
            "model_type",
            cfg_map.get("model_type").cloned().unwrap_or(PyValue::None),
        ),
        (
            "card_keywords",
            PyValue::List(
                sorted_texts(card_keywords)
                    .into_iter()
                    .map(PyValue::Str)
                    .collect(),
            ),
        ),
        ("verdict", PyValue::str("info")),
    ]));
    let is_reasoning = !card.is_empty() && mentions_reasoning(&card);
    if is_reasoning {
        recs.push(PyValue::str("reasoning 모델 — 스모크 max_tokens 충분히(finish_reason=stop) · reasoning-parser 검토."));
    }
    checks.push(check(vec![
        ("name", PyValue::str("reasoning")),
        ("verdict", PyValue::str("info")),
        ("is_reasoning", flag(is_reasoning)),
    ]));

    // 6. external VRAM double check
    if hf_repo_id.is_truthy() {
        let extv: PyDict = if !matches!(local_parse_error.peel(), PyValue::None) {
            let mut dict = PyDict::new();
            dict.set("verdict", PyValue::str("MISMATCH"));
            dict.set("mode", PyValue::str("managed-doublecheck"));
            dict.set("repo_id", hf_repo_id.clone());
            dict.set("local_num_params", PyValue::None);
            dict.set("external_total", PyValue::None);
            dict.set("reason", PyValue::str("local_parse_failed"));
            dict.set("note", local_parse_error.clone());
            dict
        } else {
            crosscheck_external_vram(local_num_params, hf_repo_id, &PyValue::Float(0.05), fetch)?
        };
        // `{"name": ..., "verdict": ev_verdict, **extv}`: extv's own keys, `verdict` included, replace the earlier ones.
        let mut merged = PyDict::new();
        merged.set("name", PyValue::str("external_vram"));
        merged.set("verdict", PyValue::None);
        for (key, value) in extv.iter() {
            merged.insert(key.clone(), value.clone());
        }
        let extv_value = PyValue::Dict(extv.clone());
        checks.push(PyValue::Dict(merged));
        if py_eq(
            &dict_get_value(&extv_value, "reason")?,
            &PyValue::str("local_parse_failed"),
        ) {
            recs.push(PyValue::str(
                "로컬 파라미터 관측 실패를 먼저 해소한다(외부 사전추정으로 대체하지 않음).",
            ));
        } else if py_eq(&item_of(&extv_value, "verdict")?, &PyValue::str("MISMATCH")) {
            recs.push(PyValue::Str(
                Message::new()
                    .lit("외부(HF API) 파라미터 총계와 로컬 실측 불일치(")
                    .val(&dict_get_value(&extv_value, "note")?)
                    .lit(") — 로컬 safetensors 파일이 손상/변형됐을 가능성. 재검증 필요.")
                    .done(),
            ));
        }
    } else {
        checks.push(check(vec![
            ("name", PyValue::str("external_vram")),
            ("verdict", PyValue::str("info")),
            (
                "reason",
                PyValue::str("hf_repo_id 미지정 — 외부 이중검증 스킵"),
            ),
        ]));
    }

    let verdict_of = |entry: &PyValue| dict_get_value(entry, "verdict").unwrap_or(PyValue::None);
    let mismatches = checks
        .iter()
        .filter(|entry| matches!(verdict_of(entry).peel(), PyValue::Str(text) if text.equals("MISMATCH")))
        .count();
    let warned = checks
        .iter()
        .any(|entry| matches!(verdict_of(entry).peel(), PyValue::Str(text) if text.equals("WARN")));
    let mut sources = PyDict::new();
    sources.set("config.json", PyValue::Bool(true));
    sources.set("README.md", flag(card_path.is_some()));
    sources.set(
        "inference_requirements",
        flag(deps.iter().any(|dep| {
            matches!(dict_get_value(dep, "src"), Ok(PyValue::Str(src)) if starts_with(&src, &s("inference")))
        })),
    );
    sources.set("safetensors_headers", flag(present));
    let mut out = PyDict::new();
    out.set("model", PyValue::Str(name));
    out.set("model_dir", model_dir.clone());
    out.set("sources_read", PyValue::Dict(sources));
    out.set("dtype_scan", dt);
    out.set("checks", PyValue::List(checks));
    out.set("recommendations", PyValue::List(recs));
    out.set(
        "verdict",
        PyValue::str(if mismatches > 0 {
            "MISMATCH"
        } else if warned {
            "WARN"
        } else {
            "OK"
        }),
    );
    out.set("n_mismatch", PyValue::int(BigInt::from(mismatches)));
    Ok(out)
}

/// `_human(r)`: the report as text.
pub fn human(report: &PyValue) -> Res<PyStr> {
    let mut lines: Vec<PyStr> = Vec::new();
    lines.push(
        Message::new()
            .lit("# 모델카드 교차검증: ")
            .val(&item_of(report, "model")?)
            .lit("  → ")
            .val(&item_of(report, "verdict")?)
            .done(),
    );
    let sources = item_of(report, "sources_read")?;
    let PyValue::Dict(sources) = sources.peel() else {
        return Err(attribute_error(&sources, "items"));
    };
    let mut read: Vec<PyStr> = Vec::new();
    for (key, value) in sources.iter() {
        if value.is_truthy() {
            read.push(key.clone());
        }
    }
    let mut joined = PyStr::default();
    for (index, key) in read.iter().enumerate() {
        if index > 0 {
            joined.push_str(", ");
        }
        joined.push_py(key);
    }
    lines.push(Message::new().lit("  sources: ").text(&joined).done());
    let checks = item_of(report, "checks")?;
    for entry in crate::pyexpr::items_of(&checks)? {
        let verdict = dict_get_value(&entry, "verdict")?;
        ensure_hashable(&verdict)?;
        let mark = match verdict.peel() {
            PyValue::Str(text) if text.equals("OK") => "✅",
            PyValue::Str(text) if text.equals("MISMATCH") => "🔴",
            PyValue::Str(text) if text.equals("WARN") => "⚠\u{fe0f}",
            _ => "·",
        };
        let name = item_of(&entry, "name")?;
        let name_is = |text: &str| matches!(name.peel(), PyValue::Str(own) if own.equals(text));
        if name_is("precision") {
            lines.push(
                Message::new()
                    .lit("  ")
                    .lit(mark)
                    .lit(" precision: config=")
                    .val(&item_of(&entry, "config")?)
                    .lit(" | 실측=")
                    .val(&item_of(&entry, "dtype_evidence")?)
                    .lit(" | 카드=")
                    .val(&item_of(&entry, "card")?)
                    .done(),
            );
            if dict_get_value(&entry, "note")?.is_truthy() {
                lines.push(
                    Message::new()
                        .lit("       ")
                        .val(&item_of(&entry, "note")?)
                        .done(),
                );
            }
        } else if name_is("mxfp4_layout") {
            lines.push(
                Message::new()
                    .lit("  ")
                    .lit(mark)
                    .lit(" mxfp4_layout: ")
                    .val(&item_of(&entry, "reason")?)
                    .done(),
            );
        } else if name_is("special_deps") {
            let flagged = item_of(&entry, "flagged")?;
            if flagged.is_truthy() {
                for dep in crate::pyexpr::items_of(&flagged)? {
                    lines.push(
                        Message::new()
                            .lit("  ⚠\u{fe0f} special-dep: ")
                            .val(&item_of(&dep, "pkg")?)
                            .lit(" (")
                            .val(&item_of(&dep, "spec")?)
                            .lit(", ")
                            .val(&item_of(&dep, "src")?)
                            .lit(") — ")
                            .val(&item_of(&dep, "why")?)
                            .done(),
                    );
                }
            } else {
                lines.push(s("  · special-deps: 없음"));
            }
        } else if name_is("context") {
            lines.push(
                Message::new()
                    .lit("  · context: config=")
                    .val(&item_of(&entry, "config_max_position_embeddings")?)
                    .lit(" | 카드=")
                    .val(&item_of(&entry, "card")?)
                    .done(),
            );
        } else if name_is("arch") {
            lines.push(
                Message::new()
                    .lit("  · arch: ")
                    .val(&item_of(&entry, "config")?)
                    .lit(" / ")
                    .val(&item_of(&entry, "model_type")?)
                    .lit(" | 카드=")
                    .val(&item_of(&entry, "card_keywords")?)
                    .done(),
            );
        } else if name_is("reasoning") {
            lines.push(
                Message::new()
                    .lit("  · reasoning: ")
                    .val(&item_of(&entry, "is_reasoning")?)
                    .done(),
            );
        } else if name_is("external_vram") {
            let info = matches!(verdict.peel(), PyValue::Str(text) if text.equals("info"));
            let reason = dict_get_value(&entry, "reason")?;
            if info && reason.is_truthy() {
                lines.push(
                    Message::new()
                        .lit("  · external_vram: ")
                        .val(&reason)
                        .done(),
                );
            } else if info {
                lines.push(
                    Message::new()
                        .lit("  · external_vram: 조회불가(")
                        .val(&reason)
                        .lit(") — 로컬 실측만으로 진행")
                        .done(),
                );
            } else {
                let shown = match as_get_target(&entry)?.get("note") {
                    Some(note) => note.clone(),
                    None => entry.clone(),
                };
                lines.push(
                    Message::new()
                        .lit("  ")
                        .lit(mark)
                        .lit(" external_vram: ")
                        .val(&shown)
                        .done(),
                );
            }
        }
    }
    let recommendations = item_of(report, "recommendations")?;
    if recommendations.is_truthy() {
        lines.push(s("  권고:"));
        for recommendation in crate::pyexpr::items_of(&recommendations)? {
            lines.push(Message::new().lit("   → ").val(&recommendation).done());
        }
    }
    let mut out = PyStr::default();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push_str("\n");
        }
        out.push_py(line);
    }
    Ok(out)
}

/// `str.endswith` convenience for callers outside this module.
pub fn text_ends_with(text: &PyStr, suffix: &str) -> bool {
    ends_with(text, suffix)
}

/// Whether a path exists (kept public for the command line).
pub fn path_exists(path: &PyStr) -> bool {
    exists(path)
}

#[allow(dead_code)]
fn unused_numbers(_: BigInt) -> bool {
    BigInt::zero().is_zero()
}
