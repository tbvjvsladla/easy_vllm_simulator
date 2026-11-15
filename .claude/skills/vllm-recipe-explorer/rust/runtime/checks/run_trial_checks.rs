//! Characterization of the pure part of `run_trial.py`: the expected values were recorded from the original's own functions on the
//! same candidate (docker command, audit failure), plus its self-test and mock path.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use recipe_runtime::run_trial::*;

const CANDIDATE: &str = r#"{"model_path_container":"/app/models/x","served_model_name":"m","max_model_len":4096,"batch":8,"max_num_batched_tokens":8192,"gpu_memory_utilization":0.9,"enforce_eager":true,"quantization":"None","moe_backend":"Auto","kv_cache_memory_bytes":1073741824,"kv_cache_quant":"fp8","tool_call_parser":"hermes","reasoning_parser":"r","model_capabilities":{"tool_call":true},"attention_backend":"FLASHINFER","extra_env":{"A":1},"serve_env":{"Z":"1","B":2}}"#;

const EXPECTED: &str = r#"docker run -d --name cn --runtime nvidia --ipc host --ulimit memlock=-1 --oom-score-adj 800 -p 8903:8903 -v /mnt/m:/app/models:ro -e MAX_JOBS=4 -e VLLM_ATTENTION_BACKEND=FLASHINFER -e A=1 -e B=2 -e Z=1 img vllm serve /app/models/x --served-model-name m --max-model-len 4096 --max-num-seqs 8 --max-num-batched-tokens 8192 --gpu-memory-utilization 0.9 --enforce-eager --kv-cache-memory-bytes 1073741824 --kv-cache-dtype fp8 --enable-auto-tool-choice --tool-call-parser hermes --host 0.0.0.0 --port 8903"#;

fn docker_cmd(candidate: &agent_runtime_support::pyvalue::PyValue) -> Vec<PyStr> {
    let (image, name, mount) = (PyStr::new("img"), PyStr::new("cn"), PyStr::new("/mnt/m"));
    let mut warnings = Vec::new();
    build_docker_cmd(
        candidate,
        &DockerOptions {
            image: &image,
            container_name: &name,
            port: 8903,
            nas_mount: &mount,
            tiktoken_host_path: None,
            jit_cache_root: None,
            max_jobs: Some(4),
            nas_container_root: None,
        },
        &mut warnings,
    )
    .expect("docker command")
}

#[test]
fn docker_command_matches_the_original() {
    let candidate = loads(CANDIDATE).unwrap();
    let cmd: Vec<String> = docker_cmd(&candidate).iter().map(|p| p.to_utf8().unwrap()).collect();
    assert_eq!(cmd.join(" "), EXPECTED);
}

#[test]
fn audit_names_the_dropped_moe_backend() {
    let candidate = loads(CANDIDATE).unwrap();
    // `moe_backend: Auto` is deliberately not emitted, yet the original audit still demands the flag for a non-null field.
    let failure = audit_emitted(&candidate, &docker_cmd(&candidate)).unwrap_err();
    assert_eq!(failure.line().to_utf8().unwrap(), "RuntimeError: emit-audit 실패 — candidate 의 비-null serve 필드가 cmd 에 누락됨: moe_backend(기대 --moe-backend) | 이 가드는 gmu 미emit 같은 회귀류 클래스를 차단한다(§9.3).");
}

#[test]
fn self_test_passes() {
    let mut log = String::new();
    assert!(self_test(&mut log).is_ok());
    assert!(log.contains("컨테이너 사망 감지(dead)"));
}

#[test]
fn mock_result_never_claims_measured() {
    let candidate = loads("{}").unwrap();
    let none = agent_runtime_support::pyvalue::PyValue::None;
    assert!(mock_result(&candidate, 1, none.clone(), none, &PyStr::new("l"), PROVENANCE_MEASURED).is_err());
}
