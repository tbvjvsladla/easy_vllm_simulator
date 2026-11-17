//! `parse_model_config.py`: the deterministic `config.json` parser (hybrid KV layer counts, MXFP4 header validation, weight
//! bytes from the index and the safetensors headers) with Python's argument and dynamic-typing behaviour, ported function by
//! function from the original source. The numeric dependencies (`dtype_bpw`, `vllm_quant_bpw_or_none`) are the in-process
//! functions of [`crate::quant_table`]. Inputs are borrowed; warnings lists are written to in place.

use crate::model_dyn::{
    compare_text, contains, ends_with, float_of, floor_div, int_of, int_over_float,
    is_os_value_or_struct_error, is_type_or_value_error, lstrip_slashes, or_chain, round_to_int,
    rstrip_slashes, s, starts_with, tail_from, KeyMap, Message, Warnings,
};
use crate::model_files::{
    basename, decode_utf8, exists, file_size, fspath, glob, handle_size, is_dir, is_file, join,
    join_values, load_json_file, load_json_file_unique, loads_bytes, loads_bytes_unique, loads_str,
    lower, open_binary, read_up_to,
};
use crate::pyexpr::{get, Res};
use crate::quant_table;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pydyn::{as_get_target, ensure_hashable};
use agent_runtime_support::pysort::{py_eq, same_or_eq, sorted_by_key};
use agent_runtime_support::pystr::{is_decimal_digit, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};

/// `DEFAULT_NAS_HOST_ROOT`: the host directory `/app/models` is mapped to.
pub const DEFAULT_NAS_HOST_ROOT: &str = "/mnt/models";
/// `_SAFETENSORS_MAX_HEADER` (safetensors `MAX_HEADER_SIZE`).
pub const SAFETENSORS_MAX_HEADER: u64 = 100_000_000;
/// `_SAFETENSORS_DTYPE_BPW`: header dtype to on-disk bytes per weight.
pub const SAFETENSORS_DTYPE_BPW: [(&str, f64); 11] = [
    ("F64", 8.0),
    ("I64", 8.0),
    ("F32", 4.0),
    ("I32", 4.0),
    ("F16", 2.0),
    ("BF16", 2.0),
    ("I16", 2.0),
    ("F8_E4M3", 1.0),
    ("F8_E5M2", 1.0),
    ("I8", 1.0),
    ("U8", 1.0),
];
/// `_SAFETENSORS_DTYPE_ELEM_BYTES`: header dtype to bytes per element.
pub const SAFETENSORS_DTYPE_ELEM_BYTES: [(&str, i64); 12] = [
    ("F64", 8),
    ("I64", 8),
    ("F32", 4),
    ("I32", 4),
    ("F16", 2),
    ("BF16", 2),
    ("I16", 2),
    ("F8_E4M3", 1),
    ("F8_E5M2", 1),
    ("I8", 1),
    ("U8", 1),
    ("BOOL", 1),
];
/// `_KV_FREE_LAYER_TYPES`: layer types that hold no standard KV cache (a closed list on purpose).
///
/// `moe` and `mlp` joined with plan_26111708 B L9: they are the FFN-only blocks of the NemotronH layer vocabulary
/// (transformers 5.19.0 `models/nemotron_h/configuration_nemotron_h.py:29-31,203` — `mlp`, `linear_attention`,
/// `full_attention`, `moe`), and in vLLM 0.32.0 only the `*` (attention) block builds an `Attention` (KV cache) —
/// `model_executor/models/nemotron_h.py:481`; the MLP (`:279-308`) and MoE (`:335-353`) mixers hold none.
pub const KV_FREE_LAYER_TYPES: [&str; 3] = ["linear_attention", "moe", "mlp"];
pub const KV_FULL_LAYER_TYPE: &str = "full_attention";

/// The sub-config names transformers' `PretrainedConfig.get_text_config()` searches when neither `decoder` nor `encoder` is
/// set: the encoder names, then the decoder names (transformers 5.19.0 `configuration_utils.py:1364-1373`).
pub const TEXT_CONFIG_KEYS: [&str; 4] = ["text_encoder", "decoder", "generator", "text_config"];

/// vLLM's config attribute remap, applied before that search: `_CONFIG_ATTRS_MAPPING = {"llm_config": "text_config"}`
/// (vllm 0.32.0 `transformers_utils/config.py:152-154`), applied by `_maybe_remap_hf_config_attrs` only when the config has no
/// `text_config` attribute (`config.py:744-751`, called at `:421`).
pub const CONFIG_ATTRS_MAPPING: [(&str, &str); 1] = [("llm_config", "text_config")];

/// transformers' legacy layer-type names and their current names (`_LEGACY_LAYER_TYPE_REMAP`, transformers 5.19.0
/// `configuration_utils.py:92-97`), applied to every config's `layer_types` at construction (`:398`).
pub const LEGACY_LAYER_TYPE_REMAP: [(&str, &str); 4] = [
    ("mamba", "linear_attention"),
    ("attention", "full_attention"),
    ("deepseek_sparse_attention", "indexed_attention"),
    ("qwen_sparse_attention", "indexed_attention"),
];

/// Where the language-model fields of a `config.json` live (see [`text_config_of`]).
#[derive(Clone, Copy, Debug)]
pub struct TextConfig<'a> {
    /// The nested text config, or `None` for a flat config (transformers returns the config itself).
    pub nested: Option<&'a PyDict>,
    /// The name transformers found it under (one of [`TEXT_CONFIG_KEYS`]).
    pub key: Option<&'static str>,
    /// The `config.json` key it was read from when vLLM's remap supplied it (`llm_config` for `text_config`).
    pub remapped_from: Option<&'static str>,
}

/// `get_hf_text_config(config)` of vLLM 0.32.0 over a `config.json` mapping, with no CLI or file state (plan_26111708 B L9 —
/// the one owner of nested text-config resolution; the explorer parser and the adversarial-benchmark roofline both call it):
///
/// 1. vLLM's remap [`CONFIG_ATTRS_MAPPING`]: a config with `llm_config` and no `text_config` key reads `llm_config` as its
///    `text_config` (`transformers_utils/config.py:152-154,744-751` — the Nemotron omni VL layout).
/// 2. transformers' search over [`TEXT_CONFIG_KEYS`] (`configuration_utils.py:1364-1391`): more than one present is the same
///    `ValueError` as transformers'; exactly one is the text config; none is the flat config itself (`nested: None`).
///    vLLM's `get_hf_text_config` (`transformers_utils/config.py:1195-1209`) delegates to it.
///
/// A sub-config counts only when it is a mapping: transformers holds sub-configs as objects, and in `config.json` a
/// sub-config is an object; a value of another type (or `null`) is not a config (the parser always ignored a non-mapping
/// `text_config`). vLLM's extra `num_attention_heads` check on the sub-config is not repeated here — the parser reports a
/// missing field as a warning instead of stopping.
pub fn text_config_of(config: &PyDict) -> Res<TextConfig<'_>> {
    fn mapping(value: Option<&PyValue>) -> Option<&PyDict> {
        match value.map(PyValue::peel) {
            Some(PyValue::Dict(map)) => Some(map),
            _ => None,
        }
    }
    let mut found: Vec<(&'static str, &PyDict, Option<&'static str>)> = Vec::new();
    for key in TEXT_CONFIG_KEYS {
        let remap = CONFIG_ATTRS_MAPPING
            .iter()
            .find(|(_, new)| *new == key && !config.contains_key(key))
            .filter(|(old, _)| config.contains_key(old));
        let (value, remapped_from) = match remap {
            Some((old, _)) => (config.get(old), Some(*old)),
            None => (config.get(key), None),
        };
        if let Some(map) = mapping(value) {
            found.push((key, map, remapped_from));
        }
    }
    match found.as_slice() {
        [] => Ok(TextConfig { nested: None, key: None, remapped_from: None }),
        [(key, map, remapped_from)] => Ok(TextConfig { nested: Some(map), key: Some(key), remapped_from: *remapped_from }),
        many => {
            let names: Vec<String> = many.iter().map(|(key, _, _)| format!("'{key}'")).collect();
            Err(Failure::new(
                "ValueError",
                &format!(
                    "Multiple valid text configs were found in the model config: [{}]. In this case, using `get_text_config()` would be ambiguous. Please specify the desired text config directly, e.g. `text_config = config.sub_config_name`",
                    names.join(", ")
                ),
            ))
        }
    }
}

/// `layer_types` with [`LEGACY_LAYER_TYPE_REMAP`] applied (a list stays a list, item by item; anything else is returned as
/// it is).
pub fn remap_legacy_layer_types(layer_types: &PyValue) -> PyValue {
    match layer_types.peel() {
        PyValue::List(items) => PyValue::List(
            items
                .iter()
                .map(|item| match item.peel() {
                    PyValue::Str(name) => LEGACY_LAYER_TYPE_REMAP
                        .iter()
                        .find(|(old, _)| name.equals(old))
                        .map_or_else(|| item.clone(), |(_, new)| PyValue::str(new)),
                    _ => item.clone(),
                })
                .collect(),
        ),
        _ => layer_types.clone(),
    }
}
/// `MMAP_OFFLOADABLE_PATTERNS`: tensors an NVMe-mmap mechanism keeps out of residency, by the tensor-name pattern the
/// original compiles (kept as the pattern text; [`matches_offload_pattern`] is the matcher).
pub const MMAP_OFFLOADABLE_PATTERNS: [(&str, &str); 2] = [
    (
        "ple_mmap",
        r"\.ple\.ple_embedding\.ngram_embedding\.shard_\d+\.weight$",
    ),
    ("engram_file", r"\.engram\.embed\.(weight|scale)$"),
];

fn elem_bytes_of(dtype: &PyValue) -> Res<i64> {
    ensure_hashable(dtype)?;
    Ok(match dtype.peel() {
        PyValue::Str(text) => SAFETENSORS_DTYPE_ELEM_BYTES
            .iter()
            .find(|(name, _)| text.equals(name))
            .map_or(2, |&(_, bytes)| bytes),
        _ => 2,
    })
}

fn bpw_of(dtype: &PyValue) -> Option<f64> {
    match dtype.peel() {
        PyValue::Str(text) => SAFETENSORS_DTYPE_BPW
            .iter()
            .find(|(name, _)| text.equals(name))
            .map(|&(_, bpw)| bpw),
        _ => None,
    }
}

/// What `resolve_kv_layers` returns: `(num_full_attention_layers, is_hybrid, counts, warnings)`.
pub struct KvLayers {
    pub num_full: Option<BigInt>,
    pub is_hybrid: bool,
    /// Layer type counts in order of first appearance (`None` when `layer_types` is absent or empty).
    pub counts: Option<Vec<(PyStr, usize)>>,
    pub warnings: Vec<PyStr>,
}

impl KvLayers {
    pub fn into_value(self) -> PyValue {
        let counts = match self.counts {
            None => PyValue::None,
            Some(counts) => {
                let mut dict = PyDict::new();
                for (name, count) in counts {
                    dict.insert(name, PyValue::int(BigInt::from(count)));
                }
                PyValue::Dict(dict)
            }
        };
        PyValue::Tuple(vec![
            self.num_full.map_or(PyValue::None, PyValue::Int),
            PyValue::Bool(self.is_hybrid),
            counts,
            PyValue::List(self.warnings.into_iter().map(PyValue::Str).collect()),
        ])
    }
}

/// `resolve_kv_layers(layer_types, num_hidden_layers, full_attention_interval=None)`: the number of layers that hold a
/// standard KV cache, from `layer_types` alone. Unknown types count as KV-bearing (conservative); a length mismatch or no
/// KV-bearing layer falls back to all layers with a warning.
pub fn resolve_kv_layers(
    layer_types: &PyValue,
    num_hidden_layers: &PyValue,
    full_attention_interval: &PyValue,
) -> Res<KvLayers> {
    let mut warnings: Vec<PyStr> = Vec::new();
    let types = match layer_types.peel() {
        PyValue::List(items) if !items.is_empty() => items,
        _ => {
            return Ok(KvLayers {
                num_full: None,
                is_hybrid: false,
                counts: None,
                warnings,
            })
        }
    };
    let mut counts: Vec<(PyStr, usize)> = Vec::new();
    for item in types {
        let key = item.display_text();
        match counts.iter_mut().find(|(name, _)| *name == key) {
            Some((_, count)) => *count += 1,
            None => counts.push((key, 1)),
        }
    }
    let is_free = |name: &PyStr| KV_FREE_LAYER_TYPES.iter().any(|free| name.equals(free));
    let kv_bearing: usize = counts
        .iter()
        .filter(|(name, _)| !is_free(name))
        .map(|(_, count)| *count)
        .sum();
    let mut unknown: Vec<PyStr> = counts
        .iter()
        .filter(|(name, _)| !is_free(name) && !name.equals(KV_FULL_LAYER_TYPE))
        .map(|(name, _)| name.clone())
        .collect();
    unknown.sort_by(compare_text);
    if !unknown.is_empty() {
        let mut joined = PyStr::default();
        for (index, name) in unknown.iter().enumerate() {
            if index > 0 {
                joined.push_str(", ");
            }
            joined.push_py(name);
        }
        warnings.push(
            Message::new()
                .lit("layer_types 에 미지 타입 ")
                .text(&joined)
                .lit(" → KV 보유로 계산(보수적·과대추정). KV-무보유가 맞다면 parse_model_config._KV_FREE_LAYER_TYPES 에 근거와 함께 등재하라")
                .done(),
        );
    }
    let kv_bearing_big = BigInt::from(kv_bearing);
    let declared = !matches!(num_hidden_layers.peel(), PyValue::None);
    if declared && BigInt::from(types.len()) != int_of(num_hidden_layers)? {
        warnings.push(
            Message::new()
                .lit("layer_types 길이(")
                .count(types.len())
                .lit(") != num_hidden_layers(")
                .val(num_hidden_layers)
                .lit(") → 층수 인지 포기, 전층 폴백(보수적)")
                .done(),
        );
        return Ok(KvLayers {
            num_full: None,
            is_hybrid: false,
            counts: Some(counts),
            warnings,
        });
    }
    if kv_bearing == 0 {
        warnings.push(s(
            "layer_types 에 KV 보유층이 0 개 → 층수 인지 포기, 전층 폴백(과소추정 방지)",
        ));
        return Ok(KvLayers {
            num_full: None,
            is_hybrid: false,
            counts: Some(counts),
            warnings,
        });
    }
    let is_hybrid = declared && kv_bearing_big < int_of(num_hidden_layers)?;
    if full_attention_interval.is_truthy() && declared {
        let expected = (|| -> Res<Option<BigInt>> {
            let layers = int_of(num_hidden_layers)?;
            let interval = int_of(full_attention_interval)?;
            crate::model_dyn::int_floor_div(&layers, &interval).map(Some)
        })();
        let expected = match expected {
            Ok(value) => value,
            Err(failure)
                if is_type_or_value_error(failure.class)
                    || failure.class == "ZeroDivisionError" =>
            {
                None
            }
            Err(failure) => return Err(failure),
        };
        if let Some(expected) = expected {
            if expected != kv_bearing_big {
                warnings.push(
                    Message::new()
                        .lit("full_attention_interval(")
                        .val(full_attention_interval)
                        .lit(")로 기대한 full 층수(")
                        .int(&expected)
                        .lit(") != layer_types 집계(")
                        .count(kv_bearing)
                        .lit(") — layer_types 를 권위로 쓴다(interval 은 참고값)")
                        .done(),
                );
            }
        }
    }
    Ok(KvLayers {
        num_full: Some(kv_bearing_big),
        is_hybrid,
        counts: Some(counts),
        warnings,
    })
}

fn attribute_error(value: &PyValue, name: &str) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute '{name}'", value.type_name()),
    )
}

/// `str` of a value that must be a string: `.rstrip`/`.startswith` of anything else is an `AttributeError`.
fn text_of<'a>(value: &'a PyValue, method: &str) -> Res<&'a PyStr> {
    match value.peel() {
        PyValue::Str(text) => Ok(text),
        other => Err(attribute_error(other, method)),
    }
}

/// `_resolve_host_path(model_path, nas_host_root, nas_container_root="/app/models")`: a container path under the
/// container root is mapped to the host NAS root, anything else is returned as it is.
pub fn resolve_host_path(
    model_path: &PyValue,
    nas_host_root: &PyValue,
    nas_container_root: &PyValue,
) -> Res<PyStr> {
    let prefix = rstrip_slashes(text_of(nas_container_root, "rstrip")?);
    let equal = py_eq(model_path, &PyValue::Str(prefix.clone()));
    let under = if equal {
        true
    } else {
        let mut with_slash = prefix.clone();
        with_slash.push_str("/");
        starts_with(text_of(model_path, "startswith")?, &with_slash)
    };
    if under {
        let path = text_of(model_path, "startswith")?;
        let relative = lstrip_slashes(&tail_from(path, prefix.len()));
        return join_values(nas_host_root, &PyValue::Str(relative));
    }
    Ok(text_of(model_path, "startswith")?.clone())
}

/// `_model_id_from_path(model_path)`: the last two path components as `<Org>/<Name>`, else the last one, else the path.
pub fn model_id_from_path(model_path: &PyValue) -> Res<PyStr> {
    let path = text_of(model_path, "rstrip")?;
    let norm = rstrip_slashes(path);
    let slash = u32::from('/');
    let parts: Vec<&[u32]> = norm
        .code_points()
        .split(|&c| c == slash)
        .filter(|part| !part.is_empty())
        .collect();
    let piece = |points: &[u32]| PyStr::from_code_points(points.to_vec());
    if parts.len() >= 2 {
        let mut id = piece(parts[parts.len() - 2]);
        id.push_str("/");
        id.push_py(&piece(parts[parts.len() - 1]));
        return Ok(id);
    }
    if let Some(last) = parts.last() {
        return Ok(piece(last));
    }
    Ok(path.clone())
}

/// `_read_safetensors_header(st_path)`: the header JSON of a safetensors file, or `None` for any problem reading it
/// (short file, length beyond the cap or the file, undecodable or invalid JSON). A header that is not an object is
/// returned as it is.
pub fn read_safetensors_header(st_path: &PyValue) -> Res<Option<PyValue>> {
    let path = fspath(st_path, false)?;
    let soft = |failure: Failure| -> Res<Option<PyValue>> {
        if is_os_value_or_struct_error(failure.class) {
            Ok(None)
        } else {
            Err(failure)
        }
    };
    let mut file = match open_binary(path) {
        Ok(file) => file,
        Err(failure) => return soft(failure),
    };
    let length_bytes = match read_up_to(&mut file, 8) {
        Ok(bytes) => bytes,
        Err(failure) => return soft(failure),
    };
    if length_bytes.len() != 8 {
        return Ok(None);
    }
    let header_len = u64::from_le_bytes(length_bytes.as_slice().try_into().expect("8 bytes"));
    let size = match handle_size(&file) {
        Ok(size) => size,
        Err(failure) => return soft(failure),
    };
    if header_len > SAFETENSORS_MAX_HEADER || header_len > size.saturating_sub(8) {
        return Ok(None);
    }
    let header_bytes = match read_up_to(&mut file, header_len) {
        Ok(bytes) => bytes,
        Err(failure) => return soft(failure),
    };
    if header_bytes.len() as u64 != header_len {
        return Ok(None);
    }
    let text = match decode_utf8(&header_bytes) {
        Ok(text) => text,
        Err(failure) => return soft(failure),
    };
    match loads_str(&text) {
        Ok(value) => Ok(Some(value)),
        Err(failure) => soft(failure),
    }
}

/// `int(d)` for one shape entry, `0` when `int()` raises `TypeError`/`ValueError` (the loop stops there).
fn numel_of(shape: &[PyValue]) -> Res<BigInt> {
    let mut numel = BigInt::from(1);
    for dimension in shape {
        match int_of(dimension) {
            Ok(value) => numel *= value,
            Err(failure) if is_type_or_value_error(failure.class) => return Ok(BigInt::zero()),
            Err(failure) => return Err(failure),
        }
    }
    Ok(numel)
}

/// `header.items()` of a header that is not empty: anything but an object has no `items`.
fn header_items(header: &PyValue) -> Res<&PyDict> {
    match header.peel() {
        PyValue::Dict(map) => Ok(map),
        other => Err(attribute_error(other, "items")),
    }
}

/// `_measure_disk_dtype(st_path)`: `(dtype, disk_bpw)` of the largest tensor (elements x bytes) of the header, or
/// `(None, None)`.
pub fn measure_disk_dtype(st_path: &PyValue) -> Res<(Option<PyValue>, Option<f64>)> {
    let Some(header) = read_safetensors_header(st_path)? else {
        return Ok((None, None));
    };
    if !header.is_truthy() {
        return Ok((None, None));
    }
    let mut best_dtype: Option<PyValue> = None;
    let mut best_size = BigInt::from(-1);
    for (name, meta) in header_items(&header)?.iter() {
        if name.equals("__metadata__") {
            continue;
        }
        let PyValue::Dict(meta) = meta.peel() else {
            continue;
        };
        let dtype = meta.get("dtype").cloned().unwrap_or(PyValue::None);
        let shape = meta.get("shape").cloned().unwrap_or(PyValue::None);
        let PyValue::List(shape) = shape.peel() else {
            continue;
        };
        if !dtype.is_truthy() {
            continue;
        }
        let numel = numel_of(shape)?;
        let size = numel * BigInt::from(elem_bytes_of(&dtype)?);
        if size > best_size {
            best_size = size;
            best_dtype = Some(dtype);
        }
    }
    match best_dtype {
        None => Ok((None, None)),
        Some(dtype) => {
            let bpw = bpw_of(&dtype);
            Ok((Some(dtype), bpw))
        }
    }
}

const INDEX_NAME: &str = "model.safetensors.index.json";

/// The weight map of an index file, as `idx.get("weight_map")` after `json.load` (the index itself may be any JSON value;
/// a value that is not an object has no `get`).
fn weight_map_of(index: &PyValue) -> Res<PyValue> {
    Ok(as_get_target(index)?
        .get("weight_map")
        .cloned()
        .unwrap_or(PyValue::None))
}

/// `sorted(set(values))` of the values of a weight map: unhashable values and unorderable mixes are `TypeError`s.
fn sorted_distinct_values(weight_map: &PyDict) -> Res<Vec<PyValue>> {
    let mut distinct: Vec<PyValue> = Vec::new();
    for (_, value) in weight_map.iter() {
        ensure_hashable(value)?;
        if !distinct.iter().any(|held| same_or_eq(held, value)) {
            distinct.push(value.clone());
        }
    }
    let keys = distinct.clone();
    sorted_by_key(distinct, &keys)
}

fn sorted_texts(mut items: Vec<PyStr>) -> Vec<PyStr> {
    items.sort_by(compare_text);
    items
}

fn caught_by_reader(failure: &Failure) -> bool {
    // `except (OSError, ValueError, json.JSONDecodeError)`
    crate::model_dyn::is_os_error(failure.class) || crate::model_dyn::is_value_error(failure.class)
}

/// `_native_weight_bytes(host_path, warnings)`: the bytes of the files the loader will really read, in this order of
/// authority: the shards the index names, all `*.safetensors`, all `*.bin`; `None` with a warning when there are none.
pub fn native_weight_bytes(
    host_path: &PyValue,
    warnings: &mut Warnings<'_>,
) -> Res<Option<BigInt>> {
    let index_path = join_values(host_path, &PyValue::str(INDEX_NAME))?;
    let host = fspath(host_path, true)?.clone();
    if is_file(&index_path) {
        let attempt = (|| -> Res<Option<BigInt>> {
            let index = load_json_file(&index_path)?;
            let weight_map = weight_map_of(&index)?;
            let PyValue::Dict(map) = weight_map.peel() else {
                return Ok(None);
            };
            if map.is_empty() {
                return Ok(None);
            }
            let shards = sorted_distinct_values(map)?;
            let mut paths: Vec<PyStr> = Vec::with_capacity(shards.len());
            for shard in &shards {
                paths.push(join_values(&PyValue::Str(host.clone()), shard)?);
            }
            let missing: Vec<&PyValue> = shards
                .iter()
                .zip(&paths)
                .filter(|(_, path)| !is_file(path))
                .map(|(shard, _)| shard)
                .collect();
            if !missing.is_empty() {
                warnings.push(
                    Message::new()
                        .lit("index weight_map 참조 샤드 ")
                        .count(missing.len())
                        .lit("개 부재(예: ")
                        .val(missing[0])
                        .lit(") → 글롭 폴백")
                        .done(),
                )?;
                return Ok(None);
            }
            let mut real = BigInt::zero();
            for path in &paths {
                real += file_size(path)?;
            }
            let metadata = match as_get_target(&index)?.get("metadata") {
                Some(value) => value.clone(),
                None => PyValue::Dict(PyDict::new()),
            };
            let total = as_get_target(&metadata)?
                .get("total_size")
                .cloned()
                .unwrap_or(PyValue::None);
            let numeric = matches!(
                total.peel(),
                PyValue::Int(_) | PyValue::Float(_) | PyValue::Bool(_)
            );
            if numeric
                && crate::model_dyn::compare(
                    agent_runtime_support::pyops::Compare::Greater,
                    &total,
                    &PyValue::int(0),
                )?
                && real > BigInt::zero()
            {
                let total_float = float_of(&total)?;
                let ratio = crate::model_dyn::int_over_float(&real, 1.0)
                    .map(|real_float| total_float / real_float)?;
                if !(0.9..=1.1).contains(&ratio) {
                    let gib = BigInt::from(1u64 << 30);
                    let total_gib = match total.peel() {
                        PyValue::Float(number) => number / (1u64 << 30) as f64,
                        other => {
                            let integer = int_of(other)?;
                            crate::model_dyn::int_over_int(&integer, &gib)?
                        }
                    };
                    let real_gib = crate::model_dyn::int_over_int(&real, &gib)?;
                    warnings.push(
                        Message::new()
                            .lit("index metadata.total_size=")
                            .text(&fixed(total_gib, 2))
                            .lit(" GiB 가 실제 샤드 합 ")
                            .text(&fixed(real_gib, 2))
                            .lit(" GiB 의 ")
                            .text(&fixed(ratio, 2))
                            .lit("배 — 발행자가 다른 dtype 기준으로 적은 값이다. 실제 파일 크기를 채택한다.")
                            .done(),
                    )?;
                }
            }
            Ok(Some(real))
        })();
        match attempt {
            Ok(Some(real)) => return Ok(Some(real)),
            Ok(None) => {}
            Err(failure) if caught_by_reader(&failure) => {
                warnings.push(s(
                    "model.safetensors.index.json 읽기 실패 → 파일 크기 합으로 폴백",
                ))?;
            }
            Err(failure) => return Err(failure),
        }
    }
    let sum_sizes = |paths: &[PyStr]| -> Res<BigInt> {
        let mut total = BigInt::zero();
        for path in paths {
            total += file_size(path)?;
        }
        Ok(total)
    };
    let safetensors = sorted_texts(glob(&join(&host, &s("*.safetensors"))));
    if !safetensors.is_empty() {
        match sum_sizes(&safetensors) {
            Ok(total) => return Ok(Some(total)),
            Err(failure) if crate::model_dyn::is_os_error(failure.class) => {
                warnings.push(
                    Message::new()
                        .lit("*.safetensors 크기 합 실패: ")
                        .text(&failure.message)
                        .done(),
                )?;
            }
            Err(failure) => return Err(failure),
        }
    }
    let bins = sorted_texts(glob(&join(&host, &s("*.bin"))));
    if !bins.is_empty() {
        match sum_sizes(&bins) {
            Ok(total) => return Ok(Some(total)),
            Err(failure) if crate::model_dyn::is_os_error(failure.class) => {
                warnings.push(
                    Message::new()
                        .lit("*.bin 크기 합 실패: ")
                        .text(&failure.message)
                        .done(),
                )?;
            }
            Err(failure) => return Err(failure),
        }
    }
    warnings.push(s(
        "weight 파일(*.safetensors/*.bin/index.json)을 찾지 못함 → native_weight_bytes=None",
    ))?;
    Ok(None)
}

/// `'%.{places}f' % number`.
pub fn fixed(number: f64, places: u32) -> PyStr {
    PyStr::new(&agent_runtime_support::pynumber::format_fixed(
        number, places,
    ))
}

/// Whether `name` matches the original's pattern for `mechanism` anywhere (`re.search`): the pattern's literal head, for
/// `ple_mmap` one or more Unicode decimal digits and `.weight`, for `engram_file` `weight` or `scale`, then `$` (the end, or
/// before a final newline).
pub fn matches_offload_pattern(mechanism: &str, name: &PyStr) -> bool {
    let points = name.code_points();
    let literal = |text: &str| -> Vec<u32> { text.chars().map(u32::from).collect() };
    let at_end = |index: usize| {
        index == points.len() || (index + 1 == points.len() && points[index] == 0x0A)
    };
    let (head, tails): (Vec<u32>, Vec<Vec<u32>>) = match mechanism {
        "ple_mmap" => (
            literal(".ple.ple_embedding.ngram_embedding.shard_"),
            vec![literal(".weight")],
        ),
        _ => (
            literal(".engram.embed."),
            vec![literal("weight"), literal("scale")],
        ),
    };
    for start in 0..points.len() {
        if !points[start..].starts_with(&head) {
            continue;
        }
        let mut index = start + head.len();
        if mechanism == "ple_mmap" {
            let first = index;
            while index < points.len() && is_decimal_digit(points[index]) {
                index += 1;
            }
            if index == first {
                continue;
            }
        }
        for tail in &tails {
            if points[index..].starts_with(tail) && at_end(index + tail.len()) {
                return true;
            }
        }
    }
    false
}

/// `offloadable_weight_bytes(host_path, mechanism, warnings)`: the bytes `mechanism` takes out of residency, read from the
/// index and the headers only. `None` = could not be computed (never corrected by a guess), `0` = computed, none present.
pub fn offloadable_weight_bytes(
    host_path: &PyValue,
    mechanism: &PyValue,
    warnings: &mut Warnings<'_>,
) -> Res<Option<BigInt>> {
    ensure_hashable(mechanism)?;
    let known = match mechanism.peel() {
        PyValue::Str(text) => MMAP_OFFLOADABLE_PATTERNS
            .iter()
            .find(|(name, _)| text.equals(name))
            .map(|&(name, _)| name),
        _ => None,
    };
    let Some(known) = known else {
        warnings.push(
            Message::new()
                .lit("mmap 기구 '")
                .val(mechanism)
                .lit("' 이 닫힌 목록에 없다 → 보정 0(추측 ✗)")
                .done(),
        )?;
        return Ok(None);
    };
    let index_path = join_values(host_path, &PyValue::str(INDEX_NAME))?;
    let host = fspath(host_path, true)?.clone();
    if !is_file(&index_path) {
        warnings.push(s(
            "index.json 부재 → mmap 오프로드 바이트 산출 불가(보정 ✗)",
        ))?;
        return Ok(None);
    }
    let loaded = load_json_file(&index_path).and_then(|index| weight_map_of(&index));
    let weight_map = match loaded {
        Ok(map) => map,
        Err(failure)
            if crate::model_dyn::is_os_error(failure.class)
                || crate::model_dyn::is_value_error(failure.class) =>
        {
            warnings.push(
                Message::new()
                    .lit("index.json 읽기 실패 → mmap 오프로드 산출 불가: ")
                    .text(&failure.message)
                    .done(),
            )?;
            return Ok(None);
        }
        Err(failure) => return Err(failure),
    };
    let PyValue::Dict(map) = weight_map.peel() else {
        warnings.push(s("weight_map 이 비었다 → mmap 오프로드 산출 불가(보정 ✗)"))?;
        return Ok(None);
    };
    if map.is_empty() {
        warnings.push(s("weight_map 이 비었다 → mmap 오프로드 산출 불가(보정 ✗)"))?;
        return Ok(None);
    }
    let mut by_shard: KeyMap<Vec<PyStr>> = KeyMap::new();
    for (name, shard) in map.iter() {
        if matches_offload_pattern(known, name) {
            by_shard.entry(shard, Vec::new)?.push(name.clone());
        }
    }
    if by_shard.is_empty() {
        return Ok(Some(BigInt::zero()));
    }
    let mut total = BigInt::zero();
    for (shard, names) in by_shard.iter() {
        let path = join_values(&PyValue::Str(host.clone()), shard)?;
        let header = (|| -> Res<PyValue> {
            let mut file = open_binary(&path)?;
            let length = read_up_to(&mut file, 8)?;
            if length.len() != 8 {
                return Err(Failure::new("error", "unpack requires a buffer of 8 bytes"));
            }
            let header_len = u64::from_le_bytes(length.as_slice().try_into().expect("8 bytes"));
            if header_len > i64::MAX as u64 {
                return Err(Failure::new(
                    "OverflowError",
                    "cannot fit 'int' into an index-sized integer",
                ));
            }
            let bytes = read_up_to(&mut file, header_len)?;
            loads_bytes(&bytes)
        })();
        let header = match header {
            Ok(header) => header,
            Err(failure) if is_os_value_or_struct_error(failure.class) => {
                warnings.push(
                    Message::new()
                        .lit("샤드 헤더 읽기 실패(")
                        .val(shard)
                        .lit(") → mmap 오프로드 산출 불가: ")
                        .text(&failure.message)
                        .done(),
                )?;
                return Ok(None);
            }
            Err(failure) => return Err(failure),
        };
        for name in names {
            let entry = {
                let found = as_get_target(&header)?
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.clone());
                match found {
                    Some(value) if value.is_truthy() => value,
                    _ => PyValue::Dict(PyDict::new()),
                }
            };
            let offsets = get(&entry, "data_offsets")?;
            match offsets.peel() {
                PyValue::List(offs) if offs.len() == 2 => {
                    total += int_of(&offs[1])? - int_of(&offs[0])?;
                }
                _ => {
                    warnings.push(
                        Message::new()
                            .text(name)
                            .lit(": data_offsets 부재 → mmap 오프로드 산출 불가")
                            .done(),
                    )?;
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(total))
}

/// `_largest_safetensors(host_path)`: the largest `*.safetensors` file in directory order (the first of equals), or `None`.
pub fn largest_safetensors(host_path: &PyValue) -> Res<Option<PyStr>> {
    let host = fspath(host_path, true)?;
    let files = glob(&join(host, &s("*.safetensors")));
    if files.is_empty() {
        return Ok(None);
    }
    let mut best: Option<(u64, &PyStr)> = None;
    for file in &files {
        match file_size(file) {
            Ok(size) => {
                if best.is_none_or(|(held, _)| size > held) {
                    best = Some((size, file));
                }
            }
            Err(failure) if crate::model_dyn::is_os_error(failure.class) => return Ok(None),
            Err(failure) => return Err(failure),
        }
    }
    Ok(best.map(|(_, file)| file.clone()))
}

/// `_count_params_from_headers(host_path)`: the sum of `numel` over the headers of the canonical shards (the index's, else
/// every `*.safetensors`); `None` when a shard header cannot be read or the sum is not positive.
pub fn count_params_from_headers(host_path: &PyValue) -> Res<Option<BigInt>> {
    let index_path = join_values(host_path, &PyValue::str(INDEX_NAME))?;
    let host = fspath(host_path, true)?.clone();
    let mut referenced: Option<Vec<PyStr>> = None;
    if is_file(&index_path) {
        let attempt = (|| -> Res<Option<Vec<PyStr>>> {
            let index = load_json_file(&index_path)?;
            let weight_map = weight_map_of(&index)?;
            match weight_map.peel() {
                PyValue::Dict(map) if !map.is_empty() => {
                    let mut set: Vec<PyStr> = Vec::new();
                    for (_, value) in map.iter() {
                        let path = join_values(&PyValue::Str(host.clone()), value)?;
                        if !set.contains(&path) {
                            set.push(path);
                        }
                    }
                    Ok(Some(sorted_texts(set)))
                }
                _ => Ok(None),
            }
        })();
        referenced = match attempt {
            Ok(files) => files,
            Err(failure) if caught_by_reader(&failure) => None,
            Err(failure) => return Err(failure),
        };
    }
    let files = match referenced {
        Some(files) if !files.is_empty() => files,
        _ => sorted_texts(glob(&join(&host, &s("*.safetensors")))),
    };
    if files.is_empty() {
        return Ok(None);
    }
    let mut total = BigInt::zero();
    for path in &files {
        let Some(header) = read_safetensors_header(&PyValue::Str(path.clone()))? else {
            return Ok(None);
        };
        if !header.is_truthy() {
            return Ok(None);
        }
        for (name, meta) in header_items(&header)?.iter() {
            if name.equals("__metadata__") {
                continue;
            }
            let PyValue::Dict(meta) = meta.peel() else {
                continue;
            };
            let Some(PyValue::List(shape)) = meta.get("shape").map(PyValue::peel) else {
                continue;
            };
            total += numel_of(shape)?;
        }
    }
    Ok(if total > BigInt::zero() {
        Some(total)
    } else {
        None
    })
}

fn mxfp4_failure(reason: &PyStr) -> Failure {
    let mut message = s("MXFP4_HEADER_LAYOUT_UNSUPPORTED: ");
    message.push_py(reason);
    Failure::with_message("ValueError", message)
}

/// `fail(f"{prefix}{exc}")` for a caught exception.
fn mxfp4_rewrap(prefix: &PyStr, caught: &Failure) -> Failure {
    let mut reason = prefix.clone();
    reason.push_py(&caught.message);
    mxfp4_failure(&reason)
}

struct Tensor {
    name: PyStr,
    dtype: &'static str,
    shape: Vec<BigInt>,
    numel: BigInt,
}

/// `count_mxfp4_params(host_path)`: validates the headers of an explicitly declared MXFP4 checkpoint (every shard, every
/// tensor, offsets, payload coverage, blocks/scales pairing) and counts logical parameters; any unsupported structure is a
/// `ValueError` starting with `MXFP4_HEADER_LAYOUT_UNSUPPORTED`. Repeated JSON keys are rejected at every object level.
pub fn count_mxfp4_params(host_path: &PyValue) -> Res<PyDict> {
    let index_path = join_values(host_path, &PyValue::str(INDEX_NAME))?;
    let host = fspath(host_path, true)?.clone();
    let float_types = ["BF16", "F16", "F32", "F64"];
    let dtype_name = |text: &PyStr| -> Option<&'static str> {
        float_types
            .iter()
            .chain(["U8"].iter())
            .find(|name| text.equals(name))
            .copied()
    };
    let mut weight_map: Option<&PyDict> = None;
    let index_value;
    let files: Vec<PyStr> = if exists(&index_path) {
        let loaded = load_json_file_unique(&index_path).and_then(|index| weight_map_of(&index));
        index_value = match loaded {
            Ok(map) => map,
            Err(failure)
                if crate::model_dyn::is_os_error(failure.class)
                    || crate::model_dyn::is_value_error(failure.class)
                    || failure.class == "AttributeError" =>
            {
                return Err(mxfp4_rewrap(&s("index 읽기 실패: "), &failure));
            }
            Err(failure) => return Err(failure),
        };
        let PyValue::Dict(map) = index_value.peel() else {
            return Err(mxfp4_failure(&s("index weight_map 부재/부정합")));
        };
        if map.is_empty() {
            return Err(mxfp4_failure(&s("index weight_map 부재/부정합")));
        }
        for (key, value) in map.iter() {
            let bad = key.is_empty()
                || !matches!(value.peel(), PyValue::Str(shard)
                    if basename(shard) == *shard && ends_with(shard, ".safetensors"));
            if bad {
                return Err(mxfp4_failure(&s("index 텐서/샤드 이름 부정합")));
            }
        }
        weight_map = Some(map);
        let mut shards: Vec<PyStr> = Vec::new();
        for (_, value) in map.iter() {
            if let PyValue::Str(shard) = value.peel() {
                if !shards.contains(shard) {
                    shards.push(shard.clone());
                }
            }
        }
        sorted_texts(shards)
    } else {
        let listed: Vec<PyStr> = glob(&join(&host, &s("*.safetensors")))
            .iter()
            .map(basename)
            .collect();
        sorted_texts(listed)
    };
    if files.is_empty() {
        return Err(mxfp4_failure(&s("정본 safetensors 샤드 부재")));
    }
    let weight_map_get = |name: &PyStr| -> Option<&PyValue> {
        weight_map.and_then(|map| {
            map.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value)
        })
    };
    let mut tensors: Vec<Tensor> = Vec::new();
    let mut positions: std::collections::HashMap<Vec<u32>, usize> =
        std::collections::HashMap::new();
    let (mut payload_bytes, mut file_bytes, mut storage_numel) =
        (BigInt::zero(), BigInt::zero(), BigInt::zero());
    for shard in &files {
        let path = join(&host, shard);
        let prefix = {
            let mut text = shard.clone();
            text.push_str(": 헤더 읽기 실패: ");
            text
        };
        let read = (|| -> Res<(u64, u64, PyValue)> {
            let size = file_size(&path)?;
            let mut file = open_binary(&path)?;
            let length = read_up_to(&mut file, 8)?;
            if length.len() != 8 {
                return Err(Failure::new("error", "unpack requires a buffer of 8 bytes"));
            }
            let n = u64::from_le_bytes(length.as_slice().try_into().expect("8 bytes"));
            if n > SAFETENSORS_MAX_HEADER || n > size.wrapping_sub(8) || size < 8 {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(shard)
                        .lit(": 헤더가 크기 상한/파일 경계를 초과")
                        .done(),
                ));
            }
            let bytes = read_up_to(&mut file, n)?;
            Ok((size, n, loads_bytes_unique(&bytes)?))
        })();
        let (size, n, header) = match read {
            Ok(parts) => parts,
            Err(failure) if is_os_value_or_struct_error(failure.class) => {
                return Err(mxfp4_rewrap(&prefix, &failure));
            }
            Err(failure) => return Err(failure),
        };
        let PyValue::Dict(header) = header.peel() else {
            return Err(mxfp4_failure(
                &Message::new().text(shard).lit(": 헤더 객체 부재").done(),
            ));
        };
        let limit = i128::from(size) - 8 - i128::from(n);
        let mut spans: Vec<(BigInt, BigInt)> = Vec::new();
        for (name, meta) in header.iter() {
            if name.equals("__metadata__") {
                continue;
            }
            let duplicate = positions.contains_key(name.code_points());
            let PyValue::Dict(meta) = meta.peel() else {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(name)
                        .lit(": 중복 텐서/헤더 부정합")
                        .done(),
                ));
            };
            if duplicate {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(name)
                        .lit(": 중복 텐서/헤더 부정합")
                        .done(),
                ));
            }
            if weight_map.is_some() {
                let mapped = weight_map_get(name);
                let same =
                    matches!(mapped.map(PyValue::peel), Some(PyValue::Str(held)) if held == shard);
                if !same {
                    return Err(mxfp4_failure(
                        &Message::new()
                            .text(name)
                            .lit(": index와 실제 샤드 불일치")
                            .done(),
                    ));
                }
            }
            let dtype = meta.get("dtype").cloned().unwrap_or(PyValue::None);
            let shape = meta.get("shape").cloned().unwrap_or(PyValue::None);
            let offsets = meta.get("data_offsets").cloned().unwrap_or(PyValue::None);
            let dt = match dtype.peel() {
                PyValue::Str(text) => dtype_name(text),
                _ => None,
            };
            let Some(dt) = dt else {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(name)
                        .lit(": 미지원 dtype ")
                        .lit(&dtype.repr())
                        .done(),
                ));
            };
            let shape_values: Option<Vec<BigInt>> = match shape.peel() {
                PyValue::List(items) => items
                    .iter()
                    .map(|d| match d {
                        PyValue::Int(number) if number > &BigInt::zero() => Some(number.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            };
            let Some(shape_values) = shape_values else {
                return Err(mxfp4_failure(
                    &Message::new().text(name).lit(": shape 부정합").done(),
                ));
            };
            let mut numel = BigInt::from(1);
            for d in &shape_values {
                numel *= d;
            }
            let elem = BigInt::from(
                SAFETENSORS_DTYPE_ELEM_BYTES
                    .iter()
                    .find(|(known, _)| *known == dt)
                    .map_or(2, |&(_, bytes)| bytes),
            );
            let span = match offsets.peel() {
                PyValue::List(pair) if pair.len() == 2 => match (&pair[0], &pair[1]) {
                    (PyValue::Int(start), PyValue::Int(stop))
                        if *start >= BigInt::zero()
                            && *stop >= BigInt::zero()
                            && stop - start == &numel * &elem
                            && stop.to_i128().is_some_and(|stop| stop <= limit) =>
                    {
                        Some((start.clone(), stop.clone()))
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(span) = span else {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(name)
                        .lit(": dtype/shape/offset/파일 경계 불일치")
                        .done(),
                ));
            };
            spans.push(span);
            storage_numel += &numel;
            positions.insert(name.code_points().to_vec(), tensors.len());
            tensors.push(Tensor {
                name: name.clone(),
                dtype: dt,
                shape: shape_values,
                numel,
            });
        }
        let mut end = BigInt::zero();
        spans.sort();
        for (start, stop) in spans {
            if start != end {
                return Err(mxfp4_failure(
                    &Message::new()
                        .text(shard)
                        .lit(": payload 중복/빈 구간")
                        .done(),
                ));
            }
            end = stop;
        }
        if end.to_i128() != Some(limit) {
            return Err(mxfp4_failure(
                &Message::new()
                    .text(shard)
                    .lit(": payload와 헤더 크기 불일치")
                    .done(),
            ));
        }
        payload_bytes += end;
        file_bytes += size;
    }
    if let Some(map) = weight_map {
        let declared_all_present = map.len() == tensors.len()
            && map
                .iter()
                .all(|(key, _)| positions.contains_key(key.code_points()));
        if !declared_all_present {
            return Err(mxfp4_failure(&s("index가 선언한 텐서 누락")));
        }
    }
    let (mut unpacked, mut packed, mut scales) = (BigInt::zero(), BigInt::zero(), BigInt::zero());
    let (mut pairs, mut expert_pairs) = (0usize, 0usize);
    let mut consumed: std::collections::HashSet<Vec<u32>> = std::collections::HashSet::new();
    for tensor in &tensors {
        let suffix = ["_blocks", ".blocks"]
            .into_iter()
            .find(|suffix| ends_with(&tensor.name, suffix));
        let Some(suffix) = suffix else {
            continue;
        };
        let stem_len = tensor.name.len() - suffix.chars().count();
        let mut scale_name =
            PyStr::from_code_points(tensor.name.code_points()[..stem_len].to_vec());
        scale_name.push_str(&suffix.replace("blocks", "scales"));
        let scale = positions
            .get(scale_name.code_points())
            .map(|&at| &tensors[at]);
        let paired = tensor.dtype == "U8"
            && tensor.shape.len() >= 2
            && tensor.shape[tensor.shape.len() - 1] == BigInt::from(16)
            && scale.is_some_and(|scale| {
                scale.dtype == "U8" && scale.shape[..] == tensor.shape[..tensor.shape.len() - 1]
            });
        let Some(scale) = scale.filter(|_| paired) else {
            return Err(mxfp4_failure(
                &Message::new()
                    .text(&tensor.name)
                    .lit(": U8 blocks[...,16]/scales[...] 쌍 부정합")
                    .done(),
            ));
        };
        consumed.insert(tensor.name.code_points().to_vec());
        consumed.insert(scale_name.code_points().to_vec());
        packed += &tensor.numel;
        scales += &scale.numel;
        pairs += 1;
        let lowered = lower(&tensor.name);
        expert_pairs += usize::from(contains(&lowered, "expert") && !contains(&lowered, "shared"));
    }
    if pairs == 0 {
        return Err(mxfp4_failure(&s("지원하는 blocks/scales 쌍 부재")));
    }
    for tensor in &tensors {
        if consumed.contains(tensor.name.code_points()) {
            continue;
        }
        if !float_types.contains(&tensor.dtype)
            || ends_with(&tensor.name, "_scales")
            || ends_with(&tensor.name, ".scales")
        {
            return Err(mxfp4_failure(
                &Message::new()
                    .text(&tensor.name)
                    .lit(": 미확인 packed 텐서/고아 scale")
                    .done(),
            ));
        }
        unpacked += &tensor.numel;
    }
    let big = |count: usize| PyValue::int(BigInt::from(count));
    let mut out = PyDict::new();
    out.set("format", PyValue::str("mxfp4-blocks16-u8-scales-u8"));
    out.set("num_shards", big(files.len()));
    out.set("tensor_count", big(tensors.len()));
    out.set(
        "logical_params",
        PyValue::int(&unpacked + BigInt::from(2) * &packed),
    );
    out.set("unpacked_params", PyValue::int(unpacked));
    out.set("packed_storage_numel", PyValue::int(packed));
    out.set("scale_metadata_numel", PyValue::int(scales));
    out.set("storage_numel", PyValue::int(storage_numel));
    out.set("payload_bytes", PyValue::int(payload_bytes));
    out.set("file_bytes", PyValue::int(file_bytes));
    out.set("packed_tensor_count", big(pairs));
    out.set("packed_expert_tensor_count", big(expert_pairs));
    Ok(out)
}

/// A dict of an `Option`al list of tuples, for the `counts`/`warnings` encodings.
fn warnings_value(warnings: Vec<PyValue>) -> PyValue {
    PyValue::List(warnings)
}

/// `parse(model_path, nas_host_root=DEFAULT_NAS_HOST_ROOT, nas_container_root="/app/models")`: the normalised description of
/// a model directory's `config.json` and weights, with warnings for everything inferred or missing.
pub fn parse(
    model_path: &PyValue,
    nas_host_root: &PyValue,
    nas_container_root: &PyValue,
) -> Res<PyDict> {
    let mut warnings: Vec<PyValue> = Vec::new();
    let mut note = |text: PyStr| warnings.push(PyValue::Str(text));
    let host_path = resolve_host_path(model_path, nas_host_root, nas_container_root)?;
    if !is_dir(&host_path) {
        return Err(Failure::with_message(
            "FileNotFoundError",
            Message::new()
                .lit("모델 디렉토리 없음: ")
                .text(&host_path)
                .lit(" (컨테이너 경로 ")
                .val(model_path)
                .lit("). 폐쇄망 — 모델 자체 다운로드 금지. NAS에 사전 배치 후 재시도.")
                .done(),
        ));
    }
    let config_path = join(&host_path, &s("config.json"));
    if !is_file(&config_path) {
        return Err(Failure::with_message(
            "FileNotFoundError",
            Message::new()
                .lit("config.json 없음: ")
                .text(&config_path)
                .lit(". 폐쇄망 — 다운로드 금지. 중단·보고.")
                .done(),
        ));
    }
    let config_value = load_json_file(&config_path)?;
    let config = as_get_target(&config_value)?;
    // plan_26111708 B L9: the nested text config is resolved by the one owner (vLLM's `llm_config` remap + transformers'
    // `get_text_config` names), not by `text_config` alone — the Nemotron omni VL config nests its language model under
    // `llm_config`, and every field (MoE, heads, layers) used to be missed.
    let resolved = text_config_of(config)?;
    let empty = PyDict::new();
    let text_config: Option<&PyDict> = resolved.nested;
    let text_or_empty: &PyDict = text_config.unwrap_or(&empty);
    if let (Some(key), true) = (resolved.key, resolved.key != Some("text_config") || resolved.remapped_from.is_some()) {
        let source = resolved.remapped_from.unwrap_or(key);
        note(
            Message::new()
                .lit("텍스트 config ← '")
                .lit(source)
                .lit("'")
                .lit(if resolved.remapped_from.is_some() {
                    " (vLLM _CONFIG_ATTRS_MAPPING llm_config→text_config · transformers_utils/config.py:152)"
                } else {
                    " (transformers get_text_config · configuration_utils.py:1366)"
                })
                .done(),
        );
    }
    let field = |key: &str| -> PyValue {
        if let Some(text) = text_config {
            if let Some(value) = text.get(key) {
                return value.clone();
            }
        }
        config.get(key).cloned().unwrap_or(PyValue::None)
    };
    let is_none = |value: &PyValue| matches!(value.peel(), PyValue::None);

    let hidden_size = field("hidden_size");
    let mut num_hidden_layers = field("num_hidden_layers");
    // plan_26111708 B L9: a NemotronH text config names its layers `layers_block_type` (transformers aliases `layer_types` to
    // it — `configuration_nemotron_h.py:84` attribute_map) and derives the layer count from its length (`num_hidden_layers`
    // property, `:229-235`); its legacy names (`mamba`, `attention`) are remapped as transformers remaps every config.
    let mut layer_types = field("layer_types");
    if is_none(&layer_types) {
        let blocks = field("layers_block_type");
        if let PyValue::List(items) = blocks.peel() {
            if !items.is_empty() {
                let count = items.len();
                layer_types = blocks.clone();
                note(s("layer_types ← layers_block_type (transformers NemotronH attribute_map · configuration_nemotron_h.py:84)"));
                if is_none(&num_hidden_layers) {
                    num_hidden_layers = PyValue::int(BigInt::from(count));
                    note(
                        Message::new()
                            .lit("num_hidden_layers ← len(layers_block_type)=")
                            .count(count)
                            .lit(" (transformers NemotronH num_hidden_layers property · configuration_nemotron_h.py:229)")
                            .done(),
                    );
                }
            }
        }
    }
    let layer_types = remap_legacy_layer_types(&layer_types);
    let num_attention_heads = field("num_attention_heads");
    if is_none(&hidden_size) {
        note(s("hidden_size 누락 (필수)"));
    }
    if is_none(&num_hidden_layers) {
        note(s("num_hidden_layers 누락 (필수)"));
    }
    if is_none(&num_attention_heads) {
        note(s("num_attention_heads 누락 (필수)"));
    }
    let mut num_key_value_heads = field("num_key_value_heads");
    if is_none(&num_key_value_heads) {
        num_key_value_heads = num_attention_heads.clone();
        if !is_none(&num_attention_heads) {
            note(s(
                "num_key_value_heads 누락 → num_attention_heads 사용 (MHA 가정)",
            ));
        }
    }
    let qk_nope_head_dim = field("qk_nope_head_dim");
    let qk_rope_head_dim = field("qk_rope_head_dim");
    let kv_lora_rank = field("kv_lora_rank");
    let is_mla =
        !is_none(&qk_nope_head_dim) || !is_none(&qk_rope_head_dim) || !is_none(&kv_lora_rank);

    let mut head_dim = field("head_dim");
    if is_none(&head_dim) {
        if is_mla && !is_none(&qk_nope_head_dim) && !is_none(&qk_rope_head_dim) {
            let sum = int_of(&qk_nope_head_dim)? + int_of(&qk_rope_head_dim)?;
            head_dim = PyValue::Int(sum);
            note(
                Message::new()
                    .lit("MLA 감지(qk_nope=")
                    .val(&qk_nope_head_dim)
                    .lit(", qk_rope=")
                    .val(&qk_rope_head_dim)
                    .lit(", kv_lora_rank=")
                    .val(&kv_lora_rank)
                    .lit(") → head_dim=")
                    .val(&head_dim)
                    .lit(" 사용. KV_cache 공식은 MLA 압축 KV 미반영 → 과대추정(보수적·OOM 안전)")
                    .done(),
            );
        } else if !is_none(&hidden_size) && num_attention_heads.is_truthy() {
            head_dim = floor_div(&hidden_size, &num_attention_heads)?;
            if is_mla {
                let again = floor_div(&hidden_size, &num_attention_heads)?;
                note(
                    Message::new()
                        .lit("head_dim 누락 + MLA 신호(kv_lora_rank=")
                        .val(&kv_lora_rank)
                        .lit(") 감지지만 qk_nope/qk_rope_head_dim 불완전 → hidden_size // num_attention_heads =")
                        .val(&again)
                        .lit(" 폴백(근거 약함, KV_cache 과대/오추정 가능)")
                        .done(),
                );
            } else {
                note(s("head_dim 누락 → hidden_size // num_attention_heads 사용"));
            }
        } else {
            note(s(
                "head_dim 누락 + hidden_size/num_attention_heads 부족 → 산출 불가",
            ));
        }
    }

    let full_attention_interval = field("full_attention_interval");
    let kv = resolve_kv_layers(&layer_types, &num_hidden_layers, &full_attention_interval)?;
    let (num_full_attention_layers, is_hybrid, layer_type_counts, lt_warnings) = (
        kv.num_full.clone(),
        kv.is_hybrid,
        KvLayers {
            num_full: None,
            is_hybrid: false,
            counts: kv.counts.clone(),
            warnings: Vec::new(),
        }
        .into_value(),
        kv.warnings,
    );
    let layer_type_counts = match layer_type_counts {
        PyValue::Tuple(mut parts) => parts.remove(2),
        other => other,
    };
    for warning in lt_warnings {
        note(warning);
    }

    let vocab_size = field("vocab_size");
    if is_none(&vocab_size) {
        note(s("vocab_size 누락"));
    }
    let max_position_embeddings = field("max_position_embeddings");
    if is_none(&max_position_embeddings) {
        note(s("max_position_embeddings 누락"));
    }
    let config_model_type = config.get("model_type").cloned().unwrap_or(PyValue::None);
    let field_model_type = field("model_type");
    let model_type = or_chain(&[&field_model_type, &config_model_type]).clone();
    let config_architectures = config
        .get("architectures")
        .cloned()
        .unwrap_or(PyValue::None);
    let field_architectures = field("architectures");
    let architectures = or_chain(&[&field_architectures, &config_architectures]).clone();

    let from_config = |key: &str| config.get(key).cloned().unwrap_or(PyValue::None);
    let from_text = |key: &str| text_or_empty.get(key).cloned().unwrap_or(PyValue::None);
    let candidates = [
        from_config("torch_dtype"),
        from_config("dtype"),
        from_text("dtype"),
        from_text("torch_dtype"),
    ];
    let mut native_dtype = or_chain(&[
        &candidates[0],
        &candidates[1],
        &candidates[2],
        &candidates[3],
    ])
    .clone();
    if is_none(&native_dtype) {
        native_dtype = PyValue::str("bfloat16");
        note(s("torch_dtype/dtype 누락 → 'bfloat16' 기본 가정"));
    }
    let (serve_bpw, dtype_warn) = match quant_table::dtype_bpw(&native_dtype)? {
        PyValue::Tuple(mut parts) => {
            let warn = parts.remove(1);
            (parts.remove(0), warn)
        }
        other => (other, PyValue::None),
    };
    if dtype_warn.is_truthy() {
        if let PyValue::Str(text) = dtype_warn {
            note(text);
        }
    }
    let serve_bpw_float = float_of(&serve_bpw)?;

    let mut quant_config = from_config("quantization_config");
    if !matches!(quant_config.peel(), PyValue::Dict(_)) {
        quant_config = from_text("quantization_config");
    }
    let prequantized = matches!(quant_config.peel(), PyValue::Dict(_));
    let mut quant_method_native = PyValue::None;
    if prequantized {
        quant_method_native = get(&quant_config, "quant_method")?;
    }

    let mut num_experts = PyValue::None;
    for key in ["num_experts", "num_local_experts", "n_routed_experts"] {
        if is_none(&num_experts) {
            num_experts = field(key);
        }
    }
    let num_experts_per_tok = field("num_experts_per_tok");
    let is_moe = !is_none(&num_experts)
        || matches!(model_type.peel(), PyValue::Str(text) if contains(&lower(text), "moe"));

    let host_value = PyValue::Str(host_path.clone());
    let parameter_count = if py_eq(&quant_method_native, &PyValue::str("mxfp4")) {
        Some(count_mxfp4_params(&host_value)?)
    } else {
        None
    };
    let native_weight_bytes = {
        let mut sink = Warnings::List(&mut warnings);
        native_weight_bytes(&host_value, &mut sink)?
    };
    let mut note = |text: PyStr| warnings.push(PyValue::Str(text));

    let mut disk_dtype: Option<PyValue> = None;
    let mut disk_bpw: Option<f64> = None;
    if let Some(largest) = largest_safetensors(&host_value)? {
        let (dtype, bpw) = measure_disk_dtype(&PyValue::Str(largest))?;
        disk_dtype = dtype;
        disk_bpw = bpw;
    }
    let disk_bpw_value = match disk_bpw {
        Some(bpw) => bpw,
        None => {
            note(s(
                "safetensors 헤더에서 disk dtype 실측 실패 → disk_bpw=serve_bpw 폴백",
            ));
            serve_bpw_float
        }
    };
    let mut disk_native_mismatch = false;
    if let Some(dtype) = &disk_dtype {
        if (disk_bpw_value - serve_bpw_float).abs() > 1e-9 {
            disk_native_mismatch = true;
            note(
                Message::new()
                    .lit("disk_dtype(")
                    .val(dtype)
                    .lit(", bpw=")
                    .float(disk_bpw_value)
                    .lit(") != native_dtype(")
                    .val(&native_dtype)
                    .lit(", bpw=")
                    .float(serve_bpw_float)
                    .lit(") — 저장 정밀도 ≠ config dtype. ")
                    .lit(if parameter_count.is_some() {
                        "num_params 는 MXFP4 blocks/scales 논리 원소 검증값(미지원 구조는 중단)"
                    } else {
                        "num_params 는 헤더 numel 합(정확) 우선, 불가 시 보수적 bpw 폴백"
                    })
                    .done(),
            );
        }
    }

    let (num_params, weight_basis): (PyValue, PyValue) = if let Some(count) = &parameter_count {
        (
            count
                .get("logical_params")
                .cloned()
                .unwrap_or(PyValue::None),
            PyValue::str("safetensors_mxfp4_logical_numel(exact)"),
        )
    } else if let Some(native) = native_weight_bytes.as_ref() {
        if prequantized {
            let (bpw, qwarn) =
                match quant_table::vllm_quant_bpw_or_none(&quant_method_native, &serve_bpw)? {
                    PyValue::Tuple(mut parts) => {
                        let warn = parts.remove(1);
                        (parts.remove(0), warn)
                    }
                    other => (other, PyValue::None),
                };
            if is_none(&bpw) && quant_table::is_mixed_precision_quant(&quant_method_native)? {
                // plan_26111708 B L9: a registered vLLM method with per-layer precision — a named reason, not "unknown".
                note(
                    Message::new()
                        .lit("quant_method ")
                        .lit(&quant_method_native.repr().to_string())
                        .lit(" = 층별 혼합 정밀도(vLLM ModelOptMixedPrecisionConfig · quantization/modelopt.py:1543 — quantization_config.quantized_layers 의 층별 quant_algo) → 단일 bpw 가 정의되지 않는다 → disk_bpw(")
                        .float(disk_bpw_value)
                        .lit(") 실측으로 num_params 폴백(best-effort · 가중치 바이트는 disk 실측 그대로)")
                        .done(),
                );
                (
                    PyValue::Int(round_to_int(int_over_float(native, disk_bpw_value)?)?),
                    PyValue::str("disk_total/disk_bpw(prequant_mixed_precision)"),
                )
            } else if is_none(&bpw) {
                note(
                    Message::new()
                        .val(&qwarn)
                        .lit(" → disk_bpw(")
                        .float(disk_bpw_value)
                        .lit(") 실측으로 num_params 폴백(best-effort)")
                        .done(),
                );
                (
                    PyValue::Int(round_to_int(int_over_float(native, disk_bpw_value)?)?),
                    PyValue::str("disk_total/disk_bpw(prequant_unknown_method)"),
                )
            } else {
                let divisor = float_of(&bpw)?;
                (
                    PyValue::Int(round_to_int(int_over_float(native, divisor)?)?),
                    PyValue::str("disk_total_prequant"),
                )
            }
        } else if let Some(exact) = count_params_from_headers(&host_value)? {
            (
                PyValue::Int(exact),
                PyValue::str("safetensors_numel_sum(exact)"),
            )
        } else if disk_native_mismatch {
            let np_disk = round_to_int(int_over_float(native, disk_bpw_value)?)?;
            let np_native = round_to_int(int_over_float(native, serve_bpw_float)?)?;
            let chosen = if np_disk >= np_native {
                np_disk.clone()
            } else {
                np_native.clone()
            };
            let basis = if chosen == np_native {
                "disk_total/serve_bpw(mixed_dtype_conservative)"
            } else {
                "disk_total/disk_bpw(mixed_dtype_conservative)"
            };
            (PyValue::Int(chosen), PyValue::str(basis))
        } else {
            (
                PyValue::Int(round_to_int(int_over_float(native, disk_bpw_value)?)?),
                PyValue::str("disk_total/disk_bpw"),
            )
        }
    } else {
        note(s(
            "native_weight_bytes 부재 → num_params=None (estimate 단계 에러표시)",
        ));
        (PyValue::None, PyValue::None)
    };

    let mut out = PyDict::new();
    out.set("model_path_container", model_path.clone());
    out.set("model_path_host", PyValue::Str(host_path));
    out.set("model_id", PyValue::Str(model_id_from_path(model_path)?));
    out.set("model_type", model_type);
    out.set("architectures", architectures);
    out.set("is_moe", PyValue::Bool(is_moe));
    out.set("native_dtype", native_dtype);
    out.set("serve_bpw", serve_bpw);
    out.set("disk_dtype", disk_dtype.unwrap_or(PyValue::None));
    out.set("disk_bpw", PyValue::Float(disk_bpw_value));
    out.set("prequantized", PyValue::Bool(prequantized));
    out.set("quant_method_native", quant_method_native);
    out.set("num_hidden_layers", num_hidden_layers);
    out.set(
        "num_full_attention_layers",
        num_full_attention_layers.map_or(PyValue::None, PyValue::Int),
    );
    out.set("is_hybrid", PyValue::Bool(is_hybrid));
    out.set("layer_type_counts", layer_type_counts);
    out.set("full_attention_interval", full_attention_interval);
    out.set("num_attention_heads", num_attention_heads);
    out.set("num_key_value_heads", num_key_value_heads);
    out.set("head_dim", head_dim);
    out.set("hidden_size", hidden_size);
    out.set("vocab_size", vocab_size);
    out.set("max_position_embeddings", max_position_embeddings);
    out.set("num_experts", num_experts);
    out.set("num_experts_per_tok", num_experts_per_tok);
    out.set(
        "native_weight_bytes",
        native_weight_bytes.map_or(PyValue::None, PyValue::Int),
    );
    out.set("num_params", num_params);
    out.set("weight_basis", weight_basis);
    if let Some(count) = parameter_count {
        out.set("parameter_count", PyValue::Dict(count));
    }
    out.set("warnings", warnings_value(warnings));
    Ok(out)
}

#[cfg(test)]
mod text_config_tests {
    //! plan_26111708 B L9: the one owner of nested text-config resolution, and the Nemotron omni VL shape end to end
    //! (parse → `is_moe` · hybrid layers → estimate without an error).
    use super::*;
    use agent_runtime_support::pyjson::loads_text;

    fn dict(json: &str) -> PyDict {
        match loads_text(&PyStr::new(json)).expect("json") {
            PyValue::Dict(map) => map,
            other => panic!("{other:?}"),
        }
    }

    fn hidden(resolved: &TextConfig<'_>) -> Option<PyValue> {
        resolved.nested.and_then(|map| map.get("hidden_size").cloned())
    }

    #[test]
    fn the_nested_text_config_is_found_where_vllm_and_transformers_look() {
        // vLLM's remap: llm_config → text_config (no text_config key)
        let nemotron = dict(r#"{"model_type": "nemotron_h_omni", "llm_config": {"hidden_size": 4096}, "vision_config": {"hidden_size": 1}}"#);
        let r = text_config_of(&nemotron).expect("one");
        assert_eq!((r.key, r.remapped_from, hidden(&r)), (Some("text_config"), Some("llm_config"), Some(PyValue::int(4096))));
        // text_config present → the remap does not apply (vLLM: `if not hasattr(config, new_attr)`)
        let both = dict(r#"{"text_config": {"hidden_size": 8}, "llm_config": {"hidden_size": 9}}"#);
        let r = text_config_of(&both).expect("one");
        assert_eq!((r.key, r.remapped_from, hidden(&r)), (Some("text_config"), None, Some(PyValue::int(8))));
        // transformers' other names
        let decoder = dict(r#"{"decoder": {"hidden_size": 7}}"#);
        assert_eq!(text_config_of(&decoder).expect("one").key, Some("decoder"));
        // flat, null, non-mapping → the config itself
        for flat in [r#"{"hidden_size": 5}"#, r#"{"text_config": null}"#, r#"{"text_config": ["nested"]}"#, r#"{"generator": "x"}"#] {
            let config = dict(flat);
            let r = text_config_of(&config).expect("flat");
            assert!(r.nested.is_none() && r.key.is_none(), "{flat}");
        }
        // negative: two text configs are transformers' ambiguity error
        let failure = text_config_of(&dict(r#"{"decoder": {}, "text_config": {}}"#)).expect_err("ambiguous");
        assert!(failure.is_value_error() && failure.message.to_utf8().unwrap().contains("['decoder', 'text_config']"));
    }

    #[test]
    fn legacy_layer_names_are_remapped_and_others_kept() {
        let types = loads_text(&PyStr::new(r#"["mamba", "attention", "moe", "full_attention", "sliding_attention", 3]"#)).unwrap();
        let want = loads_text(&PyStr::new(r#"["linear_attention", "full_attention", "moe", "full_attention", "sliding_attention", 3]"#)).unwrap();
        assert_eq!(remap_legacy_layer_types(&types), want);
        assert_eq!(remap_legacy_layer_types(&PyValue::None), PyValue::None);
    }

    /// The Nemotron-3.5-Super-VL layout in miniature: the language model under `llm_config` (MoE · `layers_block_type` with
    /// legacy names · no `num_hidden_layers`), a top-level ModelOpt MIXED_PRECISION `quantization_config`, one weight file.
    fn nemotron_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rx_nemotron_vl_{}_{label}", std::process::id()));
        let model = dir.join("nvidia/Nemotron-VL");
        std::fs::create_dir_all(&model).unwrap();
        let blocks: Vec<&str> = (0..11).map(|i| match i % 11 { 3 | 9 => "\"attention\"", i if i % 2 == 0 => "\"mamba\"", _ => "\"moe\"" }).collect();
        std::fs::write(
            model.join("config.json"),
            format!(
                r#"{{"architectures": ["NemotronH_Omni_Reasoning_V3"], "model_type": "nemotron_h_omni", "dtype": "bfloat16",
                    "quantization_config": {{"quant_method": "modelopt_mixed", "quant_algo": "MIXED_PRECISION",
                                             "quantized_layers": {{"language_model.backbone.layers.1.mixer": {{"quant_algo": "NVFP4"}}}}}},
                    "vision_config": {{"hidden_size": 1280, "num_attention_heads": 16}},
                    "llm_config": {{"model_type": "nemotron_h", "hidden_size": 4096, "num_attention_heads": 32, "num_key_value_heads": 2,
                                    "head_dim": 128, "n_routed_experts": 512, "num_experts_per_tok": 22, "vocab_size": 131072,
                                    "max_position_embeddings": 1048576, "layers_block_type": [{}]}}}}"#,
                blocks.join(", ")
            ),
        )
        .unwrap();
        let weights = std::fs::File::create(model.join("pytorch_model.bin")).unwrap();
        weights.set_len(64 << 20).unwrap();
        dir
    }

    #[test]
    fn a_nemotron_omni_config_parses_as_a_hybrid_moe_and_estimates_without_an_error() {
        let dir = nemotron_dir("parse");
        let parsed = parse(&PyValue::str("/app/models/nvidia/Nemotron-VL"), &PyValue::str(dir.to_str().unwrap()), &PyValue::str("/app/models")).expect("parse");
        let at = |key: &str| parsed.get(key).cloned().unwrap_or(PyValue::None);
        assert_eq!(at("is_moe"), PyValue::Bool(true));
        assert_eq!(at("num_experts"), PyValue::int(512));
        assert_eq!(at("num_hidden_layers"), PyValue::int(11));
        assert_eq!(at("num_full_attention_layers"), PyValue::int(2));
        assert_eq!(at("is_hybrid"), PyValue::Bool(true));
        assert_eq!((at("num_key_value_heads"), at("head_dim")), (PyValue::int(2), PyValue::int(128)));
        assert_eq!(at("weight_basis"), PyValue::str("disk_total/disk_bpw(prequant_mixed_precision)"));
        let warnings = at("warnings").dumps_ascii().unwrap();
        assert!(!warnings.contains("unknown quant_method") && !warnings.contains("hidden_size \\ub204\\ub77d"), "{warnings}");
        let parsed = PyValue::Dict(parsed);
        let candidate = |quant: &str| PyValue::Dict(dict(&format!(r#"{{"id": "r1", "quantization": "{quant}", "max_model_len": 4096, "gpu_memory_utilization": 0.9}}"#)));
        let result = crate::estimate_vram::estimate(&parsed, &candidate("modelopt_mixed"), &PyValue::int(2), &PyValue::Float(96.0), &PyValue::Float(0.9), &PyValue::int(2)).expect("estimate");
        assert_eq!(get(&result, "error").unwrap(), PyValue::None, "{}", result.dumps_ascii().unwrap());
        assert_eq!(get(&result, "kv_layers").unwrap(), PyValue::int(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// (b) the named reason: a non-prequantized checkpoint cannot be served as `modelopt_mixed`; an unregistered method keeps
    /// the generic `unknown_quant_method`.
    #[test]
    fn modelopt_mixed_without_a_prequantized_checkpoint_has_a_named_reason() {
        let parsed = PyValue::Dict(dict(r#"{"prequantized": false, "num_params": 1000000, "serve_bpw": 2.0, "num_hidden_layers": 2,
                                         "num_key_value_heads": 2, "head_dim": 64}"#));
        let run = |quant: &str| {
            let candidate = PyValue::Dict(dict(&format!(r#"{{"id": "r1", "quantization": "{quant}", "max_model_len": 4096, "gpu_memory_utilization": 0.9}}"#)));
            let result = crate::estimate_vram::estimate(&parsed, &candidate, &PyValue::int(1), &PyValue::Float(96.0), &PyValue::Float(0.9), &PyValue::int(2)).unwrap();
            get(&result, "error").unwrap()
        };
        assert_eq!(run("modelopt_mixed"), PyValue::str("mixed_precision_requires_prequantized_checkpoint"));
        assert_eq!(run("no-such-method"), PyValue::str("unknown_quant_method"));
        let failure = crate::estimate_vram::weights_bytes_for(&parsed, &PyValue::str("MODELOPT_MIXED")).expect_err("named");
        assert!(failure.message.to_utf8().unwrap().contains("mixed_precision_requires_prequantized_checkpoint"));
    }
}
