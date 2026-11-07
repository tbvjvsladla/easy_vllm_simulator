//! `parse_model_config.py --self-test`: the hybrid-KV checks A1-A8 and the MXFP4 header / managed cross-check checks M1-M29 of the
//! original, run for real against small fixture files written to a scratch directory. The original patched
//! `fetch_hf_safetensors_total` and `parse` with `unittest.mock`; here the same behaviour is a dependency the check hands to the
//! function under test (a stand-in fetch, a stand-in parse), so no global state is changed.

use crate::cli::Run;
use crate::estimate_vram;
use crate::model_card::{self, Fetch};
use crate::model_cli::{crosscheck_main, parse_main, Dependencies, Streams};
use crate::model_config::{self, resolve_kv_layers, SAFETENSORS_MAX_HEADER};
use crate::model_files::PrivateDirectory;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pysort::py_eq;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use std::path::{Path, PathBuf};

/// The failures found so far, as the original's `check` collects them.
struct Checks {
    failures: Vec<String>,
}

impl Checks {
    fn check(&mut self, name: &str, got: &PyValue, want: &PyValue) {
        if !py_eq(got, want) {
            self.failures
                .push(format!("{name}: got={} want={}", got.repr(), want.repr()));
        }
    }

    fn fail(&mut self, message: &str) {
        self.failures.push(message.to_string());
    }
}

fn int(number: i64) -> PyValue {
    PyValue::int(BigInt::from(number))
}

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn member(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn dict_of(value: Res<PyDict>) -> PyValue {
    match value {
        Ok(dict) => PyValue::Dict(dict),
        Err(failure) => PyValue::str(&format!(
            "<{}>",
            failure.line().to_utf8().unwrap_or_default()
        )),
    }
}

// ── the fixture files ───────────────────────────────────────────────────────────────────────────

/// One tensor entry of a safetensors header.
#[derive(Clone)]
struct Meta {
    dtype: &'static str,
    shape: Vec<i64>,
    offsets: [i64; 2],
}

fn meta(dtype: &'static str, shape: &[i64], offsets: [i64; 2]) -> Meta {
    Meta {
        dtype,
        shape: shape.to_vec(),
        offsets,
    }
}

type Header = Vec<(String, Meta)>;
type Headers = Vec<(String, Header)>;

const BLOCKS: &str = "model.experts.proj_blocks";
const SCALES: &str = "model.experts.proj_scales";

fn base() -> Headers {
    vec![
        (
            "a.safetensors".to_string(),
            vec![
                ("norm.weight".to_string(), meta("BF16", &[10], [0, 20])),
                (BLOCKS.to_string(), meta("U8", &[2, 16], [20, 52])),
            ],
        ),
        (
            "b.safetensors".to_string(),
            vec![(SCALES.to_string(), meta("U8", &[2], [0, 2]))],
        ),
    ]
}

fn meta_value(entry: &Meta) -> PyValue {
    let mut dict = PyDict::new();
    dict.set("dtype", text(entry.dtype));
    dict.set(
        "shape",
        PyValue::List(entry.shape.iter().map(|&d| int(d)).collect()),
    );
    dict.set(
        "data_offsets",
        PyValue::List(entry.offsets.iter().map(|&o| int(o)).collect()),
    );
    PyValue::Dict(dict)
}

fn header_value(header: &Header) -> PyValue {
    let mut dict = PyDict::new();
    for (name, entry) in header {
        dict.set(name, meta_value(entry));
    }
    PyValue::Dict(dict)
}

/// `json.dumps(value)` (default separators, ASCII).
fn json_text(value: &PyValue) -> String {
    value.dumps_ascii().expect("fixture JSON is plain")
}

fn config_value(quantization: Option<&str>) -> PyValue {
    let mut dict = PyDict::new();
    dict.set("model_type", text("fixture"));
    dict.set("hidden_size", int(16));
    dict.set("num_hidden_layers", int(2));
    dict.set("num_attention_heads", int(2));
    dict.set("torch_dtype", text("bfloat16"));
    if let Some(method) = quantization {
        let mut quant = PyDict::new();
        quant.set("quant_method", text(method));
        dict.set("quantization_config", PyValue::Dict(quant));
    }
    PyValue::Dict(dict)
}

/// A scratch directory of the self-test (`tempfile.TemporaryDirectory(prefix="mxfp4-headers-")`): created exclusively under
/// a new name and the only thing removed afterwards.
struct Scratch {
    _directory: PrivateDirectory,
    root: PathBuf,
}

impl Scratch {
    fn new() -> Scratch {
        let directory =
            PrivateDirectory::create("mxfp4-headers-").expect("a private scratch directory");
        let root = directory.path().to_path_buf();
        Scratch {
            _directory: directory,
            root,
        }
    }

    fn path(&self) -> PyValue {
        PyValue::Str(PyStr::from_surrogateescape(
            std::os::unix::ffi::OsStrExt::as_bytes(self.root.as_os_str()),
        ))
    }

    fn file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// `write(headers, config, mapping)`: clear the directory and write the shards, the index and the config.
    fn write(&self, headers: &Headers, config: &PyValue, mapping: Option<&PyValue>) {
        for entry in std::fs::read_dir(&self.root).expect("read the scratch directory") {
            std::fs::remove_file(entry.expect("entry").path())
                .expect("clear the scratch directory");
        }
        let mut weight_map = PyDict::new();
        for (shard, header) in headers {
            let encoded = json_text(&header_value(header));
            let length = header
                .iter()
                .map(|(_, entry)| entry.offsets[1])
                .max()
                .unwrap_or(0);
            let mut bytes = (encoded.len() as u64).to_le_bytes().to_vec();
            bytes.extend_from_slice(encoded.as_bytes());
            bytes.extend(std::iter::repeat_n(
                0u8,
                usize::try_from(length).expect("a small length"),
            ));
            std::fs::write(self.file(shard), bytes).expect("write a shard");
            for (name, _) in header {
                weight_map.set(name, text(shard));
            }
        }
        let mut index = PyDict::new();
        index.set(
            "weight_map",
            mapping.cloned().unwrap_or(PyValue::Dict(weight_map)),
        );
        std::fs::write(
            self.file("model.safetensors.index.json"),
            json_text(&PyValue::Dict(index)),
        )
        .expect("write the index");
        std::fs::write(self.file("config.json"), json_text(config)).expect("write the config");
    }

    fn write_default(&self) {
        self.write(&base(), &config_value(Some("mxfp4")), None);
    }
}

fn parse_directory(path: &PyValue) -> Res<PyDict> {
    model_config::parse(
        path,
        &text(model_config::DEFAULT_NAS_HOST_ROOT),
        &text("/app/models"),
    )
}

fn never_fetch(_: &PyValue) -> Res<PyValue> {
    Err(Failure::new("AssertionError", "must not estimate"))
}

/// A stand-in for `fetch_hf_safetensors_total` that answers the given dict.
fn answering(fields: Vec<(&str, PyValue)>) -> impl Fn(&PyValue) -> Res<PyValue> {
    let mut dict = PyDict::new();
    for (key, value) in fields {
        dict.set(key, value);
    }
    let answer = PyValue::Dict(dict);
    move |_| Ok(answer.clone())
}

fn crosscheck_with(root: &PyValue, repo: &PyValue, local: &PyValue, fetch: Fetch<'_>) -> PyValue {
    dict_of(model_card::crosscheck(
        root,
        repo,
        local,
        &PyValue::None,
        fetch,
    ))
}

// ── the checks ──────────────────────────────────────────────────────────────────────────────────

fn hybrid_checks(checks: &mut Checks) {
    let layer_types = |count: usize, every: usize| {
        PyValue::List(
            (0..count)
                .map(|i| {
                    text(if (i + 1) % every == 0 {
                        "full_attention"
                    } else {
                        "linear_attention"
                    })
                })
                .collect(),
        )
    };
    let counts_of = |pairs: &[(&str, i64)]| {
        let mut dict = PyDict::new();
        for (name, count) in pairs {
            dict.set(name, int(*count));
        }
        PyValue::Dict(dict)
    };
    let resolve = |types: &PyValue, layers: i64, interval: Option<i64>| {
        resolve_kv_layers(types, &int(layers), &interval.map_or(PyValue::None, int))
            .expect("resolves")
    };
    let has = |warnings: &[PyStr], needle: &str| {
        warnings
            .iter()
            .any(|warning| warning.to_utf8().is_some_and(|text| text.contains(needle)))
    };
    let no_warnings = PyValue::List(Vec::new());
    let value_of = |layers: &model_config::KvLayers| {
        layers.num_full.clone().map_or(PyValue::None, PyValue::Int)
    };

    // A1: 64 layers, 16 full and 48 linear.
    let found = resolve(&layer_types(64, 4), 64, Some(4));
    checks.check("A1.num_full", &value_of(&found), &int(16));
    checks.check(
        "A1.is_hybrid",
        &PyValue::Bool(found.is_hybrid),
        &PyValue::Bool(true),
    );
    let counts = found
        .counts
        .as_ref()
        .map(|counts| {
            let mut dict = PyDict::new();
            for (name, count) in counts {
                dict.insert(name.clone(), int(*count as i64));
            }
            PyValue::Dict(dict)
        })
        .unwrap_or(PyValue::None);
    checks.check(
        "A1.counts",
        &counts,
        &counts_of(&[("linear_attention", 48), ("full_attention", 16)]),
    );
    checks.check(
        "A1.no_warning",
        &PyValue::List(found.warnings.iter().cloned().map(PyValue::Str).collect()),
        &no_warnings,
    );
    // A2: all full attention (not hybrid).
    let all_full = PyValue::List((0..32).map(|_| text("full_attention")).collect());
    let found = resolve(&all_full, 32, None);
    checks.check("A2.num_full", &value_of(&found), &int(32));
    checks.check(
        "A2.is_hybrid",
        &PyValue::Bool(found.is_hybrid),
        &PyValue::Bool(false),
    );
    checks.check(
        "A2.no_warning",
        &PyValue::List(found.warnings.iter().cloned().map(PyValue::Str).collect()),
        &no_warnings,
    );
    // A3: no layer_types (old config).
    let found = resolve(&PyValue::None, 64, None);
    checks.check("A3.num_full", &value_of(&found), &PyValue::None);
    checks.check(
        "A3.is_hybrid",
        &PyValue::Bool(found.is_hybrid),
        &PyValue::Bool(false),
    );
    checks.check(
        "A3.counts",
        &found.counts.map_or(PyValue::None, |_| PyValue::Bool(true)),
        &PyValue::None,
    );
    // A4: a length mismatch is never used silently.
    let ten = PyValue::List((0..10).map(|_| text("full_attention")).collect());
    let found = resolve(&ten, 64, None);
    checks.check("A4.num_full", &value_of(&found), &PyValue::None);
    if !has(&found.warnings, "길이") {
        checks.fail("A4: 길이 불일치 경고 누락 — 침묵 폴백은 금지다");
    }
    // A5: no KV-bearing layer is KV = 0, so all layers with a warning.
    let linear = PyValue::List((0..8).map(|_| text("linear_attention")).collect());
    let found = resolve(&linear, 8, None);
    checks.check("A5.num_full", &value_of(&found), &PyValue::None);
    if !has(&found.warnings, "KV 보유층이 0") {
        checks.fail("A5: KV 보유층 0 경고 누락 — 과소추정이 조용히 통과한다");
    }
    // A6: an unknown type counts as KV-bearing, with a warning.
    let mut mixed: Vec<PyValue> = Vec::new();
    mixed.extend((0..4).map(|_| text("sliding_attention")));
    mixed.extend((0..4).map(|_| text("full_attention")));
    mixed.extend((0..8).map(|_| text("linear_attention")));
    let found = resolve(&PyValue::List(mixed), 16, None);
    checks.check("A6.num_full", &value_of(&found), &int(8));
    checks.check(
        "A6.is_hybrid",
        &PyValue::Bool(found.is_hybrid),
        &PyValue::Bool(true),
    );
    if !has(&found.warnings, "미지 타입") {
        checks.fail("A6: 미지 타입 경고 누락 — 조용한 보수화도 침묵이다");
    }
    // A7: the interval cross-check is a warning, layer_types is the authority.
    let found = resolve(&layer_types(64, 4), 64, Some(8));
    checks.check("A7.num_full", &value_of(&found), &int(16));
    if !has(&found.warnings, "full_attention_interval") {
        checks.fail("A7: interval 교차검증 경고 누락 — tripwire 가 죽었다");
    }
    // A8: the consumer really takes the value (the estimate reads it end to end).
    let parsed = |full: Option<i64>| {
        let mut dict = PyDict::new();
        dict.set("num_hidden_layers", int(64));
        if let Some(full) = full {
            dict.set("num_full_attention_layers", int(full));
        }
        dict.set("num_key_value_heads", int(4));
        dict.set("head_dim", int(256));
        PyValue::Dict(dict)
    };
    let per_token = |parsed: &PyValue| match estimate_vram::per_token_kv_bytes(parsed, &int(2)) {
        Ok(bytes) => PyValue::Int(bytes),
        Err(failure) => PyValue::str(&format!(
            "<{}>",
            failure.line().to_utf8().unwrap_or_default()
        )),
    };
    checks.check(
        "A8.hybrid_per_token",
        &per_token(&parsed(Some(16))),
        &int(65536),
    );
    checks.check(
        "A8.dense_per_token",
        &per_token(&parsed(None)),
        &int(262144),
    );
}

/// `reject(name, headers, alter)`: the directory must be refused with the MXFP4 layout error.
fn reject(
    checks: &mut Checks,
    scratch: &Scratch,
    name: &str,
    headers: &Headers,
    alter: &dyn Fn(&Scratch),
) {
    scratch.write(headers, &config_value(Some("mxfp4")), None);
    alter(scratch);
    match parse_directory(&scratch.path()) {
        Err(failure) if crate::model_dyn::is_value_error(failure.class) => {
            let refused = failure
                .message
                .to_utf8()
                .is_some_and(|message| message.contains("MXFP4_HEADER_LAYOUT_UNSUPPORTED"));
            checks.check(name, &PyValue::Bool(refused), &PyValue::Bool(true));
        }
        Err(failure) => panic!("{name}: {}", failure.line().to_utf8().unwrap_or_default()),
        Ok(_) => checks.check(name, &text("accepted"), &text("rejected")),
    }
}

fn base_shard<'a>(headers: &'a mut Headers, shard: &str) -> &'a mut Header {
    &mut headers
        .iter_mut()
        .find(|(name, _)| name == shard)
        .expect("a shard")
        .1
}

fn entry<'a>(header: &'a mut Header, name: &str) -> &'a mut Meta {
    &mut header
        .iter_mut()
        .find(|(held, _)| held == name)
        .expect("a tensor")
        .1
}

fn replace_in_file(path: &Path, from: &str, to: &str) {
    let content = std::fs::read_to_string(path).expect("read a fixture text file");
    std::fs::write(path, content.replace(from, to)).expect("write a fixture text file");
}

fn mxfp4_checks(checks: &mut Checks) {
    let scratch = Scratch::new();
    let root = scratch.path();
    let parse = |root: &PyValue| dict_of(parse_directory(root));

    scratch.write_default();
    let observed = parse(&root);
    let details = member(&observed, "parameter_count");
    checks.check("M1.logical", &member(&observed, "num_params"), &int(74));
    checks.check("M1.storage", &member(&details, "storage_numel"), &int(44));
    checks.check(
        "M1.scale_metadata",
        &member(&details, "scale_metadata_numel"),
        &int(2),
    );
    checks.check("M1.payload", &member(&details, "payload_bytes"), &int(54));
    checks.check(
        "M1.physical",
        &member(&observed, "native_weight_bytes"),
        &member(&details, "file_bytes"),
    );
    let doubled = match member(&details, "file_bytes") {
        PyValue::Int(bytes) => bytes * 2,
        other => panic!("{}", other.repr()),
    };
    checks.check(
        "M1.headers_not_params",
        &PyValue::Bool(member(&observed, "num_params") != PyValue::Int(doubled)),
        &PyValue::Bool(true),
    );
    std::fs::write(
        scratch.file("unreferenced.safetensors"),
        b"not the canonical checkpoint",
    )
    .expect("write");
    checks.check(
        "M2.index_canonical",
        &member(&parse(&root), "parameter_count"),
        &details,
    );

    let mut dotted = base();
    for (_, header) in dotted.iter_mut() {
        for (name, _) in header.iter_mut() {
            *name = name
                .replace("_blocks", ".blocks")
                .replace("_scales", ".scales");
        }
    }
    scratch.write(&dotted, &config_value(Some("mxfp4")), None);
    checks.check(
        "M3.dot_loader_naming",
        &member(&parse(&root), "num_params"),
        &int(74),
    );
    std::fs::remove_file(scratch.file("model.safetensors.index.json")).expect("remove the index");
    checks.check(
        "M3.no_index_all_shards",
        &member(&parse(&root), "num_params"),
        &int(74),
    );

    let nothing = |_: &Scratch| {};
    let mut bad = base();
    *base_shard(&mut bad, "b.safetensors") =
        vec![("opaque_u8".to_string(), meta("U8", &[2], [0, 2]))];
    reject(
        checks,
        &scratch,
        "M4.unknown_u8_missing_pair",
        &bad,
        &nothing,
    );
    let mut bad = base();
    base_shard(&mut bad, "b.safetensors")
        .push(("orphan_scales".to_string(), meta("U8", &[1], [2, 3])));
    reject(checks, &scratch, "M5.orphan_scale", &bad, &nothing);
    let mut bad = base();
    entry(base_shard(&mut bad, "b.safetensors"), SCALES).shape = vec![1, 2];
    reject(checks, &scratch, "M6.pair_shape", &bad, &nothing);
    let mut bad = base();
    *entry(base_shard(&mut bad, "b.safetensors"), SCALES) = meta("F16", &[2], [0, 4]);
    reject(checks, &scratch, "M7.scale_dtype", &bad, &nothing);
    let mut bad = base();
    entry(base_shard(&mut bad, "a.safetensors"), BLOCKS).shape = vec![4, 8];
    *entry(base_shard(&mut bad, "b.safetensors"), SCALES) = meta("U8", &[4], [0, 4]);
    reject(checks, &scratch, "M8.block_width", &bad, &nothing);
    let mut bad = base();
    entry(base_shard(&mut bad, "a.safetensors"), BLOCKS).offsets = [20, 51];
    reject(checks, &scratch, "M9.payload_length", &bad, &nothing);
    let mut bad = base();
    entry(base_shard(&mut bad, "a.safetensors"), BLOCKS).offsets = [19, 51];
    reject(checks, &scratch, "M10.overlap", &bad, &nothing);
    reject(checks, &scratch, "M11.truncated", &base(), &|scratch| {
        let path = scratch.file("a.safetensors");
        let mut bytes = std::fs::read(&path).expect("read");
        bytes.pop();
        std::fs::write(&path, bytes).expect("write");
    });
    reject(checks, &scratch, "M12.missing_shard", &base(), &|scratch| {
        std::fs::remove_file(scratch.file("b.safetensors")).expect("remove");
    });
    reject(
        checks,
        &scratch,
        "M13.all_shards_absent",
        &base(),
        &|scratch| {
            for shard in ["a.safetensors", "b.safetensors"] {
                std::fs::remove_file(scratch.file(shard)).expect("remove");
            }
        },
    );
    reject(
        checks,
        &scratch,
        "M14.malformed_index",
        &base(),
        &|scratch| {
            std::fs::write(scratch.file("model.safetensors.index.json"), "{}").expect("write");
        },
    );
    let mapping_of = |extra: (&str, &str)| {
        let mut map = PyDict::new();
        for (shard, header) in base() {
            for (name, _) in header {
                map.set(&name, text(&shard));
            }
        }
        map.set(extra.0, text(extra.1));
        let mut index = PyDict::new();
        index.set("weight_map", PyValue::Dict(map));
        json_text(&PyValue::Dict(index))
    };
    let wrong_map = mapping_of((SCALES, "a.safetensors"));
    reject(
        checks,
        &scratch,
        "M15.wrong_index_map",
        &base(),
        &|scratch| {
            std::fs::write(scratch.file("model.safetensors.index.json"), &wrong_map)
                .expect("write");
        },
    );
    let missing_map = mapping_of(("missing.weight", "a.safetensors"));
    reject(
        checks,
        &scratch,
        "M16.missing_index_tensor",
        &base(),
        &|scratch| {
            std::fs::write(scratch.file("model.safetensors.index.json"), &missing_map)
                .expect("write");
        },
    );

    // M17-M20: the managed cross-check with a stand-in for the external total.
    scratch.write_default();
    let repo = text("fixture/model");
    let total_74 = answering(vec![
        ("ok", PyValue::Bool(true)),
        ("total", int(74)),
        ("parameters", PyValue::Dict(PyDict::new())),
    ]);
    let result = crosscheck_with(&root, &repo, &int(74), &total_74);
    checks.check(
        "M17.valid_crosscheck",
        &member(&result, "verdict"),
        &text("OK"),
    );
    checks.check(
        "M17.mixed_evidence",
        &member(&member(&result, "dtype_scan"), "is_mixed_precision"),
        &PyValue::Bool(true),
    );
    checks.check(
        "M17.expert_evidence",
        &member(&member(&result, "dtype_scan"), "fourbit_experts"),
        &PyValue::Bool(true),
    );
    checks.check(
        "M18.real_external_mismatch",
        &member(
            &crosscheck_with(&root, &repo, &int(100), &total_74),
            "verdict",
        ),
        &text("MISMATCH"),
    );
    let external = |local: PyValue, fetch: Fetch<'_>| {
        dict_of(model_card::crosscheck_external_vram(
            &local,
            &repo,
            &PyValue::Float(0.05),
            fetch,
        ))
    };
    checks.check(
        "M18.tolerance_unchanged",
        &member(&external(int(77), &total_74), "verdict"),
        &text("OK"),
    );
    checks.check(
        "M19.ephemeral_unchanged",
        &member(&external(PyValue::None, &total_74), "mode"),
        &text("ephemeral-estimate"),
    );
    let unavailable = answering(vec![
        ("ok", PyValue::Bool(false)),
        ("reason", text("unavailable")),
    ]);
    checks.check(
        "M20.optional_unavailable",
        &member(&external(int(74), &unavailable), "verdict"),
        &text("UNAVAILABLE"),
    );

    // M21: a failed local parse is kept, and no estimate is made in its place.
    let failing_parse = |_: &PyValue| -> Res<PyDict> {
        Err(Failure::new("ValueError", "local observation failed"))
    };
    let dependencies = Dependencies {
        parse: &failing_parse,
        fetch: &never_fetch,
    };
    let arguments = [
        root.display_text(),
        PyStr::new("--hf-repo-id"),
        PyStr::new("fixture/model"),
        PyStr::new("--json"),
    ];
    let run = crosscheck_main(&arguments, Some("80"), &dependencies);
    checks.check(
        "M21.managed_parse_failure_exit",
        &int(i64::from(run.status)),
        &int(2),
    );
    let last_reason = loads_text(&run.stdout)
        .ok()
        .and_then(|report| match member(&report, "checks") {
            PyValue::List(items) => items.last().map(|last| member(last, "reason")),
            _ => None,
        })
        .unwrap_or(PyValue::None);
    checks.check(
        "M21.managed_failure_preserved",
        &last_reason,
        &text("local_parse_failed"),
    );

    std::fs::remove_file(scratch.file("b.safetensors")).expect("remove");
    checks.check(
        "M22.layout_gate_without_external",
        &member(
            &crosscheck_with(&root, &PyValue::None, &PyValue::None, &never_fetch),
            "verdict",
        ),
        &text("MISMATCH"),
    );
    let run: Run = parse_main(&[root.display_text(), PyStr::new("--json")], Some("80"));
    checks.check(
        "M22.parse_cli_failure",
        &int(i64::from(run.status)),
        &int(2),
    );

    // M23-M25: other formats and an undeclared U8 are never taken for MXFP4; a dense model keeps its count.
    let undeclared = config_value(None);
    scratch.write(&base(), &undeclared, None);
    checks.check(
        "M23.undeclared_u8",
        &member(
            &member(
                &dict_of(model_card::crosscheck(
                    &root,
                    &PyValue::None,
                    &PyValue::None,
                    &PyValue::None,
                    &never_fetch,
                )),
                "dtype_scan",
            ),
            "fourbit_experts",
        ),
        &PyValue::Bool(false),
    );
    let high_mixed: Headers = vec![(
        "a.safetensors".to_string(),
        vec![
            ("experts.packed".to_string(), meta("U8", &[16], [0, 16])),
            ("experts.scale".to_string(), meta("F8_E8M0", &[1], [16, 17])),
            ("norm".to_string(), meta("BF16", &[1], [17, 19])),
        ],
    )];
    scratch.write(&high_mixed, &config_value(Some("fp8")), None);
    checks.check(
        "M24.other_coarse_gate",
        &member(
            &dict_of(model_card::crosscheck(
                &root,
                &PyValue::None,
                &PyValue::None,
                &PyValue::None,
                &never_fetch,
            )),
            "verdict",
        ),
        &text("MISMATCH"),
    );
    let dense: Headers = vec![(
        "a.safetensors".to_string(),
        vec![("weight".to_string(), meta("BF16", &[10], [0, 20]))],
    )];
    scratch.write(&dense, &undeclared, None);
    checks.check(
        "M25.dense_count_unchanged",
        &member(&parse(&root), "num_params"),
        &int(10),
    );
    let has_count = match parse(&root) {
        PyValue::Dict(map) => map.get("parameter_count").is_some(),
        _ => true,
    };
    checks.check(
        "M25.dense_fields_unchanged",
        &PyValue::Bool(has_count),
        &PyValue::Bool(false),
    );

    // M26: the header read is bounded; the legacy reader has the same bound.
    reject(
        checks,
        &scratch,
        "M26.header_read_bound",
        &base(),
        &|scratch| {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(scratch.file("a.safetensors"))
                .expect("open for change");
            file.seek(SeekFrom::Start(0)).expect("seek");
            file.write_all(&(SAFETENSORS_MAX_HEADER + 1).to_le_bytes())
                .expect("write");
            file.set_len(SAFETENSORS_MAX_HEADER + 9)
                .expect("sparse extend");
        },
    );
    let legacy = model_config::read_safetensors_header(&PyValue::Str(PyStr::new(
        scratch.file("a.safetensors").to_str().expect("a text path"),
    )));
    checks.check(
        "M26.legacy_reader_bound",
        &match legacy {
            Ok(None) => PyValue::None,
            _ => text("read"),
        },
        &PyValue::None,
    );

    // M27: a repeated key in a header, M28: in the index.
    reject(
        checks,
        &scratch,
        "M27.duplicate_header_key",
        &base(),
        &|scratch| {
            let path = scratch.file("a.safetensors");
            let data = std::fs::read(&path).expect("read");
            let n = u64::from_le_bytes(data[..8].try_into().expect("8 bytes")) as usize;
            let header = String::from_utf8(data[8..8 + n].to_vec())
                .expect("a text header")
                .replace(
                    "\"dtype\": \"BF16\"",
                    "\"dtype\": \"U8\", \"dtype\": \"BF16\"",
                );
            let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
            bytes.extend_from_slice(header.as_bytes());
            bytes.extend_from_slice(&data[8 + n..]);
            std::fs::write(&path, bytes).expect("write");
        },
    );
    reject(
        checks,
        &scratch,
        "M28.duplicate_index_key",
        &base(),
        &|scratch| {
            replace_in_file(
                &scratch.file("model.safetensors.index.json"),
                "\"norm.weight\": \"a.safetensors\"",
                "\"norm.weight\": \"b.safetensors\", \"norm.weight\": \"a.safetensors\"",
            );
        },
    );
    let mut bad = base();
    *entry(base_shard(&mut bad, "a.safetensors"), BLOCKS) = meta("U8", &[16], [20, 36]);
    *entry(base_shard(&mut bad, "b.safetensors"), SCALES) = meta("U8", &[], [0, 1]);
    reject(
        checks,
        &scratch,
        "M29.loader_requires_group_axis",
        &bad,
        &nothing,
    );
}

/// `_self_test()`: every check, then the report.
pub fn parse_self_test() -> Run {
    let mut checks = Checks {
        failures: Vec::new(),
    };
    hybrid_checks(&mut checks);
    mxfp4_checks(&mut checks);
    let mut streams = Streams::default();
    if checks.failures.is_empty() {
        streams.write(&PyStr::new(
            "[parse_model_config --self-test] OK — A1~A8 + M1~M29 통과 (하이브리드 KV · MXFP4 헤더/관리형 교차검증)\n",
        ));
        return streams.finish(0);
    }
    let mut report = format!(
        "[parse_model_config --self-test] FAIL {} 건:\n",
        checks.failures.len()
    );
    for failure in &checks.failures {
        report.push_str(&format!("  - {failure}\n"));
    }
    streams.stderr.push_str(&report);
    streams.finish(1)
}
