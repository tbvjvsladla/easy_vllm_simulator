# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090618_gpt-oss-20b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 45.48 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 19.63 |
| `floor_tps` | 16.69 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.317 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 45.86 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 37.1 |
| `lite_kv_gib` | 48.96 |
| `measured_utc` | 2026-09-06T09:40:20Z |

## 측정 구성 — 무엇으로 쟀나(기재 · 게이트 아님)

> 도구·버전이 다르면 수치를 나란히 놓기 전에 조건부터 본다. 부재 키는 그 시점에 그 필드가
> 없었다는 뜻이다(합성하지 않는다).

| 항목 | 값 |
|---|---|
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `bench_tool_version_source` | measured(benchmarks.json metadata.guidellm_version) |

## 강한 일치 키 — 하나라도 다르면 이 수치는 **무효**다

| 키 | 값 |
|---|---|
| `model` | gpt-oss-20b |
| `gpu_model` | NVIDIA GB10 |
| `vllm_version` | 0.18.0 |
| `quantization` | mxfp4 |
| `topology` | single |
| `tensor_parallel_size` | 1 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 580.173.02 |
| `cuda_version` | 132 |
| `image_tag` | easy-vllm:0.18.0-cu130-aarch64-wheel |
| `image_digest` | sha256:d022edd3bfaf101e0c978930716da1b6845ebb16a72c1479eb9d764469568f44 |
| `max_model_len` | 131072 |
| `max_num_seqs` | 16 |
| `kv_cache_memory_bytes` | 52567672969 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | marlin |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090618_gpt-oss-20b_GB10_0.18.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 45.48 | 40.57 | 213.48 | 106.00 | 21.66 | 14/2 |
| 2 | 43.10 | 82.22 | 432.59 | 143.31 | 22.72 | 16/0 |
| 4 | 35.31 | 95.86 | 504.37 | 171.82 | 27.86 | 13/4 |
| 8 | 28.82 | 142.64 | 750.53 | 151.96 | 34.17 | 13/3 |
| 16 | 23.08 | 267.43 | 1407.13 | 584.77 | 41.21 | 17/1 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._

## like-with-like 한정자 (Agent)

**비교 가능**: 같은 GB10 단일 노드 · vLLM 0.18.0 · mxfp4 · TP=1 · `max_model_len 131072` ·
GuideLLM 0.7.3 · 입력 1024/출력 256 · `ignore_eos` on · request-rate=inf 인 판. 위 §강한 일치 키가
하나라도 다르면 이 수치는 **무효**다.

**비교 불가 — 특히 주의할 셋**:
1. **엔드포인트.** 이 판은 **chat**(`/v1/chat/completions`)으로 쟀고 판정점에서 16건 중 2건이
   harmony 파서 파손으로 errored 다. 형제 태그 `gb10-sub-native` 는 **완결**(`/v1/completions`)로
   18/18 · 오류 0 이다. 두 값(45.48 vs 44.99)은 가까워 보이지만 **같은 조건의 측정이 아니다.**
2. **예산 선택.** 이 판은 KV 50,133 MiB · `batch 16`, 형제 판은 KV 61,442 MiB · `batch 20` 이다.
   batch 를 내주고 컨텍스트를 얻을지는 레시피의 **선택**이지 하드웨어의 성질이 아니다.
3. **버전.** 0.19.0 판(`gb10x2-cluster-native`)과는 엔진 버전도 토폴로지도 다르다. 같은
   하드웨어라는 이유로 나란히 놓지 마라.

**부하 곡선을 읽는 법**: 동시성 1→16 에서 decode 45.48 → 23.08 t/s 로 떨어지지만 총 처리량은
213 → 1,407 tok/s 로 오른다. 어느 쪽이 목표인지에 따라 같은 표가 반대 결론을 준다 —
**판정점(동시성 1)만 보면 부호가 뒤집힌다.**

**결측 표기**: `accept_len`·`sweep_truncated`·`enforce_eager`·`ngc_base_tag` 의 `N/A` 는
그 시점에 그 필드가 없었다는 뜻이며 0 이 아니다.
