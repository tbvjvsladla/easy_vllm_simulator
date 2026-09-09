# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090917_44_39_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 69.76 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 22.58 |
| `floor_tps` | 19.19 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 3.089 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `lite_included` | false |
| `lite_gen_tps_warm` | N/A |
| `lite_gen_src` | N/A |
| `lite_cold_ttft_ms` | N/A |
| `lite_kv_gib` | N/A |
| `measured_utc` | 2026-09-09T08:44:39Z |

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
| `model` | qwen3.8-27b |
| `gpu_model` | NVIDIA RTX PRO 6000 Blackwell Server Edition |
| `vllm_version` | 0.28.0 |
| `quantization` | fp8 |
| `topology` | single |
| `tensor_parallel_size` | 2 |

## 소프트 지문 — 다르면 stale, 재측정 권고

| 키 | 값 |
|---|---|
| `driver_version` | 595.71.05 |
| `cuda_version` | 132 |
| `image_tag` | vllm-src-022:clean |
| `image_digest` | N/A |
| `max_model_len` | 262144 |
| `max_num_seqs` | 16 |
| `kv_cache_memory_bytes` | 72965891286 |
| `kv_cache_dtype` | fp8 |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090917_44_39_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 69.76 | 69.74 | 348.98 | 105.83 | 13.98 | 16/0 |
| 2 | 67.71 | 135.84 | 679.72 | 125.50 | 14.01 | 16/0 |
| 4 | 63.82 | 227.98 | 1140.80 | 373.02 | 14.27 | 14/0 |
| 8 | 56.64 | 361.86 | 1810.70 | 708.22 | 14.95 | 18/0 |
| 16 | 46.20 | 500.24 | 2503.17 | 723.15 | 16.95 | 18/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._

## like-with-like 한정자 (Agent)

- **측정 조건**: input=1024 tok, output=256 tok(random synthetic), `--ignore-eos`, `--backend
  openai`(완결 엔드포인트), warmup 2회 제외.
- **동일 캠페인(TP=2) 자매 hint와 비교 가능**: `…tp2/qfp8-len262144-kvauto`(셀 E, KV
  무양자화·batch=8)·`…tp2/len262144-kvfp8`(셀 G, bf16w)·`…tp2/qfp8-len1000000-kvfp8`(셀 H,
  YaRN 1M)와 같은 스크립트·GPU·vLLM 버전·TP. 이 셀이 **집계처리량 최대**(2503.2, E 대비
  1.27배) — 같은 가중치양자화(fp8)인데 KV 만 fp8 로 바뀐 차이가 batch 상한을 8→16 으로
  거의 2배 늘렸다.
- **TP=1 자매 hint와 비교 가능하나 배율에 유의**: `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/
  qfp8-len262144-kvfp8`(셀 B, 같은 축 조합·TP=1·그 캠페인의 승자)와 나란히 놓으면 단일스트림
  속도 ~1.58배·batch 상한 2.67배·집계처리량 2.12배로 배율이 서로 다르다(02-narrative.md 참조).
- **다른 GPU/버전의 hint와 비교 금지**: GB10 통합메모리 계열 등(대역폭 규모가 다름).
- **재현 시 필수 재확인**: `tensor_parallel_size=2` 필수. `roofline.py` 는 TP>1 통신비용을
  0으로 취급(interconnect 미상) — floor=19.19 는 낙관적 상한(자매 hint 셀 E §5 참조).
