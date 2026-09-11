# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091112_qwen3.8-flash-next-fp8_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 35.37 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 2.97 |
| `floor_tps` | 2.52 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 11.909 |
| `spec_on` | true |
| `accept_len` | 2.547545059042884 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 30.7 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 381.4 |
| `lite_kv_gib` | 20.0 |
| `measured_utc` | 2026-09-11T03:44:17Z |

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
| `max_model_len` | 262144 |
| `max_num_seqs` | N/A |
| `kv_cache_memory_bytes` | 21474836480 |
| `kv_cache_dtype` | auto |
| `gpu_memory_utilization` | 0.85 |
| `moe_backend` | triton |
| `enforce_eager` | true |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26091112_qwen3.8-flash-next-fp8_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 35.37 | 31.86 | 159.30 | 679.58 | 73.09 | 16/0 |
| 2 | 28.49 | 49.73 | 248.65 | 687.55 | 89.07 | 16/0 |
| 4 | 18.49 | 67.31 | 336.56 | 876.33 | 130.75 | 16/0 |
| 8 | 13.91 | 93.06 | 465.29 | 2074.98 | 180.05 | 16/0 |
| 16 | 8.60 | 107.73 | 538.63 | 4776.10 | 265.14 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

이 수치(동시성1=35.37 t/s)는 **같은 강한 일치 키 안에서만** 비교 가능하다. FP8 체크포인트는
Unsloth 제조(`weight_block_size=[128,128]`, `modules_to_not_convert=943`)로 **직전 캠페인의
Qwen 제조 체크포인트(block_shape=[64,64])와 다르다** — `new_baseline` 이며 그쪽과 편차 계산을
하지 않는다. 같은 캠페인의 NVFP4 셀(`nv4-f8-262k-mmp` 38.18)과는 체크포인트 자체가 달라 절대
우열 비교는 사용자 판단. 자매 셀 `fp8-f8-262k-mmp`(kv=fp8_e4m3, 34.04)와는 kv dtype 만 다르므로
가장 안전한 비교쌍이다 — kv 양자화가 이 아치에서 순수하게 약 4% 손실을 낸다.
