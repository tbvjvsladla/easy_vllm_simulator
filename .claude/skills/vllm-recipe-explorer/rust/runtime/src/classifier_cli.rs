//! The command line of `sim_classify.py` as a function from arguments to status and stream text (its `_main`): read a trial's
//! result from a JSON file, classify it and print the answer as JSON (indent 2, non-ASCII text kept) and a newline. Program
//! name in usage and help is the original `.py` name; an exception nothing handles ends the run with the complete final
//! `Class: message` block on standard error, status 1.

use crate::classifier::classify;
use crate::cli::Run;
use crate::cliparse::{parse as parse_arguments, Kind, Opt, Outcome, Spec, Value};
use crate::model_cli::{uncaught, Streams};
use crate::model_files::load_json_file;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

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

/// The parser of the original `_main`.
pub fn spec() -> Spec {
    Spec {
        prog: "sim_classify.py",
        description: "트라이얼 결과 결정론 분류기 (vllm-recipe-explorer Phase 2).",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None, false),
            opt(&["--trial"], "trial", Kind::Text, Some("run_trial 출력 JSON 경로"), true),
            opt(&["--budget"], "budget", Kind::Float, Some("vram_budget_gib"), true),
            opt(
                &["--margin"],
                "margin",
                Kind::Float,
                Some("safety_margin = 예산 검증 게이트 승수(gate_margin · 배포 gmu 가 아니다)"),
                true,
            ),
            opt(
                &["--typical-request-tokens"],
                "typical_request_tokens",
                Kind::Int,
                Some("(선택) KV-fit 대표 요청 길이 — 주면 클램프 트라이얼의 batch 를 엔진 보고 KV-fit 으로 재검증"),
                false,
            ),
        ],
        group_required: false,
        positional: None,
    }
}

/// `sim_classify.py ...`: `_main(argv)`.
pub fn sim_classify_main(args: &[PyStr], columns: Option<&str>) -> Run {
    let spec = spec();
    let values = match parse_arguments(&spec, args, columns) {
        Outcome::Values(values) => values,
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
            return Run {
                status,
                stdout,
                stderr,
                writes,
            };
        }
    };
    let streams = Streams::default();
    let Some(Value::Text(trial_path)) = values.get("trial") else {
        unreachable!("--trial is required")
    };
    let number = |dest: &str| match values.get(dest) {
        Some(Value::Float(number)) => PyValue::Float(*number),
        Some(Value::Int(number)) => PyValue::Int(number.clone()),
        _ => PyValue::None,
    };
    let trial = match load_json_file(trial_path) {
        Ok(trial) => trial,
        Err(failure) => return uncaught(streams, &failure),
    };
    let result = match classify(
        &trial,
        &number("budget"),
        &number("margin"),
        &number("typical_request_tokens"),
    ) {
        Ok(result) => PyValue::Dict(result),
        Err(failure) => return uncaught(streams, &failure),
    };
    let mut streams = streams;
    match result.dumps_pretty() {
        Ok(text) => streams.print(&text),
        Err(error) => {
            let message = error.to_string();
            let prefix = format!("{}: ", error.python_type());
            return uncaught(
                streams,
                &agent_runtime_support::failure::Failure::new(
                    error.python_type(),
                    message.strip_prefix(&prefix).unwrap_or(&message),
                ),
            );
        }
    }
    streams.finish(0)
}
