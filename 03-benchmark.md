# 3. 벤치 결과 — 그리고 무엇과 비교할 수 있나

> 아래 수치는 인증서에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다.
> 출처: `benchmark_26090917_39_35_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.yaml`

## 성능

| 항목 | 값 |
|---|---|
| `benchmark_mode` | full |
| `verdict` | PASS |
| `decode_tps_conc1` | 69.81 |
| `rubric_authority` | weak |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 22.58 |
| `floor_tps` | 19.19 |
| `tolerance` | 0.15 |
| `ratio_M_over_primary` | 3.092 |
| `spec_on` | false |
| `accept_len` | N/A |
| `sweep_levels` | 1,2,4,8 |
| `sweep_truncated` | N/A |
| `lite_included` | false |
| `lite_gen_tps_warm` | N/A |
| `lite_gen_src` | N/A |
| `lite_cold_ttft_ms` | N/A |
| `lite_kv_gib` | N/A |
| `measured_utc` | 2026-09-09T08:39:35Z |

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
| `max_num_seqs` | 8 |
| `kv_cache_memory_bytes` | 71484875293 |
| `kv_cache_dtype` | auto |
| `gpu_memory_utilization` | 0.9 |
| `moe_backend` | N/A |
| `enforce_eager` | N/A |
| `ngc_base_tag` | N/A |

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26090917_39_35_qwen3.8-27b_RTXPRO6000BlackwellServerEdition_0.28.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 69.81 | 69.75 | 349.03 | 119.75 | 13.91 | 16/0 |
| 2 | 69.70 | 139.54 | 698.23 | 61.81 | 14.09 | 16/0 |
| 4 | 67.78 | 245.75 | 1229.73 | 120.91 | 14.34 | 18/0 |
| 8 | 63.27 | 392.83 | 1965.70 | 200.02 | 15.08 | 18/0 |

_절삭된 레벨 없음(요청 전 레벨 완주)._


## 결손 기재

_결손 없음 — 이 절이 요구하는 증거가 모두 도착했다._

## like-with-like 한정자 (Agent)

- **측정 조건**: input=1024 tok, output=256 tok(random synthetic), `--ignore-eos`, `--backend
  openai`(완결 엔드포인트), warmup 2회 제외.
- **동일 캠페인(TP=2) 자매 hint와 비교 가능**: `…tp2/qfp8-len262144-kvfp8`(셀 F, KV도 fp8 —
  이 캠페인의 최대집계처리량 승자, 집계 2503.2)·`…tp2/len262144-kvfp8`(셀 G, bf16가중치)·
  `…tp2/qfp8-len1000000-kvfp8`(셀 H, YaRN 1M)와 같은 스크립트·GPU·vLLM 버전·TP.
- **TP=1 자매 hint와 비교 가능하나 배율에 유의**: `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/
  qfp8-len262144-kvauto`(셀 A, 같은 축 조합·TP=1)와 나란히 놓으면 이 셀(TP=2)의 단일스트림
  decode(69.81)가 셀 A(43.91) 대비 **약 1.59배** — 완전한 2배가 아니다(TP 통신 오버헤드로
  추정, 정량 미검증). 동시접속 한계(batch)도 8 vs 3 로 커졌다(가중치가 카드당 절반이 된 효과).
- **다른 GPU/버전의 hint와 비교 금지**: GB10 통합메모리 계열 등(대역폭 규모가 다름).
- **재현 시 필수 재확인**: `tensor_parallel_size=2` 필수. `roofline.py` 는 이 TP>1 판정에서
  interconnect 대역폭 미상이라 **통신 비용을 0으로 취급**한다(`comm_bound=false` — roofline.json
  자체 주석 "과소추정 주의") — floor=19.19 는 낙관적 상한이라는 점을 감안하라(이 셀은 ratio
  3.09 로 여유 있게 PASS 했지만, 더 타이트한 마진의 조합에서는 이 낙관 편향이 오판을 만들 수
  있다).
