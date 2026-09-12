# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091304_qwen3.8-flash-next-nvfp4_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 38.81 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 40.87 |
| `floor_tps` | 34.74 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 0.95 |
| `spec_on` | true |
| `accept_len` | 2.5932953826691967 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 33.03 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 2156.6 |
| `lite_kv_gib` | 20.0 |
| `measured_utc` | 2026-09-12T19:32:52Z |

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
| `model` | qwen3.8-flash-next-nvfp4 |
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
| `moe_backend` | flashinfer_cutlass |
| `enforce_eager` | true |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26091304_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 38.81 | 34.99 | 174.95 | 598.03 | 68.78 | 16/0 |
| 2 | 31.10 | 53.53 | 267.64 | 617.49 | 78.24 | 16/0 |
| 4 | 23.05 | 79.16 | 395.79 | 839.13 | 107.20 | 16/0 |
| 8 | 16.73 | 105.15 | 525.77 | 1370.81 | 140.69 | 16/0 |
| 16 | 10.93 | 122.54 | 612.71 | 4160.29 | 204.68 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

**비교 가능**: ① 같은 캠페인의 resident 자매셀(46.70 @conc1) — 같은 이미지·노드·측정사양이고
갈린 축은 PLE 모드와 그에 딸린 설정뿐이다. ② 이 모델의 **레거시 측정**(35.39/29.53/22.64/15.90/
11.51) — 같은 축·같은 측정사양이라 직접 대조되며, 실제로 같은 범위를 재현했다(음성대조).

**비교 불가**:
- 외부 DGX Spark 수치 전반 — 공개된 2노드 구성(median 53.7)은 **PLE resident** 이고, 1노드(32.5)는
  노드 수가 다르다. mmap × 2노드의 like-with-like 외부 수치는 **존재하지 않는다**. 그래서 이 셀의
  판정은 외부 레퍼런스가 아니라 루프라인 기대치(expected 40.87)를 문턱으로 썼다(달성률 0.95).
- 다른 vLLM 버전의 hint — 이 아키텍처는 0.29.x 계열에서 지원이 움직이는 중이다.
- 입력 길이·데이터셋이 다른 측정 — 이 수치는 in 1024 / out 256 / n 16 / warmup 2 폐기 기준이다.
