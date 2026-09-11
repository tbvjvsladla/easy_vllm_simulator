# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091115_qwen3.8-flash-next-nvfp4_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 34.86 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 3.02 |
| `floor_tps` | 2.57 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 11.543 |
| `spec_on` | true |
| `accept_len` | 2.4529094181163766 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 35.32 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 2551.5 |
| `lite_kv_gib` | 20.0 |
| `measured_utc` | 2026-09-11T06:47:16Z |

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
| `max_model_len` | 524288 |
| `max_num_seqs` | N/A |
| `kv_cache_memory_bytes` | 21474836480 |
| `kv_cache_dtype` | fp8_e4m3 |
| `gpu_memory_utilization` | 0.85 |
| `moe_backend` | flashinfer_cutlass |
| `enforce_eager` | true |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26091115_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 34.86 | 31.99 | 159.96 | 622.55 | 69.94 | 16/0 |
| 2 | 29.74 | 53.55 | 267.75 | 722.09 | 80.34 | 16/0 |
| 4 | 20.02 | 68.92 | 344.59 | 829.75 | 116.23 | 16/0 |
| 8 | 15.04 | 92.07 | 460.33 | 1889.84 | 153.50 | 16/0 |
| 16 | 9.61 | 117.57 | 587.84 | 6425.59 | 230.92 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

이 수치(동시성=1, input_len=1024, output_len=256, num_prompts=16, `vllm bench serve`
완결 엔드포인트)는 **같은 캠페인 내 262k/512k 다른 kv dtype·mmap 조합끼리만** 직접 비교하라
(예: `len262144-kvfp8e4m3-plemmap` 태그 — 같은 NVFP4 변종, context만 다름). context=262144
쪽은 34.86이 아니라 38.18로, 컨텍스트가 커질수록 decode t/s가 내려가는 경향을 그대로 보여준다
— 이는 speculative decoding의 accept_len이 컨텍스트에 따라 미세하게 갈리기 때문일 수 있다
(단정하지 않는다).

**비교 불가**: (1) `qwen3.8-flash-next-fp8` 변종(다른 체크포인트, 172.78GiB vs 123.57GiB) —
같은 context/kv dtype이어도 가중치 정밀도가 다르므로 t/s를 직접 대조하지 마라(별도 태그
`.../qwen3.8-flash-next-fp8/.../len524288-kv*-plemmap` 참조). (2) 다른 vLLM 버전의 hint —
이 태그는 0.29.0rc6 한정이며, 0.29.0 계열이 아니면 측정 조건(엔진 스케줄러·커널)이 달라
비교 근거가 없다. (3) `plemap` 없는(res, PLE resident) 태그와는 애초에 비교 대상이 아니다 —
그 조합은 이 캠페인에서 512k+ 컨텍스트 자체가 서빙 불가(watchdog kill)로 끝났다.
