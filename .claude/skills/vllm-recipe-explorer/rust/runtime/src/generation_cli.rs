//! The two command lines of the generation owner, `gen_recipe_set.py` and `feedback_log.py`, as functions from arguments to
//! status and stream text (the programs' `main`s). Program names in usage and help are the original `.py` names. An exception
//! nothing handles ends the run with the complete final `Class: message` block of Python's report on standard error.
//!
//! `gen_recipe_set --check-parity` is looked for in the raw arguments before any parsing (so it wins over a malformed
//! command line) and reads the companion Rust trial sources of the deployed skill (see [`crate::anchor`]). `feedback_log
//! --record` still needs every estimated field on the command line: the parser requires them, as the original's does.

use crate::anchor::{default_log_path, trial_source_paths};
use crate::cli::Run;
use crate::cliparse::{parse as parse_arguments, Kind, Opt, Outcome, Spec, Value};
use crate::feedback::{self, Clock, Estimate};
use crate::model_cli::{uncaught, Streams};
use crate::model_dyn::Message;
use crate::model_files::{loads_str, read_text};
use crate::pyexpr::Res;
use crate::recipe_set::{self, is_os_failure, Request};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeMap;

fn opt(
    flags: &'static [&'static str],
    dest: &'static str,
    kind: Kind,
    help: Option<&'static str>,
    required: bool,
) -> Opt {
    Opt {
        flags,
        dest,
        kind,
        help,
        required,
        group: None,
    }
}

fn exited(outcome: Outcome) -> Result<BTreeMap<&'static str, Value>, Run> {
    match outcome {
        Outcome::Values(values) => Ok(values),
        Outcome::Exit {
            status,
            stdout,
            stderr,
        } => {
            let writes = if stdout.is_empty() {
                Vec::new()
            } else {
                vec![stdout.clone()]
            };
            Err(Run {
                status,
                stdout,
                stderr,
                writes,
            })
        }
    }
}

fn text_value(values: &BTreeMap<&'static str, Value>, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

fn number_value(values: &BTreeMap<&'static str, Value>, dest: &str) -> PyValue {
    match values.get(dest) {
        Some(Value::Int(number)) => PyValue::Int(number.clone()),
        Some(Value::Float(number)) => PyValue::Float(*number),
        _ => PyValue::None,
    }
}

fn flag(values: &BTreeMap<&'static str, Value>, dest: &str) -> bool {
    matches!(values.get(dest), Some(Value::Flag))
}

fn or_none(text: Option<PyStr>) -> PyValue {
    text.map_or(PyValue::None, PyValue::Str)
}

// ── gen_recipe_set ──────────────────────────────────────────────────────────────────────────────

pub fn generator_spec() -> Spec {
    Spec {
        prog: "gen_recipe_set.py",
        description: "DGX Spark 서빙용 3종 세트(.yaml+.sh+.env) 생성",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(
                &["--parsed"],
                "parsed",
                Kind::Text,
                Some("parse_model_config 결과 JSON 경로"),
                true,
            ),
            opt(
                &["--recipe"],
                "recipe",
                Kind::Text,
                Some("선택된 레시피 후보 JSON 경로"),
                true,
            ),
            opt(
                &["--name"],
                "name",
                Kind::Text,
                Some("3종 세트 base 이름"),
                true,
            ),
            opt(
                &["--repo"],
                "repo",
                Kind::Text,
                Some("레포 루트(configs/, envs/)"),
                true,
            ),
            opt(&["--port"], "port", Kind::Int, Some("SERVING_PORT"), true),
            opt(
                &["--served-model-name"],
                "served_model_name",
                Kind::Text,
                Some("--served-model-name 값"),
                true,
            ),
            opt(
                &["--image"],
                "image",
                Kind::Text,
                Some("컨테이너 이미지 태그 → env 의 IMAGE_TAG"),
                false,
            ),
            opt(
                &["--topology"],
                "topology",
                Kind::Choice(&["single", "multi"]),
                Some("env 형태를 가른다. multi = master/slave 컨테이너 쌍 + 분산 키"),
                false,
            ),
            opt(
                &["--vllm-version"],
                "vllm_version",
                Kind::Text,
                Some("multi env 의 VLLM_VERSION(이미지 정체성)"),
                false,
            ),
            opt(
                &["--build-dockerfile"],
                "build_dockerfile",
                Kind::Text,
                Some("multi env 의 BUILD_DOCKERFILE"),
                false,
            ),
            opt(
                &["--force"],
                "force",
                Kind::Flag,
                Some("기존 파일 덮어쓰기 허용"),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `--check-parity`: the parity tripwire over the companion trial sources of the deployed skill (the Rust trial owner's
/// `run_trial.rs` and `run_trial_session.rs`, read in that order and joined; an unreadable one ends the check with status 2).
fn check_parity(skill_root: &PyStr) -> Run {
    let mut streams = Streams::default();
    let mut source = PyStr::default();
    for path in trial_source_paths(skill_root) {
        match read_text(&path) {
            Ok(text) => {
                source.push_py(&text);
                source.push_str("\n");
            }
            Err(failure) if is_os_failure(&failure) => {
                streams.eprint(
                    &Message::new()
                        .lit("parity: run_trial 소스를 읽지 못했다 — ")
                        .text(&failure.message)
                        .done(),
                );
                return streams.finish(2);
            }
            Err(failure) => return uncaught(streams, &failure),
        }
    }
    let gaps = match recipe_set::rust_trial_serve_knob_gaps(&source) {
        Ok(gaps) => gaps,
        Err(failure) => return uncaught(streams, &failure),
    };
    if !gaps.is_empty() {
        let mut names = PyStr::default();
        for (index, gap) in gaps.iter().enumerate() {
            if index > 0 {
                names.push_str(", ");
            }
            names.push_py(gap);
        }
        streams.eprint(
            &Message::new()
                .lit("parity FAIL — run_trial 이 candidate 에서 읽지만 3종 세트에 도달하지 않는 필드: ")
                .text(&names)
                .done(),
        );
        streams.eprint(&PyStr::new("  → SERVE_KNOB_KEYS/_build_yaml 에 추가하거나, serve 노브가 아니면 candidate 를 읽지 말고 opts 로 받아라(평면 분리). 면제 등록은 마지막 수단이다."));
        return streams.finish(1);
    }
    streams.print(&PyStr::new("parity ok — 3종 세트 도달 갭 0"));
    streams.finish(0)
}

/// `gen_recipe_set.py ...`: `main(argv)`.
pub fn generator_main(args: &[PyStr], columns: Option<&str>, skill_root: &PyStr) -> Run {
    if args.iter().any(|arg| arg.equals("--check-parity")) {
        return check_parity(skill_root);
    }
    let spec = generator_spec();
    let values = match exited(parse_arguments(&spec, args, columns)) {
        Ok(values) => values,
        Err(run) => return run,
    };
    let mut streams = Streams::default();
    let text = |dest: &str| text_value(&values, dest).expect("a required option is present");
    let parsed = match recipe_set::load_json(&PyValue::Str(text("parsed"))) {
        Ok(parsed) => parsed,
        Err(failure) => return uncaught(streams, &failure),
    };
    let recipe = match recipe_set::load_json(&PyValue::Str(text("recipe"))) {
        Ok(recipe) => recipe,
        Err(failure) => return uncaught(streams, &failure),
    };
    let name = PyValue::Str(text("name"));
    let repo = PyValue::Str(text("repo"));
    let port = number_value(&values, "port");
    let served = PyValue::Str(text("served_model_name"));
    let force = PyValue::Bool(flag(&values, "force"));
    let topology =
        PyValue::Str(text_value(&values, "topology").unwrap_or_else(|| PyStr::new("single")));
    let vllm_version = or_none(text_value(&values, "vllm_version"));
    let build_dockerfile = or_none(text_value(&values, "build_dockerfile"));
    let image = or_none(text_value(&values, "image"));
    let request = Request {
        parsed: &parsed,
        recipe: &recipe,
        name: &name,
        repo_root: &repo,
        port: &port,
        served_model_name: &served,
        force: &force,
        topology: &topology,
        vllm_version: &vllm_version,
        build_dockerfile: &build_dockerfile,
        image: &image,
    };
    let mut warnings = PyStr::default();
    let outcome = recipe_set::generate(&request, &mut warnings);
    streams.stderr.push_py(&warnings);
    match outcome {
        Ok(paths) => {
            for path in &paths {
                streams.print(&Message::new().lit("생성됨: ").text(path).done());
            }
            streams.finish(0)
        }
        Err(failure) if failure.class == "FileExistsError" => {
            streams.eprint(
                &Message::new()
                    .lit("[gen_recipe_set] 중단: ")
                    .text(&failure.message)
                    .done(),
            );
            streams.finish(2)
        }
        Err(failure) => uncaught(streams, &failure),
    }
}

// ── feedback_log ────────────────────────────────────────────────────────────────────────────────

pub fn feedback_spec() -> Spec {
    Spec {
        prog: "feedback_log.py",
        description: "vllm-recipe-explorer 되먹임 로그(JSONL) append.",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(
                &["--path"],
                "path",
                Kind::Text,
                Some("JSONL 로그 경로(기본=feedback/recipe_feedback.jsonl)"),
                false,
            ),
            opt(&["--model-id"], "model_id", Kind::Text, None, true),
            opt(&["--quantization"], "quantization", Kind::Text, None, true),
            opt(&["--max-model-len"], "max_model_len", Kind::Int, None, true),
            opt(
                &["--gpu-memory-utilization"],
                "gpu_memory_utilization",
                Kind::Float,
                None,
                true,
            ),
            opt(
                &["--vram-budget-gb"],
                "vram_budget_gb",
                Kind::Float,
                None,
                true,
            ),
            opt(
                &["--estimated-vram-gb"],
                "estimated_vram_gb",
                Kind::Float,
                None,
                true,
            ),
            opt(
                &["--tensor-parallel-size"],
                "tensor_parallel_size",
                Kind::Int,
                None,
                true,
            ),
            opt(
                &["--safety-margin-threshold"],
                "safety_margin_threshold",
                Kind::Float,
                None,
                true,
            ),
            opt(
                &["--selection-timestamp"],
                "selection_timestamp",
                Kind::Text,
                Some("미지정 시 now()"),
                false,
            ),
            opt(
                &["--record"],
                "record",
                Kind::Text,
                Some("완성 record JSON 문자열(주면 개별 인자 대신 이 record 를 append)"),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `feedback_log.py ...`: `main(argv)`. `clock` is the clock of the records the command line builds; `skill_root` gives the
/// default of `--path`.
pub fn feedback_main(
    args: &[PyStr],
    columns: Option<&str>,
    skill_root: &PyStr,
    clock: &Clock,
) -> Run {
    let spec = feedback_spec();
    let values = match exited(parse_arguments(&spec, args, columns)) {
        Ok(values) => values,
        Err(run) => return run,
    };
    let mut streams = Streams::default();
    let path = text_value(&values, "path").unwrap_or_else(|| default_log_path(skill_root));
    let record: Res<PyValue> = match text_value(&values, "record") {
        Some(given) => loads_str(&given),
        None => {
            let text = |dest: &str| or_none(text_value(&values, dest));
            let model_id = text("model_id");
            let quantization = text("quantization");
            let max_model_len = number_value(&values, "max_model_len");
            let gpu_memory_utilization = number_value(&values, "gpu_memory_utilization");
            let vram_budget_gb = number_value(&values, "vram_budget_gb");
            let estimated_vram_gb = number_value(&values, "estimated_vram_gb");
            let tensor_parallel_size = number_value(&values, "tensor_parallel_size");
            let safety_margin_threshold = number_value(&values, "safety_margin_threshold");
            let selection_timestamp = text("selection_timestamp");
            Ok(PyValue::Dict(feedback::build_record(
                &Estimate {
                    model_id: &model_id,
                    quantization: &quantization,
                    max_model_len: &max_model_len,
                    gpu_memory_utilization: &gpu_memory_utilization,
                    vram_budget_gb: &vram_budget_gb,
                    estimated_vram_gb: &estimated_vram_gb,
                    tensor_parallel_size: &tensor_parallel_size,
                    safety_margin_threshold: &safety_margin_threshold,
                    selection_timestamp: &selection_timestamp,
                },
                clock,
            )))
        }
    };
    let record = match record {
        Ok(record) => record,
        Err(failure) => return uncaught(streams, &failure),
    };
    let written = match feedback::append(&record, &PyValue::Str(path), clock) {
        Ok(written) => written,
        Err(failure) => return uncaught(streams, &failure),
    };
    streams.eprint(
        &Message::new()
            .lit("appended 1 record -> ")
            .val(&written)
            .done(),
    );
    streams.finish(0)
}
