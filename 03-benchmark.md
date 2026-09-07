# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090707_gpt-oss-20b_GB10_0.18.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 44.99 |
| `rubric_authority` | explore |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 19.63 |
| `floor_tps` | 16.69 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 2.292 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1 |
| `sweep_truncated` | N/A |
| `lite_included` | true |
| `lite_gen_tps_warm` | 46.52 |
| `lite_gen_src` | median_tpot |
| `lite_cold_ttft_ms` | 109.3 |
| `lite_kv_gib` | 60.0 |
| `measured_utc` | 2026-09-06T22:37:02Z |

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
| `image_digest` | N/A |
| `max_model_len` | 131072 |
| `max_num_seqs` | 20 |
| `kv_cache_memory_bytes` | 64426421846 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090707_gpt-oss-20b_GB10_0.18.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 44.99 | 45.06 | 225.49 | 160.86 | 21.68 | 16/0 |

> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. 곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것).

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

> ⚠ 아래 정보가 **부재한 채로 발행**됐다. 부재는 실패가 아니라 **기록**이며, 이 태그를
> 소비할 때 그만큼을 모른 채 소비한다는 뜻이다(강행 발행 정책 · plan_26090616 Q7/Q8).

| 사유코드 | 뜻 |
|---|---|
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개뿐 — 부하 거동을 알 수 없다. |

## like-with-like 한정자 (Agent)

**비교 가능**: GB10 단일 노드 · vLLM 0.18.0 · mxfp4 · TP=1 · `max_model_len 131072` ·
GuideLLM 0.7.3 · **완결 엔드포인트**(`/v1/completions`) · 입력 1024/출력 256 · `ignore_eos` on ·
`batch 20` 인 판. §강한 일치 키가 하나라도 다르면 무효다.

**비교 불가 — 셋**:
1. **엔드포인트.** 이 판은 완결(18/18 · 오류 0), 형제 `gb10-main-native` 는 chat(16건 중 2건
   errored). 44.99 vs 45.48 은 가까워 보이지만 **같은 조건의 측정이 아니다.**
2. **예산 선택.** 이 판은 KV 61,442 MiB · batch 20, 형제 판은 50,133 MiB · batch 16 이다.
   batch 를 내주고 컨텍스트를 얻는 것은 레시피의 **선택**이지 하드웨어의 성질이 아니다.
3. **레벨 범위.** **레벨 1 만 측정됐다.** 상위 레벨(2·4·8·16)은 **절삭이 아니라 미실행**이다 —
   위임 세션이 종료돼 착수하지 못했다(`PAYLOAD.missing[]` 에 그대로 실린다). 곡선이 필요하면
   네가 재라. 완주한 곡선과 나란히 놓지 마라.

**결손**: `sweep_levels_2_4_8_16_not_executed`. 이것은 서브가 **스스로 선언한** 결손이며,
"돌다 절삭됐다" 와 "착수하지 못했다" 는 다른 사실이다.
