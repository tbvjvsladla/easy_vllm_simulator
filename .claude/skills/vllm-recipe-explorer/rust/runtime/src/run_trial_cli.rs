//! The command line of `scripts/run_trial.py` (`_main`): `--self-test` and the docker-free paths (`--dry-run`, `--mock-profile`)
//! are ported; the real docker session lives in `run_trial_session` (its two Python-owned neighbours stop it with status 2).
//! Program name in usage and help is the original `.py` name.

use crate::cli::{dumps, load_json, println_text, Run};
use crate::cliparse::{exit_with, parse, terminal_width, Kind, Opt, Outcome, Spec, Value};
use crate::pyexpr::{get, is_none};
use crate::run_trial::{
    mock_result, self_test, DEFAULT_PORT, DEFAULT_TIMEOUT, NAS_MOUNT,
    PROVENANCE_DRY_RUN, PROVENANCE_MOCK,
};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

fn opt(flags: &'static [&'static str], dest: &'static str, kind: Kind, help: Option<&'static str>) -> Opt {
    Opt { flags, dest, kind, help, required: false, group: None }
}

/// The parser of the original `_main`.
pub fn spec() -> Spec {
    Spec {
        prog: "run_trial.sh",
        description: "단일 트라이얼 실행기 (vllm-recipe-explorer Phase 2).",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help, None),
            opt(&["--candidate"], "candidate", Kind::Text, Some("candidate(lock-set) JSON 경로")),
            opt(&["--self-test"], "self_test", Kind::Flag, Some("생존검사·gmu 캡 회귀(docker/모델/하드웨어 불요)")),
            opt(&["--simlog-dir"], "simlog_dir", Kind::Text, Some("trialNN_vllm.log 등을 기록할 디렉토리")),
            opt(&["--trial-number"], "trial_number", Kind::Int, None),
            opt(&["--image"], "image", Kind::Text, Some("서빙 docker 이미지")),
            opt(&["--timeout"], "timeout", Kind::Float, Some("/health 폴링 타임아웃(초)")),
            opt(&["--port"], "port", Kind::Int, Some("호스트 노출 포트")),
            opt(&["--smoke-timeout"], "smoke_timeout", Kind::Float, Some("스모크 HTTP 타임아웃(초)")),
            opt(&["--served-model-name"], "served_model_name", Kind::Text, Some("스모크용 served_model_name")),
            opt(&["--container-name"], "container_name", Kind::Text, Some("컨테이너 이름(기본 vllm_trialNN)")),
            opt(&["--nas-mount"], "nas_mount", Kind::Text, Some("호스트 NAS 모델 루트(config.nas_host_root; 기본 /mnt/models). terraforming_node CLI 배선")),
            opt(&["--tiktoken-host-path"], "tiktoken_host_path", Kind::Text, Some("에어갭 tiktoken 인코딩 자산 호스트 경로(config.tiktoken_host_path) → /encodings:ro 마운트(C8)")),
            opt(&["--dry-run"], "dry_run", Kind::Flag, Some("docker 없이 mock_profile/빈 결과를 반환(루프 테스트용)")),
            opt(&["--mock-profile"], "mock_profile", Kind::Text, Some("docker 없이 사용할 vllm_profile(+functional) JSON 경로")),
            opt(&["--manifest"], "manifest", Kind::Text, Some("manifest.yaml 경로. nodes[role=main].node_id 로 선언 기록 위치를 파생한다(노드 사실의 단일 권위 — 브랜치·파일명으로 추론하지 않는다)")),
            opt(&["--budget-node-dir"], "budget_node_dir", Kind::Text, Some("선언을 기록할 노드 디렉터리(docs/logs/<node_id>). --manifest 파생을 덮어쓴다")),
            opt(&["--model-host-path"], "model_host_path", Kind::Text, Some("모델 디렉터리 **호스트** 경로 — 체크포인트 크기(weights_mib) 파생용. 미지정 시 candidate.model_host_path/model_path 를 시도한다")),
            opt(&["--tp"], "tp", Kind::Int, Some("tensor parallel 수 — weights_mib 파생용(ckpt÷tp). 미지정 시 1. candidate 에서 추측하지 않는다(manifest 가 tp 의 권위 · recipe.resolve_tp)")),
            opt(&["--overhead-mib"], "overhead_mib", Kind::Int, Some("예산 선언의 overhead(MiB). **선언 필수**(기본값 없음 · 2026-09-05 G-B1) — 후보 config `overhead_mib` 또는 env TRIAL_BUDGET_OVERHEAD_MIB 로도 선언할 수 있다. 낮게 잡으면 선언 바닥이 높아져 워치독이 정상 서빙을 무장 밴드에 넣는다")),
            opt(&["--no-budget"], "no_budget", Kind::Flag, Some("예산 선언 생략(무보호 진입). 생략 사실은 budget_skipped 이벤트로 남는다(침묵 금지)")),
        ],
        group_required: false,
        positional: None,
    }
}

fn single(status: i32, stdout: PyStr, stderr: PyStr) -> Run {
    let writes = if stdout.is_empty() { Vec::new() } else { vec![stdout.clone()] };
    Run { status, stdout, stderr, writes }
}

fn uncaught(failure: &Failure) -> Run {
    let mut err = failure.line();
    err.push_str("\n");
    single(1, PyStr::default(), err)
}

fn text_of(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> Option<PyStr> {
    match values.get(dest) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

/// `_main(argv)`.
pub fn run_trial_main(args: &[PyStr], columns: Option<&str>) -> Run {
    let parser = spec();
    let values = match parse(&parser, args, columns) {
        Outcome::Values(values) => values,
        Outcome::Exit { status, stdout, stderr } => return single(status, stdout, stderr),
    };
    if values.contains_key("self_test") {
        let mut log = String::new();
        return match self_test(&mut log) {
            Ok(line) => single(0, PyStr::new(&line), PyStr::new(&log)),
            Err(failures) => {
                let mut err = log;
                err.push_str(&format!("[run_trial --self-test] FAIL {} 건:\n", failures.len()));
                for f in &failures {
                    err.push_str(&format!("  - {}\n", f));
                }
                single(1, PyStr::default(), PyStr::new(&err))
            }
        };
    }
    let candidate_path = text_of(&values, "candidate");
    let simlog_dir = text_of(&values, "simlog_dir");
    let missing: Vec<&str> = [("--candidate", &candidate_path), ("--simlog-dir", &simlog_dir)]
        .iter()
        .filter(|(_, v)| v.as_ref().map_or(true, |t| t.is_empty()))
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        let message = format!("{} 는 필수다(--self-test 는 예외)", missing.join(", "));
        return match exit_with(&parser, terminal_width(columns), &message) {
            Outcome::Exit { status, stdout, stderr } => single(status, stdout, stderr),
            Outcome::Values(_) => unreachable!("exit_with always exits"),
        };
    }
    let dry_run = values.contains_key("dry_run");
    let mock_path = text_of(&values, "mock_profile").filter(|p| !p.is_empty());
    if !dry_run && mock_path.is_none() {
        return real_main(&values, &candidate_path.unwrap(), &simlog_dir.unwrap());
    }
    match mock_main(&values, &candidate_path.unwrap(), &simlog_dir.unwrap(), mock_path) {
        Ok(run) => run,
        Err(failure) => uncaught(&failure),
    }
}

fn mock_main(
    values: &std::collections::BTreeMap<&'static str, Value>,
    candidate_path: &PyStr,
    simlog_dir: &PyStr,
    mock_path: Option<PyStr>,
) -> Result<Run, Failure> {
    let candidate = load_json(candidate_path)?;
    if let Ok(bytes) = simlog_dir.to_surrogateescape_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let _ = std::fs::create_dir_all(std::ffi::OsString::from_vec(bytes));
    }
    let trial_number: i64 = match values.get("trial_number") {
        Some(Value::Int(n)) => n.to_string().parse().unwrap_or(1),
        _ => 1,
    };
    // The unused option defaults of the original live here only to document them (the docker path reads them). The image has
    // no default any more (plan_26111708 B L1 · `run_trial::MISSING_IMAGE`).
    let _ = (DEFAULT_TIMEOUT, DEFAULT_PORT, NAS_MOUNT);
    let mut log_path = simlog_dir.clone();
    log_path.push_str(&format!("/trial{:02}_vllm.log", trial_number));
    if let Some(trimmed) = simlog_dir.to_utf8() {
        // os.path.join: no doubled separator when the directory already ends with one.
        if trimmed.ends_with('/') {
            log_path = PyStr::new(&format!("{}trial{:02}_vllm.log", trimmed, trial_number));
        }
    }
    let (mut profile, mut functional) = (PyValue::None, PyValue::None);
    if let Some(path) = &mock_path {
        let blob = load_json(path)?;
        let wrapped = matches!(blob.peel(), PyValue::Dict(d) if d.contains_key("vllm_profile") || d.contains_key("functional"));
        if wrapped {
            profile = get(&blob, "vllm_profile")?;
            functional = get(&blob, "functional")?;
        } else {
            profile = blob;
        }
    }
    let provenance = if mock_path.is_some() { PROVENANCE_MOCK } else { PROVENANCE_DRY_RUN };
    let _ = is_none(&profile);
    let result = mock_result(&candidate, trial_number, profile, functional, &log_path, provenance)?;
    let text = println_text(dumps(&result)?);
    Ok(single(0, text, PyStr::default()))
}

fn int_opt(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> Option<i64> {
    match values.get(dest) {
        Some(Value::Int(n)) => n.to_string().parse().ok(),
        _ => None,
    }
}

fn float_opt(values: &std::collections::BTreeMap<&'static str, Value>, dest: &str) -> Option<f64> {
    match values.get(dest) {
        Some(Value::Float(f)) => Some(*f),
        _ => None,
    }
}

/// The real docker session: the grounding/budget gates first (they stop before the candidate is even read, as the original's
/// backstop does), then the session, then the same JSON print as the original.
fn real_main(
    values: &std::collections::BTreeMap<&'static str, Value>,
    candidate_path: &PyStr,
    simlog_dir: &PyStr,
) -> Run {
    use crate::run_trial_session::{pre_session_gates, repo_root, run_session, SessionError, SessionOpts};
    let opts = SessionOpts {
        image: text_of(values, "image"),
        timeout: float_opt(values, "timeout"),
        port: int_opt(values, "port"),
        smoke_timeout: float_opt(values, "smoke_timeout"),
        served_model_name: text_of(values, "served_model_name"),
        container_name: text_of(values, "container_name").filter(|c| !c.is_empty()),
        nas_mount: text_of(values, "nas_mount").filter(|c| !c.is_empty()),
        tiktoken_host_path: text_of(values, "tiktoken_host_path").filter(|c| !c.is_empty()),
        manifest: text_of(values, "manifest"),
        budget_node_dir: text_of(values, "budget_node_dir"),
        model_host_path: text_of(values, "model_host_path"),
        tp: int_opt(values, "tp").filter(|t| *t != 0),
        overhead_mib: int_opt(values, "overhead_mib").filter(|t| *t != 0),
        no_budget: values.contains_key("no_budget"),
        ..SessionOpts::default()
    };
    if let Err(stop) = pre_session_gates(&repo_root(), &opts) {
        return match stop {
            SessionError::Stop { status, text } => single(status, PyStr::default(), PyStr::new(&text)),
            SessionError::Failure(f) => uncaught(&f),
        };
    }
    let outcome = (|| -> Result<Run, SessionError> {
        let candidate = load_json(candidate_path)?;
        if let Ok(bytes) = simlog_dir.to_surrogateescape_bytes() {
            use std::os::unix::ffi::OsStringExt;
            let _ = std::fs::create_dir_all(std::ffi::OsString::from_vec(bytes));
        }
        let trial_number = int_opt(values, "trial_number").unwrap_or(1);
        let result = run_session(&candidate, simlog_dir, trial_number, &opts)?;
        Ok(single(0, println_text(dumps(&result)?), PyStr::default()))
    })();
    match outcome {
        Ok(run) => run,
        Err(SessionError::Stop { status, text }) => single(status, PyStr::default(), PyStr::new(&text)),
        Err(SessionError::Failure(f)) => uncaught(&f),
    }
}
