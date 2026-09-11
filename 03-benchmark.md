# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26091118_qwen3.8-flash-next-nvfp4_GB10_0.29.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 35.92 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 3.02 |
| `floor_tps` | 2.57 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 11.894 |
| `spec_on` | true |
| `accept_len` | 2.4035190615835775 |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 30.68 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 2466.3 |
| `lite_kv_gib` | 20.0 |
| `measured_utc` | 2026-09-11T09:36:51Z |

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
| `max_model_len` | 1048576 |
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
> 출처: `bench_report_26091118_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 35.92 | 33.41 | 167.03 | 597.65 | 66.86 | 16/0 |
| 2 | 32.66 | 56.24 | 281.18 | 612.35 | 76.06 | 16/0 |
| 4 | 25.36 | 79.01 | 395.07 | 705.93 | 96.19 | 16/0 |
| 8 | 16.81 | 110.02 | 550.09 | 1413.15 | 148.11 | 16/0 |
| 16 | 10.77 | 117.04 | 585.18 | 4257.34 | 220.34 | 16/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SLAVE_ATTESTATION` | 슬레이브 ABI attestation 부재 — 멀티에서 두 노드가 같은 것을 돌렸다는 증거가 성공 경로에 보존되지 않았다. |

## like-with-like 한정자 (Agent)

이 수치(35.92 t/s @동시성1, input_len=1024/output_len=256/n=16/warmup=2)는 **같은 측정
구성**(input/output 길이·동시성·`vllm bench serve`·완결 엔드포인트 `/v1/completions`)에서 잰
다른 hint와만 직접 비교 가능하다. 이 캠페인 내 같은 모델·다른 context 길이 hint(예:
`len262144-kvfp8e4m3-plemmap` 38.18 t/s, `len524288-kvauto-plemmap` 33.72 t/s)와는
**context 길이가 다르므로 절대값 비교는 의미가 제한적**이다 — context가 길수록 KV 어텐션
비용이 늘어 decode t/s가 완만히 하락하는 경향은 있지만, 이 캠페인은 그 하락률을 정밀 특성화
하지 않았다(explore 권한, roofline 기반 서술 판정). **다른 vLLM 버전**(0.29.0 이외)의 hint와는
엔진 최적화·스케줄러 변경이 있을 수 있어 비교 불가 — 버전이 다르면 재측정을 권고한다. 같은
모델의 **PLE resident** 변종(res 계열)은 이 캠페인 전체에서 측정치 자체가 없다(11/11 실패,
watchdog_kill_ack 또는 budget_block) — 비교 대상이 아니라 "측정 불가"다.
