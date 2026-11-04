//! `roofline self-test`: native checks of the roofline computation for a freshly built node.
//!
//! It needs neither Python nor a model nor a GPU: every scenario builds its own header-only safetensors fixtures in a
//! private temporary directory (removed afterwards) and calls the same functions the command line uses. The first
//! scenario is the original `selftest_roofline.py` (sparse lookup tables - PLE and Engram - never change the dense or
//! routed traffic, with the exact figures of that test); the others cover the branches whose wrong results would move
//! a verdict: the `accept_len` provenance contract, the three sources of the tensor-parallel size, the bandwidth
//! lookup, the published-size cross-check and the communication-bound regime. The transcript names itself a
//! self-test and prints no result object, so it cannot be mistaken for evidence about a served model.

use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::roofline::{
    active_bytes_moe, resolve_accept_len, run, RooflineOptions, Stop, ACCEPT_LEN_ABSENT,
    ACCEPT_LEN_DECLARED,
};
use crate::selftest::SelfTestReport;
use num_bigint::BigInt;
use std::path::{Path, PathBuf};

const TAG: &str = "[roofline self-test]";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("roofline_selftest_{}_{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("private temporary directory");
        Scratch(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).expect("fixture directory");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_shard(dir: &Path, name: &str, tensors: &[(&str, u64)]) {
    let mut offset = 0;
    let entries: Vec<String> = tensors
        .iter()
        .map(|(key, size)| {
            let entry = format!("\"{key}\": {{\"dtype\": \"U8\", \"shape\": [{size}], \"data_offsets\": [{offset}, {}]}}", offset + size);
            offset += size;
            entry
        })
        .collect();
    let header = format!("{{{}}}", entries.join(", "));
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header.as_bytes());
    std::fs::write(dir.join(name), bytes).expect("shard file");
}

/// The tensors of `selftest_roofline.py`, in its order.
const SPARSE_TENSORS: [(&str, u64); 10] = [
    ("model.layers.0.self_attn.weight", 1000),
    ("model.layers.0.mlp.shared_experts.weight", 400),
    ("model.embed.weight", 200),
    ("model.layers.0.engram.embed_aux.weight", 50),
    ("model.layers.0.mlp.experts.0.weight", 1600),
    ("model.layers.0.mlp.experts.0.scale", 400),
    (
        "model.layers.0.ple.ple_embedding.ngram_embedding.shard_0.weight",
        4096,
    ),
    ("model.layers.1.ngram_embedding.weight", 1024),
    ("model.mtp.layers.0.weight", 3000),
    ("model.mtp.layers.0.engram.embed.weight", 50000),
];

/// Two shards with the tensors split by index parity, an index naming them, optionally with Engram tables added.
fn sparse_model(scratch: &Scratch, name: &str, with_engram: bool) -> PathBuf {
    let dir = scratch.dir(name);
    let mut tensors: Vec<(&str, u64)> = SPARSE_TENSORS.to_vec();
    if with_engram {
        tensors.push(("model.layers.1.engram.embed.weight", 100_000));
        tensors.push(("model.layers.1.engram.embed.scale", 20_000));
    }
    let mut weight_map = Vec::new();
    for shard in 0..2 {
        let file = format!("part-{shard}.safetensors");
        let chosen: Vec<(&str, u64)> = tensors
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, _)| i % 2 == shard)
            .map(|(_, t)| t)
            .collect();
        write_shard(&dir, &file, &chosen);
        weight_map.extend(
            chosen
                .iter()
                .map(|(key, _)| format!("\"{key}\": \"{file}\"")),
        );
    }
    std::fs::write(
        dir.join("model.safetensors.index.json"),
        format!("{{\"weight_map\": {{{}}}}}", weight_map.join(", ")),
    )
    .expect("index");
    dir
}

fn path_text(path: &Path) -> PyStr {
    PyStr::new(path.to_str().expect("UTF-8 scratch path"))
}

fn kinds_text(kinds: &[(&'static str, BigInt)]) -> String {
    kinds
        .iter()
        .map(|(kind, bytes)| format!("{kind}={bytes}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn number(result: &PyDict, key: &str) -> Option<f64> {
    match result.get(key)? {
        PyValue::Float(value) => Some(*value),
        PyValue::Int(value) => value.to_string().parse().ok(),
        _ => None,
    }
}

fn text(result: &PyDict, key: &str) -> String {
    result
        .get(key)
        .map(|v| v.display_text().to_utf8().unwrap_or_default())
        .unwrap_or_default()
}

/// Dense model of 2.73e9 bytes declared by one header: at 273 GB/s that is 10 ms per pass, R_fp = 100.
fn dense_model(scratch: &Scratch, name: &str, index_total: Option<u64>) -> PathBuf {
    let dir = scratch.dir(name);
    std::fs::write(
        dir.join("config.json"),
        "{\"hidden_size\": 64, \"num_hidden_layers\": 2}",
    )
    .expect("config");
    write_shard(&dir, "model.safetensors", &[("w", 2_730_000_000)]);
    if let Some(total) = index_total {
        std::fs::write(
            dir.join("model.safetensors.index.json"),
            format!("{{\"metadata\": {{\"total_size\": {total}}}, \"weight_map\": {{\"w\": \"model.safetensors\"}}}}"),
        )
        .expect("index");
    }
    dir
}

fn options(dir: &Path) -> RooflineOptions {
    let mut options = RooflineOptions::new(path_text(dir));
    options.bandwidth_gbps = Some(273.0);
    options
}

/// Runs every scenario and returns the transcript.
pub fn run_self_test() -> SelfTestReport {
    let scratch = Scratch::new();
    let mut lines = String::new();
    let mut failures: Vec<String> = Vec::new();
    let mut check = |name: &str, condition: bool| {
        lines.push_str(&format!(
            "{TAG} {} {name}\n",
            if condition { "PASS" } else { "FAIL" }
        ));
        if !condition {
            failures.push(name.to_string());
        }
    };

    // R1: the original selftest_roofline.py, with its exact figures.
    let before_dir = sparse_model(&scratch, "sparse-before", false);
    let after_dir = sparse_model(&scratch, "sparse-after", true);
    let moe = |dir: &Path| active_bytes_moe(&path_text(dir), &PyValue::int(8), &PyValue::int(2));
    let (before, after) = (moe(&before_dir), moe(&after_dir));
    let figures = |m: &Result<crate::roofline::MoeBytes, Stop>| {
        m.as_ref().ok().map(|m| {
            (
                m.active.to_string(),
                m.dense.to_string(),
                m.routed.to_string(),
                m.experts_per_token.to_string(),
                m.experts.to_string(),
                m.lookup.to_string(),
                kinds_text(&m.lookup_by_kind),
            )
        })
    };
    let want_before = ("2150", "1650", "2000", "2", "8", "5120", "ple=5120");
    let want_after = (
        "2150",
        "1650",
        "2000",
        "2",
        "8",
        "125120",
        "ple=5120,engram=120000",
    );
    let matches = |got: Option<(String, String, String, String, String, String, String)>,
                   want: (&str, &str, &str, &str, &str, &str, &str)| {
        got.is_some_and(|g| {
            (
                g.0.as_str(),
                g.1.as_str(),
                g.2.as_str(),
                g.3.as_str(),
                g.4.as_str(),
                g.5.as_str(),
                g.6.as_str(),
            ) == want
        })
    };
    check("R1 희소 룩업(PLE·Engram)은 dense·routed 트래픽을 바꾸지 않는다 — active 2150 · dense 1650 · routed 2000 · ple 5120 · engram +120000", matches(figures(&before), want_before) && matches(figures(&after), want_after));

    // R2: dense = header sum; a published total that is off is announced, not adopted.
    let dir = dense_model(&scratch, "dense-index", Some(5_460_000_000));
    let run_dense = run(&options(&dir));
    let ok = run_dense.result.as_ref().ok();
    check(
        "R2 dense: active=헤더 합 · R_fp 100 · index 발행 크기가 2배면 stderr 로 알린다",
        ok.is_some_and(|r| {
            number(r, "R_fp") == Some(100.0) && number(r, "active_bytes") == Some(2.73e9)
        }) && run_dense.stderr.contains("발행자 오기재")
            && ok.is_some_and(|r| text(r, "tp_source").starts_with("fallback(manifest")),
    );
    let plain = run(&options(&dense_model(&scratch, "dense-plain", None)));
    check(
        "R3 index 가 없으면 경고 없이 같은 R_fp · expected = 0.35 × accept(1.0) × R_fp",
        plain.stderr.is_empty()
            && plain.result.as_ref().ok().is_some_and(|r| {
                number(r, "expected_achievable") == Some(35.0)
                    && text(r, "accept_len_source") == ACCEPT_LEN_ABSENT
            }),
    );

    // R4: the accept_len provenance contract.
    let declared = resolve_accept_len(Some(2.0), None).ok();
    let contradiction = matches!(
        resolve_accept_len(Some(2.0), Some("absent")),
        Err(Stop::Die(_))
    );
    let missing_value = matches!(
        resolve_accept_len(None, Some("measured")),
        Err(Stop::Die(_))
    );
    let invalid = matches!(resolve_accept_len(Some(0.5), None), Err(Stop::Die(_)))
        && matches!(resolve_accept_len(Some(f64::NAN), None), Err(Stop::Die(_)));
    check(
        "R4 accept_len 조합 계약: 값만=declared · 값∧absent 모순 · 출처 measured 인데 값 없음 · <1/NaN 거부",
        declared == Some((2.0, ACCEPT_LEN_DECLARED.to_string())) && contradiction && missing_value && invalid && resolve_accept_len(None, None).ok() == Some((1.0, ACCEPT_LEN_ABSENT.to_string())),
    );
    let mut with_accept = options(&dense_model(&scratch, "dense-accept", None));
    with_accept.accept_len = Some(2.0);
    with_accept.accept_len_source = Some("measured".into());
    let accepted = run(&with_accept);
    check(
        "R5 measured accept 2.0 은 R_token 과 expected 양쪽에 곱해진다 — R_token 200 · expected 70",
        accepted.result.as_ref().ok().is_some_and(|r| {
            number(r, "R_token") == Some(200.0)
                && number(r, "expected_achievable") == Some(70.0)
                && text(r, "accept_len_source") == "measured"
        }),
    );

    // R6: tensor-parallel size and its provenance.
    let manifest_dir = scratch.dir("manifests");
    let write_manifest = |name: &str, body: &str| {
        let path = manifest_dir.join(name);
        std::fs::write(&path, body).expect("manifest");
        path_text(&path)
    };
    let model = dense_model(&scratch, "dense-tp", None);
    let tp_of = |manifest: Option<PyStr>, tp: Option<i64>| {
        let mut o = options(&model);
        o.manifest = manifest;
        o.tp = tp.map(BigInt::from);
        o.interconnect_gbps = Some(200.0);
        run(&o).result.ok().map(|r| {
            (
                r.get("tp").map(PyValue::repr).unwrap_or_default(),
                text(&r, "tp_source"),
            )
        })
    };
    let single = tp_of(Some(write_manifest("single.yaml", "topology: single\ngpus_per_node: 1\ngpu_model: NVIDIA GB10\nnodes:\n  - role: main\n  - role: sub\n")), None);
    let multi = tp_of(
        Some(write_manifest(
            "multi.yaml",
            "topology: multi\ngpus_per_node: 2\ngpu_model: NVIDIA GB10\nnodes: [a, b]\n",
        )),
        None,
    );
    let explicit = tp_of(None, Some(4));
    let neither = tp_of(None, None);
    check(
        "R6 TP 출처: single=gpus_per_node(서브는 텐서 워커가 아니다) · multi=nodes×gpus · 명시 · 폴백은 출처를 밝힌다",
        single == Some(("1".into(), "manifest(single: gpus_per_node)".into()))
            && multi == Some(("4".into(), "manifest(nodes×gpus_per_node)".into()))
            && explicit == Some(("4".into(), "explicit(--tp)".into()))
            && neither.is_some_and(|(tp, source)| tp == "1" && source.starts_with("fallback(manifest 부재")),
    );

    // R7: the bandwidth lookup, and no guess for an unknown GPU.
    let mut unknown = options(&model);
    unknown.bandwidth_gbps = None;
    unknown.manifest = Some(write_manifest(
        "unknown-gpu.yaml",
        "topology: single\ngpus_per_node: 1\ngpu_model: Mystery 9000\n",
    ));
    let mut known = options(&model);
    known.bandwidth_gbps = None;
    known.manifest = Some(write_manifest(
        "gb10.yaml",
        "topology: single\ngpus_per_node: 1\ngpu_model: NVIDIA GB10\n",
    ));
    check(
        "R7 대역폭: GB10 은 273 GB/s(lookup) · 미지 GPU 는 추측하지 않고 exit 2 계열로 멈춘다",
        matches!(run(&unknown).result, Err(Stop::Die(_)))
            && run(&known).result.ok().is_some_and(|r| {
                number(&r, "bandwidth_gbps") == Some(273.0)
                    && text(&r, "bandwidth_source") == "lookup:gb10"
            }),
    );

    // R8: communication-bound regime on a slow link.
    let mut slow = options(&model);
    slow.tp = Some(BigInt::from(2));
    slow.interconnect_gbps = Some(0.0001);
    let comm = run(&slow).result.ok();
    check(
        "R8 느린 링크에서 comm_bound=true 이고 t_comm 이 계산된다",
        comm.is_some_and(|r| {
            matches!(r.get("comm_bound"), Some(PyValue::Bool(true)))
                && number(&r, "t_comm_ms").is_some_and(|t| t > 0.0)
        }),
    );

    // R9: a missing model is refused, never guessed.
    let missing = RooflineOptions::new(path_text(&scratch.0.join("no-such-model")));
    check(
        "R9 없는 모델 디렉토리는 다운로드·추측 없이 멈춘다",
        matches!(run(&missing).result, Err(Stop::Die(_))),
    );

    if failures.is_empty() {
        let count = lines.lines().count();
        lines.push_str(&format!("{TAG} OK — {count} 시나리오 통과 (헤더만 있는 합성 fixture 점검일 뿐 · 서빙 측정 증거가 아니다)\n"));
        return SelfTestReport {
            stdout: lines,
            stderr: String::new(),
            exit_code: 0,
        };
    }
    let names: Vec<String> = failures
        .iter()
        .map(|name| PyStr::new(name).repr())
        .collect();
    SelfTestReport {
        stdout: lines,
        stderr: format!("{TAG} FAIL {} 건: [{}]\n", failures.len(), names.join(", ")),
        exit_code: 1,
    }
}
