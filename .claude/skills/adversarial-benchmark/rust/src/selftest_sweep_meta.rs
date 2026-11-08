//! `selftest_sweep_meta`: the deterministic regression check of the `meta` that `sweep_bench.sh` assembles. A Rust
//! port of `selftest_sweep_meta.py`.
//!
//! History kept as the reason: the 2026-08-23 R7 report was published with `quantization: N/A` because both values
//! were read from the config yaml only while the engine had reported `fp8`; the speculative-decoding declaration
//! fingerprint (`spec_declared*`) is checked the same way, because the judge tells a missing accept length from a
//! normal 1.0 only through it.
//!
//! **This check does not copy the parser.** The producer is the deployed one: `sweep_bench.sh` calls the launcher
//! `assemble_sweep_meta.sh`, which starts the installed Rust program, and every fixture below is one real run of that
//! chain ([`crate::sweep_meta_deployed`]). The shell function `_spec_axis_args` is cut out of the script byte for
//! byte and run under `set -euo pipefail`. A producer that is not installed, that `sweep_bench.sh` no longer calls, or
//! that does not start ends the check before any scenario; nothing here falls back to another implementation.
//!
//! [`SweepMeta::build_meta`] is also what the lite report check (`selftest_lite_report`, L8) uses.

use crate::bench_verify_support::{py_str, py_strip, st};
use crate::captured_process::{run_captured_in, ChildSettings};
use crate::conformance::{py_equals, splitlines, Raised};
use crate::dataset_prep::Console;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::sweep_meta_deployed::{DeployedProducer, ProducerRun};
use crate::verify_fixture::{
    dumps, get, is_file, load_json, read_text, repo_root_unknown_message, write_str, write_text,
    TemporaryDirectory,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_sweep_meta";

/// The script directory of the benchmark skill, relative to a repository root.
const SCRIPTS: &str = crate::verify_fixture::BENCHMARK_SCRIPTS;
const SWEEP_SCRIPT: &str = "sweep_bench.sh";
const AXIS_OPEN: &str = "# >>> spec-axis-args";
const AXIS_CLOSE: &str = "# <<< spec-axis-args\n";
/// The wall-clock limit of the shell function call (`timeout=60`).
const AXIS_SECONDS: u64 = 60;

/// The engine configuration line of a real campaign log (2026-08-23 R7, excerpted). Two traps are in it:
/// `quantization_config=None` follows `quantization=fp8` closely, and other lines of the same log carry a runner
/// echo and a FlashInfer value that a global search would pick.
fn engine_line(quant: &str, kv_cache_dtype: &str) -> String {
    format!(
        "INFO 08-23 12:20:10 [core.py:99] Initializing a V1 LLM engine (v0.27.1.dev0+g4bdc8a788.\
d20260814) with config: model='/app/models/Qwen/Qwen3.8/Qwen3.8-27B', \
speculative_config=SpeculativeConfig(method='mtp', num_spec_tokens=3), \
dtype=torch.bfloat16, max_seq_len=1000000, tensor_parallel_size=1, \
quantization={quant}, quantization_config=None, enforce_eager=False, \
kv_cache_dtype={kv_cache_dtype}, device_config=cuda, seed=None\n"
    )
}

/// The engine line of the fixtures as the lite report check writes it (`ENGINE_LINE.format(quant=, kvdt=)`).
pub fn engine_log_line(quant: &str, kv_cache_dtype: &str) -> String {
    engine_line(quant, kv_cache_dtype)
}

const DECOY_LINES: &str = "[runner] KV_CACHE_DTYPE=DECOY_A\n\
[runner] QUANTIZATION=DECOY_B\n\
INFO 08-23 12:20:30 [flashinfer.py:1] FlashInfer resolved backend, \
kv_cache_dtype=torch.float8_e4m3fn, arch=sm121\n";

fn system_exit(message: &str) -> Raised {
    Raised::new("SystemExit", message)
}

/// `re.search(r"# >>> spec-axis-args[^\n]*\n(.*?)# <<< spec-axis-args\n", text, re.S)` with the group-contains check.
fn spec_axis_function(script: &str) -> Option<&str> {
    let mut from = 0;
    while let Some(found) = script[from..].find(AXIS_OPEN) {
        let after_marker = from + found + AXIS_OPEN.len();
        if let Some(line_end) = script[after_marker..].find('\n') {
            let body_start = after_marker + line_end + 1;
            if let Some(length) = script[body_start..].find(AXIS_CLOSE) {
                let body = &script[body_start..body_start + length];
                return body.contains("_spec_axis_args()").then_some(body);
            }
        }
        from += found + 1;
    }
    None
}

/// The deployed script and the producer installed beside it: what the original module held in `SWEEP` and `SRC`.
pub struct SweepMeta {
    script_text: String,
    producer: DeployedProducer,
}

impl SweepMeta {
    /// Reads `sweep_bench.sh` and binds to the installed producer. A script that does not call it, a hidden old
    /// program, a missing launcher or a program that does not start is a stop before any check (`SystemExit`), never a
    /// pass: a check whose subject has gone must not read as green.
    pub fn load(scripts_dir: &Path) -> Result<SweepMeta, Raised> {
        let producer = DeployedProducer::bind(scripts_dir, PROGRAM)?;
        let script_text = read_text(&scripts_dir.join(SWEEP_SCRIPT))?;
        Ok(SweepMeta {
            script_text,
            producer,
        })
    }

    /// The deployed script text (`SWEEP.read_text`), for the cross-checks that read the script itself.
    pub fn script_text(&self) -> &str {
        &self.script_text
    }
}

/// The keyword arguments of the original `build_meta` (a missing one is the original's default).
#[derive(Default, Clone)]
pub struct BuildMeta<'a> {
    /// `quantization:` line of the config yaml; none = no such line.
    pub yaml_quant: Option<&'a str>,
    pub yaml_kvdt: Option<&'a str>,
    /// Engine self-report `(quantization, kv_cache_dtype)`; none = no engine log content.
    pub engine: Option<(&'a str, &'a str)>,
    /// `with_decoys=False` turns the decoy lines off (the original default is on).
    pub without_decoys: bool,
    pub engine_log_text: Option<String>,
    pub cfg_extra: Option<&'a str>,
    pub sh_text: Option<&'a str>,
    pub no_cfg: bool,
    pub cfg_text: Option<&'a str>,
    /// Existing `sweep_index.json` meta: the `--reassemble-only` path.
    pub prior_meta: Option<PyValue>,
}

impl SweepMeta {
    /// Lays out one fixture and runs the **deployed producer** on it; returns the `meta` it wrote. What the producer
    /// prints reaches the verifier's streams as it does for any owner.
    pub fn build_meta(
        &self,
        console: &mut dyn Console,
        arguments: &BuildMeta<'_>,
    ) -> Result<PyValue, Raised> {
        let directory = TemporaryDirectory::create("tmp")?;
        let outcome = self.run_fixture(console, arguments, directory.path());
        directory.close()?;
        outcome
    }

    fn run_fixture(
        &self,
        console: &mut dyn Console,
        arguments: &BuildMeta<'_>,
        root: &Path,
    ) -> Result<PyValue, Raised> {
        let sweep_dir = root.join("sweep_x");
        std::fs::create_dir_all(sweep_dir.join("level_01")).map_err(|error| {
            Raised::os_error(&error, &crate::verify_fixture::path_text(&sweep_dir))
        })?;

        let mut cfg_lines: Vec<String> = vec![
            "model: /models/acme/Acme-7B".into(),
            "max-model-len: 262144".into(),
            "gpu-memory-utilization: 0.9".into(),
            "# 레시피 주석: quant=DECOY_C kv-cache-dtype: DECOY_D".into(),
        ];
        if let Some(quant) = arguments.yaml_quant {
            cfg_lines.push(format!("quantization: {quant}"));
        }
        if let Some(kvdt) = arguments.yaml_kvdt {
            cfg_lines.push(format!("kv-cache-dtype: {kvdt}"));
        }
        if let Some(extra) = arguments.cfg_extra {
            cfg_lines.push(extra.to_string());
        }
        if let Some(cfg_text) = arguments.cfg_text {
            write_str(&root.join("cfg.yaml"), cfg_text)?;
        } else if !arguments.no_cfg {
            write_str(
                &root.join("cfg.yaml"),
                &format!("{}\n", cfg_lines.join("\n")),
            )?;
        }
        if let Some(sh_text) = arguments.sh_text {
            // The runner is `<cfg>.sh` beside the config yaml (the embedded program's `_shtext` convention).
            write_str(&root.join("x.sh"), sh_text)?;
        }
        write_str(
            &root.join(".env.x"),
            "IMAGE_TAG=easy-vllm:9.9.9-test\nSERVING_MODEL_NAME=Acme-7B\n\
QUANTIZATION=DECOY_E\nKV_CACHE_DTYPE=DECOY_F\n",
        )?;
        write_str(
            &root.join("manifest.yaml"),
            "gpu_model: NVIDIA GB10\ngpus_per_node: 1\n\
topology: single\ndriver_version: 1.2.3\n\
cuda_version: 132\n",
        )?;

        let log = if let Some(whole) = &arguments.engine_log_text {
            whole.clone()
        } else if let Some((quant, kvdt)) = arguments.engine {
            let line = engine_line(quant, kvdt);
            if arguments.without_decoys {
                line
            } else {
                format!("{DECOY_LINES}{line}{DECOY_LINES}")
            }
        } else if arguments.without_decoys {
            String::new()
        } else {
            DECOY_LINES.to_string()
        };
        write_str(&sweep_dir.join("level_01").join("engine_x.log"), &log)?;
        write_str(
            &sweep_dir.join("level_01").join("measured.json"),
            &json_text(&object(vec![("measurement_ok", PyValue::Bool(true))]))?,
        )?;
        write_str(&sweep_dir.join("truncation.log"), "")?;
        if let Some(prior) = &arguments.prior_meta {
            write_str(
                &sweep_dir.join("sweep_index.json"),
                &json_text(&object(vec![
                    ("generated_utc", st("2026-01-01T00:00:00Z")),
                    ("meta", prior.clone()),
                ]))?,
            )?;
        }

        // The measurement-shape record (plan_26101011): the program refuses to write an index without it. This check
        // asks for the meta, so it lays the record's shape down instead of running the resolver (test plane).
        let knob_record = root.join("bench_knobs.fixture.json");
        write_str(
            &knob_record,
            &json_text(&object(vec![
                ("bench", st("full-bench")),
                ("preset", st("fixture")),
                ("knobs", PyValue::Dict(PyDict::new())),
            ]))?,
        )?;
        let path_text = |name: &str| root.join(name).to_string_lossy().into_owned();
        let environment: Vec<(&str, String)> = vec![
            ("KNOB_RECORD", knob_record.to_string_lossy().into_owned()),
            ("CONFIG", "x".into()),
            ("TOPO", "single".into()),
            ("CFGYAML", path_text("cfg.yaml")),
            ("EF", path_text(".env.x")),
            ("MANIFEST", path_text("manifest.yaml")),
            ("SWEEPDIR", sweep_dir.to_string_lossy().into_owned()),
            ("VLLM_VER", String::new()),
            ("COMPLETED", "1".into()),
            ("ILEN", "1024".into()),
            ("IMAGE_TAG_ACTUAL", "NA".into()),
            (
                "REASSEMBLE",
                if arguments.prior_meta.is_some() {
                    "1"
                } else {
                    "0"
                }
                .into(),
            ),
            ("LITE_RAW", String::new()),
        ];
        let run = self.producer.run(&environment)?;
        replay_streams(console, &run)?;
        if run.status != 0 {
            return Err(Raised::new(
                "RuntimeError",
                &format!(
                    "생성부가 비정상 종료했다(rc={}): {}",
                    run.status,
                    crate::verify_fixture::tail(String::from_utf8_lossy(&run.stderr).trim(), 300)
                ),
            ));
        }
        let index = load_json(&sweep_dir.join("sweep_index.json"))?;
        crate::conformance::subscript(&index, "meta")
    }
}

/// What the producer printed reaches the verifier's own streams: its standard output as written, its standard error
/// at once (the order between the two is not observable, they are separate streams).
pub fn replay_streams(console: &mut dyn Console, run: &ProducerRun) -> Result<(), Raised> {
    console
        .write(&ProducerRun::text(&run.stdout)?)
        .map_err(Raised::from_failure)?;
    console.error(&ProducerRun::text(&run.stderr)?);
    Ok(())
}

fn object(entries: Vec<(&str, PyValue)>) -> PyValue {
    crate::verify_fixture::object(entries)
}

/// `json.dumps(value)` as text (ASCII-escaped, default separators).
fn json_text(value: &PyValue) -> Result<String, Raised> {
    Ok(dumps(value)?.to_utf8().unwrap_or_default())
}

// ── cases ────────────────────────────────────────────────────────────────────────────

/// An expected value of the case tables (`want`).
#[derive(Clone, Copy)]
enum Want {
    Text(&'static str),
    Number(i64),
    Absent,
}

impl Want {
    fn value(self) -> PyValue {
        match self {
            Want::Text(text) => PyValue::str(text),
            Want::Number(number) => PyValue::int(number),
            Want::Absent => PyValue::None,
        }
    }
}

struct Case {
    name: &'static str,
    arguments: BuildMeta<'static>,
    expect: Vec<(&'static str, Want)>,
}

fn declared(quant: &'static str, kvdt: &'static str) -> BuildMeta<'static> {
    BuildMeta {
        yaml_quant: Some(quant),
        yaml_kvdt: Some(kvdt),
        ..BuildMeta::default()
    }
}

const FP8_FOUR: [(&str, Want); 8] = [
    ("quantization", Want::Text("fp8")),
    ("quantization_source", Want::Text("declared(config yaml)")),
    ("quantization_declared", Want::Text("fp8")),
    ("quantization_mismatch", Want::Text("unknown")),
    ("kv_cache_dtype", Want::Text("fp8")),
    ("kv_cache_dtype_source", Want::Text("declared(config yaml)")),
    ("kv_cache_dtype_declared", Want::Text("fp8")),
    ("kv_cache_dtype_mismatch", Want::Text("unknown")),
];

/// The quantization / KV cache dtype cases C1–C10: the four-field contract is a value, its source, the declaration
/// and the mismatch note.
fn quantization_cases() -> Vec<Case> {
    let measured = |quant: &'static str, kv: &'static str| {
        vec![
            ("quantization", Want::Text(quant)),
            ("quantization_source", Want::Text("measured(engine log)")),
            ("kv_cache_dtype", Want::Text(kv)),
            ("kv_cache_dtype_source", Want::Text("measured(engine log)")),
        ]
    };
    vec![
        Case {
            name: "C1 yaml-only — 선언만 있으면 선언을 싣고 출처를 밝힌다",
            arguments: declared("fp8", "fp8"),
            expect: FP8_FOUR.to_vec(),
        },
        Case {
            name: "C2 argv-only — ★R7 회귀: yaml 무언·엔진 fp8 → NA 가 아니라 fp8",
            arguments: BuildMeta {
                engine: Some(("fp8", "fp8")),
                ..BuildMeta::default()
            },
            expect: vec![
                ("quantization", Want::Text("fp8")),
                ("quantization_source", Want::Text("measured(engine log)")),
                ("quantization_declared", Want::Text("NA")),
                ("quantization_mismatch", Want::Text("unknown")),
                ("kv_cache_dtype", Want::Text("fp8")),
                ("kv_cache_dtype_source", Want::Text("measured(engine log)")),
                ("kv_cache_dtype_declared", Want::Text("NA")),
                ("kv_cache_dtype_mismatch", Want::Text("unknown")),
            ],
        },
        Case {
            name: "C3 둘 다 — 측정이 이기고 선언은 병기, 일치하면 mismatch=no",
            arguments: BuildMeta {
                engine: Some(("fp8", "fp8")),
                ..declared("fp8", "fp8")
            },
            expect: vec![
                ("quantization", Want::Text("fp8")),
                ("quantization_source", Want::Text("measured(engine log)")),
                ("quantization_declared", Want::Text("fp8")),
                ("quantization_mismatch", Want::Text("no")),
                ("kv_cache_dtype", Want::Text("fp8")),
                ("kv_cache_dtype_source", Want::Text("measured(engine log)")),
                ("kv_cache_dtype_declared", Want::Text("fp8")),
                ("kv_cache_dtype_mismatch", Want::Text("no")),
            ],
        },
        Case {
            name: "C4 양쪽 부재 — 여기서만 NA 다",
            arguments: BuildMeta::default(),
            expect: vec![
                ("quantization", Want::Text("NA")),
                ("quantization_source", Want::Text("absent(both)")),
                ("quantization_declared", Want::Text("NA")),
                ("quantization_mismatch", Want::Text("unknown")),
                ("kv_cache_dtype", Want::Text("NA")),
                ("kv_cache_dtype_source", Want::Text("absent(both)")),
                ("kv_cache_dtype_declared", Want::Text("NA")),
                ("kv_cache_dtype_mismatch", Want::Text("unknown")),
            ],
        },
        Case {
            name: "C5 불일치 — 선언 fp8·실측 none/auto → 시끄럽게 YES",
            arguments: BuildMeta {
                engine: Some(("None", "auto")),
                ..declared("fp8", "fp8")
            },
            expect: vec![
                ("quantization", Want::Text("none")),
                ("quantization_source", Want::Text("measured(engine log)")),
                ("quantization_declared", Want::Text("fp8")),
                (
                    "quantization_mismatch",
                    Want::Text("YES(measured=none declared=fp8)"),
                ),
                ("kv_cache_dtype", Want::Text("auto")),
                ("kv_cache_dtype_source", Want::Text("measured(engine log)")),
                ("kv_cache_dtype_declared", Want::Text("fp8")),
                (
                    "kv_cache_dtype_mismatch",
                    Want::Text("YES(measured=auto declared=fp8)"),
                ),
            ],
        },
        Case {
            name:
                "C6 음성정직 — 엔진이 None 을 보고하면 그것은 측정된 기본값(none) 이지 NA 가 아니다",
            arguments: BuildMeta {
                engine: Some(("None", "auto")),
                ..BuildMeta::default()
            },
            expect: measured("none", "auto"),
        },
        Case {
            name: "C7 함정 — quantization_config=None 이 quantization=fp8 을 가리지 않는다",
            arguments: BuildMeta {
                engine: Some(("fp8", "fp8")),
                ..BuildMeta::default()
            },
            expect: vec![("quantization", Want::Text("fp8"))],
        },
        Case {
            name:
                "C8 함정 — 러너 에코·FlashInfer 해석값을 집지 않는다(2026-08-23 러너 6g 실증 결함)",
            arguments: BuildMeta {
                engine: Some(("fp8", "fp8")),
                ..BuildMeta::default()
            },
            expect: vec![
                ("kv_cache_dtype", Want::Text("fp8")),
                ("quantization", Want::Text("fp8")),
            ],
        },
        Case {
            name: "C9 함정 — 엔진 라인이 없으면 decoy 만 있어도 measured 로 승격하지 않는다",
            arguments: BuildMeta::default(),
            expect: vec![
                ("quantization_source", Want::Text("absent(both)")),
                ("kv_cache_dtype_source", Want::Text("absent(both)")),
            ],
        },
        Case {
            name: "C10 대소문자 정규화 — FP8 도 fp8 로 눕는다(정확일치 게이트 보호)",
            arguments: BuildMeta {
                engine: Some(("FP8", "FP8")),
                ..BuildMeta::default()
            },
            expect: vec![
                ("quantization", Want::Text("fp8")),
                ("kv_cache_dtype", Want::Text("fp8")),
            ],
        },
    ]
}

const MTP_THREE: &str =
    "speculative-config: '{\"method\":\"mtp\",\"num_speculative_tokens\":3}'   # 주석: MTP k=3";
const DECLARED_SOURCE: &str = "declared(config yaml speculative-config)";

/// The speculative-decoding declaration fingerprint cases S1–S10 (2026-09-14, plan_26091407 §4.1).
fn speculation_cases() -> Vec<Case> {
    let prior_on = || {
        object(vec![
            ("spec_declared", st("on")),
            ("spec_declared_k", PyValue::int(3)),
            ("spec_declared_source", st(DECLARED_SOURCE)),
        ])
    };
    vec![
        Case {
            name: "S1 JSON 선언(+행말 주석) → on · k=3",
            arguments: BuildMeta {
                cfg_extra: Some(MTP_THREE),
                ..BuildMeta::default()
            },
            expect: vec![
                ("spec_declared", Want::Text("on")),
                ("spec_declared_k", Want::Number(3)),
                ("spec_declared_source", Want::Text(DECLARED_SOURCE)),
            ],
        },
        Case {
            name: "S2 ★함정 주석 줄(#speculative-config: MTP OFF)은 선언이 아니다 → off",
            arguments: BuildMeta {
                cfg_extra: Some("#speculative-config: MTP OFF (결정론 분리 시험)"),
                ..BuildMeta::default()
            },
            expect: vec![("spec_declared", Want::Text("off")), ("spec_declared_k", Want::Absent)],
        },
        Case {
            name: "S3 줄 부재 → off",
            arguments: BuildMeta::default(),
            expect: vec![("spec_declared", Want::Text("off")), ("spec_declared_k", Want::Absent)],
        },
        Case {
            name: "S4 블록 매핑 형태 → on · k=7",
            arguments: BuildMeta {
                cfg_extra: Some(
                    "speculative-config:\n  method: dspark\n  num_speculative_tokens: 7\nenforce-eager: true",
                ),
                ..BuildMeta::default()
            },
            expect: vec![("spec_declared", Want::Text("on")), ("spec_declared_k", Want::Number(7))],
        },
        Case {
            name: "S5 명시 null → off(선언된 부재)",
            arguments: BuildMeta {
                cfg_extra: Some("speculative-config: null"),
                ..BuildMeta::default()
            },
            expect: vec![("spec_declared", Want::Text("off"))],
        },
        Case {
            name: "S6 ★음성대조 config yaml 부재 → unknown(off 로 접지 않는다)",
            arguments: BuildMeta {
                no_cfg: true,
                ..BuildMeta::default()
            },
            expect: vec![("spec_declared", Want::Text("unknown")), ("spec_declared_k", Want::Absent)],
        },
        Case {
            name: "S7 러너 sh 가 인자로 넣는 선언 → on",
            arguments: BuildMeta {
                sh_text: Some(
                    "#!/bin/bash\n# --speculative-config 주석은 무시\n\
EXTRA_ARGS+=(--speculative-config '{\"method\":\"mtp\",\"num_speculative_tokens\":2}')\n",
                ),
                ..BuildMeta::default()
            },
            expect: vec![
                ("spec_declared", Want::Text("on")),
                ("spec_declared_k", Want::Number(2)),
                ("spec_declared_source", Want::Text("declared(runner sh --speculative-config)")),
            ],
        },
        Case {
            name: "S8 러너 sh 의 주석 줄만 있으면 off",
            arguments: BuildMeta {
                sh_text: Some("# --speculative-config '{}'\n"),
                ..BuildMeta::default()
            },
            expect: vec![("spec_declared", Want::Text("off"))],
        },
        // Reassembly inherits the declaration of the measurement time; a yaml edited after it is not read again.
        Case {
            name: "S9 ★재조립 · 기존 index 에 지문 on 이 있으면 승계(현재 yaml 에서 spec 줄이 사라져도 off 로 바꾸지 않는다)",
            arguments: BuildMeta {
                cfg_extra: Some("#speculative-config: 측정 뒤 주석 처리"),
                prior_meta: Some(prior_on()),
                ..BuildMeta::default()
            },
            expect: vec![
                ("spec_declared", Want::Text("on")),
                ("spec_declared_k", Want::Number(3)),
                ("spec_declared_source", Want::Text(DECLARED_SOURCE)),
            ],
        },
        Case {
            name: "S10 재조립 · 기존 index 에 지문이 없으면 현재 파일로 계산하되 retro 표지",
            arguments: BuildMeta {
                cfg_extra: Some(MTP_THREE),
                prior_meta: Some(object(vec![("model", st("x"))])),
                ..BuildMeta::default()
            },
            expect: vec![
                ("spec_declared", Want::Text("on")),
                ("spec_declared_k", Want::Number(3)),
                (
                    "spec_declared_source",
                    Want::Text(
                        "retro(reassemble · 현재 config 기준 · 측정 시점 보장 없음) declared(config yaml speculative-config)",
                    ),
                ),
            ],
        },
    ]
}

/// One real cell yaml and what the fingerprint must say about it.
struct RealConfigCase {
    name: &'static str,
    config: &'static str,
    expect: Vec<(&'static str, Want)>,
}

/// The three real shapes of a campaign cell yaml (JSON declaration · long line-end comment · commented-out OFF),
/// excerpted into the check itself (2026-09-14, ⑧ finding T5): reading operator snapshots by path made the
/// regression skip on a fresh clone and read the other topology's lane on a shared working tree.
fn real_config_cases() -> Vec<RealConfigCase> {
    vec![
        RealConfigCase {
            name: "RC1 승자 셀 MTP k=3(JSON 문자열 · 곁에 레버 주석 줄)",
            config: "model: /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4\n\
tensor-parallel-size: 2\n\
gpu-memory-utilization: 0.80   # 마진 레버 — KV↔피크 상쇄의 천장 가설 검증\n\
enforce-eager: true            # 승계 통제변인(캡처 스파이크 ~10GiB 회피)\n\
async-scheduling: false        # MTP+async 금지(0.29 자동비활성 안 탐 — 명시 필수)\n\
speculative-config: '{\"method\":\"mtp\",\"num_speculative_tokens\":3}'\n\
max-num-batched-tokens: 2048    # 피크 억제 레버 ② — 프로파일링 더미 배치가 사살 지점이었다\n",
            expect: vec![("spec_declared", Want::Text("on")), ("spec_declared_k", Want::Number(3))],
        },
        RealConfigCase {
            name: "RC2 dspark k=7(행말 긴 주석 · 주석 안에 콜론·숫자)",
            config: "model: /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731\n\
enforce-eager: true            # L1: spec 레버 단독 평가를 위해 eager 유지(교차는 L4)\n\
speculative-config: '{\"method\":\"dspark\",\"num_speculative_tokens\":7}'   # ≥ dspark_block_size 5 \
(config.json:67) · 0.29 stock method 확인(config/speculative.py:613)            # 첫 트라이얼: arch-wall 확정용 최소 표면 \
(cudagraph·MATMUL_DECODE 계열은 스윕 축)\n\
reasoning-parser: deepseek_v4  # 등록명 확인(vllm/reasoning/__init__.py)\n",
            expect: vec![("spec_declared", Want::Text("on")), ("spec_declared_k", Want::Number(7))],
        },
        RealConfigCase {
            name: "RC3 MTP OFF 주석 줄(주석 안의 method=mtp · k=3 문구는 선언이 아니다)",
            config: "model: /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4\n\
kv-cache-memory-bytes: 21474836480   # 20GiB/노드 절대클립 — 스모크용 보수값\n\
# MTP 스모크 = B1/B2 의 직접 중재 대상. method=mtp 는 speculative.py:821 이 model_type 을\n\
# qwen4_exp_mtp 로 자동 유도(architectures=[Qwen4ExpMTP]). k=3 = 공식 체크포인트 실측 최적.\n\
#speculative-config: MTP OFF (결정론 분리 시험 2026-09-09)\n\
# 금지 조합(커뮤니티 그라운딩): MTP + --async-scheduling — 명시 미설정 유지\n",
            expect: vec![("spec_declared", Want::Text("off"))],
        },
    ]
}

/// What `_spec_axis_args` is expected to pass on.
#[derive(Clone, Copy)]
enum AxisMode {
    /// `["--accept-len-src", <warm json path>]`.
    Warm,
    /// `["--spec-axis-absent"]`.
    Absent,
}

struct AxisCase {
    name: &'static str,
    raw_document: Option<PyValue>,
    warm_text: Option<&'static str>,
    mode: AxisMode,
    /// The phrase the truncation log must hold; `None` = the log must stay empty.
    log_phrase: Option<&'static str>,
}

const WARM_WITH_SPEC: &str = r#"{"completed": 3, "failed": 0, "median_tpot_ms": 21.4, "spec_decode_acceptance_length": 2.2}"#;
const WARM_WITHOUT_SPEC: &str = r#"{"completed": 3, "failed": 0, "median_tpot_ms": 30.0}"#;

/// The multi-node pointer document the lite raw is: it names the warm JSON, it is not the inheritance source.
fn pointer_document(warm: &str) -> PyValue {
    object(vec![
        ("topology", st("multi")),
        ("burst_n", PyValue::int(3)),
        ("bench_warm_json", st(warm)),
        ("nodes", PyValue::List(vec![])),
    ])
}

fn axis_cases() -> Vec<AxisCase> {
    let pointer = || pointer_document("<WARM>");
    vec![
        AxisCase {
            name: "W1 ★포인터 → warm JSON 경로를 넘긴다(포인터 문서 자체 ✗)",
            raw_document: Some(pointer()),
            warm_text: Some(WARM_WITH_SPEC),
            mode: AxisMode::Warm,
            log_phrase: None,
        },
        AxisCase {
            name: "W2 spec off warm(수용길이 키 없음 · completed 있음)도 승계원이다",
            raw_document: Some(pointer()),
            warm_text: Some(WARM_WITHOUT_SPEC),
            mode: AxisMode::Warm,
            log_phrase: None,
        },
        AxisCase {
            name: "W3 ★잘린 warm JSON → 부재 명시로 강등 · 사유 기재(스윕을 죽이지 않는다)",
            raw_document: Some(pointer()),
            warm_text: Some("{\"completed\":3,\"spec_decode_acceptance_len"),
            mode: AxisMode::Absent,
            log_phrase: Some("JSONDecodeError"),
        },
        AxisCase {
            name: "W4 ★bench 결과 모양이 아닌 warm → 강등 · 파서 술어의 사유 기재",
            raw_document: Some(pointer()),
            warm_text: Some("{\"error\":\"x\"}"),
            mode: AxisMode::Absent,
            log_phrase: Some("vllm bench serve"),
        },
        AxisCase {
            name: "W5 포인터에 bench_warm_json 없음 → 강등 · 사유 기재",
            raw_document: Some(object(vec![
                ("topology", st("multi")),
                ("nodes", PyValue::List(vec![])),
            ])),
            warm_text: None,
            mode: AxisMode::Absent,
            log_phrase: Some("bench_warm_json 이 없다"),
        },
        AxisCase {
            name: "W6 포인터가 가리키는 warm 부재 → 강등 · 사유 기재",
            raw_document: Some(pointer()),
            warm_text: None,
            mode: AxisMode::Absent,
            log_phrase: Some("부재/빈 파일"),
        },
        AxisCase {
            name: "W7 lite 절삭(빈 인자) → 부재 명시 · 로그 추가 없음(이미 기재됨)",
            raw_document: None,
            warm_text: None,
            mode: AxisMode::Absent,
            log_phrase: None,
        },
    ]
}

/// What running the function produced: the arguments it set, the truncation log, and the paths it was given.
struct AxisRun {
    arguments: Vec<String>,
    log: String,
    warm_path: String,
    raw_path: String,
}

/// Cuts nothing and copies nothing: runs the deployed function `_spec_axis_args` on one fixture under
/// `set -euo pipefail` and reads back what it set. A function that dies under `set -e` is a `RuntimeError`.
fn run_spec_axis(
    function_source: &str,
    scripts_dir: &Path,
    root: &Path,
    case: &AxisCase,
) -> Result<AxisRun, Raised> {
    let log = root.join("truncation.log");
    write_str(&log, "")?;
    let warm = root.join("lite_warm_x.json");
    if let Some(warm_text) = case.warm_text {
        write_str(&warm, warm_text)?;
    }
    let mut raw_path: Option<PathBuf> = None;
    if let Some(document) = &case.raw_document {
        let mut copy = document.clone();
        if let PyValue::Dict(map) = &mut copy {
            if map.get("bench_warm_json").is_some_and(
                |value| matches!(value.peel(), PyValue::Str(own) if own.equals("<WARM>")),
            ) {
                map.set("bench_warm_json", st(&warm.to_string_lossy()));
            }
        }
        let path = root.join("lite_raw_x.json");
        write_text(&path, &dumps(&copy)?)?;
        raw_path = Some(path);
    }
    // The function echoes warnings on standard output; the result array goes to a file so the two do not mix.
    let arguments_file = root.join("spec_axis_args.txt");
    let script = format!(
        "{function_source}\n_spec_axis_args \"$1\" \"$2\" \"$3\"\nprintf \"%s\\n\" \"${{SPEC_AXIS_ARGS[@]}}\" > \"$4\"\n"
    );
    let command: Vec<OsString> = vec![
        "bash".into(),
        "-c".into(),
        format!("set -euo pipefail\n{script}").into(),
        "_".into(),
        raw_path
            .as_ref()
            .map_or_else(OsString::new, |path| path.clone().into_os_string()),
        log.clone().into_os_string(),
        scripts_dir.as_os_str().to_os_string(),
        arguments_file.clone().into_os_string(),
    ];
    let shown: Vec<PyValue> = command
        .iter()
        .map(|word| PyValue::Str(crate::verify_fixture::path_text(Path::new(word))))
        .collect();
    let captured = run_captured_in(
        &command,
        &shown,
        Duration::from_secs(AXIS_SECONDS),
        &AXIS_SECONDS.to_string(),
        &ChildSettings::default(),
    )
    .map_err(Raised::from_failure)?;
    if captured.status != 0 {
        let error_text = crate::captured_process::captured_text(&captured.stderr)
            .map_err(Raised::from_failure)?
            .to_utf8()
            .unwrap_or_default();
        return Err(Raised::new(
            "RuntimeError",
            &format!(
                "함수 실행이 set -e 아래에서 죽었다(rc={}): {}",
                captured.status,
                crate::verify_fixture::tail(&error_text, 300)
            ),
        ));
    }
    let arguments = splitlines(&PyStr::new(&read_text(&arguments_file)?))
        .iter()
        .map(|line| line.to_utf8().unwrap_or_default())
        .collect();
    Ok(AxisRun {
        arguments,
        log: read_text(&log)?,
        warm_path: warm.to_string_lossy().into_owned(),
        raw_path: raw_path.map_or_else(
            || "None".to_string(),
            |path| path.to_string_lossy().into_owned(),
        ),
    })
}

/// A `Raised` as the original's progress lines show it: `type(exc).__name__: exc`.
fn exception_text(raised: &Raised) -> (String, String) {
    (
        raised.failure().class.to_string(),
        raised.failure().message.to_utf8().unwrap_or_default(),
    )
}

/// The real engine logs of the three 2026-08-23 simlog runs, if the working tree still has them.
const REAL_LOGS: [(&str, &str, &str, &str); 3] = [
    (
        "R6a 1M·fp8 KV",
        "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r6a_1m_fp8/benchlog_sweep/level_01/engine_qwen38-27b.log",
        "none",
        "fp8",
    ),
    (
        "R6b 512k·bf16 KV",
        "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r6b_512k_bf16/benchlog_sweep/level_01/engine_qwen38-27b.log",
        "none",
        "auto",
    ),
    (
        "R7 1M·fp8 KV·fp8 가중치",
        "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r7_quant_fp8/benchlog_sweep/level_01/engine_qwen38-27b.log",
        "fp8",
        "fp8",
    ),
];

/// The program. `verbose` is `-v` among the arguments.
#[allow(clippy::too_many_lines)]
pub fn main(
    repo_root: Option<&Path>,
    verbose: bool,
    console: &mut dyn Console,
) -> Result<i32, Raised> {
    let say = |console: &mut dyn Console, line: &str| -> Result<(), Raised> {
        console
            .print(&PyStr::new(line))
            .map_err(Raised::from_failure)
    };
    let Some(repo_root) = repo_root else {
        console.error(&PyStr::new(&format!(
            "{}\n",
            repo_root_unknown_message(PROGRAM)
        )));
        return Ok(2);
    };
    let canonical = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let scripts_dir = canonical.join(SCRIPTS);
    let meta = SweepMeta::load(&scripts_dir)?;
    let mut failures: Vec<String> = Vec::new();

    let compare = |failures: &mut Vec<String>,
                   prefix: &str,
                   document: &PyValue,
                   expect: &[(&str, Want)]|
     -> Result<(), Raised> {
        for (key, want) in expect {
            let got = get(document, key)?;
            if !py_equals(&got, &want.value()) {
                failures.push(format!(
                    "{prefix}: meta[{}] = {} (기대 {})",
                    PyStr::new(key).repr(),
                    got.repr(),
                    want.value().repr()
                ));
            }
        }
        Ok(())
    };

    for case in quantization_cases() {
        match meta.build_meta(console, &case.arguments) {
            Ok(document) => compare(&mut failures, case.name, &document, &case.expect)?,
            Err(raised) if raised.is_system_exit() => return Err(raised),
            Err(raised) => {
                let (class, message) = exception_text(&raised);
                failures.push(format!("{}: 실행 예외 {class}: {message}", case.name));
                continue;
            }
        }
        if verbose {
            say(
                console,
                &format!(
                    "  {:<4} {}",
                    if failures.is_empty() { "ok" } else { ".." },
                    case.name
                ),
            )?;
        }
    }

    for case in speculation_cases() {
        match meta.build_meta(console, &case.arguments) {
            Ok(document) => {
                compare(&mut failures, case.name, &document, &case.expect)?;
                if verbose {
                    say(
                        console,
                        &format!(
                            "  ..   {} → {}/{}",
                            case.name,
                            py_str(&get(&document, "spec_declared")?),
                            py_str(&get(&document, "spec_declared_k")?)
                        ),
                    )?;
                }
            }
            Err(raised) if raised.is_system_exit() => return Err(raised),
            Err(raised) => {
                let (class, message) = exception_text(&raised);
                failures.push(format!("{}: 실행 예외 {class}: {message}", case.name));
            }
        }
    }

    let Some(function_source) = spec_axis_function(meta.script_text()) else {
        return Err(system_exit(
            "[selftest_sweep_meta] sweep_bench.sh 의 _spec_axis_args 구간을 찾지 못했다 — 중단",
        ));
    };
    for case in axis_cases() {
        let directory = TemporaryDirectory::create("tmp")?;
        let run = run_spec_axis(function_source, &scripts_dir, directory.path(), &case);
        directory.close()?;
        let run = match run {
            Ok(run) => run,
            Err(raised) => {
                let (class, message) = exception_text(&raised);
                failures.push(format!("{}: 실행 예외 {class}: {message}", case.name));
                continue;
            }
        };
        let accepted = match case.mode {
            AxisMode::Warm => {
                run.arguments == ["--accept-len-src".to_string(), run.warm_path.clone()]
                    && !run.arguments.contains(&run.raw_path)
            }
            AxisMode::Absent => run.arguments == ["--spec-axis-absent".to_string()],
        };
        if !accepted {
            let shown = PyValue::List(run.arguments.iter().map(|word| st(word)).collect()).repr();
            let mode = match case.mode {
                AxisMode::Warm => "warm",
                AxisMode::Absent => "absent",
            };
            failures.push(format!(
                "{}: SPEC_AXIS_ARGS={shown} (기대 {mode})",
                case.name
            ));
        }
        match case.log_phrase {
            None if !py_strip(&run.log).is_empty() => failures.push(format!(
                "{}: 절삭 로그가 비어 있어야 한다: {}",
                case.name,
                PyStr::new(&crate::verify_fixture::tail(&run.log, 200)).repr()
            )),
            Some(phrase) if !run.log.contains(phrase) => failures.push(format!(
                "{}: 절삭 로그에 {} 없음: {}",
                case.name,
                PyStr::new(phrase).repr(),
                PyStr::new(&crate::verify_fixture::tail(&run.log, 300)).repr()
            )),
            _ => {}
        }
        if verbose {
            let shown = PyValue::List(run.arguments.iter().map(|word| st(word)).collect()).repr();
            say(console, &format!("  ..   {} → {shown}", case.name))?;
        }
    }
    // Negative control: the pointer document itself as the inheritance source is refused (why the call site
    // had to be corrected). The owner of that refusal is the native GuideLLM library.
    let pointer = pointer_document("lite_warm_x.json");
    match crate::guidellm::inherit_accept_len(&pointer, &st("lite_raw_x.json")) {
        Ok(_) => failures
            .push("W1 음성대조: 포인터 문서가 승계원으로 수용됐다(조용한 None 승계)".to_string()),
        Err(failure) if failure.class == "ValueError" => {}
        Err(failure) => return Err(Raised::from_failure(failure)),
    }

    for case in real_config_cases() {
        let RealConfigCase {
            name,
            config,
            expect,
        } = case;
        let arguments = BuildMeta {
            cfg_text: Some(config),
            ..BuildMeta::default()
        };
        let document = match meta.build_meta(console, &arguments) {
            Ok(document) => document,
            Err(raised) if raised.is_system_exit() => return Err(raised),
            Err(raised) => {
                let (class, message) = exception_text(&raised);
                failures.push(format!("실 config {name}: 실행 예외 {class}: {message}"));
                continue;
            }
        };
        for (key, want) in &expect {
            let got = get(&document, key)?;
            if !py_equals(&got, &want.value()) {
                failures.push(format!(
                    "실 config {name}: meta[{}] = {} (기대 {})",
                    PyStr::new(key).repr(),
                    got.repr(),
                    want.value().repr()
                ));
            } else if verbose {
                say(
                    console,
                    &format!("  ok   실 config {name} · {key}={}", py_str(&got)),
                )?;
            }
        }
    }

    for (name, relative, quant, kvdt) in REAL_LOGS {
        let path = canonical.join(relative);
        if !is_file(&path)? {
            say(
                console,
                &format!("[selftest_sweep_meta] SKIP 실로그 부재 — {relative}"),
            )?;
            continue;
        }
        let bytes = crate::verify_fixture::read_bytes(&path)?;
        let arguments = BuildMeta {
            engine_log_text: Some(String::from_utf8_lossy(&bytes).into_owned()),
            ..BuildMeta::default()
        };
        let document = match meta.build_meta(console, &arguments) {
            Ok(document) => document,
            Err(raised) if raised.is_system_exit() => return Err(raised),
            Err(raised) => {
                let (class, message) = exception_text(&raised);
                failures.push(format!("실로그 {name}: 실행 예외 {class}: {message}"));
                continue;
            }
        };
        for (key, expected) in [("quantization", quant), ("kv_cache_dtype", kvdt)] {
            let got = get(&document, key)?;
            if !py_equals(&got, &st(expected)) {
                failures.push(format!(
                    "실로그 {name}: meta[{}] = {} (기대 {})",
                    PyStr::new(key).repr(),
                    got.repr(),
                    PyStr::new(expected).repr()
                ));
            } else if verbose {
                say(
                    console,
                    &format!("  ok   실로그 {name} · {key}={}", py_str(&got)),
                )?;
            }
        }
    }

    if !failures.is_empty() {
        let mut report = format!("[selftest_sweep_meta] FAIL {} 건:\n", failures.len());
        for failure in &failures {
            report.push_str(&format!("  - {failure}\n"));
        }
        console.error(&PyStr::new(&report));
        return Ok(1);
    }
    say(
        console,
        "[selftest_sweep_meta] OK — C1~C10 + S1~S10(spec 선언 지문·재조립 승계) + W1~W7(spec 축 승계원 호출부) \
+ 실 config·실로그 회귀 전부 통과 (측정>선언 · 기본값≠미상 · 함정 3종 차단 · 주석≠선언 · 부재≠off)",
    )?;
    Ok(0)
}
