# 경량 성능 보고서(lite) — `fixture-model-7b` @ NVIDIA GB10 · vLLM 0.30.0

mode: lite

> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정의 **lite 스냅샷**이다(그때-그 HW/config). 성능 판정·루프라인·동시성 곡선·반복이 **없다** — lite 는 verdict_rule 에 투입하지 않는다. 성능 baseline 으로 승격하지 말고 소비 전 재측정할 것. 생성일 2026-09-29T01:02:03Z.

## lite 판정 (성립 · 성능 판정 아님)

| 키 | 값 |
|---|---|
| lite_verdict | N/A |

## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)

> 같은 모델·하드웨어라도 도구·반복·모드가 다르면 수치를 나란히 놓지 않는다. `bench_mode=lite` 는 full 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않은 측정이다 — 선언된 lite-only 이거나 반복 불성립으로 강등된 셀이다(강등이면 `downgrade_reason`). 이 표는 기재이지 판정이 아니다.

| 키 | 값 |
|---|---|
| bench_mode | lite |
| bench_mode_kind | declared-lite |
| bench_mode_source | declared(lite_bench --publish-report · lite-only 셀(full 을 시도하지 않았다)) |
| downgrade_reason | N/A |
| downgrade_reason_source | N/A |
| bench_tool | vllm-bench-serve |
| bench_tool_version | N/A |
| bench_tool_version_source | declared(도구가 버전을 자기보고하지 않는다) |
| repeats | 1 |
| repeats_source | lite(cold 1회 + warm burst 1회 · 반복 축 없음 — full 정의 밖) |
| repeats_completed | 1 |
| dataset_kind | synthetic |
| dataset_source | N/A |
| dataset_id | N/A |
| dataset_revision | N/A |
| dataset_source_sha256 | N/A |
| dataset_column_map | N/A |
| dataset_preprocess_strategy | N/A |
| dataset_seed | N/A |
| dataset_duplicate_sampling | N/A |
| dataset_tokenizer | N/A |

## lite 지표 (서빙 성공 직후 스냅샷 · inform-only)

> cold(warmup 0 단일요청 → cold TTFT) 1회 + warm burst(N=3 · conc=1 · warmup 1) 1회. 산정·표 렌더는 `lite_metrics.sh`(결정론)이며 이 문서는 그 표를 옮기지 않고 그대로 싣는다. 결측은 `N/A`(대체값 ✗).

| 메트릭 | 값 |
|---|---|
| gen tokens/sec (warm) | N/A |
| cold-start TTFT | N/A |
| GPU VRAM 점유 | N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음) |
| KV cache 점유 | N/A (serve 로그 KV 라인 부재 — fail-soft) |
| 시스템 RAM 점유 | 73.5 GiB (60%) |

## 측정 환경 스냅샷

| 키 | 값 |
|---|---|
| model | fixture-model-7b |
| model_source | derived(model_path basename · lowercased) |
| serving_config | fixture |
| gpu_model | NVIDIA GB10 |
| vllm_version | 0.30.0 |
| vllm_version_source | derived(envfile IMAGE_TAG) |
| vllm_build | 0.30.0 |
| topology | single |
| quantization | N/A |
| quantization_source | absent(both) |
| tensor_parallel_size | 1 |
| burst_n | 3 |
| backend | N/A |
| endpoint | N/A |
| gen_src | N/A |
| lite_accept_len | N/A |
| lite_accept_len_src | N/A |

---
_결정론 렌더 = `render_report.sh --lite-only`(LLM 표저작 ✗) · 참조 체인: lite raw → 이 report → testlog(판정) → devlog(서사)._
