//! The two records of the max-model-len envelope walk: one level's result and the index of the whole walk.

use super::document::write_pretty;
use super::{Console, Context, Done};
use crate::dataset_prep::dump_failure;
use crate::guidellm::Failure;
use crate::lite_engine_log::decode_log_text;
use crate::ospath;
use crate::pynumber::int_from_str;
use crate::pystr::{is_python_space, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_text::cat;
use crate::sweep_meta_scan as scan;
use num_bigint::BigInt;

fn int_of(text: &PyStr) -> Result<BigInt, Failure> {
    int_from_str(text).map_err(Failure::from)
}

/// `max-level`: one level's result as a JSON line (`ML`, `SE`, `ST` from the environment).
pub(super) fn max_level(context: &Context<'_>, console: &mut Console) -> Done {
    let length = int_of(&context.required("ML")?)?;
    let exit_status = int_of(&context.required("SE")?)?;
    let status = context.required("ST")?;
    let mut result = PyDict::new();
    result.set("max_model_len", PyValue::int(length));
    result.set("serve_smoke_exit", PyValue::int(exit_status));
    result.set("status", PyValue::Str(status));
    let shown = PyValue::Dict(result)
        .dumps_unicode()
        .map_err(dump_failure)?;
    console.print(shown);
    Ok(())
}

/// `open(path, encoding="utf-8", errors="replace").read()`, or nothing for any `OSError`.
fn read_text(path: &PyStr) -> Vec<char> {
    match crate::pyfs::read_bytes(path) {
        Ok(bytes) => decode_log_text(&bytes),
        Err(_) => Vec::new(),
    }
}

/// `text.split()`: the words between whitespace of any Unicode kind.
fn words(text: &PyStr) -> Vec<PyStr> {
    text.code_points()
        .split(|&point| is_python_space(point))
        .filter(|word| !word.is_empty())
        .map(|word| PyStr::from_code_points(word.to_vec()))
        .collect()
}

/// A declared value of the config or manifest text, `"NA"`-style fallback applied by the caller.
fn declared(text: &[char], key: &str) -> Option<PyStr> {
    scan::grep_yaml(text, key)
        .filter(|value| !value.is_empty())
        .map(|value| PyStr::new(&value))
}

fn or_text(value: Option<PyStr>, default: &str) -> PyStr {
    value.unwrap_or_else(|| PyStr::new(default))
}

/// One level's recorded result, or the `safe` default when the level directory has no readable result.
fn level_result(max_dir: &PyStr, level: &BigInt) -> Result<PyValue, Failure> {
    let path = ospath::join_all(
        &ospath::join_all(max_dir, &[&format!("level_{level}")]),
        &["result.json"],
    );
    match crate::pyfs::load_json_or_none(&path)? {
        Some(result) => Ok(result),
        None => {
            let mut standard = PyDict::new();
            standard.set("max_model_len", PyValue::int(level.clone()));
            standard.set("status", PyValue::str("safe"));
            Ok(PyValue::Dict(standard))
        }
    }
}

/// `int(os.environ[name]) if os.environ.get(name) else None`.
fn optional_count(context: &Context<'_>, name: &str) -> Result<PyValue, Failure> {
    match context.non_empty(name) {
        Some(text) => Ok(PyValue::int(int_of(&text)?)),
        None => Ok(PyValue::None),
    }
}

/// `max-index`: the index of the walk (`max_index.json` in `MAXDIR`) and one summary line.
pub(super) fn max_index(context: &Context<'_>, console: &mut Console) -> Done {
    // The original put the scripts directory on its import path first; the variable stays required.
    context.required("SDIR")?;
    let config = context.required("CONFIG")?;
    let topology = context.required("TOPO")?;
    let max_dir = context.required("MAXDIR")?;
    let manifest = read_text(&context.required("MANIFEST")?);
    let serve_config = read_text(&context.required("CFGYAML")?);
    let gpu = or_text(declared(&manifest, "gpu_model"), "NA");
    let gpu_key = crate::bench_naming::gpu_key(&PyValue::Str(gpu.clone()));
    let vllm = match context.non_empty("EASY_VLLM_VERSION") {
        Some(version) => version,
        None => or_text(
            scan::comment_version(&serve_config).map(|v| PyStr::new(&v)),
            "NA",
        ),
    };
    let tensor_parallel = or_text(declared(&serve_config, "tensor-parallel-size"), "1");
    let mut levels: Vec<BigInt> = Vec::new();
    for word in words(&context.env.get("SAFE_LEVELS").unwrap_or_default()) {
        levels.push(int_of(&word)?);
    }
    levels.sort();
    let mut safe_levels = Vec::new();
    for level in &levels {
        safe_levels.push(level_result(&max_dir, level)?);
    }
    let truncation = read_text(&ospath::join_all(&max_dir, &["truncation.log"]));
    let truncated: Vec<PyValue> = scan::split_lines(&truncation)
        .iter()
        .map(|line| scan::to_string(line))
        .map(|line| {
            line.trim_matches(|c: char| is_python_space(u32::from(c)))
                .to_string()
        })
        .filter(|line| line.contains("unsafe") || line.contains("skip") || line.contains("미구현"))
        .map(|line| PyValue::str(&line))
        .collect();
    let generated = (context.clock)();
    let mut meta = PyDict::new();
    meta.set("model", PyValue::Str(config.clone()));
    meta.set("gpu_model", PyValue::Str(gpu));
    meta.set("gpu_key", PyValue::Str(gpu_key));
    meta.set("vllm_version", PyValue::Str(vllm));
    meta.set("topology", PyValue::Str(topology.clone()));
    meta.set("tensor_parallel_size", PyValue::Str(tensor_parallel));
    meta.set(
        "driver_version",
        PyValue::Str(or_text(declared(&manifest, "driver_version"), "NA")),
    );
    meta.set(
        "quantization",
        PyValue::Str(or_text(declared(&serve_config, "quantization"), "NA")),
    );
    meta.set(
        "moe_backend",
        PyValue::Str(or_text(declared(&serve_config, "moe-backend"), "NA")),
    );
    meta.set("image_tag", PyValue::str("NA"));
    let ceiling = optional_count(context, "SAFE_CEIL")?;
    let original = optional_count(context, "ORIG_LEN")?;
    let (level_count, truncated_count) = (safe_levels.len(), truncated.len());
    let mut index = PyDict::new();
    index.set("config", PyValue::Str(config));
    index.set("topology", PyValue::Str(topology));
    index.set("axis", PyValue::str("max-model-len"));
    index.set("generated_utc", PyValue::str(&generated));
    index.set("safe_ceiling_max_model_len", ceiling.clone());
    index.set("original_max_model_len", original);
    index.set("meta", PyValue::Dict(meta));
    index.set("safe_levels", PyValue::List(safe_levels));
    index.set("truncated", PyValue::List(truncated));
    let output = ospath::join_all(&max_dir, &["max_index.json"]);
    write_pretty(&output, &PyValue::Dict(index), false)?;
    console.print(cat!(
        "[max_envelope] max_index.json → ",
        output,
        " (안전상한 max-model-len=",
        ceiling.display_text(),
        " · 안전레벨 ",
        level_count.to_string(),
        " · 절삭 ",
        truncated_count.to_string(),
        ")"
    ));
    Ok(())
}
