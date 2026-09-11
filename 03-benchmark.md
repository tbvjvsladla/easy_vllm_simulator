# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091117_qwen3.8-flash-next-fp8_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 35.94 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 2.97 |
| `floor_tps` | 2.52 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 12.101 |
| `spec_on` | true |
| `accept_len` | 2.473397823458283 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 30.8 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 2572.2 |
| `lite_kv_gib` | 20.0 |
| `measured_utc` | 2026-09-11T08:26:28Z |

## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)

> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가
> 없었다는 뜻이다(합성하지 않는다).

| 항목 | 값 |
|---|---|
| `bench_tool` | vllm-bench-serve |
| `bench_tool_version` | N/A |
| `bench_tool_version_source` | declared(도구가 버전을 자기보고하지 않는다) |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | qwen3.8-flash-next-fp8 |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.29.0 |
| `quantization` | N/A |
| `topology` | multi |
| `tensor_parallel_size` | 2 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.29.0rc6-cu133-aarch64-source |
| `image_digest` | sha256:85cef27827c3afcdd39721ccafe623fb5457d11978e927edc216c1bf3fede931 |
| `max_model_len` | 524288 |
| `max_num_seqs` | N/A |
| `kv_cache_memory_bytes` | 21474836480 |
| `kv_cache_dtype` | fp8_e4m3 |
| `gpu_memory_utilization` | 0.85 |
| `moe_backend` | triton |
| `enforce_eager` | true |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26091117_qwen3.8-flash-next-fp8_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 35.94 | 31.58 | 157.90 | 686.49 | 72.73 | 16/0 |
| 2 | 25.72 | 44.42 | 222.12 | 718.77 | 89.30 | 16/0 |
| 4 | 18.14 | 58.33 | 291.63 | 910.35 | 128.77 | 16/0 |
| 8 | 12.53 | 78.38 | 391.91 | 1504.10 | 183.80 | 16/0 |
| 16 | 8.77 | 110.35 | 551.75 | 5061.18 | 260.81 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

이 수치(동시성=1, input_len=1024, output_len=256, num_prompts=16, `vllm bench serve`
완결 엔드포인트)는 **같은 FP8 체크포인트의 다른 kv dtype·context 조합끼리만** 직접 비교하라.
같은 context, kv=auto 태그(`len524288-kvauto-plemmap`, 37.61)보다 소폭 낮다 — kv dtype을
fp8로 바꾸면 이 조합에서는 소폭 저하가 관측됐다(단정하지 않는다, 5레벨 스윕 상세는 위 표
참조). context=262144 동형(`len262144-kvfp8e4m3-plemmap`, 34.04)보다는 오히려 높게 나왔다.

**비교 불가**: (1) `qwen3.8-flash-next-nvfp4` 변종 — 체크포인트 정밀도가 다르므로 t/s를
직접 대조하지 마라. (2) 다른 vLLM 버전의 hint. (3) `plemap` 없는(res) 태그 — 그 조합은
예산 게이트에서 즉시 차단되어 측정 자체가 없다.
